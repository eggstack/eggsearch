use crate::core::SearchWarning;
use crate::core::WebSearchRequest;
use crate::meta::engines::error::EngineError;
use crate::meta::engines::models::SearchResult;
use crate::meta::engines::EngineSearchRequest;
use crate::meta::planner::{build_search_plan, SearchPlan};
use crate::meta::provider_diagnostics::CapabilityEnforcementTelemetry;
use crate::meta::response::{ProviderFailure, WebSearchResponse};
use crate::meta::search_cache::{CacheDisposition, SearchCacheKey, SearchFanout};
use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use super::error::*;
use super::execution::*;
use super::normalization::*;
use super::selection::{select, SelectionInputs, SelectionParams};
use super::{EngineList, MetadataSearchAdapter};

impl MetadataSearchAdapter {
    /// Run a metasearch query. This is the primary entry point used by
    /// the MCP `web_search` tool. If `req.providers` is non-empty, only
    /// those engines are queried (and the caller is expected to have
    /// already rejected unknown ids via `select_engines`).
    ///
    /// `effective_max_results` is the caller's final SourceCard count
    /// (after the server's `max_results_cap` clamp). `max_results_cap`
    /// is the configured server cap used to bound the candidate pool
    /// requested from each provider so intent-aware reranking can
    /// promote results that would otherwise be truncated.
    ///
    /// Uses per-engine timeouts via `JoinSet` so that a global deadline
    /// preserves partial results from engines that responded in time.
    ///
    /// This is the explicit-intent entry point: it reports no derived
    /// intent and reranks with `req.intent`. The MCP `web_search` tool
    /// calls [`Self::web_search_with_intent`] instead, so omitting the
    /// caller's `intent` stays distinguishable from an explicit `web`.
    pub async fn web_search(
        &self,
        req: &WebSearchRequest,
        effective_max_results: usize,
        max_results_cap: usize,
    ) -> WebSearchResponse {
        self.web_search_with_intent(
            req,
            effective_max_results,
            max_results_cap,
            Some(req.intent),
        )
        .await
    }

