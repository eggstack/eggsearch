use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;

use chrono::Utc;

use super::discovery::extract_subjects;
use super::index::{
    classify_index_freshness, database_path, EvidenceIndex, IndexFreshness, IndexHit, IndexRecord,
};
use super::plan::{day_search_window, plan_subqueries, EmptyStateReason};
use super::snapshot::BUNDLED_CUTOFF_SNAPSHOT;
use crate::core::config::AppConfig;
use crate::core::evidence_postprocess::build_retrieval_summary_from_attempts;
use crate::core::identity::{canonicalize_url, knowledge_delta_id};
use crate::core::knowledge_delta::{
    calendar_date_from_provider_timestamp, classify, compare_cutoff, parse_calendar_date,
    reported_after_cutoff, resolve_cutoff, DateBasis, DatePrecision, DatedEvidence, RefreshMode,
    ResolvedCutoff, TemporalRelationship, EXCERPT_CHAR_CAP,
};
use crate::core::query::{Freshness, SearchDateRange, WebSearchRequest};
use crate::core::retrieval_status::{
    RetrievalAttempt, RetrievalAttemptOutcome, TruncationEvidence,
};
use crate::core::sanitize::{bound_text, strip_control_chars};
use crate::core::source_card::SourceCard;
use crate::meta::adapter::MetadataSearchAdapter;
use crate::meta::fetch_ranking::{
    rank_and_select, DiversityConfig, FetchCandidateBuilder, FetchRankMode, RankContext,
};
use crate::meta::workflow::RetrievalAttemptSet;

#[derive(Clone, Debug)]
pub struct BriefQuery {
    pub topic: String,
    pub model_id: Option<String>,
    pub caller_cutoff: Option<ResolvedCutoff>,
    pub max_items: usize,
    pub max_chars: usize,
    pub refresh: RefreshMode,
    pub retain: bool,
    pub related_topics: bool,
    pub language: Option<String>,
    pub partition: String,
}

#[derive(Clone, Debug)]
pub struct SelectedDelta {
    pub id: String,
    pub excerpt: String,
    pub trust: String,
    pub source_ids: Vec<String>,
    pub urls: Vec<String>,
    pub date_basis: String,
    pub precision: String,
    pub temporal_relationship: String,
    pub conflicted: bool,
    pub payload_chars: usize,
    pub provenance: String,
    pub edge_source_ids: Vec<String>,
    pub expanded: bool,
    pub normalized_date: Option<String>,
    pub excerpt_clipped: bool,
    pub source_truncated: bool,
    pub source_title: String,
    pub spans: Vec<serde_json::Value>,
    pub indexable: bool,
}

#[derive(Clone, Debug)]
pub struct FetchedEvidence {
    pub excerpt: String,
    pub source_title: Option<String>,
    pub spans: Vec<serde_json::Value>,
    pub cacheable: bool,
    pub source_truncated: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DateAccounting {
    selected_cards: usize,
    dated: usize,
    undated: usize,
    at_or_before_cutoff: usize,
    not_after_cutoff: usize,
}

const MAX_REPORTED_PROVIDERS: usize = 64;

pub type EvidenceFetchFuture<'a> =
    Pin<Box<dyn Future<Output = Result<FetchedEvidence, String>> + Send + 'a>>;

pub trait KnowledgeEvidenceFetcher: Send + Sync {
    fn fetch<'a>(&'a self, url: &'a str, topic: &'a str) -> EvidenceFetchFuture<'a>;
}

#[derive(Debug)]
pub enum BriefFailure {
    InvalidInput(String),
    RetrievalFailed(String),
}

pub struct BriefRun<'a> {
    pub adapter: &'a MetadataSearchAdapter,
    pub config: &'a AppConfig,
    pub max_results_cap: usize,
    pub config_dir: std::path::PathBuf,
    pub evidence_fetcher: Option<&'a dyn KnowledgeEvidenceFetcher>,
}

