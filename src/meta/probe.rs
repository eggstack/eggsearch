//! Shared provider liveness probe service.
//!
//! Used by CLI diagnostics, MCP `provider_status(probe=true)`, and
//! live-smoke tests so all three report the same outcome semantics.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::core::provider::{ProviderSkipCode, KNOWN_PROVIDER_IDS};
use crate::meta::adapter::{classify, ErrorClass, MetadataSearchAdapter};
use crate::meta::engines::error::EngineError;
use crate::meta::engines::EngineSearchRequest;
use crate::meta::provider_diagnostics::{bound_error_message, FailureClass};

/// Narrowest valid probe query proving transport/auth/parser viability.
pub const PROBE_QUERY: &str = "test";
/// Number of results demanded per probe; never consumes large sets.
pub const PROBE_MAX_RESULTS: usize = 1;
/// Per-provider probe deadline in milliseconds.
pub const PROBE_PER_PROVIDER_TIMEOUT_MS: u64 = 5_000;
/// Aggregate deadline for the whole probe batch in milliseconds.
pub const PROBE_AGGREGATE_TIMEOUT_MS: u64 = 20_000;
/// Maximum concurrent provider probes.
pub const PROBE_MAX_CONCURRENCY: usize = 4;
/// Maximum diagnostic message length in characters.
pub const PROBE_MAX_MESSAGE_CHARS: usize = 256;

/// Request for a provider liveness probe batch.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderProbeRequest {
    /// Provider ids to probe; empty means all known descriptors.
    #[serde(default)]
    pub providers: Vec<String>,
    /// Override per-provider timeout in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_per_provider_ms: Option<u64>,
    /// Override result demand per probe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
}

/// Machine-readable outcome for one provider probe.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct ProviderProbeOutcome {
    /// Stable provider id.
    pub provider_id: String,
    /// Whether a live request was attempted.
    pub attempted: bool,
    /// Whether the provider was routable at probe time.
    pub routable: bool,
    /// Whether the live request succeeded.
    pub success: bool,
    /// Stable failure class when attempted and failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<String>,
    /// Elapsed milliseconds for attempted probes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    /// Upstream HTTP status when a typed BadStatus error occurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    /// Stable skip code when not attempted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_code: Option<ProviderSkipCode>,
    /// Bounded sanitized diagnostic message, never raw payloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Typed probe section returned by `provider_status(probe=true)`.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderProbeSummary {
    /// Whether a probe was requested.
    pub requested: bool,
    /// Whether live probing is implemented.
    pub implemented: bool,
    /// Number of providers actually probed.
    pub started: usize,
    /// Number of successful probes.
    pub succeeded: usize,
    /// Number of failed probes.
    pub failed: usize,
    /// Number of skipped providers.
    pub skipped: usize,
    /// Per-provider outcomes in sorted id order.
    pub outcomes: Vec<ProviderProbeOutcome>,
}

impl ProviderProbeSummary {
    /// Cheap process-local response when no probe was requested.
    pub fn not_requested() -> Self {
        Self {
            requested: false,
            implemented: true,
            started: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            outcomes: Vec::new(),
        }
    }
}

/// Bound a diagnostic message for safe exposure.
pub fn bound_probe_message(msg: &str) -> Option<String> {
    let bounded = bound_error_message(msg)?;
    if bounded.chars().count() <= PROBE_MAX_MESSAGE_CHARS {
        return Some(bounded);
    }
    let truncated: String = bounded.chars().take(PROBE_MAX_MESSAGE_CHARS).collect();
    Some(format!("{truncated}…"))
}

fn skipped_outcome(
    provider_id: &str,
    routable: bool,
    skip_code: Option<ProviderSkipCode>,
) -> ProviderProbeOutcome {
    let message = skip_code.map(|c| format!("[{}] {}", c.as_str(), c.display_name()));
    ProviderProbeOutcome {
        provider_id: provider_id.to_string(),
        attempted: false,
        routable,
        success: false,
        failure_class: None,
        latency_ms: None,
        http_status: None,
        skip_code,
        message: message.and_then(|m| bound_probe_message(&m)),
    }
}

fn http_status_for(err: &EngineError) -> Option<u16> {
    match err {
        EngineError::BadStatus { status, .. } => Some(*status),
        _ => None,
    }
}

