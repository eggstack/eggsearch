use crate::core::sanitize::{
    bound_text, frame, scan_injection_markers, strip_control_chars, TrustMarkers,
    SNIPPET_MAX_CHARS, TITLE_MAX_CHARS,
};
use crate::core::SourceCard;
use crate::core::SourceMetadata;
use crate::core::TrustLevel;
use crate::meta::engines::models::{AggregatedResult, ResultMetadata, SearchResult};
use crate::meta::response::ProviderFailure;
use tracing::debug;

// ---------------------------------------------------------------------------
// RRF aggregation (vendored from metadata-search-engine-rs)
// ---------------------------------------------------------------------------

use std::collections::HashMap;

// Standard RRF constant (Cormack et al., 2009).
pub(crate) const RRF_K: f64 = 60.0;

/// Compute the candidate pool size for reranking. The pool is
/// intentionally larger than the final `max_results` so that
/// intent/freshness reranking can promote results just outside the
/// final window.
///
/// `candidate_cap` is the configured server cap (typically
/// `[search].max_results_cap`) used to bound the candidate pool. The
/// returned value is guaranteed to be:
///
/// - at least `final_max_results` (so the final window is always
///   coverable from the candidate pool),
/// - at most `max(final_max_results, candidate_cap)` (so a final
///   count larger than the cap still wins),
/// - never panics when `final_max_results > candidate_cap`.
///
/// In practice, for `final_max_results <= candidate_cap`, the helper
/// returns `min(final_max_results * 3, candidate_cap)`.
pub(crate) fn candidate_pool_size(final_max_results: usize, candidate_cap: usize) -> usize {
    if final_max_results == 0 {
        return 0;
    }
    let desired = final_max_results.saturating_mul(3);
    desired.min(candidate_cap.max(final_max_results))
}

pub(crate) fn apply_domain_filters(
    cards: Vec<SourceCard>,
    include_raw: &[String],
    exclude_raw: &[String],
) -> Vec<SourceCard> {
    use crate::core::query::{domain_matches_filter, hostname_from_url, normalize_domain};

    let include: Vec<String> = include_raw
        .iter()
        .filter_map(|d| normalize_domain(d).ok())
        .collect();
    let exclude: Vec<String> = exclude_raw
        .iter()
        .filter_map(|d| normalize_domain(d).ok())
        .collect();
    if include.is_empty() && exclude.is_empty() {
        return cards;
    }
    cards
        .into_iter()
        .filter(|card| {
            let Some(host) = hostname_from_url(&card.url) else {
                return include.is_empty();
            };
            if !include.is_empty() && !include.iter().any(|f| domain_matches_filter(&host, f)) {
                return false;
            }
            if exclude.iter().any(|f| domain_matches_filter(&host, f)) {
                return false;
            }
            true
        })
        .collect()
}

pub(crate) fn local_result_budget(effective_max_results: usize) -> usize {
    if effective_max_results == 0 {
        0
    } else {
        (effective_max_results / 2).max(1)
    }
}