pub async fn run_knowledge_brief(
    run: &BriefRun<'_>,
    query: &BriefQuery,
) -> Result<serde_json::Value, BriefFailure> {
    let planned = plan_subqueries(&query.topic, None, run.config.search.max_query_chars)
        .map_err(BriefFailure::InvalidInput)?;
    let cutoff = resolve_cutoff(
        query.model_id.as_deref(),
        query.caller_cutoff.as_ref(),
        &run.config.knowledge_delta.cutoffs,
        BUNDLED_CUTOFF_SNAPSHOT,
    );
    let today = Utc::now().date_naive();
    let window = cutoff
        .as_ref()
        .and_then(|item| day_search_window(&item.date, today));
    let mut attempts = RetrievalAttemptSet::new();
    let mut warnings = Vec::new();
    let mut cards = Vec::new();
    let mut selected_provider_ids = BTreeSet::new();
    let mut providers_ok = 0usize;
    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_millis(run.config.search.timeout_ms.max(1));
    if query.refresh.searches_live() {
        for (index, planned_query) in planned.iter().enumerate() {
            let mut request = WebSearchRequest::new(planned_query.query.clone());
            request.max_results = Some(query.max_items);
            request.freshness = Freshness::Any;
            request.language = query.language.clone();
            if let Some((start, end)) = window {
                request.date_range = Some(SearchDateRange::new(
                    start.format("%Y-%m-%d").to_string(),
                    end.format("%Y-%m-%d").to_string(),
                ));
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                warnings.push(
                    "knowledge_brief aggregate deadline reached during live retrieval".into(),
                );
                break;
            }
            let response = match tokio::time::timeout(
                remaining,
                run.adapter
                    .web_search(&request, query.max_items, run.max_results_cap),
            )
            .await
            {
                Ok(response) => response,
                Err(_) => {
                    warnings.push(
                        "knowledge_brief aggregate deadline interrupted live retrieval".into(),
                    );
                    break;
                }
            };
            for warning in &response.warnings {
                let line = format!("{}: {}", warning.provider_id, warning.message);
                if !warnings.contains(&line) {
                    warnings.push(line);
                }
            }
            selected_provider_ids.extend(response.providers_queried.iter().cloned());
            record_attempts(
                &mut attempts,
                index,
                &response.providers_queried,
                &response.providers_failed,
                &response.results,
            );
            providers_ok += response
                .providers_queried
                .len()
                .saturating_sub(response.providers_failed.len());
            cards.extend(response.results);
        }
        if providers_ok == 0 {
            return Err(BriefFailure::RetrievalFailed(
                "every selected provider failed; this is not evidence that no developments occurred".into(),
            ));
        }
    }
    let selected = select_cards(&cards);
    let date_accounting = account_dates(&selected, cutoff.as_ref());
    let mut deltas = package_deltas(&selected, cutoff.as_ref(), query, "live_checked");
    let provider_descriptors = run.adapter.provider_status();
    let selected_providers: Vec<String> = selected_provider_ids
        .into_iter()
        .take(MAX_REPORTED_PROVIDERS)
        .collect();
    let timestamp_capable_providers: Vec<String> = selected_providers
        .iter()
        .filter(|provider_id| {
            provider_descriptors.iter().any(|descriptor| {
                descriptor.id.as_str() == provider_id.as_str()
                    && descriptor.capabilities.supports_result_timestamps
            })
        })
        .cloned()
        .collect();
    if let Some(warning) =
        cutoff_unverifiable_warning(cutoff.as_ref(), &timestamp_capable_providers)
    {
        warnings.push(warning);
    }
    let fetch_candidates = deltas.len();
    let mut fetch_invoked = false;
    let mut fetch_attempted = 0usize;
    let mut fetch_completed = 0usize;
    if query.refresh.searches_live() {
        if let Some(fetcher) = run.evidence_fetcher {
            for delta in deltas.iter_mut().take(query.max_items.min(3)) {
                let Some(url) = delta.urls.first() else {
                    continue;
                };
                fetch_invoked = true;
                fetch_attempted += 1;
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    warnings.push(
                        "knowledge_brief aggregate deadline reached before source fetch".into(),
                    );
                    break;
                }
                match tokio::time::timeout(remaining, fetcher.fetch(url, &query.topic)).await {
                    Ok(Ok(evidence)) => {
                        fetch_completed += 1;
                        let (stripped, _) = strip_control_chars(&evidence.excerpt);
                        let (excerpt, clipped) = bound_text(&stripped, EXCERPT_CHAR_CAP);
                        if !excerpt.trim().is_empty() {
                            delta.excerpt = excerpt;
                            delta.spans = evidence.spans;
                            delta.excerpt_clipped |= clipped;
                        }
                        if let Some(title) = evidence.source_title {
                            let (title, _) = strip_control_chars(&title);
                            delta.source_title = bound_text(&title, 512).0;
                        }
                        delta.source_truncated = evidence.source_truncated;
                        delta.indexable = evidence.cacheable;
                        if evidence.source_truncated {
                            warnings.push(format!(
                                "source excerpt was fetched with truncation: {}",
                                delta.id
                            ));
                        }
                        measure(delta);
                    }
                    Ok(Err(_)) => {
                        warnings.push(format!("bounded source fetch failed: {}", delta.id))
                    }
                    Err(_) => {
                        warnings.push(format!("source fetch deadline interrupted: {}", delta.id))
                    }
                }
            }
        } else if !deltas.is_empty() {
            warnings.push("bounded source fetch path unavailable".into());
        }
    }
    let mut index_status = "not_opened";
    let mut expanded_count = 0usize;
    let mut index_matches = 0usize;
    let mut index_freshness = None;
    let reads_index = query.refresh.reads_index() || query.related_topics;
    let writes_index = query.retain;
    let mut index_to_retain = None;
    if reads_index || writes_index {
        if !run.config.knowledge_delta.index.enabled {
            index_status = "disabled";
            warnings.push(
                "knowledge_brief_index_disabled: the local index is disabled; set [knowledge_delta.index].enabled = true to read or retain evidence".into(),
            );
            if query.refresh == RefreshMode::Index {
                warnings.push("index refresh did not search live; freshness is unverified".into());
            }
        } else {
            match open_index(run, &query.partition) {
                Ok(index) => {
                    index_status = "open";
                    if query.refresh.reads_index() {
                        match index.status(&query.partition) {
                            Ok(status) => {
                                index_freshness = Some(classify_index_freshness(
                                    &status,
                                    run.config.knowledge_delta.index.retention_days,
                                    Utc::now().timestamp(),
                                ));
                            }
                            Err(err) => {
                                index_status = "degraded";
                                warnings.push(err.message());
                            }
                        }
                        match index.lookup(&query.partition, &query.topic, query.max_items) {
                            Ok(hits) => {
                                index_matches += merge_index_hits(
                                    &mut deltas,
                                    hits,
                                    query.refresh,
                                    cutoff.as_ref(),
                                );
                            }
                            Err(err) => {
                                index_status = "degraded";
                                warnings.push(err.message());
                            }
                        }
                    }
                    if query.related_topics {
                        let from = expansion_subjects(&query.topic, &deltas);
                        match index.expand(&query.partition, &from, query.max_items) {
                            Ok(hits) => {
                                let before = deltas.len();
                                merge_expanded(&mut deltas, hits, cutoff.as_ref());
                                expanded_count = deltas.len().saturating_sub(before);
                            }
                            Err(err) => warnings.push(err.message()),
                        }
                    }
                    if writes_index {
                        index_to_retain = Some(index);
                    }
                }
                Err(err) => {
                    index_status = "offline";
                    warnings.push(err.message());
                    if query.refresh == RefreshMode::Index {
                        warnings.push(
                            "index refresh did not search live; freshness is unverified".into(),
                        );
                    }
                }
            }
        }
    }
    let candidate_delta_count = deltas.len();
    let (emitted, budget_exhausted, truncated) =
        budget_deltas(deltas, query.max_items, query.max_chars);
    let empty_state_reason = empty_state_reason(
        cutoff.as_ref(),
        &date_accounting,
        candidate_delta_count,
        index_matches,
        query.refresh,
        index_status,
        index_freshness,
    );
    if empty_state_reason != EmptyStateReason::DeltasEmitted {
        warnings.push(empty_state_warning(empty_state_reason, index_status));
    } else if emitted.is_empty() && budget_exhausted {
        warnings.push(
            "knowledge_brief_deltas_emitted: matching delta candidates were omitted because the caller output budget was exhausted".into(),
        );
    }
    if let Some(index) = index_to_retain {
        let records = index_records(&emitted, &query.partition);
        if let Err(err) = index.retain(&records) {
            warnings.push(err.message());
        }
    }
    let summary = build_retrieval_summary_from_attempts(attempts.as_slice());
    let date_range_applied = window.is_some() && query.refresh.searches_live();
    let date_range_reason = if cutoff
        .as_ref()
        .is_some_and(|item| item.date.precision != crate::core::knowledge_delta::DatePrecision::Day)
    {
        "coarser_than_day"
    } else if window.is_none() && cutoff.is_some() {
        "window_empty"
    } else if date_range_applied {
        "applied"
    } else {
        "not_requested"
    };
    let expansion = if !query.related_topics {
        "not_requested"
    } else if expanded_count > 0 {
        "added"
    } else {
        "no_new_evidence"
    };
    let mut value = serde_json::json!({
        "topic": query.topic,
        "model_resolution": model_resolution(query.model_id.as_deref(), cutoff.as_ref()),
        "deltas": emitted.iter().map(delta_json).collect::<Vec<_>>(),
        "total_payload_chars": emitted.iter().map(|item| item.payload_chars).sum::<usize>(),
        "retrieval_summary": summary,
        "coverage": {
            "subqueries": if query.refresh.searches_live() { planned.len() } else { 0 },
            "english_hints": true,
            "hints_are_not_translation": true,
            "date_range_applied": date_range_applied,
            "date_range_reason": date_range_reason,
            "freshness_unverified": query.refresh == RefreshMode::Index || index_status == "offline",
            "index_status": index_status,
            "direct_vs_expanded": { "direct": emitted.iter().filter(|item| !item.expanded).count(), "expanded": emitted.iter().filter(|item| item.expanded).count() },
            "expansion": expansion,
            "no_temporal_claim": cutoff.is_none(),
            "selected_cards": date_accounting.selected_cards,
            "dated": date_accounting.dated,
            "undated": date_accounting.undated,
            "at_or_before_cutoff": date_accounting.at_or_before_cutoff,
            "not_after_cutoff": date_accounting.not_after_cutoff,
            "empty_state_reason": empty_state_reason.as_str(),
            "selected_providers": selected_providers,
            "timestamp_capable_providers": timestamp_capable_providers,
            "fetch_invoked": fetch_invoked,
            "fetch_candidates": fetch_candidates,
            "fetch_attempted": fetch_attempted,
            "fetch_completed": fetch_completed,
            "partition": query.partition,
        },
        "warnings": warnings,
        "truncated": truncated,
        "budget_exhausted": budget_exhausted,
        "next_actions": next_actions(&emitted, &query.topic),
    });
    if let Some(object) = value.as_object_mut() {
        if attempts.is_empty() && query.refresh == RefreshMode::Index {
            object.insert(
                "retrieval_summary".into(),
                serde_json::json!({"freshness": "unverified", "source": "index"}),
            );
        }
    }
    let _ = value;
    Ok(value)
}