/// Probe routable providers with bounded concurrency and deadlines.
///
/// Non-routable providers yield skipped outcomes with stable skip codes
/// rather than network failures. Attempted probes record advisory health
/// state but never change explicit provider selection semantics.
pub async fn probe_providers(
    adapter: &MetadataSearchAdapter,
    request: ProviderProbeRequest,
) -> ProviderProbeSummary {
    let per_provider_ms = request
        .timeout_per_provider_ms
        .unwrap_or(PROBE_PER_PROVIDER_TIMEOUT_MS);
    let max_results = request.max_results.unwrap_or(PROBE_MAX_RESULTS).max(1);

    let descriptors = adapter.provider_status();
    let mut descriptor_map: BTreeMap<String, (bool, Option<ProviderSkipCode>)> = BTreeMap::new();
    for d in &descriptors {
        descriptor_map.insert(d.id.clone(), (d.routable, d.skip_code));
    }

    let target_ids: Vec<String> = if request.providers.is_empty() {
        descriptors.iter().map(|d| d.id.clone()).collect()
    } else {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for id in request.providers {
            if seen.insert(id.clone()) {
                out.push(id);
            }
        }
        out
    };

    let mut skipped: Vec<ProviderProbeOutcome> = Vec::new();
    let mut to_probe: Vec<(String, Arc<dyn crate::meta::engines::SearchEngine>)> = Vec::new();

    for id in target_ids {
        let (routable, skip_code) = match descriptor_map.get(&id) {
            Some((r, s)) => (*r, *s),
            None => {
                if KNOWN_PROVIDER_IDS.contains(&id.as_str()) {
                    (false, Some(ProviderSkipCode::NotBuilt))
                } else {
                    skipped.push(ProviderProbeOutcome {
                        provider_id: id.clone(),
                        attempted: false,
                        routable: false,
                        success: false,
                        failure_class: None,
                        latency_ms: None,
                        http_status: None,
                        skip_code: Some(ProviderSkipCode::UnknownProvider),
                        message: bound_probe_message(&format!(
                            "[{}] {}",
                            ProviderSkipCode::UnknownProvider.as_str(),
                            ProviderSkipCode::UnknownProvider.display_name()
                        )),
                    });
                    continue;
                }
            }
        };
        if !routable {
            skipped.push(skipped_outcome(&id, false, skip_code));
            continue;
        }
        let (engines, _) = adapter.select_engines(std::slice::from_ref(&id));
        if let Some(engine) = engines.into_iter().next() {
            to_probe.push((id, engine));
        } else {
            skipped.push(skipped_outcome(
                &id,
                false,
                skip_code.or(Some(ProviderSkipCode::NotBuilt)),
            ));
        }
    }

    if to_probe.is_empty() {
        let skipped_count = skipped.len();
        let mut outcomes = skipped;
        outcomes.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        return ProviderProbeSummary {
            requested: true,
            implemented: true,
            started: 0,
            succeeded: 0,
            failed: 0,
            skipped: skipped_count,
            outcomes,
        };
    }

    let semaphore = Arc::new(tokio::sync::Semaphore::new(PROBE_MAX_CONCURRENCY));
    let mut join_set = tokio::task::JoinSet::new();
    for (id, engine) in to_probe.clone() {
        let sem = semaphore.clone();
        let health = adapter.health().clone();
        let per_timeout = Duration::from_millis(per_provider_ms);
        join_set.spawn(async move {
            let _permit = sem.acquire_owned().await.map_err(|_| id.clone())?;
            let req = EngineSearchRequest::simple(PROBE_QUERY, max_results, per_timeout);
            let start = Instant::now();
            let outcome = tokio::time::timeout(per_timeout, async {
                let fut = engine.search(&req);
                futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(fut)).await
            })
            .await;
            let latency_ms = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            let probe_outcome = match outcome {
                Err(_) => {
                    health.record_failure(
                        &id,
                        FailureClass::Timeout,
                        "provider timed out",
                        latency_ms,
                    );
                    ProviderProbeOutcome {
                        provider_id: id.clone(),
                        attempted: true,
                        routable: true,
                        success: false,
                        failure_class: Some(ErrorClass::Timeout.as_str().to_string()),
                        latency_ms: Some(latency_ms),
                        http_status: None,
                        skip_code: None,
                        message: bound_probe_message("provider timed out"),
                    }
                }
                Ok(Err(_panic)) => {
                    health.record_failure(
                        &id,
                        FailureClass::Panic,
                        "task panicked during dispatch",
                        latency_ms,
                    );
                    ProviderProbeOutcome {
                        provider_id: id.clone(),
                        attempted: true,
                        routable: true,
                        success: false,
                        failure_class: Some(ErrorClass::Panic.as_str().to_string()),
                        latency_ms: Some(latency_ms),
                        http_status: None,
                        skip_code: None,
                        message: bound_probe_message("task panicked during dispatch"),
                    }
                }
                Ok(Ok(Err(e))) => {
                    let class = classify(&e);
                    let http_status = http_status_for(&e);
                    let msg = e.to_string();
                    health.record_failure(&id, class.into(), &msg, latency_ms);
                    ProviderProbeOutcome {
                        provider_id: id.clone(),
                        attempted: true,
                        routable: true,
                        success: false,
                        failure_class: Some(class.as_str().to_string()),
                        latency_ms: Some(latency_ms),
                        http_status,
                        skip_code: None,
                        message: bound_probe_message(&msg),
                    }
                }
                Ok(Ok(Ok(results))) => {
                    health.record_success(&id, latency_ms);
                    ProviderProbeOutcome {
                        provider_id: id.clone(),
                        attempted: true,
                        routable: true,
                        success: true,
                        failure_class: None,
                        latency_ms: Some(latency_ms),
                        http_status: None,
                        skip_code: None,
                        message: bound_probe_message(&format!("ok ({} result(s))", results.len())),
                    }
                }
            };
            Ok::<ProviderProbeOutcome, String>(probe_outcome)
        });
    }

    let aggregate_timeout = Duration::from_millis(PROBE_AGGREGATE_TIMEOUT_MS);
    let deadline = tokio::time::Instant::now() + aggregate_timeout;
    let mut probed: Vec<ProviderProbeOutcome> = Vec::new();
    let mut pending_ids: HashSet<String> = to_probe.iter().map(|(id, _)| id.clone()).collect();

    loop {
        if join_set.is_empty() {
            break;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            join_set.abort_all();
            break;
        }
        match tokio::time::timeout(remaining, join_set.join_next()).await {
            Ok(Some(Ok(Ok(outcome)))) => {
                pending_ids.remove(&outcome.provider_id);
                probed.push(outcome);
            }
            Ok(Some(Ok(Err(_)))) => {}
            Ok(Some(Err(_))) => {}
            Ok(None) => break,
            Err(_) => {
                join_set.abort_all();
                break;
            }
        }
    }

    for id in pending_ids {
        adapter.health().record_failure(
            &id,
            FailureClass::Timeout,
            "aggregate probe deadline exceeded",
            PROBE_AGGREGATE_TIMEOUT_MS,
        );
        probed.push(ProviderProbeOutcome {
            provider_id: id,
            attempted: true,
            routable: true,
            success: false,
            failure_class: Some(ErrorClass::Timeout.as_str().to_string()),
            latency_ms: Some(PROBE_AGGREGATE_TIMEOUT_MS),
            http_status: None,
            skip_code: None,
            message: bound_probe_message("aggregate probe deadline exceeded"),
        });
    }

    let started = probed.len();
    let succeeded = probed.iter().filter(|o| o.success).count();
    let failed = probed.iter().filter(|o| !o.success).count();
    let skipped_count = skipped.len();
    let mut outcomes = skipped;
    outcomes.extend(probed);
    outcomes.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));

    ProviderProbeSummary {
        requested: true,
        implemented: true,
        started,
        succeeded,
        failed,
        skipped: skipped_count,
        outcomes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bound_probe_message_truncates() {
        let long = "x".repeat(PROBE_MAX_MESSAGE_CHARS + 50);
        let bounded = bound_probe_message(&long).expect("bounded");
        assert!(bounded.chars().count() <= PROBE_MAX_MESSAGE_CHARS + 1);
    }

    #[test]
    fn bound_probe_message_empty_is_none() {
        assert!(bound_probe_message("   ").is_none());
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn probe_constants_are_bounded() {
        assert!(PROBE_PER_PROVIDER_TIMEOUT_MS > 0);
        assert!(PROBE_AGGREGATE_TIMEOUT_MS >= PROBE_PER_PROVIDER_TIMEOUT_MS);
        assert!(PROBE_MAX_CONCURRENCY > 0 && PROBE_MAX_CONCURRENCY <= 8);
        assert_eq!(PROBE_MAX_RESULTS, 1);
    }
}
