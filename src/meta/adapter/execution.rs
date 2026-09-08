use crate::core::provider::built_in_provider_descriptor;
use crate::core::sanitize::TrustMarkers;
use crate::core::SearchWarning;
use crate::core::SourceCard;
use crate::meta::engines::error::EngineError;
use crate::meta::engines::models::SearchResult;
use crate::meta::engines::SearchEngine;
use crate::meta::response::ProviderFailure;
use std::sync::Arc;
use std::time::Duration;

use super::error::*;
use super::normalization::*;
use super::PlannedSubquery;

pub(crate) async fn dispatch_subqueries(
    engines: &[Arc<dyn SearchEngine>],
    subqueries: Vec<PlannedSubquery>,
    candidate_limit: usize,
    effective_timeout: Duration,
    search_scope: &str,
    max_concurrent_jobs: usize,
    max_concurrent_per_provider: usize,
) -> crate::meta::dispatch::DispatchOutput {
    use crate::meta::dispatch::{
        dispatch_parallel, partition_roles_for_engine, CapabilityDisposition, DispatchConfig,
        DispatchJob,
    };

    let config = DispatchConfig {
        candidate_limit,
        global_timeout: effective_timeout,
        max_concurrent_jobs,
        max_concurrent_per_provider,
    };

    // Build flat job list: one (subquery, provider) pair per job
    let mut jobs = Vec::with_capacity(subqueries.len() * engines.len());
    for (subquery_idx, subquery) in subqueries.iter().enumerate() {
        for (provider_idx, engine) in engines.iter().enumerate() {
            let intended_roles = if !subquery.intended_roles.is_empty() {
                subquery.intended_roles.clone()
            } else {
                crate::core::retrieval_status::map_provider_to_intended_roles(
                    engine.name(),
                    &subquery.label,
                )
            };
            let partition = partition_roles_for_engine(engine.as_ref(), &intended_roles);
            if !partition.supported_roles.is_empty() {
                let capability_disposition = if partition.unsupported_roles.is_empty() {
                    CapabilityDisposition::FullySupported
                } else {
                    CapabilityDisposition::PartiallySupported {
                        supported_roles: partition.supported_roles.clone(),
                        unsupported_roles: partition.unsupported_roles.clone(),
                    }
                };
                jobs.push(DispatchJob {
                    subquery_id: subquery.label.clone(),
                    query: subquery.query.clone(),
                    provider_id: engine.name().to_string(),
                    provider: Arc::clone(engine),
                    priority: subquery.priority,
                    subquery_order: subquery_idx,
                    provider_order: provider_idx,
                    intended_roles: partition.supported_roles,
                    capability_disposition,
                    repo_scope: subquery.repo_scope.clone(),
                    excerpt_count: subquery.excerpt_count,
                });
            }
            if !partition.unsupported_roles.is_empty() {
                jobs.push(DispatchJob {
                    subquery_id: subquery.label.clone(),
                    query: subquery.query.clone(),
                    provider_id: engine.name().to_string(),
                    provider: Arc::clone(engine),
                    priority: subquery.priority,
                    subquery_order: subquery_idx,
                    provider_order: provider_idx,
                    intended_roles: partition.unsupported_roles.clone(),
                    capability_disposition: CapabilityDisposition::Unsupported {
                        unsupported_roles: partition.unsupported_roles,
                    },
                    repo_scope: subquery.repo_scope.clone(),
                    excerpt_count: subquery.excerpt_count,
                });
            }
        }
    }

    dispatch_parallel(jobs, config, search_scope).await
}

pub(crate) fn aggregate_source_cards(
    raw_results: Vec<(String, Vec<SearchResult>)>,
    candidate_limit: usize,
    sanitize_output: bool,
) -> Vec<SourceCard> {
    aggregate_rrf(raw_results, candidate_limit)
        .into_iter()
        .filter_map(|a| convert_aggregated(a, sanitize_output))
        .collect()
}

pub(crate) fn push_deadline_warning(
    warnings: &mut Vec<SearchWarning>,
    scope: &str,
    deadline: &crate::meta::dispatch::RequestDeadlineStats,
) {
    if deadline.exceeded {
        warnings.push(SearchWarning::new(
            "_system",
            format!(
                "request_deadline_exceeded: {scope} returned partial results ({} interrupted, {} skipped)",
                deadline.subqueries_interrupted, deadline.subqueries_skipped
            ),
        ));
    }
}