fn open_index(
    run: &BriefRun<'_>,
    partition: &str,
) -> Result<EvidenceIndex, super::index::IndexError> {
    if !run.config.knowledge_delta.index.enabled {
        return Err(super::index::IndexError::Unavailable(
            "index is disabled; a live call does not open it unless retain or an index refresh was requested".into(),
        ));
    }
    let path = database_path(
        &run.config.knowledge_delta.index,
        &run.config_dir,
        partition,
    );
    EvidenceIndex::open(&path, &run.config.knowledge_delta.index)
}

fn record_attempts(
    attempts: &mut RetrievalAttemptSet,
    index: usize,
    queried: &[String],
    failed: &[crate::meta::response::ProviderFailure],
    results: &[SourceCard],
) {
    for provider in queried {
        let failure = failed.iter().find(|item| item.id == *provider);
        let count = results
            .iter()
            .filter(|card| card.providers.iter().any(|id| id == provider))
            .count();
        let (outcome, error_class) = if let Some(failure) = failure {
            let outcome = match failure.error_class.as_str() {
                "timeout" => RetrievalAttemptOutcome::TimedOut,
                "rate_limited" => RetrievalAttemptOutcome::RateLimited,
                _ => RetrievalAttemptOutcome::Failed,
            };
            (outcome, Some(failure.error_class.clone()))
        } else if count == 0 {
            (RetrievalAttemptOutcome::SuccessZeroResults, None)
        } else {
            (RetrievalAttemptOutcome::SuccessWithResults, None)
        };
        attempts.record(RetrievalAttempt {
            provider_id: provider.clone(),
            subquery_id: Some(format!("q{index}")),
            operation_id: Some(format!("q{index}:{provider}")),
            intended_roles: Vec::new(),
            outcome,
            result_count: count,
            error_class,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: TruncationEvidence::None,
            query_fingerprint: None,
            duration_ms: None,
        });
    }
}

