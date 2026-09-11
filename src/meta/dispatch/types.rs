use std::sync::Arc;
use std::time::Duration;

use crate::core::evidence_role::EvidenceRole;
use crate::core::retrieval_status::RetrievalAttempt;
use crate::meta::engines::error::EngineError;
use crate::meta::engines::models::SearchResult;
use crate::meta::engines::SearchEngine;

/// A single (subquery, provider) job to dispatch.
pub(crate) struct DispatchJob {
    /// Stable subquery identifier (label or id).
    pub subquery_id: String,
    /// The query text for this job.
    pub query: String,
    /// Provider engine identifier.
    pub provider_id: String,
    /// The engine to dispatch to.
    pub provider: Arc<dyn SearchEngine>,
    /// Lower number = higher priority. Ties broken by subquery_order then provider_order.
    pub priority: i32,
    /// Stable subquery ordering (assigned before dispatch).
    pub subquery_order: usize,
    /// Stable provider ordering within the subquery.
    pub provider_order: usize,
    /// Evidence roles this job was intended to produce.
    pub intended_roles: Vec<EvidenceRole>,
    pub capability_disposition: CapabilityDisposition,
    /// Provider-neutral repository scope for native repo filtering.
    pub repo_scope: Option<crate::meta::engines::request::RepoScope>,
    /// Bounded excerpt demand for this job.
    pub excerpt_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CapabilityDisposition {
    FullySupported,
    PartiallySupported {
        supported_roles: Vec<EvidenceRole>,
        unsupported_roles: Vec<EvidenceRole>,
    },
    Unsupported {
        unsupported_roles: Vec<EvidenceRole>,
    },
    #[allow(dead_code)]
    NotApplicable {
        roles: Vec<EvidenceRole>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Deterministic supported and unsupported role subsets for one provider.
pub struct RoleCapabilityPartition {
    /// Roles this provider can execute.
    pub supported_roles: Vec<EvidenceRole>,
    /// Roles this provider cannot execute.
    pub unsupported_roles: Vec<EvidenceRole>,
}

/// Deduplicate and partition intended roles according to provider capability.
pub fn partition_roles_for_engine(
    engine: &dyn SearchEngine,
    intended_roles: &[EvidenceRole],
) -> RoleCapabilityPartition {
    let mut seen = std::collections::HashSet::new();
    let mut supported_roles = Vec::new();
    let mut unsupported_roles = Vec::new();
    for role in intended_roles {
        if !seen.insert(*role) {
            continue;
        }
        if engine.supports_role(role) {
            supported_roles.push(*role);
        } else {
            unsupported_roles.push(*role);
        }
    }
    RoleCapabilityPartition {
        supported_roles,
        unsupported_roles,
    }
}

/// Configuration for parallel dispatch.
#[derive(Debug, Clone)]
pub(crate) struct DispatchConfig {
    /// Maximum results to request per engine call.
    pub candidate_limit: usize,
    /// Global timeout for the entire dispatch.
    pub global_timeout: Duration,
    /// Maximum total in-flight (subquery, provider) jobs.
    pub max_concurrent_jobs: usize,
    /// Maximum concurrent jobs for any single provider.
    pub max_concurrent_per_provider: usize,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            candidate_limit: 30,
            global_timeout: Duration::from_secs(8),
            max_concurrent_jobs: 8,
            max_concurrent_per_provider: 2,
        }
    }
}

/// A single result from a dispatched job, tagged with ordering metadata.
#[derive(Debug)]
pub(crate) struct DispatchedResult {
    #[allow(dead_code)]
    pub subquery_id: String,
    pub subquery_order: usize,
    pub provider_id: String,
    pub provider_order: usize,
    pub results: Vec<SearchResult>,
    pub duration_ms: u64,
}

/// A single failure from a dispatched job, tagged with ordering metadata.
#[derive(Debug)]
pub(crate) struct DispatchedFailure {
    #[allow(dead_code)]
    pub subquery_id: String,
    pub subquery_order: usize,
    pub provider_id: String,
    pub provider_order: usize,
    pub error: EngineError,
    pub duration_ms: u64,
}

/// Identity retained for in-flight dispatch tasks so an abnormal exit
/// can release counters and record a failure.
pub(crate) type ActiveTaskInfo = (String, String, usize, usize, Vec<EvidenceRole>, String);

/// Deadline tracking statistics.
#[derive(Default, Debug)]
pub(crate) struct RequestDeadlineStats {
    pub exceeded: bool,
    pub subqueries_skipped: usize,
    pub subqueries_interrupted: usize,
    pub subqueries_completed: usize,
    pub subqueries_partially_completed: usize,
}

/// Output of the parallel dispatch.
#[derive(Default, Debug)]
pub(crate) struct DispatchOutput {
    /// Successful results, sorted deterministically.
    pub raw_results: Vec<(String, Vec<SearchResult>)>,
    /// Failures, sorted deterministically.
    pub raw_failures: Vec<(String, EngineError)>,
    pub result_latencies_ms: Vec<u64>,
    pub failure_latencies_ms: Vec<u64>,
    /// Deadline tracking.
    pub deadline: RequestDeadlineStats,
    /// Attempt records for every dispatched and skipped job.
    pub attempts: Vec<RetrievalAttempt>,
    /// Provider-neutral retrieval metadata per successful job,
    /// as `(provider_id, subquery_id, metadata)`, sorted deterministically.
    pub retrieval_metadata: Vec<(
        String,
        String,
        crate::meta::engines::models::EngineRetrievalMetadata,
    )>,
}

/// Result returned by a spawned task, including ordering metadata.
pub(crate) struct TaskResult {
    pub(crate) subquery_id: String,
    pub(crate) subquery_order: usize,
    pub(crate) provider_id: String,
    pub(crate) provider_order: usize,
    pub(crate) intended_roles: Vec<EvidenceRole>,
    pub(crate) query_fingerprint: String,
    pub(crate) result: Result<crate::meta::engines::models::EngineSearchBatch, EngineError>,
}

pub(crate) type JobKey = (String, String, Vec<EvidenceRole>);