    /// Run a metasearch query with an optional caller-supplied intent.
    ///
    /// `caller_intent == None` means the caller omitted `intent`:
    /// the query is classified once with `derive_intent` and the
    /// result is reported on the response. The planner always receives
    /// `req.intent`, so query text and provider selection stay
    /// byte-identical to the explicit-intent path. Reranking consumes
    /// the derived intent only when the intent module's apply switch
    /// is on.
    pub async fn web_search_with_intent(
        &self,
        req: &WebSearchRequest,
        effective_max_results: usize,
        max_results_cap: usize,
        caller_intent: Option<crate::core::query::SearchIntent>,
    ) -> WebSearchResponse {
        let final_max_results = effective_max_results;
        let candidate_cap = max_results_cap;
        let (engines, queried_ids) = self.selected_engines(&req.providers);

        // Per-request timeout override, bounded above by the global timeout.
        let effective_timeout = self.effective_timeout(req.timeout_ms);

        // Compute the candidate pool size BEFORE provider fan-out so
        // each provider is asked for the candidate limit rather than
        // the final return count. This is what lets intent-aware
        // reranking promote results just outside the final window.
        // When domain filters are present, double the pool (bounded by
        // the cap) so local post-filtering does not trivially starve
        // final results.
        let has_domain_filters = !req.include_domains.is_empty() || !req.exclude_domains.is_empty();
        let base_candidate = candidate_pool_size(final_max_results, candidate_cap);
        let candidate_limit = if has_domain_filters {
            base_candidate
                .saturating_mul(2)
                .min(candidate_cap.max(final_max_results))
        } else {
            base_candidate
        };

        let plan = build_search_plan(req, &queried_ids);

        let derived = caller_intent
            .is_none()
            .then(|| crate::meta::intent::derive_intent(&req.query, &plan.hints));
        let rerank_intent = match (
            &derived,
            crate::meta::intent::APPLY_DERIVED_INTENT_TO_RERANKING,
        ) {
            (Some(derived), true) => derived.intent,
            _ => req.intent,
        };

        debug!(
            query = %req.query,
            providers = ?queried_ids,
            final_max_results,
            candidate_limit,
            timeout_ms = effective_timeout.as_millis(),
            intent = %req.intent.as_str(),
            derived_intent = ?derived.as_ref().map(|d| d.intent.as_str()),
            derived_intent_reasons = ?derived.as_ref().map(|d| d.reasons.clone()),
            derived_intent_confidence = ?derived.as_ref().map(|d| d.confidence.as_str()),
            rerank_intent = %rerank_intent.as_str(),
            generic_query = %plan.generic_query,
            has_repo_hints = plan.hints.has_any(),
            "dispatching metasearch"
        );

        let (fanout, cache_disposition) = match self.search_cache.as_ref() {
            Some(cache) => {
                let cache_key = SearchCacheKey::from_request(
                    req,
                    caller_intent,
                    &queried_ids,
                    final_max_results,
                    candidate_limit,
                );
                cache
                    .resolve(cache_key, || {
                        self.dispatch_web_fanout(
                            &engines,
                            &plan,
                            req,
                            candidate_limit,
                            effective_timeout,
                            &queried_ids,
                        )
                    })
                    .await
                    .into_parts()
            }
            None => (
                self.dispatch_web_fanout(
                    &engines,
                    &plan,
                    req,
                    candidate_limit,
                    effective_timeout,
                    &queried_ids,
                )
                .await,
                CacheDisposition::Disabled,
            ),
        };

        debug!(
            providers = ?queried_ids,
            cache = cache_disposition.as_str(),
            "metasearch fan-out resolved"
        );

        let raw_results = fanout.lists;
        let raw_failures = fanout.failures;

        if cache_disposition.conducted_fanout() {
            self.record_provider_health(
                &queried_ids,
                &raw_results,
                &fanout.result_latencies_ms,
                &raw_failures,
                &fanout.failure_latencies_ms,
                effective_timeout.as_millis().min(u128::from(u64::MAX)) as u64,
            );
        }

        // Rank providers by `queried_ids` order once, up front: both
        // `providers_failed` and the attempts ledger are drained from a
        // JoinSet (task completion order), so both need this map to be
        // emitted deterministically.
        let provider_rank: std::collections::HashMap<&str, usize> = queried_ids
            .iter()
            .enumerate()
            .map(|(rank, id)| (id.as_str(), rank))
            .collect();

        // Build attempt records from raw results and failures before
        // aggregating/consuming them for provider failure classification.
        let mut web_search_attempts: Vec<crate::core::retrieval_status::RetrievalAttempt> =
            Vec::new();
        for (id, results) in &raw_results {
            web_search_attempts.push(crate::core::retrieval_status::RetrievalAttempt {
                provider_id: id.clone(),
                subquery_id: None,
                operation_id: None,
                intended_roles: crate::core::retrieval_status::map_provider_to_intended_roles(
                    id,
                    req.intent.as_str(),
                ),
                outcome: if results.is_empty() {
                    crate::core::retrieval_status::RetrievalAttemptOutcome::SuccessZeroResults
                } else {
                    crate::core::retrieval_status::RetrievalAttemptOutcome::SuccessWithResults
                },
                result_count: results.len(),
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: Default::default(),
                query_fingerprint: Some(
                    crate::core::retrieval_status::query_fingerprint_from_query(&req.query),
                ),
                duration_ms: None,
            });
        }
        for (id, err) in &raw_failures {
            let ec = classify(err);
            let outcome = match ec {
                ErrorClass::Timeout => {
                    crate::core::retrieval_status::RetrievalAttemptOutcome::TimedOut
                }
                ErrorClass::RateLimited => {
                    crate::core::retrieval_status::RetrievalAttemptOutcome::RateLimited
                }
                _ => crate::core::retrieval_status::RetrievalAttemptOutcome::Failed,
            };
            web_search_attempts.push(crate::core::retrieval_status::RetrievalAttempt {
                provider_id: id.clone(),
                subquery_id: None,
                operation_id: None,
                intended_roles: crate::core::retrieval_status::map_provider_to_intended_roles(
                    id,
                    req.intent.as_str(),
                ),
                outcome,
                result_count: 0,
                error_class: Some(ec.as_str().to_string()),
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: Default::default(),
                query_fingerprint: Some(
                    crate::core::retrieval_status::query_fingerprint_from_query(&req.query),
                ),
                duration_ms: None,
            });
        }

        // Collect the set of provider ids that already completed (success
        // or individual failure) so we don't double-count.
        let mut accounted: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (id, _) in &raw_results {
            accounted.insert(id.clone());
        }
        for (id, _) in &raw_failures {
            accounted.insert(id.clone());
        }

        // Aggregate up to the candidate pool size, convert, enforce
        // local domain filters, then run bounded intent/freshness
        // reranking before truncating to the caller's effective
        // `max_results`, so ranking results just outside the naive
        // top-k can still be promoted into the returned set.
        let outcome = select(
            raw_results,
            candidate_limit,
            final_max_results,
            SelectionInputs {
                intent: rerank_intent,
                freshness: req.freshness,
                include_domains: &req.include_domains,
                exclude_domains: &req.exclude_domains,
                sanitize: self.sanitize_output,
            },
            SelectionParams::production(),
        );
        let mut results = outcome.results;
        let trust_markers = outcome.trust_markers;

        // --- capability enforcement telemetry + warnings ---
        // Telemetry is the source of truth; human-readable warnings are
        // derived from the same decision so they cannot disagree.
        let telemetry = CapabilityEnforcementTelemetry::for_web_search(req, &queried_ids);
        let mut capability_warnings: Vec<SearchWarning> = Vec::new();

        if telemetry.not_enforced.iter().any(|c| c == "safe_search") {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "safe_search_unenforced: safe_search requested but no selected provider enforces safe search filtering",
            ));
        }
        if telemetry.not_enforced.iter().any(|c| c == "freshness") {
            capability_warnings.push(SearchWarning::new(
                "_system",
                format!(
                    "freshness_unenforced: freshness hint '{}' requested but no provider applies server-side freshness filtering",
                    req.freshness.as_str()
                ),
            ));
        }
        if telemetry.not_enforced.iter().any(|c| c == "date_range") {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "date_range_unenforced: exact date_range requested but no selected provider enforces server-side date filtering",
            ));
        }
        if telemetry.not_enforced.iter().any(|c| c == "language") {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "language_unenforced: language hint requested but no selected provider enforces language filtering",
            ));
        }
        if telemetry.not_enforced.iter().any(|c| c == "region") {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "region_unenforced: region hint requested but no selected provider enforces region filtering",
            ));
        }
        if telemetry.approximated.iter().any(|c| c == "domain_filters") {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "domain_filters_local: domain filters enforced locally on result URLs, not provider-native",
            ));
        }

        // 3. Code intent with no native code/repository providers.
        if req.intent == crate::core::query::SearchIntent::Code
            && !any_engine_supports(&engines, |c| {
                c.supports_code_search || c.supports_repo_filter
            })
        {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "native_code_search_unavailable: intent=code requested but no provider has native code/repository search; results are from generic text search",
            ));
        }

        // 4. Issues intent with no issue providers.
        if req.intent == crate::core::query::SearchIntent::Issues
            && !any_engine_supports(&engines, |c| c.supports_issue_search)
        {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "native_issue_search_unavailable: intent=issues requested but no provider has native issue search; results are from generic text search",
            ));
        }

        // 5. Releases intent with no release providers.
        if req.intent == crate::core::query::SearchIntent::Releases
            && !any_engine_supports(&engines, |c| c.supports_release_search)
        {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "native_release_search_unavailable: intent=releases requested but no provider has native release search; results are from generic text search",
            ));
        }

        // 6. Security intent with no advisory provider.
        if req.intent == crate::core::query::SearchIntent::Security
            && !any_engine_supports(&engines, |c| c.supports_security_search)
        {
            capability_warnings.push(SearchWarning::new(
                "_system",
                "native_advisory_search_unavailable: intent=security requested but no provider has native security advisory search; results are from generic/contextual search",
            ));
        }

        // Engines still in the join_set when the deadline hit are
        // considered timed-out. They were cancelled by the JoinSet drop.
        //
        // `raw_failures` is drained from a JoinSet, so its order is task
        // completion order. Emit in `queried_ids` order instead, matching the
        // shared `provider_failures` helper the other three workflows use, so
        // `providers_failed` and the warnings derived from it are deterministic.
        let mut providers_failed: Vec<ProviderFailure> = raw_failures
            .into_iter()
            .map(|(id, err)| ProviderFailure {
                id,
                error_class: classify(&err).as_str().to_string(),
                message: err.to_string(),
            })
            .collect();
        providers_failed.sort_by_key(|failure| {
            provider_rank
                .get(failure.id.as_str())
                .copied()
                .unwrap_or(usize::MAX)
        });

        for id in &queried_ids {
            if !accounted.contains(id.as_str()) {
                providers_failed.push(ProviderFailure {
                    id: id.clone(),
                    error_class: ErrorClass::Timeout.as_str().to_string(),
                    message: "provider timed out".to_string(),
                });
                web_search_attempts.push(crate::core::retrieval_status::RetrievalAttempt {
                    provider_id: id.clone(),
                    subquery_id: None,
                    operation_id: None,
                    intended_roles: crate::core::retrieval_status::map_provider_to_intended_roles(
                        id,
                        req.intent.as_str(),
                    ),
                    outcome:
                        crate::core::retrieval_status::RetrievalAttemptOutcome::InterruptedByDeadline,
                    result_count: 0,
                    error_class: None,
                    deadline_interrupted: true,
                    truncated: false,
                    truncation_evidence: Default::default(),
                    query_fingerprint: Some(crate::core::retrieval_status::query_fingerprint_from_query(&req.query)),
                    duration_ms: None,
                });
            }
        }

        // The attempts ledger is also completion-ordered (results, failures,
        // then the deadline rows appended above). Sort it into `queried_ids`
        // order so `retrieval_summary.dimensions` is stable across runs with
        // identical inputs and outcomes.
        web_search_attempts.sort_by_key(|attempt| {
            provider_rank
                .get(attempt.provider_id.as_str())
                .copied()
                .unwrap_or(usize::MAX)
        });

        let providers_queried: Vec<String> = queried_ids;

        let mut warnings: Vec<SearchWarning> = providers_failed
            .iter()
            .map(|f| SearchWarning::new(f.id.clone(), format!("[{}] {}", f.error_class, f.message)))
            .collect();
        warnings.extend(capability_warnings);

        crate::core::evidence_postprocess::materialize_evidence_roles(&mut results);

        let retrieval_failures = build_retrieval_failures(
            &providers_failed,
            &providers_queried,
            &web_search_attempts,
            "source",
        );
        let postprocess_result = crate::core::evidence_postprocess::postprocess(
            &results,
            &providers_failed,
            &providers_queried,
            None,
            &retrieval_failures,
            None,
            &web_search_attempts,
        );

        WebSearchResponse {
            query: req.query.clone(),
            mode: "live_metasearch",
            results,
            providers_queried,
            providers_failed,
            warnings,
            trust_markers,
            evidence_postprocess: Some(postprocess_result),
            capability_enforcement: if telemetry.requested.is_empty() {
                None
            } else {
                Some(telemetry)
            },
            derived_intent: derived
                .as_ref()
                .map(|derived| derived.intent.as_str().to_string()),
            derived_intent_reasons: derived
                .as_ref()
                .map(|derived| derived.reasons.iter().map(|r| (*r).to_string()).collect()),
        }
    }

    /// Dispatch the provider fan-out for `web_search`.
    ///
    /// Uses per-engine timeouts via `JoinSet` so that a global deadline
    /// preserves partial results from engines that responded in time.
    /// Reports whether any queried provider failed to answer before the
    /// deadline so the caller can refuse to cache a partial fan-out.
    async fn dispatch_web_fanout(
        &self,
        engines: &EngineList,
        plan: &SearchPlan,
        req: &WebSearchRequest,
        candidate_limit: usize,
        effective_timeout: Duration,
        queried_ids: &[String],
    ) -> SearchFanout {
        let mut join_set = tokio::task::JoinSet::new();
        let mut active_tasks: std::collections::HashMap<tokio::task::Id, (String, Instant)> =
            std::collections::HashMap::new();
        for engine in engines {
            let engine = Arc::clone(engine);
            let provider_id = engine.name().to_string();
            let provider_id_panic = provider_id.clone();
            let query = plan
                .provider_queries
                .get(&provider_id)
                .cloned()
                .unwrap_or_else(|| plan.generic_query.clone());
            let engine_request = EngineSearchRequest::from_web_request(
                req,
                query,
                candidate_limit,
                effective_timeout,
            );
            let spawn_started = Instant::now();
            let provider_id_for_map = provider_id.clone();
            let abort_handle = join_set.spawn(async move {
                let started = Instant::now();
                let inner = async move {
                    let result = engine.search(&engine_request).await;
                    (provider_id, result)
                };
                let outcome = AssertUnwindSafe(inner)
                    .catch_unwind()
                    .await
                    .unwrap_or_else(|_| {
                        (
                            provider_id_panic,
                            Err(crate::meta::engines::error::EngineError::NetworkError {
                                engine: "dispatch",
                                reason: "task panicked during dispatch".to_string(),
                            }),
                        )
                    });
                (
                    outcome.0,
                    outcome.1,
                    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                )
            });
            active_tasks.insert(abort_handle.id(), (provider_id_for_map, spawn_started));
        }

        let deadline = tokio::time::Instant::now() + effective_timeout;
        let mut raw_results: Vec<(String, Vec<SearchResult>)> = Vec::new();
        let mut result_latencies_ms: Vec<u64> = Vec::new();
        let mut raw_failures: Vec<(String, EngineError)> = Vec::new();
        let mut failure_latencies_ms: Vec<u64> = Vec::new();

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                warn!(
                    "metasearch global timeout exceeded with {} engines still pending",
                    join_set.len()
                );
                join_set.abort_all();
                break;
            }
            match tokio::time::timeout(remaining, join_set.join_next_with_id()).await {
                Ok(Some(Ok((task_id, (name, Ok(results), duration_ms))))) => {
                    active_tasks.remove(&task_id);
                    raw_results.push((name, results));
                    result_latencies_ms.push(duration_ms);
                }
                Ok(Some(Ok((task_id, (name, Err(err), duration_ms))))) => {
                    active_tasks.remove(&task_id);
                    raw_failures.push((name, err));
                    failure_latencies_ms.push(duration_ms);
                }
                Ok(Some(Err(join_err))) => {
                    warn!(?join_err, "engine task panicked");
                    if let Some((provider_id, started)) = active_tasks.remove(&join_err.id()) {
                        let latency_ms =
                            started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                        raw_failures.push((
                            provider_id,
                            crate::meta::engines::error::EngineError::NetworkError {
                                engine: "dispatch",
                                reason: "task panicked during dispatch".to_string(),
                            },
                        ));
                        failure_latencies_ms.push(latency_ms);
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    warn!(
                        "metasearch global timeout exceeded with {} engines still pending",
                        join_set.len()
                    );
                    join_set.abort_all();
                    break;
                }
            }
        }

        let mut accounted: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (id, _) in &raw_results {
            accounted.insert(id.clone());
        }
        for (id, _) in &raw_failures {
            accounted.insert(id.clone());
        }
        let deadline_interrupted = queried_ids.iter().any(|id| !accounted.contains(id));

        // Additional excerpts are opt-in: without explicit excerpt
        // demand, provider passages stay out of the merged cards so
        // default search output remains compact discovery-only cards.
        // Generic result timestamps are always preserved.
        if req.effective_excerpt_count() == 0 {
            for (_, results) in raw_results.iter_mut() {
                for result in results.iter_mut() {
                    result.excerpts.clear();
                }
            }
        }

        SearchFanout {
            lists: raw_results,
            failures: raw_failures,
            result_latencies_ms,
            failure_latencies_ms,
            deadline_interrupted,
        }
    }
}