fn account_dates(cards: &[SourceCard], cutoff: Option<&ResolvedCutoff>) -> DateAccounting {
    let mut accounting = DateAccounting {
        selected_cards: cards.len(),
        ..DateAccounting::default()
    };
    for card in cards {
        let date = card
            .metadata
            .published_at
            .as_deref()
            .and_then(calendar_date_from_provider_timestamp);
        let Some(date) = date else {
            accounting.undated += 1;
            continue;
        };
        accounting.dated += 1;
        if let Some(cutoff) = cutoff {
            let relation = compare_cutoff(&date, DateBasis::Publication, &cutoff.date);
            if relation == TemporalRelationship::OnOrBeforeCutoff {
                accounting.at_or_before_cutoff += 1;
            }
            if relation != TemporalRelationship::AfterCutoff {
                accounting.not_after_cutoff += 1;
            }
        }
    }
    accounting
}

fn empty_state_reason(
    cutoff: Option<&ResolvedCutoff>,
    accounting: &DateAccounting,
    candidate_delta_count: usize,
    index_matches: usize,
    refresh: RefreshMode,
    index_status: &str,
    index_freshness: Option<IndexFreshness>,
) -> EmptyStateReason {
    if cutoff.is_none() {
        EmptyStateReason::NoCutoffResolved
    } else if candidate_delta_count > 0 {
        EmptyStateReason::DeltasEmitted
    } else if refresh == RefreshMode::Index && index_matches == 0 {
        if index_status == "open" {
            match index_freshness {
                Some(IndexFreshness::NeverSeeded) => EmptyStateReason::IndexNotSeeded,
                Some(IndexFreshness::Stale) => EmptyStateReason::IndexStale,
                Some(IndexFreshness::Fresh) | None => EmptyStateReason::NoMatchingIndexHits,
            }
        } else {
            EmptyStateReason::NoMatchingIndexHits
        }
    } else if accounting.dated == 0 {
        EmptyStateReason::NoDatedSources
    } else {
        EmptyStateReason::AllSourcesAtOrBeforeCutoff
    }
}

