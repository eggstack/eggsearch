//! Evidence bundle builder.
//!
//! Pure, testable logic for constructing an [`EvidenceBundle`] from
//! source-card inputs and fetch-response inputs. The builder
//! deduplicates sources, links fetches to sources, applies bundle
//! caps, merges trust markers, computes deterministic gaps, and
//! produces a bounded response.

use crate::core::evidence_bundle::{
    compute_bundle_id, compute_fetch_id, compute_source_id, EvidenceBundle,
    EvidenceBundleFetchedItem, EvidenceBundleLimits, EvidenceBundleLink, EvidenceBundleLinkReason,
    EvidenceBundleRequest, EvidenceBundleSource, EvidenceFetchInput, EvidenceGap, EvidenceGapKind,
    EvidenceProviderCount, EvidenceProviderSummary, EvidenceSourceInput, EvidenceTrustSummary,
    DEFAULT_MAX_FETCHED_ITEMS, DEFAULT_MAX_SOURCES, DEFAULT_MAX_TOTAL_CHARS, MAX_FETCHED_ITEMS_CAP,
    MAX_SOURCES_CAP, MAX_TOTAL_CHARS_CAP,
};
use crate::core::fetch::FetchTrust;
use crate::core::result::TrustLevel;
use crate::core::source_card::SourceKind;

/// Build an evidence bundle from a request. Pure function — no I/O.
pub fn build_evidence_bundle(request: EvidenceBundleRequest) -> EvidenceBundle {
    let now = chrono::Utc::now().to_rfc3339();

    // Clamp caps to server-enforced bounds
    let max_sources = request
        .max_sources
        .unwrap_or(DEFAULT_MAX_SOURCES)
        .min(MAX_SOURCES_CAP);
    let max_fetched_items = request
        .max_fetched_items
        .unwrap_or(DEFAULT_MAX_FETCHED_ITEMS)
        .min(MAX_FETCHED_ITEMS_CAP);
    let max_total_chars = request
        .max_total_chars
        .unwrap_or(DEFAULT_MAX_TOTAL_CHARS)
        .min(MAX_TOTAL_CHARS_CAP);

    let include_unfetched = request.include_unfetched_sources.unwrap_or(true);

    // Phase 1: Convert source inputs to bundle sources with deterministic IDs
    let mut sources: Vec<EvidenceBundleSource> =
        request.sources.iter().map(source_input_to_bundle).collect();

    // Phase 2: Deduplicate sources by URL (richer metadata wins)
    deduplicate_sources(&mut sources);

    // Phase 3: Apply source cap
    let sources_truncated = sources.len() > max_sources;
    if sources_truncated {
        sources.truncate(max_sources);
    }

    // Phase 4: Convert fetch inputs to bundle fetched items with deterministic IDs
    let mut fetch_items: Vec<EvidenceBundleFetchedItem> =
        request.fetches.iter().map(fetch_input_to_bundle).collect();

    // Phase 5: Link fetches to sources
    let source_links = link_fetches_to_sources(&sources, &mut fetch_items);

    // Phase 6: Filter unfetched sources if requested
    if !include_unfetched {
        let fetched_source_ids: std::collections::HashSet<&str> = fetch_items
            .iter()
            .filter_map(|f| f.source_id.as_deref())
            .collect();
        sources.retain(|s| fetched_source_ids.contains(s.source_id.as_str()));
    }

    // Phase 7: Apply fetched items cap and total chars budget
    let (fetched_items_truncated, total_chars_exceeded) =
        apply_fetch_caps(&mut fetch_items, max_fetched_items, max_total_chars);

    // Phase 8: Compute trust summary
    let trust_summary = compute_trust_summary(&sources, &fetch_items);

    // Phase 9: Compute provider summary
    let provider_summary = compute_provider_summary(&sources);

    // Phase 10: Compute deterministic gaps
    let gaps = compute_gaps(&sources, &fetch_items, &request.warnings);

    // Phase 11: Compute deterministic bundle ID
    let source_ids: Vec<String> = sources.iter().map(|s| s.source_id.clone()).collect();
    let fetch_ids: Vec<String> = fetch_items.iter().map(|f| f.fetch_id.clone()).collect();
    let bundle_id = compute_bundle_id(request.goal.as_deref(), &source_ids, &fetch_ids);

    let limits = EvidenceBundleLimits {
        max_sources,
        max_fetched_items,
        max_total_chars,
        sources_truncated,
        fetched_items_truncated,
        total_chars_exceeded,
    };

    EvidenceBundle {
        bundle_id,
        goal: request.goal,
        created_at: now,
        sources,
        fetched_items: fetch_items,
        source_links,
        trust_summary,
        provider_summary,
        gaps,
        structured_warnings: crate::core::warning::convert_warnings(&request.warnings),
        warnings: request.warnings,
        limits,
        research_claims: request.research_claims,
        research_conflicts: request.research_conflicts,
    }
}

/// Convert a source input to a bundle source with a deterministic ID.
fn source_input_to_bundle(input: &EvidenceSourceInput) -> EvidenceBundleSource {
    let source_kind = input
        .metadata
        .as_ref()
        .map(|m| m.source_kind)
        .unwrap_or(SourceKind::Unknown);

    let source_id = compute_source_id(
        input.providers.first().map(|s| s.as_str()),
        input.url.as_deref(),
        input.title.as_deref(),
        Some(source_kind),
    );

    let source_role = input
        .metadata
        .as_ref()
        .and_then(|m| m.code_evidence.as_ref())
        .map(|ce| format!("{:?}", ce.source_role).to_lowercase());

    let rank_reasons: Vec<String> = input
        .metadata
        .as_ref()
        .map(|m| {
            m.rank_reasons
                .iter()
                .map(|r| format!("{r:?}").to_lowercase())
                .collect()
        })
        .unwrap_or_default();

    let structured_repo_fetch = input.metadata.as_ref().and_then(|m| {
        m.code.as_ref().and_then(|c| {
            let host = c.host?;
            let owner = c.owner.clone()?;
            let repo = c.repo.clone()?;
            let path = c.path.clone()?;
            Some(crate::core::repo_fetch::RepoLocator {
                kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
                host: Some(host),
                owner: Some(owner),
                repo: Some(repo),
                ref_name: None,
                commit_sha: None,
                path,
                workspace_root: None,
            })
        })
    });

    EvidenceBundleSource {
        source_id,
        original_id: input.id.clone(),
        url: input.url.clone(),
        title: input.title.clone(),
        source_kind: Some(source_kind),
        source_role,
        provider_id: input.providers.first().cloned(),
        rank: None,
        score: input.score,
        rank_reasons,
        trust: input.trust.unwrap_or(TrustLevel::ExternalUntrusted),
        trust_markers: input.trust_markers.clone().unwrap_or_default(),
        quality: input.quality.clone(),
        stable: None,
        structured_repo_fetch,
        metadata: input.metadata.clone(),
    }
}

