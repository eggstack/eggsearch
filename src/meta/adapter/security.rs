use crate::core::sanitize::TrustMarkers;
use crate::core::SearchWarning;
use crate::core::SourceCard;
use crate::core::WebSearchRequest;
use crate::meta::planner::build_search_plan;
use crate::meta::response::ProviderFailure;

use super::execution::*;
use super::normalization::*;
use super::{MetadataSearchAdapter, PlannedSubquery};

impl MetadataSearchAdapter {
    /// Run a security-oriented search with parallel dispatch. Generates
    /// security-specific subqueries, fans out to enabled providers via
    /// the bounded parallel dispatcher, aggregates results, and returns
    /// SourceCards for downstream grouping and advisory enrichment.
    /// Returns `(cards, warnings, providers_failed, trust_markers)`.
    pub async fn security_search_subqueries(
        &self,
        query: &str,
        providers: &[String],
        effective_max: usize,
        max_results_cap: usize,
        timeout_ms: Option<u64>,
    ) -> (
        Vec<SourceCard>,
        Vec<SearchWarning>,
        Vec<ProviderFailure>,
        TrustMarkers,
        Vec<crate::core::retrieval_status::RetrievalAttempt>,
    ) {
        use crate::core::query::SearchIntent;

        let (engines, queried_ids) = self.selected_engines(providers);
        let effective_timeout = self.effective_timeout(timeout_ms);
        let candidate_limit = candidate_pool_size(effective_max, max_results_cap);

        // Build search plan for the generic security query
        let mut web_req = WebSearchRequest::new(query.to_string());
        web_req.intent = SearchIntent::Security;
        web_req.providers = providers.to_vec();
        let plan = build_search_plan(&web_req, &queried_ids);

        // Generate security-specific subqueries with priorities
        let subqueries = vec![
            PlannedSubquery {
                label: "advisory".to_string(),
                query: plan.generic_query.clone(),
                priority: security_subquery_priority("advisory"),
                intended_roles: Vec::new(),
                repo_scope: None,
                excerpt_count: 0,
            },
            PlannedSubquery {
                label: "vendor".to_string(),
                query: format!("{query} vendor advisory security bulletin"),
                priority: security_subquery_priority("vendor"),
                intended_roles: Vec::new(),
                repo_scope: None,
                excerpt_count: 0,
            },
            PlannedSubquery {
                label: "defensive".to_string(),
                query: format!("{query} mitigation workaround fix patch"),
                priority: security_subquery_priority("defensive"),
                intended_roles: Vec::new(),
                repo_scope: None,
                excerpt_count: 0,
            },
        ];

        let dispatch = dispatch_subqueries(
            &engines,
            subqueries,
            candidate_limit,
            effective_timeout,
            "security_search",
            self.multiquery_concurrency,
            self.multiquery_provider_concurrency,
        )
        .await;

        let attempt_set =
            crate::meta::workflow::RetrievalAttemptSet::from(dispatch.attempts.clone());

        // Record provider health from raw results and failures
        self.record_provider_health(
            &queried_ids,
            &dispatch.raw_results,
            &dispatch.result_latencies_ms,
            &dispatch.raw_failures,
            &dispatch.failure_latencies_ms,
            effective_timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        );

        let providers_failed = provider_failures(
            &queried_ids,
            &dispatch.raw_results,
            &dispatch.raw_failures,
            attempt_set.as_slice(),
        );

        let mut warnings: Vec<SearchWarning> = Vec::new();
        push_deadline_warning(&mut warnings, "security_search", &dispatch.deadline);
        push_failure_warnings(&mut warnings, &dispatch.raw_results, &dispatch.raw_failures);

        // Aggregate into SourceCards
        let cards =
            aggregate_source_cards(dispatch.raw_results, candidate_limit, self.sanitize_output);
        let mut trust_markers = TrustMarkers::default();
        for card in &cards {
            trust_markers.merge(&card.trust_markers);
        }

        (
            cards,
            warnings,
            providers_failed,
            trust_markers,
            dispatch.attempts,
        )
    }
}

/// Assign priority for security_search subqueries. Lower = higher priority.
pub(crate) fn security_subquery_priority(label: &str) -> i32 {
    match label {
        "advisory" => 0,
        "vendor" => 1,
        "package" => 2,
        "patch" => 3,
        "defensive" => 4,
        "exploit" => 5,
        _ => 10,
    }
}
