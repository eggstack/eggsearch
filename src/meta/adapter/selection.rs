//! The deterministic post-dispatch selection sequence.
//!
//! `MetadataSearchAdapter::web_search` and the mock-gated candidate
//! snapshot harness both call [`select`] so that fusion, dedup, local
//! domain filtering, reranking, and truncation have exactly one
//! implementation. [`SelectionParams::production`] is the only value
//! production passes; the harness passes explicit alternatives to seed
//! known-bad selectors through the same code path.

use crate::core::query::{Freshness, SearchIntent};
use crate::core::sanitize::TrustMarkers;
use crate::core::SourceCard;
use crate::meta::engines::models::SearchResult;

use super::normalization::{
    aggregate_rrf_with, apply_domain_filters, apply_intent_reranking, convert_aggregated, RRF_K,
};

/// Selector constants for the post-dispatch sequence.
///
/// `production()` is the production configuration: the standard RRF
/// constant, URL deduplication on, and truncation after reranking so
/// the enlarged candidate pool can promote results into the final
/// window.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectionParams {
    pub(crate) rrf_k: f64,
    pub(crate) dedup: bool,
    pub(crate) truncate_before_rerank: bool,
}

impl SelectionParams {
    /// The constants production uses.
    pub(crate) const fn production() -> Self {
        Self {
            rrf_k: RRF_K,
            dedup: true,
            truncate_before_rerank: false,
        }
    }
}

impl Default for SelectionParams {
    fn default() -> Self {
        Self::production()
    }
}

/// Query-scoped inputs for the selection sequence.
pub(crate) struct SelectionInputs<'a> {
    pub(crate) intent: SearchIntent,
    pub(crate) freshness: Freshness,
    pub(crate) include_domains: &'a [String],
    pub(crate) exclude_domains: &'a [String],
    pub(crate) sanitize: bool,
}

/// The retained cards plus the merged trust markers of that set.
pub(crate) struct SelectionOutcome {
    pub(crate) results: Vec<SourceCard>,
    pub(crate) trust_markers: TrustMarkers,
}

/// Run the production selection sequence over per-provider candidate
/// lists.
///
/// Order: RRF aggregation into the candidate pool, conversion to
/// source cards with the caller's sanitization policy, local domain
/// filtering when filters are present, bounded intent/freshness
/// reranking, then truncation to `final_max_results`.
pub(crate) fn select(
    raw_results: Vec<(String, Vec<SearchResult>)>,
    candidate_limit: usize,
    final_max_results: usize,
    inputs: SelectionInputs<'_>,
    params: SelectionParams,
) -> SelectionOutcome {
    let aggregated = aggregate_rrf_with(raw_results, candidate_limit, params.rrf_k, params.dedup);

    let mut results: Vec<SourceCard> = Vec::with_capacity(aggregated.len());
    let mut trust_markers = TrustMarkers::default();
    for a in aggregated {
        if let Some(card) = convert_aggregated(a, inputs.sanitize) {
            trust_markers.merge(&card.trust_markers);
            results.push(card);
        }
    }

    if !inputs.include_domains.is_empty() || !inputs.exclude_domains.is_empty() {
        results = apply_domain_filters(results, inputs.include_domains, inputs.exclude_domains);
        let mut filtered_markers = TrustMarkers::default();
        for card in &results {
            filtered_markers.merge(&card.trust_markers);
        }
        trust_markers = filtered_markers;
    }

    if params.truncate_before_rerank {
        results.truncate(final_max_results);
    }

    apply_intent_reranking(&mut results, inputs.intent, inputs.freshness);

    results.truncate(final_max_results);

    SelectionOutcome {
        results,
        trust_markers,
    }
}