pub(crate) fn merge_excerpts(
    existing: &mut Vec<crate::core::source_card::SourceExcerpt>,
    snippet: Option<&str>,
    mut incoming: Vec<crate::core::source_card::SourceExcerpt>,
) {
    use std::cmp::Ordering;
    incoming.sort_by(|a, b| match (a.score, b.score) {
        (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    let mut seen: std::collections::HashSet<String> = existing
        .iter()
        .map(|e| crate::core::source_card::excerpt_normalized_key(&e.text))
        .collect();
    if let Some(s) = snippet {
        seen.insert(crate::core::source_card::excerpt_normalized_key(s));
    }
    for e in incoming {
        if existing.len() >= crate::core::source_card::MAX_EXCERPTS_PER_CARD {
            break;
        }
        if e.text.trim().is_empty() {
            continue;
        }
        let key = crate::core::source_card::excerpt_normalized_key(&e.text);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        existing.push(e);
    }
}

pub(crate) fn merge_published_at(existing: &mut Option<String>, incoming: &Option<String>) {
    if existing.is_none() {
        *existing = incoming
            .as_deref()
            .and_then(crate::core::source_card::parse_result_timestamp);
    }
}

pub(crate) fn aggregate_rrf(
    mut engine_results: Vec<(String, Vec<SearchResult>)>,
    max_results: usize,
) -> Vec<AggregatedResult> {
    engine_results.sort_by(|a, b| a.0.cmp(&b.0));

    let mut map: HashMap<String, AggregatedResult> = HashMap::new();

    for (engine_name, results) in engine_results {
        for (index, mut result) in results.into_iter().enumerate() {
            let rank = index + 1;
            let rrf_score = 1.0 / (RRF_K + rank as f64);

            let key = match crate::meta::engines::normalizer::normalize(&result.url) {
                Some(k) => k,
                None => {
                    debug!(url = %result.url, "skipping result with un-normalizable URL");
                    continue;
                }
            };

            match map.get_mut(&key) {
                Some(existing) => {
                    existing.score += rrf_score;
                    if !existing.engines.contains(&engine_name) {
                        existing.engines.push(engine_name.clone());
                    }
                    if existing.snippet.is_none() && result.snippet.is_some() {
                        existing.snippet = result.snippet.take();
                    }
                    merge_excerpts(
                        &mut existing.excerpts,
                        existing.snippet.as_deref(),
                        std::mem::take(&mut result.excerpts),
                    );
                    merge_published_at(&mut existing.published_at, &result.published_at);
                    // Preserve the richer structured metadata. A row
                    // from `github_issues` carries real IssueMetadata
                    // and must not be replaced by `ResultMetadata::None`
                    // when a generic HTML scraper also returned the
                    // same URL.
                    let existing_had_metadata = !matches!(existing.metadata, ResultMetadata::None);
                    let incoming_has_metadata = !matches!(result.metadata, ResultMetadata::None);
                    existing.metadata =
                        std::mem::take(&mut existing.metadata).merge(result.metadata);
                    // When the incoming row is the first structured one,
                    // adopt its display fields too so the precise native
                    // title (e.g. an issue subject) wins over a generic
                    // scraper's title regardless of completion order.
                    if !existing_had_metadata && incoming_has_metadata {
                        existing.title = result.title;
                    }
                }
                None => {
                    let snippet = result.snippet;
                    let mut excerpts = Vec::new();
                    merge_excerpts(&mut excerpts, snippet.as_deref(), result.excerpts);
                    let mut published_at = None;
                    merge_published_at(&mut published_at, &result.published_at);
                    map.insert(
                        key,
                        AggregatedResult {
                            title: result.title,
                            url: result.url,
                            snippet,
                            engines: vec![engine_name.clone()],
                            score: rrf_score,
                            metadata: result.metadata,
                            excerpts,
                            published_at,
                        },
                    );
                }
            }
        }
    }

    let mut ranked: Vec<AggregatedResult> = map.into_values().collect();

    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.url.cmp(&b.url))
    });

    ranked.truncate(max_results);
    ranked
}

// ---------------------------------------------------------------------------
// Conversion to eggsearch types
// ---------------------------------------------------------------------------

