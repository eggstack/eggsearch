//! Mock-gated access to the production selection sequence.
//!
//! The candidate-snapshot replay harness in `tests/retrieval_quality.rs`
//! calls [`select`] so per-provider candidate lists are scored by the
//! same function `MetadataSearchAdapter::web_search` uses. Nothing here
//! exists in a default build (see the `mock` feature gate).

use crate::core::config::AppConfig;
use crate::core::query::{Freshness, SearchIntent};
use crate::meta::engines::models::SearchResult;

use super::normalization::candidate_pool_size;
use super::selection::{select as run_selection, SelectionInputs, SelectionParams};

/// The intent production derives for a query the caller omitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedIntentView {
    /// Classified intent.
    pub intent: SearchIntent,
    /// Stable reason codes, in precedence order.
    pub reasons: Vec<&'static str>,
    /// `high`, `medium`, or `low`.
    pub confidence: &'static str,
}

/// Classify `query` exactly as `web_search_with_intent` does when the
/// caller omits `intent`.
pub fn derive_intent(query: &str) -> DerivedIntentView {
    let bounded: String = query
        .chars()
        .take(crate::meta::intent::MAX_INTENT_QUERY_CHARS)
        .collect();
    let hints = crate::core::repo_query::RepoQueryHints::parse(&bounded);
    let derived = crate::meta::intent::derive_intent(&bounded, &hints);
    DerivedIntentView {
        intent: derived.intent,
        reasons: derived.reasons,
        confidence: derived.confidence.as_str(),
    }
}

/// Whether production reranking consumes the derived intent.
pub const fn reranking_uses_derived_intent() -> bool {
    crate::meta::intent::APPLY_DERIVED_INTENT_TO_RERANKING
}

/// Maximum query prefix the classifier scans.
pub const fn max_classified_query_chars() -> usize {
    crate::meta::intent::MAX_INTENT_QUERY_CHARS
}

/// One selected candidate row after the production selection sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectedRow {
    /// Content-derived stable id (`src_<16hex>`).
    pub stable_id: String,
    /// URL of the first provider that returned this document.
    pub url: String,
    /// Selected title, sanitized with the same policy as production.
    pub title: String,
    /// Providers that contributed to this row, in ascending id order.
    pub providers: Vec<String>,
    /// Final score after aggregation, domain filtering, and reranking.
    pub score: Option<f64>,
}

/// Explicit selector configuration for seeded negative controls.
///
/// [`SelectionKnobs::production`] is the configuration production uses.
/// The remaining constructors exist so a harness can prove it notices a
/// worse selector; they are never reachable from production code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionKnobs {
    /// RRF constant applied to every provider rank.
    pub rrf_k: f64,
    /// Whether rows that normalize to the same URL collapse into one slot.
    pub dedup: bool,
    /// Whether the candidate pool is cut to `max_results` before reranking.
    pub truncate_before_rerank: bool,
}

impl SelectionKnobs {
    /// The production constants.
    pub fn production() -> Self {
        let params = SelectionParams::production();
        Self {
            rrf_k: params.rrf_k,
            dedup: params.dedup,
            truncate_before_rerank: params.truncate_before_rerank,
        }
    }
}

impl Default for SelectionKnobs {
    fn default() -> Self {
        Self::production()
    }
}

/// Run the production selection sequence over per-provider candidate
/// lists.
///
/// `provider_lists` holds one `(provider_id, ranked results)` pair per
/// provider, in the order the fixture records them. The candidate pool
/// is sized with the configured `[search].max_results_cap` exactly as
/// `web_search` sizes it, and the final window is `max_results`.
pub fn select(
    provider_lists: Vec<(String, Vec<SearchResult>)>,
    max_results: usize,
    intent: SearchIntent,
) -> Vec<SelectedRow> {
    select_with_knobs(
        provider_lists,
        max_results,
        intent,
        SelectionKnobs::production(),
    )
}

