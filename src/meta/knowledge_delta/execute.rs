use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;

use chrono::Utc;

use super::discovery::extract_subjects;
use super::index::{database_path, EvidenceIndex, IndexHit, IndexRecord};
use super::plan::{day_search_window, plan_subqueries};
use super::snapshot::BUNDLED_CUTOFF_SNAPSHOT;
use crate::core::config::AppConfig;
use crate::core::evidence_postprocess::build_retrieval_summary_from_attempts;
use crate::core::identity::{canonicalize_url, knowledge_delta_id};
use crate::core::knowledge_delta::{
    calendar_date_from_provider_timestamp, classify, reported_after_cutoff, resolve_cutoff,
    DateBasis, DatedEvidence, RefreshMode, ResolvedCutoff, EXCERPT_CHAR_CAP,
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

pub type EvidenceFetchFuture<'a> =
    Pin<Box<dyn Future<Output = Result<FetchedEvidence, String>> + Send + 'a>>;

pub trait KnowledgeEvidenceFetcher: Send + Sync {
    fn fetch<'a>(&'a self, url: &'a str, topic: &'a str) -> EvidenceFetchFuture<'a>;
}

#[derive(Debug)]
pub enum BriefFailure {
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
    let mut providers_ok = 0usize;
    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_millis(run.config.search.timeout_ms.max(1));
    if query.refresh.searches_live() {
        let planned = plan_subqueries(&query.topic, None);
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
    let mut deltas = package_deltas(&selected, cutoff.as_ref(), query, "live_checked");
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
                        delta.payload_chars = measure(delta);
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
    let reads_index = query.refresh.reads_index() || query.related_topics;
    let writes_index = query.retain;
    let mut index_to_retain = None;
    if reads_index || writes_index {
        match open_index(run, &query.partition) {
            Ok(index) => {
                index_status = "open";
                if query.refresh.reads_index() {
                    match index.lookup(&query.partition, &query.topic, query.max_items) {
                        Ok(hits) => merge_index_hits(&mut deltas, hits, query.refresh),
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
                            merge_expanded(&mut deltas, hits);
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
                    warnings
                        .push("index refresh did not search live; freshness is unverified".into());
                }
            }
        }
    }
    let (emitted, budget_exhausted, truncated) =
        budget_deltas(deltas, query.max_items, query.max_chars);
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
            "subqueries": if query.refresh.searches_live() { plan_subqueries(&query.topic, None).len() } else { 0 },
            "english_hints": true,
            "hints_are_not_translation": true,
            "date_range_applied": date_range_applied,
            "date_range_reason": date_range_reason,
            "freshness_unverified": query.refresh == RefreshMode::Index || index_status == "offline",
            "index_status": index_status,
            "direct_vs_expanded": { "direct": emitted.iter().filter(|item| !item.expanded).count(), "expanded": emitted.iter().filter(|item| item.expanded).count() },
            "expansion": expansion,
            "no_temporal_claim": cutoff.is_none(),
            "fetch_invoked": fetch_invoked,
            "fetch_candidates": fetch_candidates,
            "fetch_attempted": fetch_attempted,
            "fetch_completed": fetch_completed,
            "partition": query.partition,
        },
        "warnings": warnings,
        "truncated": truncated,
        "budget_exhausted": budget_exhausted,
        "next_actions": next_actions(&emitted),
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
        delta.payload_chars = measure(&delta);
        let _ = query;
        deltas.push(delta);
    }
    deltas.sort_by(|left, right| left.id.cmp(&right.id));
    deltas.dedup_by(|left, right| left.id == right.id);
    deltas
}

fn merge_index_hits(deltas: &mut Vec<SelectedDelta>, hits: Vec<IndexHit>, refresh: RefreshMode) {
    let live: BTreeSet<_> = deltas.iter().map(|item| item.id.clone()).collect();
    for hit in hits {
        if live.contains(&hit.record.kdelta_id) {
            if let Some(existing) = deltas
                .iter_mut()
                .find(|item| item.id == hit.record.kdelta_id)
            {
                existing.provenance = "live_checked".into();
            }
            continue;
        }
        if refresh == RefreshMode::Live {
            continue;
        }
        deltas.push(hit_delta(hit, "index_only", false));
    }
    deltas.sort_by(|left, right| left.id.cmp(&right.id));
}

fn merge_expanded(deltas: &mut Vec<SelectedDelta>, hits: Vec<IndexHit>) {
    let known: BTreeSet<_> = deltas.iter().map(|item| item.id.clone()).collect();
    for hit in hits {
        if known.contains(&hit.record.kdelta_id) {
            continue;
        }
        deltas.push(hit_delta(hit, "index_only", true));
    }
    deltas.sort_by(|left, right| left.id.cmp(&right.id));
}

fn hit_delta(hit: IndexHit, provenance: &str, expanded: bool) -> SelectedDelta {
    let mut delta = SelectedDelta {
        id: hit.record.kdelta_id,
        excerpt: hit.record.excerpt,
        trust: "external_untrusted".into(),
        source_ids: vec![hit.record.source_id],
        urls: vec![hit.record.canonical_url],
        date_basis: hit.record.basis,
        precision: hit.record.precision,
        temporal_relationship: hit.record.temporal_relationship,
        conflicted: hit.record.conflicted,
        payload_chars: 0,
        provenance: provenance.to_string(),
        edge_source_ids: hit.edge_source_ids,
        expanded,
        normalized_date: hit.record.normalized_date,
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
    delta.payload_chars = measure(&delta);
    delta
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

fn measure(delta: &SelectedDelta) -> usize {
    delta_json(delta).to_string().chars().count()
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
    let Some(cutoff) = cutoff else {
        return serde_json::json!({
            "model_id": model_id,
            "status": "unknown",
            "origin": "unknown",
            "provider_official": false,
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
    })
}

fn next_actions(deltas: &[SelectedDelta]) -> Vec<serde_json::Value> {
    let mut actions = Vec::new();
    if let Some(delta) = deltas.first() {
        if let Some(url) = delta.urls.first() {
            actions.push(serde_json::json!({
                "tool": "fetch",
                "reason_code": "read_cited_source",
                "priority": 1,
                "input_template": {"url": url},
                "source_ids": delta.source_ids,
            }));
        }
    }
    actions.push(serde_json::json!({
        "tool": "research_search",
        "reason_code": "broaden_evidence",
        "priority": 2,
        "input_template": {},
        "source_ids": [],
    }));
    actions
}
