use crate::meta::engines::error::EngineError;
use crate::meta::engines::AdvisoryCapabilities;
use std::time::{Duration, Instant};

use super::{EngineList, MetadataSearchAdapter};

/// Native advisory operation executed by a selected provider.
#[derive(Clone, Debug)]
pub enum NativeAdvisoryOperation {
    /// Look up one advisory identifier.
    LookupById {
        /// Identifier supplied to the provider.
        vulnerability_id: String,
    },
    /// Query advisories for a package coordinate.
    QueryByPackage {
        /// Package ecosystem name.
        ecosystem: String,
        /// Package name.
        package: String,
        /// Optional package version.
        version: Option<String>,
    },
}

/// Terminal status for a provider-scoped advisory operation.
#[derive(Debug)]
pub enum ProviderAdvisoryStatus<T> {
    /// The selected provider does not implement this operation.
    CapabilityUnavailable,
    /// The adapter's global deadline elapsed before this provider ran.
    InterruptedByDeadline,
    /// The provider ran and returned a value or an error.
    Completed(Result<T, EngineError>),
}

/// Provider identity, operation, result, and duration for one advisory attempt.
#[derive(Debug)]
pub struct ProviderAdvisoryOutcome<T> {
    /// Canonical provider identifier supplied by the executing engine.
    pub provider_id: String,
    /// Operation sent to the provider.
    pub operation: NativeAdvisoryOperation,
    /// Terminal operation status.
    pub status: ProviderAdvisoryStatus<T>,
    /// Elapsed operation time in milliseconds.
    pub duration_ms: u64,
}

impl MetadataSearchAdapter {
    fn selected_advisory_engines(&self, allowed_provider_ids: &[String]) -> EngineList {
        if allowed_provider_ids.is_empty() {
            self.engines.clone()
        } else {
            self.select_engines(allowed_provider_ids).0
        }
    }

    /// Return advisory capabilities for the selected provider set.
    pub fn advisory_provider_capabilities(
        &self,
        allowed_provider_ids: &[String],
    ) -> Vec<(String, AdvisoryCapabilities)> {
        self.selected_advisory_engines(allowed_provider_ids)
            .into_iter()
            .map(|engine| (engine.name().to_string(), engine.advisory_capabilities()))
            .collect()
    }

    /// Execute an advisory ID lookup once per selected provider.
    pub async fn lookup_advisory_scoped(
        &self,
        allowed_provider_ids: &[String],
        vulnerability_id: &str,
    ) -> Vec<ProviderAdvisoryOutcome<Option<crate::core::security::VulnerabilityMetadata>>> {
        self.lookup_advisory_scoped_with_timeout(
            allowed_provider_ids,
            vulnerability_id,
            self.global_timeout,
        )
        .await
    }

    pub(crate) async fn lookup_advisory_scoped_with_timeout(
        &self,
        allowed_provider_ids: &[String],
        vulnerability_id: &str,
        timeout: Duration,
    ) -> Vec<ProviderAdvisoryOutcome<Option<crate::core::security::VulnerabilityMetadata>>> {
        let operation = NativeAdvisoryOperation::LookupById {
            vulnerability_id: vulnerability_id.to_string(),
        };
        let deadline = Instant::now() + timeout;
        let mut outcomes = Vec::new();
        for engine in self.selected_advisory_engines(allowed_provider_ids) {
            let provider_id = engine.name().to_string();
            let capabilities = engine.advisory_capabilities();
            let start = Instant::now();
            let status = if !capabilities.lookup_by_id {
                ProviderAdvisoryStatus::CapabilityUnavailable
            } else {
                let timeout = deadline.saturating_duration_since(Instant::now());
                if timeout.is_zero() {
                    ProviderAdvisoryStatus::InterruptedByDeadline
                } else {
                    match tokio::time::timeout(
                        timeout,
                        engine.lookup_advisory(vulnerability_id, timeout),
                    )
                    .await
                    {
                        Ok(result) => ProviderAdvisoryStatus::Completed(result),
                        Err(_) => ProviderAdvisoryStatus::InterruptedByDeadline,
                    }
                }
            };
            outcomes.push(ProviderAdvisoryOutcome {
                provider_id,
                operation: operation.clone(),
                status,
                duration_ms: start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            });
        }
        outcomes
    }