fn cutoff_unverifiable_warning(
    cutoff: Option<&ResolvedCutoff>,
    timestamp_capable_providers: &[String],
) -> Option<String> {
    let cutoff = cutoff?;
    if !timestamp_capable_providers.is_empty() {
        return None;
    }
    let date = cutoff
        .date
        .normalized()
        .unwrap_or_else(|| cutoff.date.year.to_string());
    Some(format!(
        "knowledge_brief_cutoff_unverifiable_no_timestamp_providers: cutoff resolved to {date} (precision: {}, origin: {}) but no selected provider advertises result_timestamps, so the dated comparison is unverified",
        cutoff.date.precision.as_str(),
        cutoff.origin.as_str()
    ))
}

fn empty_state_warning(reason: EmptyStateReason, index_status: &str) -> String {
    let message = match reason {
        EmptyStateReason::NoCutoffResolved => {
            "no model or caller cutoff resolved; dated sources cannot be compared".to_string()
        }
        EmptyStateReason::NoDatedSources => {
            "selected live cards contain no parseable provider-declared publication timestamp"
                .to_string()
        }
        EmptyStateReason::AllSourcesAtOrBeforeCutoff => {
            "no selected dated source was reported after the resolved cutoff".to_string()
        }
        EmptyStateReason::NoMatchingIndexHits => format!(
            "index refresh returned no matching post-cutoff delta (index status: {index_status})"
        ),
        EmptyStateReason::IndexNotSeeded => {
            "the enabled index holds no retained documents for this partition; retain: true after a live call is the seeding step".to_string()
        }
        EmptyStateReason::IndexStale => {
            "every retained document is older than the retention window; a fresh retain renews it".to_string()
        }
        EmptyStateReason::DeltasEmitted => "a matching delta candidate was selected".to_string(),
    };
    format!("knowledge_brief_{}: {message}", reason.as_str())
}

fn select_cards(cards: &[SourceCard]) -> Vec<SourceCard> {
    let mut by_url: BTreeMap<String, SourceCard> = BTreeMap::new();
    for card in cards {
        let key = canonicalize_url(&card.url);
        by_url.entry(key).or_insert_with(|| card.clone());
    }
    let cards: Vec<_> = by_url.into_values().collect();
    let candidates: Vec<_> = cards
        .iter()
        .enumerate()
        .map(|(order, card)| {
            FetchCandidateBuilder::from_card(card, card.url.clone())
                .group("knowledge_brief".into())
                .original_order(order)
                .build()
        })
        .collect();
    let ctx = RankContext {
        mode: FetchRankMode::Research,
        ..RankContext::default()
    };
    let ranked = rank_and_select(
        candidates,
        &ctx,
        &DiversityConfig {
            max_per_domain: 2,
            max_per_group: 0,
            total_cap: cards.len(),
        },
    );
    let mut selected = Vec::new();
    for candidate in ranked {
        if let Some(card) = cards.iter().find(|card| card.url == candidate.url) {
            selected.push(card.clone());
        }
    }
    selected
}