pub(crate) fn convert_aggregated(a: AggregatedResult, sanitize: bool) -> Option<SourceCard> {
    if a.url.is_empty() {
        return None;
    }
    if !crate::meta::engines::is_http_url(&a.url) {
        tracing::debug!(
            url = %a.url,
            engines = ?a.engines,
            "dropping aggregated result with non-http URL"
        );
        return None;
    }
    let providers: Vec<String> = a.engines.into_iter().collect();

    // Deterministic source metadata from URL/domain heuristics.
    let (source_kind, code, domain) = crate::core::code_metadata::classify_and_extract(&a.url);

    // Allocate the id first so the framing can identify which card
    // the title/snippet text came from. The id is the deterministic
    // stable_id derived from (provider, url, title, source_kind), so
    // repeated runs over the same inputs produce the same id.
    let id = crate::core::identity::source_id(
        providers.first().map(|s| s.as_str()),
        Some(&a.url),
        Some(&a.title),
        Some(source_kind),
    );

    let mut warnings: Vec<String> = Vec::new();
    let (title, title_markers) = sanitize_field(
        &a.title,
        "title",
        &id,
        TITLE_MAX_CHARS,
        sanitize,
        &mut warnings,
    );
    let mut trust_markers = title_markers;
    debug_assert!(warnings.is_empty(), "title field should not emit warnings");

    // Drop empty snippets before sanitization so the card keeps
    // `snippet: None` for the (legitimate) empty-snippet case.
    let snippet = match a.snippet {
        Some(s) if !s.is_empty() => {
            let (sn, sm) = sanitize_field(
                &s,
                "snippet",
                &id,
                SNIPPET_MAX_CHARS,
                sanitize,
                &mut warnings,
            );
            trust_markers.merge(&sm);
            debug_assert!(
                warnings.is_empty(),
                "snippet field should not emit warnings"
            );
            Some(sn)
        }
        _ => None,
    };

    let mut excerpts: Vec<crate::core::source_card::SourceExcerpt> = Vec::new();
    let mut excerpt_chars = 0usize;
    for e in a.excerpts {
        if excerpts.len() >= crate::core::source_card::MAX_EXCERPTS_PER_CARD {
            break;
        }
        if e.text.trim().is_empty() {
            continue;
        }
        let (text, markers) = sanitize_field(
            &e.text,
            "excerpt",
            &id,
            crate::core::source_card::MAX_EXCERPT_CHARS,
            sanitize,
            &mut warnings,
        );
        trust_markers.merge(&markers);
        let chars = text.chars().count();
        if excerpt_chars + chars > crate::core::source_card::MAX_EXCERPT_TOTAL_CHARS {
            break;
        }
        excerpt_chars += chars;
        excerpts.push(crate::core::source_card::SourceExcerpt {
            text,
            score: e.score,
            provenance: e.provenance,
        });
    }

    let mut rank_reasons: Vec<crate::core::source_card::RankReason> = Vec::new();
    if providers.len() > 1 {
        rank_reasons.push(crate::core::source_card::RankReason::RrfMultiProvider);
    }

    let (issue, release, vulnerability) = match &a.metadata {
        ResultMetadata::Issue(m) => {
            if providers.iter().any(|p| p == "github_issues") {
                rank_reasons.push(crate::core::source_card::RankReason::ProviderNativeIssueSearch);
            }
            (Some(m.clone()), None, None)
        }
        ResultMetadata::Release(m) => {
            if providers.iter().any(|p| p == "github_releases") {
                rank_reasons
                    .push(crate::core::source_card::RankReason::ProviderNativeReleaseSearch);
            }
            (None, Some(m.clone()), None)
        }
        ResultMetadata::Advisory(m) => {
            if providers.iter().any(|p| {
                p == "osv"
                    || p == "github_advisory"
                    || p == "nvd"
                    || p == "cisa_kev"
                    || p == "rustsec"
            }) {
                rank_reasons
                    .push(crate::core::source_card::RankReason::ProviderNativeAdvisorySearch);
            }
            (None, None, Some(m.clone()))
        }
        ResultMetadata::CodeSearch(_) | ResultMetadata::None => (None, None, None),
    };

    // Extract matched_symbol from CodeSearch metadata for code evidence enrichment.
    let code_search_symbol = match &a.metadata {
        ResultMetadata::CodeSearch(m) => m.matched_symbol.as_deref(),
        _ => None,
    };

    let mut source_card = SourceCard {
        id: id.clone(),
        stable_id: Some(id),
        title,
        url: a.url.clone(),
        providers,
        score: Some(a.score),
        trust: TrustLevel::ExternalUntrusted,
        fetched: false,
        snippet,
        excerpts,
        trust_markers,
        metadata: SourceMetadata {
            source_kind,
            domain,
            rank_reasons,
            code: code.clone(),
            issue,
            release,
            vulnerability,
            code_evidence: code.as_ref().and_then(|c| {
                crate::core::code_evidence::build_code_evidence(c, Some(&a.url), code_search_symbol)
            }),
            local_repo_match: None,
            is_generated: None,
            is_vendor: None,
            is_test: None,
            is_example: None,
            is_config: None,
            is_lockfile: None,
            evidence_role: None,
            published_at: a.published_at,
        },
        quality: None,
    };

    // Compute deterministic quality metadata for the card.
    source_card.quality = Some(crate::core::quality::compute_card_quality(&source_card));

    Some(source_card)
}