/// Convert a fetch input to a bundle fetched item with a deterministic ID.
fn fetch_input_to_bundle(input: &EvidenceFetchInput) -> EvidenceBundleFetchedItem {
    let text_prefix = input
        .text
        .as_deref()
        .map(|t| if t.len() > 64 { &t[..64] } else { t });

    let fetch_id = compute_fetch_id(
        input.url.as_deref(),
        input.locator.as_ref(),
        input.line_start,
        input.line_end,
        text_prefix,
    );

    EvidenceBundleFetchedItem {
        fetch_id,
        source_id: input.source_id.clone(),
        url: input.url.clone(),
        locator: input.locator.clone(),
        fetched: input.fetched,
        content_type: input.content_type.clone(),
        language: input.language.clone(),
        selected_span: input.selected_span.clone(),
        code_span_id: input.code_span_id.clone(),
        line_start: input.line_start,
        line_end: input.line_end,
        text: input.text.clone(),
        truncated: input.truncated,
        trust: input.trust.unwrap_or(FetchTrust::ExternalUntrusted),
        trust_markers: input.trust_markers.clone().unwrap_or_default(),
        warnings: input.warnings.clone(),
    }
}

/// Deduplicate sources by URL, keeping the richer metadata.
fn deduplicate_sources(sources: &mut Vec<EvidenceBundleSource>) {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut to_remove = Vec::new();

    for (i, source) in sources.iter().enumerate() {
        if let Some(url) = &source.url {
            if let Some(&existing_idx) = seen.get(url) {
                // Keep the one with more metadata (more rank_reasons = richer)
                let existing_len = sources[existing_idx].rank_reasons.len();
                let current_len = source.rank_reasons.len();
                if current_len > existing_len {
                    to_remove.push(existing_idx);
                    seen.insert(url.clone(), i);
                } else {
                    to_remove.push(i);
                }
            } else {
                seen.insert(url.clone(), i);
            }
        }
    }

    // Remove duplicates in reverse order to preserve indices
    to_remove.sort_unstable();
    to_remove.dedup();
    for &idx in to_remove.iter().rev() {
        sources.remove(idx);
    }
}

/// Link fetched items to sources by URL, locator, or explicit source ID.
fn link_fetches_to_sources(
    sources: &[EvidenceBundleSource],
    fetch_items: &mut [EvidenceBundleFetchedItem],
) -> Vec<EvidenceBundleLink> {
    let mut links = Vec::new();

    for fetch in fetch_items.iter_mut() {
        // Priority 1: explicit source ID
        if let Some(ref source_id) = fetch.source_id {
            if sources.iter().any(|s| s.source_id == *source_id) {
                links.push(EvidenceBundleLink {
                    source_id: source_id.clone(),
                    fetch_id: fetch.fetch_id.clone(),
                    link_reason: EvidenceBundleLinkReason::SourceIdMatch,
                });
                continue;
            }
        }

        // Priority 2: URL match
        if let Some(ref fetch_url) = fetch.url {
            if let Some(matched) = sources.iter().find(|s| {
                s.url
                    .as_ref()
                    .map(|u| urls_equal(u, fetch_url))
                    .unwrap_or(false)
            }) {
                fetch.source_id = Some(matched.source_id.clone());
                links.push(EvidenceBundleLink {
                    source_id: matched.source_id.clone(),
                    fetch_id: fetch.fetch_id.clone(),
                    link_reason: EvidenceBundleLinkReason::UrlMatch,
                });
                continue;
            }
        }

        // Priority 3: structured locator match
        if let Some(ref fetch_locator) = fetch.locator {
            if let Some(matched) = sources.iter().find(|s| {
                s.structured_repo_fetch
                    .as_ref()
                    .map(|loc| locators_equal(loc, fetch_locator))
                    .unwrap_or(false)
            }) {
                fetch.source_id = Some(matched.source_id.clone());
                links.push(EvidenceBundleLink {
                    source_id: matched.source_id.clone(),
                    fetch_id: fetch.fetch_id.clone(),
                    link_reason: EvidenceBundleLinkReason::LocatorMatch,
                });
            }
        }
    }

    links
}

/// Loose URL equality: normalize trailing slashes and case for host.
fn urls_equal(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('/');
    let b = b.trim_end_matches('/');
    a.eq_ignore_ascii_case(b)
}

/// Loose locator equality: compare host, owner, repo, path.
fn locators_equal(
    a: &crate::core::repo_fetch::RepoLocator,
    b: &crate::core::repo_fetch::RepoLocator,
) -> bool {
    let a_host = a.host.map(|h| format!("{h:?}"));
    let b_host = b.host.map(|h| format!("{h:?}"));
    let a_owner = a.owner.as_deref().unwrap_or("");
    let b_owner = b.owner.as_deref().unwrap_or("");
    let a_repo = a.repo.as_deref().unwrap_or("");
    let b_repo = b.repo.as_deref().unwrap_or("");
    a_host == b_host
        && a_owner.eq_ignore_ascii_case(b_owner)
        && a_repo.eq_ignore_ascii_case(b_repo)
        && a.path == b.path
}

/// Apply fetched items cap and total chars budget.
fn apply_fetch_caps(
    fetch_items: &mut Vec<EvidenceBundleFetchedItem>,
    max_fetched_items: usize,
    max_total_chars: usize,
) -> (bool, bool) {
    let fetched_items_truncated = fetch_items.len() > max_fetched_items;
    if fetched_items_truncated {
        fetch_items.truncate(max_fetched_items);
    }

    let mut total_chars = 0usize;
    let mut total_chars_exceeded = false;
    for item in fetch_items.iter_mut() {
        let text_len = item.text.as_ref().map(|t| t.len()).unwrap_or(0);
        if total_chars + text_len > max_total_chars {
            // Truncate this item's text to fit the budget
            let remaining = max_total_chars.saturating_sub(total_chars);
            if remaining > 0 {
                if let Some(ref mut text) = item.text {
                    // Reserve 1 char for the '…' truncation marker
                    let safe_cap = remaining.saturating_sub(1);
                    if safe_cap > 0 {
                        text.truncate(safe_cap);
                    } else {
                        text.clear();
                    }
                    text.push('…');
                }
            } else {
                item.text = None;
                item.truncated = true;
            }
            total_chars_exceeded = true;
            break;
        }
        total_chars += text_len;
    }

    (fetched_items_truncated, total_chars_exceeded)
}