fn package_deltas(
    cards: &[SourceCard],
    cutoff: Option<&ResolvedCutoff>,
    query: &BriefQuery,
    provenance: &str,
) -> Vec<SelectedDelta> {
    let Some(cutoff) = cutoff else {
        return Vec::new();
    };
    let mut deltas = Vec::new();
    for card in cards {
        let Some(raw) = card.metadata.published_at.as_deref() else {
            continue;
        };
        let Some(date) = calendar_date_from_provider_timestamp(raw) else {
            continue;
        };
        let classified = classify(
            &[DatedEvidence {
                date,
                basis: DateBasis::Publication,
            }],
            &cutoff.date,
        );
        let Some((basis, date)) = reported_after_cutoff(&classified) else {
            continue;
        };
        let Some(normalized) = date.normalized() else {
            continue;
        };
        let (excerpt, clipped) = excerpt_for(card);
        let id = knowledge_delta_id(
            &card.url,
            basis.as_str(),
            date.precision.as_str(),
            &normalized,
        );
        let source_id = card.stable_id.clone().unwrap_or_else(|| card.id.clone());
        let mut delta = SelectedDelta {
            id,
            excerpt,
            trust: "external_untrusted".into(),
            source_ids: vec![source_id],
            urls: vec![card.url.clone()],
            date_basis: basis.as_str().to_string(),
            precision: date.precision.as_str().to_string(),
            temporal_relationship: "after_cutoff".into(),
            conflicted: classified.judgments.iter().any(|item| item.conflicted),
            payload_chars: 0,
            provenance: provenance.to_string(),
            edge_source_ids: Vec::new(),
            expanded: false,
            normalized_date: Some(normalized),
            excerpt_clipped: clipped,
            source_truncated: false,
            source_title: card.title.clone(),
            spans: Vec::new(),
            indexable: false,
        };
        measure(&mut delta);
        let _ = query;
        deltas.push(delta);
    }
    deltas.sort_by(|left, right| left.id.cmp(&right.id));
    deltas.dedup_by(|left, right| left.id == right.id);
    deltas
}

fn merge_index_hits(
    deltas: &mut Vec<SelectedDelta>,
    hits: Vec<IndexHit>,
    refresh: RefreshMode,
    cutoff: Option<&ResolvedCutoff>,
) -> usize {
    let live: BTreeSet<_> = deltas.iter().map(|item| item.id.clone()).collect();
    let mut matched = 0usize;
    for hit in hits {
        if live.contains(&hit.record.kdelta_id) {
            if let Some(existing) = deltas
                .iter_mut()
                .find(|item| item.id == hit.record.kdelta_id)
            {
                existing.provenance = "live_checked".into();
                matched += 1;
            }
            continue;
        }
        if refresh == RefreshMode::Live {
            continue;
        }
        let Some(cutoff) = cutoff else {
            continue;
        };
        if let Some(delta) = hit_delta(hit, "index_only", false, cutoff) {
            matched += 1;
            deltas.push(delta);
        }
    }
    deltas.sort_by(|left, right| left.id.cmp(&right.id));
    matched
}

fn merge_expanded(
    deltas: &mut Vec<SelectedDelta>,
    hits: Vec<IndexHit>,
    cutoff: Option<&ResolvedCutoff>,
) {
    let known: BTreeSet<_> = deltas.iter().map(|item| item.id.clone()).collect();
    for hit in hits {
        if known.contains(&hit.record.kdelta_id) {
            continue;
        }
        let Some(cutoff) = cutoff else {
            continue;
        };
        if let Some(delta) = hit_delta(hit, "index_only", true, cutoff) {
            deltas.push(delta);
        }
    }
    deltas.sort_by(|left, right| left.id.cmp(&right.id));
}

