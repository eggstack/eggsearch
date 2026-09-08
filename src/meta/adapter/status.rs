use crate::core::provider::{
    built_in_provider_descriptor, provider_configured_state, CapabilityOption, ProviderDescriptor,
    ProviderSkipCode, KNOWN_PROVIDER_IDS,
};
use crate::meta::engines::error::EngineError;
use crate::meta::engines::models::SearchResult;
use crate::meta::provider_diagnostics::FailureClass;
use tracing::warn;

use super::error::*;
use super::MetadataSearchAdapter;

impl MetadataSearchAdapter {
    /// Record provider health from raw dispatch results and failures.
    /// Call after dispatch completes but before `provider_failures()`.
    /// Also records timeout failures for providers that never responded.
    /// `deadline_ms` is reported as the latency for providers that did
    /// not respond before the dispatch deadline.
    pub(crate) fn record_provider_health(
        &self,
        queried_ids: &[String],
        raw_results: &[(String, Vec<SearchResult>)],
        result_latencies_ms: &[u64],
        raw_failures: &[(String, EngineError)],
        failure_latencies_ms: &[u64],
        deadline_ms: u64,
    ) {
        if raw_results.len() != result_latencies_ms.len() {
            warn!(
                results_len = raw_results.len(),
                latencies_len = result_latencies_ms.len(),
                "raw_results and result_latencies_ms must be aligned; skipping health record"
            );
            return;
        }
        if raw_failures.len() != failure_latencies_ms.len() {
            warn!(
                failures_len = raw_failures.len(),
                latencies_len = failure_latencies_ms.len(),
                "raw_failures and failure_latencies_ms must be aligned; skipping health record"
            );
            return;
        }
        debug_assert_eq!(
            raw_results.len(),
            result_latencies_ms.len(),
            "raw_results and result_latencies_ms must be aligned"
        );
        debug_assert_eq!(
            raw_failures.len(),
            failure_latencies_ms.len(),
            "raw_failures and failure_latencies_ms must be aligned"
        );
        // Track which providers responded (success or failure)
        let mut responded: std::collections::HashSet<&str> = std::collections::HashSet::new();

        // Record successes (at least one result = success)
        for (index, (id, _)) in raw_results.iter().enumerate() {
            self.health.record_success(id, result_latencies_ms[index]);
            responded.insert(id.as_str());
        }

        // Record failures
        for (index, (id, err)) in raw_failures.iter().enumerate() {
            let class = classify(err);
            self.health.record_failure(
                id,
                class.into(),
                &err.to_string(),
                failure_latencies_ms[index],
            );
            responded.insert(id.as_str());
        }

        // Record timeout for providers that never responded
        for id in queried_ids {
            if !responded.contains(id.as_str()) {
                self.health.record_failure(
                    id,
                    FailureClass::Timeout,
                    "provider timed out",
                    deadline_ms,
                );
            }
        }
    }

    /// Check whether all queried providers support a given capability
    /// option. Returns the list of provider ids that do NOT support it.
    /// When `provider_ids` is empty, checks all enabled providers.
    pub fn unsupported_providers(
        &self,
        provider_ids: &[String],
        option: &CapabilityOption,
    ) -> Vec<String> {
        let to_check: Vec<&str> = if provider_ids.is_empty() {
            self.provider_ids.iter().map(|s| s.as_str()).collect()
        } else {
            provider_ids.iter().map(|s| s.as_str()).collect()
        };

        let mut unsupported = Vec::new();
        for id in &to_check {
            // Build descriptor to check capabilities.
            // For API providers not in KNOWN_PROVIDER_IDS, skip capability
            // check (they won't have a descriptor).
            let configured = if *id == "searxng" {
                self.searxng_configured
            } else if let Some(&configured) = self.api_configured.get(*id) {
                configured
            } else {
                true
            };
            if let Some(desc) =
                built_in_provider_descriptor(id, true, false, configured, false, None, None)
            {
                if !desc.capabilities.supports(option) {
                    unsupported.push(id.to_string());
                }
            }
        }
        unsupported
    }

    /// Per-provider status report. Includes both enabled providers in
    /// this adapter and the full set of known provider ids, so callers
    /// can see what is available vs. what is enabled.
    ///
    /// API-key providers (e.g. `github_code`, `brave_api`) are emitted
    /// from the [`api_configured`](Self::api_configured) map so their
    /// `configured` flag reflects the actual runtime env-var check,
    /// not a hardcoded `true`.
    pub fn provider_status(&self) -> Vec<ProviderDescriptor> {
        let enabled: std::collections::BTreeSet<&str> =
            self.provider_ids.iter().map(|s| s.as_str()).collect();
        let defaults: std::collections::BTreeSet<&str> =
            self.default_providers.iter().map(|s| s.as_str()).collect();
        let mut descriptors: Vec<ProviderDescriptor> = KNOWN_PROVIDER_IDS
            .iter()
            .filter_map(|id| {
                let is_enabled = enabled.contains(id);
                let is_default = defaults.contains(id);
                let configured = provider_configured_state(
                    id,
                    self.searxng_configured,
                    self.api_configured.get(*id).copied().unwrap_or(false),
                    false,
                );
                let routable = is_enabled && configured;
                let skip_code =
                    known_provider_skip_code(id, is_enabled, configured, self.searxng_configured);
                let skip_reason = provider_skip_reason(skip_code);
                built_in_provider_descriptor(
                    id,
                    is_enabled,
                    is_default,
                    configured,
                    routable,
                    skip_reason,
                    skip_code,
                )
            })
            .collect();

        for (id, &configured) in &self.api_configured {
            if KNOWN_PROVIDER_IDS.contains(&id.as_str()) {
                continue;
            }
            let is_enabled = enabled.contains(id.as_str());
            let is_default = defaults.contains(id.as_str());
            let routable = is_enabled && configured;
            let skip_code = if routable {
                None
            } else if !is_enabled {
                Some(ProviderSkipCode::NotBuilt)
            } else {
                Some(ProviderSkipCode::MissingApiKey)
            };
            let skip_reason = provider_skip_reason(skip_code);
            if let Some(desc) = built_in_provider_descriptor(
                id,
                is_enabled,
                is_default,
                configured,
                routable,
                skip_reason,
                skip_code,
            ) {
                descriptors.push(desc);
            }
        }

        descriptors
    }
}