/// Compute aggregated trust summary.
fn compute_trust_summary(
    sources: &[EvidenceBundleSource],
    fetch_items: &[EvidenceBundleFetchedItem],
) -> EvidenceTrustSummary {
    let mut summary = EvidenceTrustSummary::default();

    for source in sources {
        match source.trust {
            TrustLevel::ExternalUntrusted => summary.external_untrusted_count += 1,
            TrustLevel::LocalTrusted => summary.local_trusted_count += 1,
            _ => {}
        }
        summary.total_injection_hits += source.trust_markers.injection_hits;
        summary.total_control_chars_removed += source.trust_markers.control_chars_removed;
        summary.any_text_sanitized =
            summary.any_text_sanitized || source.trust_markers.text_sanitized;
        summary.any_text_truncated =
            summary.any_text_truncated || source.trust_markers.text_truncated;
        summary.any_text_framed = summary.any_text_framed || source.trust_markers.text_framed;
    }

    for item in fetch_items {
        summary.total_injection_hits += item.trust_markers.injection_hits;
        summary.total_control_chars_removed += item.trust_markers.control_chars_removed;
        summary.any_text_sanitized =
            summary.any_text_sanitized || item.trust_markers.text_sanitized;
        summary.any_text_truncated =
            summary.any_text_truncated || item.trust_markers.text_truncated;
        summary.any_text_framed = summary.any_text_framed || item.trust_markers.text_framed;
    }

    summary
}

/// Compute aggregated provider summary.
fn compute_provider_summary(sources: &[EvidenceBundleSource]) -> EvidenceProviderSummary {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for source in sources {
        if let Some(ref provider) = source.provider_id {
            *counts.entry(provider.clone()).or_insert(0) += 1;
        }
    }

    let providers_used: Vec<String> = counts.keys().cloned().collect();
    let per_provider_counts: Vec<EvidenceProviderCount> = counts
        .into_iter()
        .map(|(provider_id, count)| EvidenceProviderCount { provider_id, count })
        .collect();

    EvidenceProviderSummary {
        providers_used,
        per_provider_counts,
    }
}

/// Compute deterministic gaps from sources, fetches, and warnings.
fn compute_gaps(
    sources: &[EvidenceBundleSource],
    fetch_items: &[EvidenceBundleFetchedItem],
    warnings: &[crate::core::result::SearchWarning],
) -> Vec<EvidenceGap> {
    let mut gaps = Vec::new();

    // Check for unfetched sources
    let fetched_source_ids: std::collections::HashSet<&str> = fetch_items
        .iter()
        .filter_map(|f| f.source_id.as_deref())
        .collect();
    for source in sources {
        if !fetched_source_ids.contains(source.source_id.as_str()) {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::SourceUnfetched,
                message: format!(
                    "source '{}' was not fetched",
                    source.title.as_deref().unwrap_or("")
                ),
                source_id: Some(source.source_id.clone()),
                provider_id: source.provider_id.clone(),
                affected_source_ids: vec![],
            });
        }
    }

    // Check for fetch failures
    for item in fetch_items {
        if !item.fetched {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::FetchFailed,
                message: item
                    .warnings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "fetch failed".to_string()),
                source_id: item.source_id.clone(),
                provider_id: None,
                affected_source_ids: vec![],
            });
        }
    }

    // Check source metadata for local checkout dirty state
    for source in sources {
        if let Some(ref meta) = source.metadata {
            if let Some(ref lrm) = meta.local_repo_match {
                if lrm.dirty_state.as_deref() == Some("dirty") {
                    gaps.push(EvidenceGap {
                        kind: EvidenceGapKind::LocalCheckoutDirty,
                        message: format!(
                            "local checkout '{}' has uncommitted changes",
                            lrm.root_name.as_deref().unwrap_or("unknown"),
                        ),
                        source_id: Some(source.source_id.clone()),
                        provider_id: source.provider_id.clone(),
                        affected_source_ids: vec![],
                    });
                }
                // LocalRemoteMismatch: local_checkout exists but matched is false
                if !lrm.matched {
                    gaps.push(EvidenceGap {
                        kind: EvidenceGapKind::LocalRemoteMismatch,
                        message: format!(
                            "local checkout '{}' remote identity does not match the requested repo",
                            lrm.root_name.as_deref().unwrap_or("unknown"),
                        ),
                        source_id: Some(source.source_id.clone()),
                        provider_id: source.provider_id.clone(),
                        affected_source_ids: vec![],
                    });
                }
            }
        }
    }

    // LocalGeneratedOrVendorOnly: all local sources are generated or vendor
    let local_sources: Vec<&EvidenceBundleSource> = sources
        .iter()
        .filter(|s| s.trust == TrustLevel::LocalTrusted)
        .collect();
    if !local_sources.is_empty()
        && local_sources.iter().all(|s| {
            s.metadata
                .as_ref()
                .is_some_and(|m| m.is_generated == Some(true) || m.is_vendor == Some(true))
        })
    {
        let affected: Vec<String> = local_sources.iter().map(|s| s.source_id.clone()).collect();
        gaps.push(EvidenceGap {
            kind: EvidenceGapKind::LocalGeneratedOrVendorOnly,
            message: "all local sources are generated or vendor files".to_string(),
            source_id: None,
            provider_id: None,
            affected_source_ids: affected,
        });
    }

    // LocalSourceUnfetched: local sources that were not fetched
    for source in &local_sources {
        if !fetched_source_ids.contains(source.source_id.as_str()) {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::LocalSourceUnfetched,
                message: format!(
                    "local source '{}' was not fetched",
                    source.title.as_deref().unwrap_or(""),
                ),
                source_id: Some(source.source_id.clone()),
                provider_id: source.provider_id.clone(),
                affected_source_ids: vec![],
            });
        }
    }

    // Check warnings for known gap patterns
    for warning in warnings {
        let kind = match warning.message.as_str() {
            m if m.starts_with("native_code_search_unavailable") => {
                Some(EvidenceGapKind::NativeRepoFilterNotEnforced)
            }
            m if m.starts_with("symbol_hint_no_native_provider") => {
                Some(EvidenceGapKind::SymbolHintNoNativeProvider)
            }
            m if m.starts_with("issue_search_no_native_provider") => {
                Some(EvidenceGapKind::IssueSearchNoNativeProvider)
            }
            m if m.starts_with("release_search_no_native_provider") => {
                Some(EvidenceGapKind::ReleaseSearchNoNativeProvider)
            }
            m if m.starts_with("freshness_unenforced") => {
                Some(EvidenceGapKind::FreshnessNotEnforced)
            }
            m if m.starts_with("native_advisory_search_unavailable") => {
                Some(EvidenceGapKind::NativeAdvisoryUnavailable)
            }
            m if m.starts_with("package_resolution_fallback") => {
                Some(EvidenceGapKind::PackageResolutionFailed)
            }
            m if m.starts_with("version_match_unavailable") => {
                Some(EvidenceGapKind::NoFixedVersionFound)
            }
            m if m.starts_with("security_applicability_unknown") => {
                Some(EvidenceGapKind::SecurityApplicabilityUnknown)
            }
            m if m.starts_with("coding_profile_degraded") => {
                Some(EvidenceGapKind::ProviderDegraded)
            }
            m if m.starts_with("profile_degraded") => Some(EvidenceGapKind::ProviderDegraded),
            m if m.starts_with("local_repo_dirty") => Some(EvidenceGapKind::LocalCheckoutDirty),
            _ => None,
        };

        if let Some(kind) = kind {
            gaps.push(EvidenceGap {
                kind,
                message: warning.message.clone(),
                source_id: None,
                provider_id: Some(warning.provider_id.clone()),
                affected_source_ids: vec![],
            });
        }
    }

    // All results external untrusted check
    if !sources.is_empty()
        && sources
            .iter()
            .all(|s| s.trust == TrustLevel::ExternalUntrusted)
    {
        gaps.push(EvidenceGap {
            kind: EvidenceGapKind::AllResultsExternalUntrusted,
            message: "all sources are external untrusted content".to_string(),
            source_id: None,
            provider_id: None,
            affected_source_ids: vec![],
        });
    }

    // Detect missing complementary evidence (tests, examples, manifests, changelogs, security policy)
    detect_missing_complementary_evidence(&mut gaps, sources);

    gaps
}