fn hit_delta(
    hit: IndexHit,
    provenance: &str,
    expanded: bool,
    cutoff: &ResolvedCutoff,
) -> Option<SelectedDelta> {
    if hit.record.conflicted {
        return None;
    }
    let basis = DateBasis::parse(&hit.record.basis)?;
    let precision = DatePrecision::parse(&hit.record.precision)?;
    let normalized = hit.record.normalized_date.as_deref()?;
    let date = parse_calendar_date(normalized, precision)?;
    let classified = classify(&[DatedEvidence { date, basis }], &cutoff.date);
    let (basis, date) = reported_after_cutoff(&classified)?;
    let normalized_date = date.normalized()?;
    let mut delta = SelectedDelta {
        id: hit.record.kdelta_id,
        excerpt: hit.record.excerpt,
        trust: "external_untrusted".into(),
        source_ids: vec![hit.record.source_id],
        urls: vec![hit.record.canonical_url],
        date_basis: basis.as_str().into(),
        precision: date.precision.as_str().into(),
        temporal_relationship: "after_cutoff".into(),
        conflicted: false,
        payload_chars: 0,
        provenance: provenance.to_string(),
        edge_source_ids: hit.edge_source_ids,
        expanded,
        normalized_date: Some(normalized_date),
        excerpt_clipped: false,
        source_truncated: false,
        source_title: hit.record.title,
        spans: hit
            .edge_spans
            .into_iter()
            .map(|edge| serde_json::json!({ "source_id": edge.source_id, "span": edge.span }))
            .collect(),
        indexable: false,
    };
    measure(&mut delta);
    Some(delta)
}

fn index_records(deltas: &[SelectedDelta], partition: &str) -> Vec<IndexRecord> {
    deltas
        .iter()
        .filter(|delta| !delta.expanded && delta.provenance == "live_checked" && delta.indexable)
        .map(|delta| IndexRecord {
            partition: partition.to_string(),
            canonical_url: canonicalize_url(delta.urls.first().map(String::as_str).unwrap_or("")),
            source_id: delta.source_ids.first().cloned().unwrap_or_default(),
            title: delta.source_title.clone(),
            excerpt: delta.excerpt.clone(),
            trust: delta.trust.clone(),
            basis: delta.date_basis.clone(),
            precision: delta.precision.clone(),
            normalized_date: delta.normalized_date.clone(),
            kdelta_id: delta.id.clone(),
            conflicted: delta.conflicted,
            temporal_relationship: delta.temporal_relationship.clone(),
        })
        .collect()
}

fn excerpt_for(card: &SourceCard) -> (String, bool) {
    let raw = card.snippet.clone().unwrap_or_else(|| card.title.clone());
    let (stripped, _) = strip_control_chars(&raw);
    let (bounded, clipped) = bound_text(&stripped, EXCERPT_CHAR_CAP);
    (bounded, clipped)
}

fn measure(delta: &mut SelectedDelta) {
    loop {
        let measured = delta_json(delta).to_string().chars().count();
        if measured == delta.payload_chars {
            break;
        }
        delta.payload_chars = measured;
    }
}

fn delta_json(delta: &SelectedDelta) -> serde_json::Value {
    serde_json::json!({
        "id": delta.id,
        "excerpt": delta.excerpt,
        "trust": delta.trust,
        "source_ids": delta.source_ids,
        "urls": delta.urls,
        "spans": delta.spans,
        "source_truncated": delta.source_truncated,
        "date_basis": delta.date_basis,
        "precision": delta.precision,
        "temporal_relationship": delta.temporal_relationship,
        "conflicted": delta.conflicted,
        "payload_chars": delta.payload_chars,
        "provenance": delta.provenance,
        "edge_source_ids": delta.edge_source_ids,
        "expanded": delta.expanded,
    })
}

fn budget_deltas(
    deltas: Vec<SelectedDelta>,
    max_items: usize,
    max_chars: usize,
) -> (Vec<SelectedDelta>, bool, bool) {
    let mut emitted = Vec::new();
    let mut used = 0usize;
    let mut exhausted = false;
    for delta in deltas {
        if emitted.len() >= max_items || used.saturating_add(delta.payload_chars) > max_chars {
            exhausted = true;
            break;
        }
        used += delta.payload_chars;
        emitted.push(delta);
    }
    let truncated = emitted.iter().any(|item| item.excerpt_clipped);
    (emitted, exhausted, truncated)
}

fn expansion_subjects(topic: &str, deltas: &[SelectedDelta]) -> Vec<String> {
    let mut from = vec![topic.to_string()];
    for delta in deltas.iter().filter(|item| !item.expanded) {
        for subject in extract_subjects(topic, &delta.excerpt) {
            if subject.verified
                && !from
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(&subject.token))
            {
                from.push(subject.token);
            }
        }
    }
    from
}