/// Same sequence as [`select`] with explicit selector knobs.
///
/// Used only by seeded negative controls: a degenerate RRF constant, a
/// disabled dedup key, or truncation before reranking. The sequence
/// itself is unchanged; only the parameters differ.
pub fn select_with_knobs(
    provider_lists: Vec<(String, Vec<SearchResult>)>,
    max_results: usize,
    intent: SearchIntent,
    knobs: SelectionKnobs,
) -> Vec<SelectedRow> {
    let config = AppConfig::default();
    let candidate_limit = candidate_pool_size(max_results, config.search.max_results_cap);
    let outcome = run_selection(
        provider_lists,
        candidate_limit,
        max_results,
        SelectionInputs {
            intent,
            freshness: Freshness::Any,
            include_domains: &[],
            exclude_domains: &[],
            sanitize: config.search.sanitize_output,
        },
        SelectionParams {
            rrf_k: knobs.rrf_k,
            dedup: knobs.dedup,
            truncate_before_rerank: knobs.truncate_before_rerank,
        },
    );
    outcome
        .results
        .into_iter()
        .map(|card| SelectedRow {
            stable_id: card.stable_id.unwrap_or_default(),
            url: card.url,
            title: card.title,
            providers: card.providers,
            score: card.score,
        })
        .collect()
}

/// One provider intent routing considered, with the disposition it chose.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingOutcome {
    /// The provider id under consideration.
    pub provider_id: String,
    /// Stable disposition code, for example `added` or `not_enabled`.
    pub disposition: String,
}

fn routing_outcomes(attempts: Vec<crate::meta::routing::RoutingAttempt>) -> Vec<RoutingOutcome> {
    attempts
        .into_iter()
        .map(|attempt| RoutingOutcome {
            provider_id: attempt.provider_id.to_string(),
            disposition: attempt.disposition.as_str().to_string(),
        })
        .collect()
}

/// Replay the production default-path routing decision for `intent`.
///
/// `built` is the set of provider ids the adapter constructed, which is
/// how the snapshot lane models engine presence.
pub fn route_default(
    intent: SearchIntent,
    base: &[String],
    enabled: &[String],
    built: &[String],
    cooling: &[String],
    max_extra: usize,
) -> Vec<RoutingOutcome> {
    routing_outcomes(crate::meta::routing::select_intent_additions(
        intent, base, enabled, built, cooling, max_extra,
    ))
}

/// Same decision as [`route_default`] but with an explicit preference order,
/// so a seeded negative control can prove the lane notices a worse ordering.
pub fn route_with_order(
    intent: SearchIntent,
    providers: &'static [&'static str],
    base: &[String],
    enabled: &[String],
    built: &[String],
    cooling: &[String],
    max_extra: usize,
) -> Vec<RoutingOutcome> {
    routing_outcomes(crate::meta::routing::select_intent_additions_with_order(
        intent, providers, base, enabled, built, cooling, max_extra,
    ))
}

/// The providers intent routing appends for a default-path call.
pub fn intent_additions(
    intent: SearchIntent,
    base: &[String],
    enabled: &[String],
    built: &[String],
    cooling: &[String],
    max_extra: usize,
) -> Vec<String> {
    crate::meta::routing::added_ids(&crate::meta::routing::select_intent_additions(
        intent, base, enabled, built, cooling, max_extra,
    ))
}

/// The enabled and built providers that hold `intent`'s native capability,
/// in preference-table order.
pub fn capable_providers(intent: SearchIntent, built: &[String]) -> Vec<String> {
    crate::meta::routing::capable_built_providers(intent, built)
}

/// Every intent the preference table discloses, with its ordered provider
/// list. Empty lists are reported as empty, never omitted.
pub fn intent_preferences() -> Vec<(SearchIntent, Vec<String>)> {
    crate::meta::routing::INTENT_PROVIDER_PREFERENCES
        .iter()
        .map(|preference| {
            (
                preference.intent,
                preference
                    .providers
                    .iter()
                    .map(|id| (*id).to_string())
                    .collect(),
            )
        })
        .collect()
}