/// Sanitize a single field of untrusted search-result text.
///
/// Tier 1 (`strip_control_chars` + `bound_text`) is always on. When
/// `sanitize = true`, Tier 2 (framing via `frame`) and Tier 3
/// (`scan_injection_markers` for the `injection_hits` count) are
/// also applied.
///
/// The per-hit warnings are NOT pushed here: search results are
/// aggregated across many cards and a single scanned marker on one
/// card would not be actionable at this layer. The per-card
/// `TrustMarkers.injection_hits` count is exposed via the card and
/// the `web_search` tool emits a per-card aggregate warning.
///
/// Returns the (possibly framed) string and a `TrustMarkers` record
/// describing what was done. The `warnings` vector is reserved for
/// future use; current search-result sanitization does not push
/// per-hit warnings (the count is enough).
pub(crate) fn sanitize_field(
    text: &str,
    field: &str,
    id: &str,
    max_chars: usize,
    sanitize: bool,
    _warnings: &mut [String],
) -> (String, TrustMarkers) {
    let mut m = TrustMarkers::default();

    // Tier 1: always on.
    let (stripped, removed) = strip_control_chars(text);
    m.control_chars_removed = removed;
    let (bounded, truncated) = bound_text(&stripped, max_chars);
    if truncated {
        m.text_truncated = true;
    }

    if sanitize {
        // Tier 3: scan for injection markers on the bounded
        // (stripped, bounded) text. The count is exposed via the
        // per-card `TrustMarkers.injection_hits`.
        let hits = scan_injection_markers(&bounded);
        m.injection_hits = hits.len();

        // Tier 2: wrap in framing delimiters.
        m.text_sanitized = true;
        m.text_framed = true;
        (frame(&bounded, field, id), m)
    } else {
        if removed > 0 || truncated {
            m.text_sanitized = true;
        }
        (bounded, m)
    }
}