fn model_resolution(model_id: Option<&str>, cutoff: Option<&ResolvedCutoff>) -> serde_json::Value {
    let catalog_as_of = BUNDLED_CUTOFF_SNAPSHOT
        .iter()
        .map(|row| row.verified_at)
        .max();
    let Some(cutoff) = cutoff else {
        return serde_json::json!({
            "model_id": model_id,
            "status": "unknown",
            "origin": "unknown",
            "provider_official": false,
            "catalog_as_of": catalog_as_of,
        });
    };
    serde_json::json!({
        "model_id": model_id,
        "status": "resolved",
        "cutoff_date": cutoff.date.normalized(),
        "precision": cutoff.date.precision.as_str(),
        "origin": cutoff.origin.as_str(),
        "source": cutoff.source,
        "source_url": cutoff.source_url,
        "verified_at": cutoff.verified_at,
        "reason": cutoff.reason,
        "provider_official": cutoff.origin == crate::core::knowledge_delta::CutoffOrigin::ProviderOfficial,
        "catalog_as_of": catalog_as_of,
    })
}

fn next_actions(
    deltas: &[SelectedDelta],
    topic: &str,
) -> Vec<crate::core::workflow::AgentNextAction> {
    let mut actions = Vec::new();
    if let Some(delta) = deltas.first() {
        if let Some(url) = delta.urls.first() {
            actions.push(crate::core::workflow::AgentNextAction::new(
                "fetch",
                "read_cited_source",
                1,
                serde_json::json!({"items": {"type": "web", "url": url}}),
                delta.source_ids.clone(),
                None,
            ));
        }
    }
    if !topic.trim().is_empty() {
        actions.push(crate::core::workflow::AgentNextAction::new(
            "research_search",
            "broaden_evidence",
            2,
            serde_json::json!({"query": topic}),
            Vec::new(),
            None,
        ));
    }
    crate::core::workflow::sanitize_next_actions(actions)
}

#[cfg(test)]
mod next_action_tests {
    use super::*;

    fn delta() -> SelectedDelta {
        SelectedDelta {
            id: "delta_1".to_string(),
            excerpt: "bounded excerpt".to_string(),
            trust: "external_untrusted".to_string(),
            source_ids: vec!["src_1".to_string()],
            urls: vec!["https://example.test/a".to_string()],
            date_basis: "published".to_string(),
            precision: "day".to_string(),
            temporal_relationship: "after_cutoff".to_string(),
            conflicted: false,
            payload_chars: 16,
            provenance: "provider".to_string(),
            edge_source_ids: vec![],
            expanded: false,
            normalized_date: None,
            excerpt_clipped: false,
            source_truncated: false,
            source_title: "Example".to_string(),
            spans: vec![],
            indexable: false,
        }
    }

    #[test]
    fn knowledge_brief_fetch_action_is_an_items_envelope() {
        let actions = next_actions(&[delta()], "rust async runtimes");
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].tool, "fetch");
        assert_eq!(actions[0].input_template["items"]["type"], "web");
        assert_eq!(
            actions[0].input_template["items"]["url"],
            "https://example.test/a"
        );
        assert_eq!(actions[1].tool, "research_search");
        assert_eq!(actions[1].input_template["query"], "rust async runtimes");
    }

    #[test]
    fn an_empty_topic_omits_the_broaden_action() {
        let actions = next_actions(&[delta()], "   ");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].tool, "fetch");
    }
}

#[cfg(test)]
mod cutoff_warning_tests {
    use super::*;

    fn cutoff() -> ResolvedCutoff {
        ResolvedCutoff {
            date: parse_calendar_date("2024-06-01", DatePrecision::Day).expect("date"),
            source: "test".into(),
            source_url: None,
            verified_at: None,
            origin: crate::core::knowledge_delta::CutoffOrigin::CallerOverride,
            reason: Some("unit test".into()),
        }
    }

    #[test]
    fn warns_only_for_a_resolved_cutoff_without_timestamp_capable_providers() {
        let cutoff = cutoff();
        let warning = cutoff_unverifiable_warning(Some(&cutoff), &[]).expect("warning");
        assert_eq!(
            warning,
            "knowledge_brief_cutoff_unverifiable_no_timestamp_providers: cutoff resolved to 2024-06-01 (precision: day, origin: caller_override) but no selected provider advertises result_timestamps, so the dated comparison is unverified"
        );
        assert!(cutoff_unverifiable_warning(None, &[]).is_none());
        assert!(cutoff_unverifiable_warning(Some(&cutoff), &["hn_algolia".into()]).is_none());
    }
}