/// Detect missing complementary evidence (tests, examples, manifests, changelogs, security policy).
fn detect_missing_complementary_evidence(
    gaps: &mut Vec<EvidenceGap>,
    sources: &[EvidenceBundleSource],
) {
    use crate::core::code_evidence::SourceRole;

    let source_role = |s: &EvidenceBundleSource| -> Option<SourceRole> {
        s.metadata
            .as_ref()
            .and_then(|m| m.code_evidence.as_ref())
            .and_then(|ce| ce.source_role)
    };

    let has_role =
        |role: SourceRole| -> bool { sources.iter().any(|s| source_role(s) == Some(role)) };

    let has_implementation = has_role(SourceRole::Implementation);

    if has_implementation {
        if !has_role(SourceRole::Test) {
            let affected: Vec<String> = sources
                .iter()
                .filter(|s| source_role(s) == Some(SourceRole::Implementation))
                .map(|s| s.source_id.clone())
                .collect();
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::MissingTests,
                message: "No test files found for implementation files".to_string(),
                source_id: None,
                provider_id: None,
                affected_source_ids: affected,
            });
        }

        if !has_role(SourceRole::Example) {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::MissingExamples,
                message: "No example files found for implementation files".to_string(),
                source_id: None,
                provider_id: None,
                affected_source_ids: vec![],
            });
        }

        if !has_role(SourceRole::Manifest) {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::MissingManifest,
                message: "No manifest found for code results".to_string(),
                source_id: None,
                provider_id: None,
                affected_source_ids: vec![],
            });
        }
    }

    let has_release_or_version = sources.iter().any(|s| {
        matches!(
            s.source_kind,
            Some(SourceKind::ReleaseNotes) | Some(SourceKind::Tag) | Some(SourceKind::Commit)
        )
    });
    if has_release_or_version && !has_role(SourceRole::Changelog) {
        gaps.push(EvidenceGap {
            kind: EvidenceGapKind::MissingChangelog,
            message: "No changelog found for version-related results".to_string(),
            source_id: None,
            provider_id: None,
            affected_source_ids: vec![],
        });
    }

    let has_security = sources
        .iter()
        .any(|s| matches!(s.source_kind, Some(SourceKind::SecurityAdvisory)));
    if has_security && !has_role(SourceRole::SecurityPolicy) {
        gaps.push(EvidenceGap {
            kind: EvidenceGapKind::MissingSecurityPolicy,
            message: "No security policy found for security-related results".to_string(),
            source_id: None,
            provider_id: None,
            affected_source_ids: vec![],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::evidence_bundle::{
        EvidenceBundleRequest, EvidenceFetchInput, EvidenceSourceInput,
    };

    fn make_source(url: &str, title: &str, provider: &str) -> EvidenceSourceInput {
        EvidenceSourceInput {
            id: Some(format!("src_{}", url.len())),
            url: Some(url.to_string()),
            title: Some(title.to_string()),
            snippet: None,
            providers: vec![provider.to_string()],
            score: Some(0.9),
            trust: Some(TrustLevel::ExternalUntrusted),
            trust_markers: None,
            metadata: None,
            quality: None,
        }
    }

    fn make_fetch(url: &str, source_id: Option<&str>) -> EvidenceFetchInput {
        EvidenceFetchInput {
            source_id: source_id.map(String::from),
            url: Some(url.to_string()),
            locator: None,
            fetched: true,
            content_type: None,
            language: None,
            selected_span: None,
            code_span_id: None,
            line_start: None,
            line_end: None,
            text: Some("fn main() {}".to_string()),
            truncated: false,
            trust: None,
            trust_markers: None,
            warnings: vec![],
        }
    }

    #[test]
    fn source_only_bundle() {
        let req = EvidenceBundleRequest {
            goal: Some("test".to_string()),
            sources: vec![
                make_source("https://docs.rs/axum", "axum", "duckduckgo"),
                make_source("https://crates.io/axum", "axum on crates.io", "brave"),
            ],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.sources.len(), 2);
        assert!(bundle.fetched_items.is_empty());
        assert!(bundle.bundle_id.starts_with("bundle_"));
        assert_eq!(bundle.provider_summary.providers_used.len(), 2);
    }

    #[test]
    fn source_with_fetch_bundle() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![make_source("https://docs.rs/axum", "axum", "duckduckgo")],
            fetches: vec![make_fetch("https://docs.rs/axum", None)],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.sources.len(), 1);
        assert_eq!(bundle.fetched_items.len(), 1);
        assert_eq!(bundle.source_links.len(), 1);
        assert_eq!(
            bundle.source_links[0].link_reason,
            EvidenceBundleLinkReason::UrlMatch
        );
    }

    #[test]
    fn deduplication_by_url() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![
                make_source("https://docs.rs/axum", "axum", "duckduckgo"),
                make_source("https://docs.rs/axum", "axum - Rust", "brave"),
            ],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.sources.len(), 1);
    }

    #[test]
    fn source_cap_truncation() {
        let sources: Vec<EvidenceSourceInput> = (0..10)
            .map(|i| {
                make_source(
                    &format!("https://example.com/{i}"),
                    &format!("page {i}"),
                    "test",
                )
            })
            .collect();

        let req = EvidenceBundleRequest {
            goal: None,
            sources,
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: Some(3),
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.sources.len(), 3);
        assert!(bundle.limits.sources_truncated);
    }

    #[test]
    fn fetch_cap_truncation() {
        let fetches: Vec<EvidenceFetchInput> = (0..10)
            .map(|i| make_fetch(&format!("https://example.com/{i}"), None))
            .collect();

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![],
            fetches,
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: Some(3),
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.fetched_items.len(), 3);
        assert!(bundle.limits.fetched_items_truncated);
    }

    #[test]
    fn total_chars_budget() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![],
            fetches: vec![
                EvidenceFetchInput {
                    source_id: None,
                    url: Some("https://a.com".to_string()),
                    locator: None,
                    fetched: true,
                    content_type: None,
                    language: None,
                    selected_span: None,
                    code_span_id: None,
                    line_start: None,
                    line_end: None,
                    text: Some("a".repeat(60)),
                    truncated: false,
                    trust: None,
                    trust_markers: None,
                    warnings: vec![],
                },
                EvidenceFetchInput {
                    source_id: None,
                    url: Some("https://b.com".to_string()),
                    locator: None,
                    fetched: true,
                    content_type: None,
                    language: None,
                    selected_span: None,
                    code_span_id: None,
                    line_start: None,
                    line_end: None,
                    text: Some("b".repeat(60)),
                    truncated: false,
                    trust: None,
                    trust_markers: None,
                    warnings: vec![],
                },
            ],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: Some(100),
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(bundle.limits.total_chars_exceeded);
        // Second item should be truncated or removed
        // Use chars().count() since budget is char-based but String::len() is byte-based
        let total: usize = bundle
            .fetched_items
            .iter()
            .filter_map(|f| f.text.as_ref().map(|t| t.chars().count()))
            .sum();
        assert!(total <= 100, "total chars {total} exceeded budget 100");
    }

    #[test]
    fn trust_markers_merge() {
        use crate::core::sanitize::TrustMarkers;

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![EvidenceSourceInput {
                id: None,
                url: Some("https://a.com".to_string()),
                title: Some("a".to_string()),
                snippet: None,
                providers: vec!["test".to_string()],
                score: None,
                trust: None,
                trust_markers: Some(TrustMarkers {
                    text_sanitized: true,
                    control_chars_removed: 5,
                    injection_hits: 1,
                    ..Default::default()
                }),
                metadata: None,
                quality: None,
            }],
            fetches: vec![EvidenceFetchInput {
                source_id: None,
                url: Some("https://a.com".to_string()),
                locator: None,
                fetched: true,
                content_type: None,
                language: None,
                selected_span: None,
                code_span_id: None,
                line_start: None,
                line_end: None,
                text: Some("content".to_string()),
                truncated: false,
                trust: None,
                trust_markers: Some(TrustMarkers {
                    text_sanitized: true,
                    control_chars_removed: 3,
                    injection_hits: 2,
                    ..Default::default()
                }),
                warnings: vec![],
            }],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(bundle.trust_summary.any_text_sanitized);
        assert_eq!(bundle.trust_summary.total_control_chars_removed, 8);
        assert_eq!(bundle.trust_summary.total_injection_hits, 3);
    }

    #[test]
    fn gap_computation_from_warnings() {
        use crate::core::result::SearchWarning;

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![SearchWarning::new(
                "_system",
                "freshness_unenforced: no provider supports freshness".to_string(),
            )],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(bundle
            .gaps
            .iter()
            .any(|g| g.kind == EvidenceGapKind::FreshnessNotEnforced));
    }

    #[test]
    fn gap_fetch_failed() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![],
            fetches: vec![EvidenceFetchInput {
                source_id: None,
                url: Some("https://example.com".to_string()),
                locator: None,
                fetched: false,
                content_type: None,
                language: None,
                selected_span: None,
                code_span_id: None,
                line_start: None,
                line_end: None,
                text: None,
                truncated: false,
                trust: None,
                trust_markers: None,
                warnings: vec!["timeout".to_string()],
            }],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(bundle
            .gaps
            .iter()
            .any(|g| g.kind == EvidenceGapKind::FetchFailed));
    }

    #[test]
    fn all_external_untrusted_gap() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![make_source("https://a.com", "a", "test")],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(bundle
            .gaps
            .iter()
            .any(|g| g.kind == EvidenceGapKind::AllResultsExternalUntrusted));
    }

    #[test]
    fn no_all_external_gap_with_local_source() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![EvidenceSourceInput {
                id: None,
                url: Some("workspace://root/src/main.rs".to_string()),
                title: Some("main.rs".to_string()),
                snippet: None,
                providers: vec!["local_workspace".to_string()],
                score: None,
                trust: Some(TrustLevel::LocalTrusted),
                trust_markers: None,
                metadata: None,
                quality: None,
            }],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(!bundle
            .gaps
            .iter()
            .any(|g| g.kind == EvidenceGapKind::AllResultsExternalUntrusted));
    }

    #[test]
    fn include_unfetched_false_filters_sources() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![
                make_source("https://a.com", "a", "test"),
                make_source("https://b.com", "b", "test"),
            ],
            fetches: vec![make_fetch("https://a.com", None)],
            include_unfetched_sources: Some(false),
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.sources.len(), 1);
        assert_eq!(bundle.sources[0].url.as_deref(), Some("https://a.com"));
    }

    #[test]
    fn bundle_deterministic_across_calls() {
        let make_req = || EvidenceBundleRequest {
            goal: Some("debug".to_string()),
            sources: vec![make_source("https://docs.rs/axum", "axum", "test")],
            fetches: vec![make_fetch("https://docs.rs/axum", None)],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let b1 = build_evidence_bundle(make_req());
        let b2 = build_evidence_bundle(make_req());
        assert_eq!(b1.bundle_id, b2.bundle_id);
        assert_eq!(b1.sources[0].source_id, b2.sources[0].source_id);
        assert_eq!(b1.fetched_items[0].fetch_id, b2.fetched_items[0].fetch_id);
    }

    #[test]
    fn provider_summary_aggregation() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![
                make_source("https://a.com", "a", "duckduckgo"),
                make_source("https://b.com", "b", "duckduckgo"),
                make_source("https://c.com", "c", "brave"),
            ],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.provider_summary.providers_used.len(), 2);
        let dd_count = bundle
            .provider_summary
            .per_provider_counts
            .iter()
            .find(|c| c.provider_id == "duckduckgo")
            .unwrap();
        assert_eq!(dd_count.count, 2);
    }

    #[test]
    fn local_checkout_dirty_from_metadata() {
        use crate::core::source_card::{LocalRepoMatch, SourceMetadata};

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![EvidenceSourceInput {
                id: None,
                url: Some("workspace://myproject/src/main.rs".to_string()),
                title: Some("main.rs".to_string()),
                snippet: None,
                providers: vec!["local_workspace".to_string()],
                score: None,
                trust: Some(TrustLevel::LocalTrusted),
                trust_markers: None,
                metadata: Some(SourceMetadata {
                    local_repo_match: Some(LocalRepoMatch {
                        matched: true,
                        dirty_state: Some("dirty".to_string()),
                        root_name: Some("myproject".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                quality: None,
            }],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(
            bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::LocalCheckoutDirty),
            "expected LocalCheckoutDirty gap from metadata, got: {:?}",
            bundle.gaps,
        );
    }

    #[test]
    fn local_checkout_dirty_from_warning() {
        use crate::core::result::SearchWarning;

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![SearchWarning::new(
                "local_workspace",
                "local_repo_dirty: local checkout has uncommitted changes".to_string(),
            )],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(
            bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::LocalCheckoutDirty),
            "expected LocalCheckoutDirty gap from warning, got: {:?}",
            bundle.gaps,
        );
    }

    // -- Source-to-fetch linking via stable_id --

    #[test]
    fn explicit_source_id_match_links_fetch() {
        use crate::core::identity::source_id;
        use crate::core::source_card::SourceKind;

        let source = make_source("https://docs.rs/axum", "axum", "duckduckgo");
        let computed_source_id = source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum"),
            Some(SourceKind::Unknown),
        );

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![source],
            fetches: vec![make_fetch(
                "https://other.com/page",
                Some(&computed_source_id),
            )],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.source_links.len(), 1);
        assert_eq!(
            bundle.source_links[0].link_reason,
            EvidenceBundleLinkReason::SourceIdMatch
        );
        assert_eq!(bundle.source_links[0].source_id, computed_source_id);
    }

    #[test]
    fn explicit_source_id_miss_falls_through_to_url() {
        let source = make_source("https://docs.rs/axum", "axum", "duckduckgo");

        // Fetch has a bogus source_id but matching URL — should link via URL match
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![source],
            fetches: vec![make_fetch("https://docs.rs/axum", Some("src_nonexistent"))],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.source_links.len(), 1);
        assert_eq!(
            bundle.source_links[0].link_reason,
            EvidenceBundleLinkReason::UrlMatch
        );
    }

    #[test]
    fn locator_match_links_fetch() {
        use crate::core::code_metadata::{CodeHost, CodeMetadata};
        use crate::core::repo_fetch::{RepoLocator, RepoLocatorKind};
        use crate::core::source_card::SourceMetadata;

        // Source with code metadata that produces structured_repo_fetch
        let source_input = EvidenceSourceInput {
            id: None,
            url: Some("https://github.com/tokio-rs/tokio/blob/main/src/lib.rs".to_string()),
            title: Some("tokio/src/lib.rs".to_string()),
            snippet: None,
            providers: vec!["duckduckgo".to_string()],
            score: None,
            trust: None,
            trust_markers: None,
            metadata: Some(SourceMetadata {
                source_kind: crate::core::source_card::SourceKind::SourceFile,
                domain: Some("github.com".to_string()),
                rank_reasons: vec![],
                code: Some(CodeMetadata {
                    host: Some(CodeHost::Github),
                    owner: Some("tokio-rs".to_string()),
                    repo: Some("tokio".to_string()),
                    path: Some("src/lib.rs".to_string()),
                    ref_name: None,
                    language: None,
                    symbol_hint: None,
                    line_start: None,
                    line_end: None,
                }),
                code_evidence: None,
                issue: None,
                release: None,
                vulnerability: None,
                local_repo_match: None,
                is_generated: None,
                is_vendor: None,
                is_test: None,
                is_example: None,
                is_config: None,
                is_lockfile: None,
            }),
            quality: None,
        };

        let fetch = EvidenceFetchInput {
            source_id: None,
            url: None,
            locator: Some(RepoLocator {
                kind: RepoLocatorKind::Remote,
                host: Some(CodeHost::Github),
                owner: Some("tokio-rs".to_string()),
                repo: Some("tokio".to_string()),
                ref_name: Some("main".to_string()),
                commit_sha: None,
                path: "src/lib.rs".to_string(),
                workspace_root: None,
            }),
            fetched: true,
            content_type: None,
            language: None,
            selected_span: None,
            code_span_id: None,
            line_start: None,
            line_end: None,
            text: Some("use tokio::main;".to_string()),
            truncated: false,
            trust: None,
            trust_markers: None,
            warnings: vec![],
        };

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![source_input],
            fetches: vec![fetch],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.source_links.len(), 1);
        assert_eq!(
            bundle.source_links[0].link_reason,
            EvidenceBundleLinkReason::LocatorMatch
        );
    }

    #[test]
    fn deduplication_by_stable_id() {
        use std::collections::HashSet;

        let sources: Vec<EvidenceSourceInput> = (0..5)
            .map(|i| {
                make_source(
                    &format!("https://example.com/page{i}"),
                    &format!("page {i}"),
                    "test",
                )
            })
            .collect();

        let req = EvidenceBundleRequest {
            goal: None,
            sources,
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert_eq!(bundle.sources.len(), 5);

        // All source_ids must be unique
        let ids: HashSet<_> = bundle.sources.iter().map(|s| &s.source_id).collect();
        assert_eq!(ids.len(), 5);
    }

    #[test]
    fn gap_analysis_by_stable_id() {
        // Two sources, only one fetched — should detect SourceUnfetched
        let s1 = make_source("https://docs.rs/axum", "axum", "ddg");
        let s2 = make_source("https://crates.io/axum", "axum crates", "brave");

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![s1, s2],
            fetches: vec![make_fetch("https://docs.rs/axum", None)],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        let unfetched: Vec<_> = bundle
            .gaps
            .iter()
            .filter(|g| g.kind == EvidenceGapKind::SourceUnfetched)
            .collect();
        assert_eq!(
            unfetched.len(),
            1,
            "expected exactly one SourceUnfetched gap"
        );
        assert!(
            unfetched[0].source_id.is_some(),
            "gap should reference a source_id"
        );
    }

    #[test]
    fn gap_analysis_detects_missing_tests() {
        use crate::core::code_evidence::{CodeEvidence, SourceRole};
        use crate::core::code_metadata::{CodeHost, CodeMetadata};
        use crate::core::source_card::SourceMetadata;

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![EvidenceSourceInput {
                id: None,
                url: Some("https://github.com/owner/repo/blob/main/src/lib.rs".to_string()),
                title: Some("lib.rs".to_string()),
                snippet: None,
                providers: vec!["duckduckgo".to_string()],
                score: None,
                trust: None,
                trust_markers: None,
                metadata: Some(SourceMetadata {
                    source_kind: crate::core::source_card::SourceKind::SourceFile,
                    domain: Some("github.com".to_string()),
                    rank_reasons: vec![],
                    code: Some(CodeMetadata {
                        host: Some(CodeHost::Github),
                        owner: Some("owner".to_string()),
                        repo: Some("repo".to_string()),
                        path: Some("src/lib.rs".to_string()),
                        ref_name: None,
                        language: None,
                        symbol_hint: None,
                        line_start: None,
                        line_end: None,
                    }),
                    code_evidence: Some(CodeEvidence {
                        host: Some(CodeHost::Github),
                        owner: Some("owner".to_string()),
                        repo: Some("repo".to_string()),
                        ref_name: None,
                        commit_sha: None,
                        path: Some("src/lib.rs".to_string()),
                        language: None,
                        source_role: Some(SourceRole::Implementation),
                        browser_url: None,
                        raw_url: None,
                        permalink_url: None,
                        raw_permalink_url: None,
                        match_line_start: None,
                        match_line_end: None,
                        context_line_start: None,
                        context_line_end: None,
                        matched_symbol: None,
                        symbol_kind: None,
                        enclosing_symbol: None,
                        evidence_confidence: None,
                        evidence_reasons: vec![],
                        imports: vec![],
                    }),
                    issue: None,
                    release: None,
                    vulnerability: None,
                    local_repo_match: None,
                    is_generated: None,
                    is_vendor: None,
                    is_test: None,
                    is_example: None,
                    is_config: None,
                    is_lockfile: None,
                }),
                quality: None,
            }],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(
            bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::MissingTests),
            "expected MissingTests gap, got: {:?}",
            bundle.gaps,
        );
        let missing_tests = bundle
            .gaps
            .iter()
            .find(|g| g.kind == EvidenceGapKind::MissingTests)
            .unwrap();
        assert_eq!(missing_tests.affected_source_ids.len(), 1);
    }

    #[test]
    fn gap_analysis_no_gap_when_tests_present() {
        use crate::core::code_evidence::{CodeEvidence, SourceRole};
        use crate::core::code_metadata::{CodeHost, CodeMetadata};
        use crate::core::source_card::SourceMetadata;

        let make_source_with_role = |role: SourceRole, url: &str, path: &str| EvidenceSourceInput {
            id: None,
            url: Some(url.to_string()),
            title: Some("file".to_string()),
            snippet: None,
            providers: vec!["duckduckgo".to_string()],
            score: None,
            trust: None,
            trust_markers: None,
            metadata: Some(SourceMetadata {
                source_kind: crate::core::source_card::SourceKind::SourceFile,
                domain: Some("github.com".to_string()),
                rank_reasons: vec![],
                code: Some(CodeMetadata {
                    host: Some(CodeHost::Github),
                    owner: Some("owner".to_string()),
                    repo: Some("repo".to_string()),
                    path: Some(path.to_string()),
                    ref_name: None,
                    language: None,
                    symbol_hint: None,
                    line_start: None,
                    line_end: None,
                }),
                code_evidence: Some(CodeEvidence {
                    host: Some(CodeHost::Github),
                    owner: Some("owner".to_string()),
                    repo: Some("repo".to_string()),
                    ref_name: None,
                    commit_sha: None,
                    path: Some(path.to_string()),
                    language: None,
                    source_role: Some(role),
                    browser_url: None,
                    raw_url: None,
                    permalink_url: None,
                    raw_permalink_url: None,
                    match_line_start: None,
                    match_line_end: None,
                    context_line_start: None,
                    context_line_end: None,
                    matched_symbol: None,
                    symbol_kind: None,
                    enclosing_symbol: None,
                    evidence_confidence: None,
                    evidence_reasons: vec![],
                    imports: vec![],
                }),
                issue: None,
                release: None,
                vulnerability: None,
                local_repo_match: None,
                is_generated: None,
                is_vendor: None,
                is_test: None,
                is_example: None,
                is_config: None,
                is_lockfile: None,
            }),
            quality: None,
        };

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![
                make_source_with_role(
                    SourceRole::Implementation,
                    "https://github.com/owner/repo/blob/main/src/lib.rs",
                    "src/lib.rs",
                ),
                make_source_with_role(
                    SourceRole::Test,
                    "https://github.com/owner/repo/blob/main/tests/lib.rs",
                    "tests/lib.rs",
                ),
            ],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(
            !bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::MissingTests),
            "should not have MissingTests gap when tests are present, got: {:?}",
            bundle.gaps,
        );
    }

    #[test]
    fn gap_analysis_missing_examples() {
        use crate::core::code_evidence::{CodeEvidence, SourceRole};
        use crate::core::code_metadata::{CodeHost, CodeMetadata};
        use crate::core::source_card::SourceMetadata;

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![EvidenceSourceInput {
                id: None,
                url: Some("https://github.com/owner/repo/blob/main/src/lib.rs".to_string()),
                title: Some("lib.rs".to_string()),
                snippet: None,
                providers: vec!["duckduckgo".to_string()],
                score: None,
                trust: None,
                trust_markers: None,
                metadata: Some(SourceMetadata {
                    source_kind: crate::core::source_card::SourceKind::SourceFile,
                    domain: Some("github.com".to_string()),
                    rank_reasons: vec![],
                    code: Some(CodeMetadata {
                        host: Some(CodeHost::Github),
                        owner: Some("owner".to_string()),
                        repo: Some("repo".to_string()),
                        path: Some("src/lib.rs".to_string()),
                        ref_name: None,
                        language: None,
                        symbol_hint: None,
                        line_start: None,
                        line_end: None,
                    }),
                    code_evidence: Some(CodeEvidence {
                        host: Some(CodeHost::Github),
                        owner: Some("owner".to_string()),
                        repo: Some("repo".to_string()),
                        ref_name: None,
                        commit_sha: None,
                        path: Some("src/lib.rs".to_string()),
                        language: None,
                        source_role: Some(SourceRole::Implementation),
                        browser_url: None,
                        raw_url: None,
                        permalink_url: None,
                        raw_permalink_url: None,
                        match_line_start: None,
                        match_line_end: None,
                        context_line_start: None,
                        context_line_end: None,
                        matched_symbol: None,
                        symbol_kind: None,
                        enclosing_symbol: None,
                        evidence_confidence: None,
                        evidence_reasons: vec![],
                        imports: vec![],
                    }),
                    issue: None,
                    release: None,
                    vulnerability: None,
                    local_repo_match: None,
                    is_generated: None,
                    is_vendor: None,
                    is_test: None,
                    is_example: None,
                    is_config: None,
                    is_lockfile: None,
                }),
                quality: None,
            }],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(
            bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::MissingExamples),
            "expected MissingExamples gap, got: {:?}",
            bundle.gaps,
        );
    }

    #[test]
    fn gap_analysis_missing_manifest() {
        use crate::core::code_evidence::{CodeEvidence, SourceRole};
        use crate::core::code_metadata::{CodeHost, CodeMetadata};
        use crate::core::source_card::SourceMetadata;

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![EvidenceSourceInput {
                id: None,
                url: Some("https://github.com/owner/repo/blob/main/src/lib.rs".to_string()),
                title: Some("lib.rs".to_string()),
                snippet: None,
                providers: vec!["duckduckgo".to_string()],
                score: None,
                trust: None,
                trust_markers: None,
                metadata: Some(SourceMetadata {
                    source_kind: crate::core::source_card::SourceKind::SourceFile,
                    domain: Some("github.com".to_string()),
                    rank_reasons: vec![],
                    code: Some(CodeMetadata {
                        host: Some(CodeHost::Github),
                        owner: Some("owner".to_string()),
                        repo: Some("repo".to_string()),
                        path: Some("src/lib.rs".to_string()),
                        ref_name: None,
                        language: None,
                        symbol_hint: None,
                        line_start: None,
                        line_end: None,
                    }),
                    code_evidence: Some(CodeEvidence {
                        host: Some(CodeHost::Github),
                        owner: Some("owner".to_string()),
                        repo: Some("repo".to_string()),
                        ref_name: None,
                        commit_sha: None,
                        path: Some("src/lib.rs".to_string()),
                        language: None,
                        source_role: Some(SourceRole::Implementation),
                        browser_url: None,
                        raw_url: None,
                        permalink_url: None,
                        raw_permalink_url: None,
                        match_line_start: None,
                        match_line_end: None,
                        context_line_start: None,
                        context_line_end: None,
                        matched_symbol: None,
                        symbol_kind: None,
                        enclosing_symbol: None,
                        evidence_confidence: None,
                        evidence_reasons: vec![],
                        imports: vec![],
                    }),
                    issue: None,
                    release: None,
                    vulnerability: None,
                    local_repo_match: None,
                    is_generated: None,
                    is_vendor: None,
                    is_test: None,
                    is_example: None,
                    is_config: None,
                    is_lockfile: None,
                }),
                quality: None,
            }],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(
            bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::MissingManifest),
            "expected MissingManifest gap, got: {:?}",
            bundle.gaps,
        );
    }

    #[test]
    fn gap_analysis_no_complementary_gaps_without_implementation() {
        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![make_source("https://docs.rs/axum", "axum docs", "ddg")],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(
            !bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::MissingTests),
            "should not detect MissingTests without implementation sources"
        );
        assert!(
            !bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::MissingExamples),
            "should not detect MissingExamples without implementation sources"
        );
        assert!(
            !bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::MissingManifest),
            "should not detect MissingManifest without implementation sources"
        );
    }

    #[test]
    fn gap_analysis_missing_changelog_for_release() {
        use crate::core::source_card::SourceMetadata;

        let req = EvidenceBundleRequest {
            goal: None,
            sources: vec![EvidenceSourceInput {
                id: None,
                url: Some("https://github.com/owner/repo/releases/tag/v1.0".to_string()),
                title: Some("v1.0 release".to_string()),
                snippet: None,
                providers: vec!["duckduckgo".to_string()],
                score: None,
                trust: None,
                trust_markers: None,
                metadata: Some(SourceMetadata {
                    source_kind: crate::core::source_card::SourceKind::ReleaseNotes,
                    ..Default::default()
                }),
                quality: None,
            }],
            fetches: vec![],
            include_unfetched_sources: None,
            max_sources: None,
            max_fetched_items: None,
            max_total_chars: None,
            warnings: vec![],
            research_claims: None,
            research_conflicts: None,
        };

        let bundle = build_evidence_bundle(req);
        assert!(
            bundle
                .gaps
                .iter()
                .any(|g| g.kind == EvidenceGapKind::MissingChangelog),
            "expected MissingChangelog gap for release results, got: {:?}",
            bundle.gaps,
        );
    }
}