/// Parse an RFC 3339 timestamp string into a `chrono::DateTime<Utc>`.
/// Returns `None` for missing, empty, or unparseable strings.
pub(crate) fn parse_timestamp(ts: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    let s = ts?;
    if s.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

/// Extract the primary freshness timestamp from a card's metadata.
/// Prefers the generic provider-neutral `published_at`; falls back to
/// specialist issue/release metadata when no generic timestamp exists.
pub(crate) fn freshness_timestamp(
    metadata: &crate::core::source_card::SourceMetadata,
) -> Option<&str> {
    if let Some(ref ts) = metadata.published_at {
        return Some(ts);
    }
    if let Some(ref issue) = metadata.issue {
        issue.updated_at.as_deref()
    } else if let Some(ref release) = metadata.release {
        release
            .published_at
            .as_deref()
            .or(release.created_at.as_deref())
    } else {
        None
    }
}

/// Check whether a timestamp falls within the requested freshness window.
/// Returns `true` only when the timestamp is within the window.
/// `Any` always returns `false` (no freshness boost needed).
pub(crate) fn matches_freshness(
    ts: chrono::DateTime<chrono::Utc>,
    freshness: crate::core::query::Freshness,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    use crate::core::query::Freshness;
    let diff = now.signed_duration_since(ts);
    match freshness {
        Freshness::Any => false,
        Freshness::Day => diff <= chrono::Duration::days(1),
        Freshness::Week => diff <= chrono::Duration::weeks(1),
        Freshness::Month => diff <= chrono::Duration::days(30),
        Freshness::Year => diff <= chrono::Duration::days(365),
    }
}

/// Apply a bounded post-RRF score adjustment based on the caller's
/// intent and freshness hints. The base RRF score remains dominant;
/// boosts are additive and capped so a single heuristic never
/// overwhelms multi-provider evidence.
pub(crate) fn apply_intent_reranking(
    results: &mut [SourceCard],
    intent: crate::core::query::SearchIntent,
    freshness: crate::core::query::Freshness,
) {
    use crate::core::query::SearchIntent;
    use crate::core::source_card::{RankReason, SourceKind};

    if results.is_empty() {
        return;
    }

    // Compute the maximum base score so boosts are proportional.
    let max_base = results
        .iter()
        .filter_map(|r| r.score)
        .fold(0.0_f64, f64::max);
    if max_base <= 0.0 {
        return;
    }

    // Boost factor: at most +30% of the max base score for a
    // perfect intent match. This keeps provider evidence dominant.
    let boost_unit = max_base * 0.10;

    // Current time for freshness checks. Only computed once per
    // reranking pass so all cards use a consistent clock.
    let now = chrono::Utc::now();

    for card in results.iter_mut() {
        let base = card.score.unwrap_or(0.0);
        let mut boost = 0.0_f64;
        let mut reasons: Vec<RankReason> = Vec::new();

        // --- intent-based domain priors ---
        let kind = card.metadata.source_kind;
        match intent {
            SearchIntent::Docs => {
                if matches!(kind, SourceKind::OfficialDocs | SourceKind::PackageRegistry) {
                    boost += boost_unit * 2.0;
                    reasons.push(RankReason::IntentMatch);
                    if kind == SourceKind::OfficialDocs {
                        reasons.push(RankReason::DomainPriorDocs);
                    }
                }
            }
            SearchIntent::Code => {
                if matches!(
                    kind,
                    SourceKind::SourceRepository
                        | SourceKind::RepositoryRoot
                        | SourceKind::SourceDirectory
                        | SourceKind::SourceFile
                        | SourceKind::PackageRegistry
                ) {
                    boost += boost_unit * 2.0;
                    reasons.push(RankReason::IntentMatch);
                    reasons.push(RankReason::DomainPriorCode);
                }
            }
            SearchIntent::Issues => {
                if matches!(kind, SourceKind::IssueThread | SourceKind::PullRequest) {
                    boost += boost_unit * 2.0;
                    reasons.push(RankReason::IntentMatch);
                }
            }
            SearchIntent::Releases => {
                if matches!(kind, SourceKind::ReleaseNotes | SourceKind::Tag) {
                    boost += boost_unit * 2.0;
                    reasons.push(RankReason::IntentMatch);
                    reasons.push(RankReason::DomainPriorRelease);
                }
            }
            SearchIntent::Security => {
                if kind == SourceKind::SecurityAdvisory {
                    boost += boost_unit * 3.0;
                    reasons.push(RankReason::IntentMatch);
                    reasons.push(RankReason::DomainPriorSecurity);
                    reasons.push(RankReason::SecurityPrimarySource);
                } else if matches!(
                    kind,
                    SourceKind::IssueThread | SourceKind::PullRequest | SourceKind::ReleaseNotes
                ) {
                    reasons.push(RankReason::SecurityMaintainerSource);
                }
            }
            SearchIntent::News => {
                if kind == SourceKind::News {
                    boost += boost_unit * 2.0;
                    reasons.push(RankReason::IntentMatch);
                }
            }
            SearchIntent::Web => {
                // No intent-based boosts for neutral web search.
            }
        }

        // --- freshness boost ---
        // Only emit FreshnessMatch when the card has actual timestamp
        // evidence and the requested freshness is not Any.
        if freshness != crate::core::query::Freshness::Any {
            if let Some(ts_str) = freshness_timestamp(&card.metadata) {
                if let Some(ts) = parse_timestamp(Some(ts_str)) {
                    if matches_freshness(ts, freshness, now) {
                        boost += boost_unit * 1.0;
                        reasons.push(RankReason::FreshnessMatch);
                    }
                }
            }
        }

        // Apply boost and collect rank reasons.
        if boost > 0.0 {
            card.score = Some(base + boost);
        }
        card.metadata.rank_reasons.extend(reasons);
    }

    // Re-sort by updated scores (stable sort preserves original
    // order for ties).
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

pub(crate) fn build_retrieval_failures(
    providers_failed: &[ProviderFailure],
    providers_queried: &[String],
    attempts: &[crate::core::retrieval_status::RetrievalAttempt],
    fallback_scope: &str,
) -> Vec<crate::core::workflow_coverage::RetrievalFailure> {
    use crate::core::workflow_coverage::{RetrievalFailure, RetrievalFailureKind};

    if !attempts.is_empty() {
        return crate::core::retrieval_status::attempts_to_failures(attempts);
    }

    use crate::core::retrieval_status::map_provider_to_intended_roles;

    let failed_set: std::collections::HashSet<&str> =
        providers_failed.iter().map(|f| f.id.as_str()).collect();

    let mut failures = Vec::new();

    for provider_id in providers_queried {
        if failed_set.contains(provider_id.as_str()) {
            if let Some(pf) = providers_failed.iter().find(|f| f.id == *provider_id) {
                let kind = if pf.error_class == "timeout" {
                    RetrievalFailureKind::DeadlinePreventedCompletion
                } else {
                    RetrievalFailureKind::ProviderFailed
                };
                let roles = map_provider_to_intended_roles(provider_id, fallback_scope);
                for role in roles {
                    failures.push(RetrievalFailure {
                        kind,
                        role,
                        message: format!("[{}] {}", pf.error_class, pf.message),
                        provider_id: Some(provider_id.clone()),
                    });
                }
            }
        }
    }

    failures
}