    /// Execute a package advisory query once per selected provider.
    pub async fn query_advisories_by_package_scoped(
        &self,
        allowed_provider_ids: &[String],
        ecosystem: &str,
        package: &str,
        version: Option<&str>,
        max_results: usize,
    ) -> Vec<ProviderAdvisoryOutcome<Vec<crate::core::security::VulnerabilityMetadata>>> {
        self.query_advisories_by_package_scoped_with_timeout(
            allowed_provider_ids,
            ecosystem,
            package,
            version,
            max_results,
            self.global_timeout,
        )
        .await
    }

    pub(crate) async fn query_advisories_by_package_scoped_with_timeout(
        &self,
        allowed_provider_ids: &[String],
        ecosystem: &str,
        package: &str,
        version: Option<&str>,
        max_results: usize,
        timeout: Duration,
    ) -> Vec<ProviderAdvisoryOutcome<Vec<crate::core::security::VulnerabilityMetadata>>> {
        let operation = NativeAdvisoryOperation::QueryByPackage {
            ecosystem: ecosystem.to_string(),
            package: package.to_string(),
            version: version.map(str::to_string),
        };
        let deadline = Instant::now() + timeout;
        let mut outcomes = Vec::new();
        for engine in self.selected_advisory_engines(allowed_provider_ids) {
            let provider_id = engine.name().to_string();
            let capabilities = engine.advisory_capabilities();
            let start = Instant::now();
            let status = if !capabilities.query_by_package {
                ProviderAdvisoryStatus::CapabilityUnavailable
            } else {
                let timeout = deadline.saturating_duration_since(Instant::now());
                if timeout.is_zero() {
                    ProviderAdvisoryStatus::InterruptedByDeadline
                } else {
                    match tokio::time::timeout(
                        timeout,
                        engine.query_advisories_by_package(
                            ecosystem,
                            package,
                            version,
                            max_results,
                            timeout,
                        ),
                    )
                    .await
                    {
                        Ok(result) => ProviderAdvisoryStatus::Completed(result),
                        Err(_) => ProviderAdvisoryStatus::InterruptedByDeadline,
                    }
                }
            };
            outcomes.push(ProviderAdvisoryOutcome {
                provider_id,
                operation: operation.clone(),
                status,
                duration_ms: start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            });
        }
        outcomes
    }

    /// Aggregate the first successful advisory lookup across enabled engines.
    pub async fn lookup_advisory(
        &self,
        vuln_id: &str,
    ) -> Result<Option<crate::core::security::VulnerabilityMetadata>, anyhow::Error> {
        let mut first_error = None;
        for outcome in self.lookup_advisory_scoped(&[], vuln_id).await {
            match outcome.status {
                ProviderAdvisoryStatus::CapabilityUnavailable => {}
                ProviderAdvisoryStatus::InterruptedByDeadline => {
                    if first_error.is_none() {
                        first_error = Some(EngineError::Timeout { engine: "adapter" });
                    }
                }
                ProviderAdvisoryStatus::Completed(Ok(Some(metadata))) => return Ok(Some(metadata)),
                ProviderAdvisoryStatus::Completed(Ok(None)) => {}
                ProviderAdvisoryStatus::Completed(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        match first_error {
            Some(error) => Err(anyhow::anyhow!(error)),
            None => Ok(None),
        }
    }

    /// Aggregate package advisories across enabled engines.
    pub async fn query_advisories_by_package(
        &self,
        ecosystem: &str,
        package: &str,
        version: Option<&str>,
        max_results: usize,
    ) -> Result<Vec<crate::core::security::VulnerabilityMetadata>, anyhow::Error> {
        let mut first_error = None;
        for outcome in self
            .query_advisories_by_package_scoped(&[], ecosystem, package, version, max_results)
            .await
        {
            match outcome.status {
                ProviderAdvisoryStatus::CapabilityUnavailable => {}
                ProviderAdvisoryStatus::InterruptedByDeadline => {
                    if first_error.is_none() {
                        first_error = Some(EngineError::Timeout { engine: "adapter" });
                    }
                }
                ProviderAdvisoryStatus::Completed(Ok(vulns)) if !vulns.is_empty() => {
                    return Ok(vulns)
                }
                ProviderAdvisoryStatus::Completed(Ok(_)) => {}
                ProviderAdvisoryStatus::Completed(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        match first_error {
            Some(error) => Err(anyhow::anyhow!(error)),
            None => Ok(Vec::new()),
        }
    }
}