pub(crate) fn push_failure_warnings(
    warnings: &mut Vec<SearchWarning>,
    raw_results: &[(String, Vec<SearchResult>)],
    raw_failures: &[(String, EngineError)],
) {
    // Count successes per provider to detect partial failures
    let mut success_count: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (id, _) in raw_results {
        *success_count.entry(id.clone()).or_insert(0) += 1;
    }

    for (id, err) in raw_failures {
        let class = classify(err);
        let successes = success_count.get(id.as_str()).copied().unwrap_or(0);
        if successes > 0 {
            // Partial failure: some jobs succeeded, some failed
            warnings.push(SearchWarning::new(
                id.clone(),
                format!(
                    "[{}] {} (partial: {} job(s) succeeded for this provider)",
                    class.as_str(),
                    err,
                    successes
                ),
            ));
        } else {
            // Total failure
            warnings.push(SearchWarning::new(
                id.clone(),
                format!("[{}] {}", class.as_str(), err),
            ));
        }
    }
}

pub(crate) fn provider_failures(
    queried_ids: &[String],
    raw_results: &[(String, Vec<SearchResult>)],
    raw_failures: &[(String, EngineError)],
    attempts: &[crate::core::retrieval_status::RetrievalAttempt],
) -> Vec<ProviderFailure> {
    // Count successes and failures per provider, and track the last error class/message
    let mut success_count: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut failure_count: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut last_error_info: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    for (id, _) in raw_results {
        *success_count.entry(id.clone()).or_insert(0) += 1;
    }
    for (id, err) in raw_failures {
        *failure_count.entry(id.clone()).or_insert(0) += 1;
        last_error_info.insert(
            id.clone(),
            (classify(err).as_str().to_string(), err.to_string()),
        );
    }

    // A provider is only failed if ALL its jobs failed (no successes)
    // or if it was never responded to (timed out).
    let mut failures: Vec<ProviderFailure> = Vec::new();

    for id in queried_ids {
        let successes = success_count.get(id.as_str()).copied().unwrap_or(0);
        let fails = failure_count.get(id.as_str()).copied().unwrap_or(0);

        if successes == 0 && fails > 0 {
            // All jobs failed — report as failed
            if let Some((error_class, message)) = last_error_info.get(id) {
                failures.push(ProviderFailure {
                    error_class: error_class.clone(),
                    message: message.clone(),
                    id: id.clone(),
                });
            }
        } else if successes == 0 && fails == 0 {
            let only_skipped = {
                let mut saw_attempt = false;
                let mut all_skipped = true;
                for a in attempts {
                    if a.provider_id.as_str() != id.as_str() {
                        continue;
                    }
                    saw_attempt = true;
                    match a.outcome {
                        crate::core::retrieval_status::RetrievalAttemptOutcome::SkippedCapabilityUnavailable
                        | crate::core::retrieval_status::RetrievalAttemptOutcome::NotApplicable => {}
                        _ => {
                            all_skipped = false;
                            break;
                        }
                    }
                }
                saw_attempt && all_skipped
            };
            if only_skipped {
                continue;
            }
            // Never responded — timed out
            failures.push(ProviderFailure {
                id: id.clone(),
                error_class: ErrorClass::Timeout.as_str().to_string(),
                message: "provider timed out".to_string(),
            });
        }
        // If successes > 0, the provider is not failed even if some jobs failed.
        // Partial failures are reported as warnings by push_failure_warnings.
    }

    failures
}

pub(crate) fn merge_card_trust_markers<'a>(
    cards: impl IntoIterator<Item = &'a SourceCard>,
) -> TrustMarkers {
    let mut trust_markers = TrustMarkers::default();
    for card in cards {
        trust_markers.merge(&card.trust_markers);
    }
    trust_markers
}

/// Check whether any engine in the list supports a given capability.
pub(crate) fn any_engine_supports(
    engines: &[Arc<dyn SearchEngine>],
    check: impl Fn(&crate::core::provider::ProviderCapabilities) -> bool,
) -> bool {
    engines.iter().any(|e| {
        // `engines` only contains successfully built providers, so
        // `configured` is always true for this check. A missing API
        // key would have prevented the engine from being built.
        let configured = true;
        built_in_provider_descriptor(e.name(), true, false, configured, true, None, None)
            .is_some_and(|desc| check(&desc.capabilities))
    })
}

// ---------------------------------------------------------------------------
// Engine construction
// ---------------------------------------------------------------------------
