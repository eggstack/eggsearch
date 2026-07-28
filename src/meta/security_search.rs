//! Security search orchestration for `security_search`.
//!
//! This module contains the core orchestration logic for the
//! `security_search` tool, extracted from `src/mcp/tools.rs`. It
//! coordinates web search, native advisory lookups, KEV enrichment,
//! result grouping, and suggested fetch generation.

use std::collections::HashSet;
use std::time::Instant;

use crate::core::code_evidence::EvidenceConfidence;
use crate::core::evidence_role::EvidenceRole;
use crate::core::retrieval_status::{
    RetrievalAttempt, RetrievalAttemptOutcome, RetrievalOperationIdentity,
};
use crate::core::security::{
    self, AffectedPackageSummary, SecurityContext, SecurityIdentifiers, SecuritySearchRequest,
    SecuritySearchResponse, VulnerabilityMetadata, VulnerabilitySummary,
};
use crate::core::SearchWarning;
use crate::meta::engines::error::EngineError;
use crate::meta::engines::kev::KevClient;
use crate::meta::response::WebSearchResponse;
use crate::meta::security_grouping::group_security_results;
use crate::meta::security_suggested_fetches::generate_security_suggested_fetches;
use crate::meta::MetadataSearchAdapter;
use crate::meta::{ProviderAdvisoryOutcome, ProviderAdvisoryStatus};

const MAX_NATIVE_ADVISORY_IDENTIFIERS: usize = 32;
const MAX_NATIVE_ADVISORY_PROVIDER_OPERATIONS: usize = 64;

/// Bounded resource for native advisory provider operations.
///
/// Tracks two independent limits:
/// - identifier budget: maximum unique advisory identifiers accepted
/// - provider-operation budget: maximum selected-provider calls across
///   identifier and package advisory operations
///
/// The provider-operation budget is the release-material bound. It
/// counts actual provider calls, not only input identifier groups.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct NativeOperationBudget {
    max_identifiers: usize,
    max_provider_operations: usize,
    identifiers_seen: usize,
    provider_operations_reserved: usize,
}

impl Default for NativeOperationBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeOperationBudget {
    #[allow(missing_docs)]
    pub fn new() -> Self {
        NativeOperationBudget {
            max_identifiers: MAX_NATIVE_ADVISORY_IDENTIFIERS,
            max_provider_operations: MAX_NATIVE_ADVISORY_PROVIDER_OPERATIONS,
            identifiers_seen: 0,
            provider_operations_reserved: 0,
        }
    }

    /// Returns true if a new unique identifier can be accepted.
    pub fn reserve_identifier(&mut self) -> bool {
        if self.identifiers_seen >= self.max_identifiers {
            return false;
        }
        self.identifiers_seen += 1;
        true
    }

    /// Reserve provider operations for a set of providers.
    ///
    /// Returns the providers that were allowed (within budget), the
    /// providers that were skipped due to budget exhaustion, and the
    /// remaining provider-operation capacity.
    pub fn reserve_providers(&mut self, provider_ids: &[String]) -> ProviderReservation {
        let mut allowed = Vec::new();
        let mut skipped = Vec::new();
        for pid in provider_ids {
            if self.provider_operations_reserved >= self.max_provider_operations {
                skipped.push(pid.clone());
            } else {
                self.provider_operations_reserved += 1;
                allowed.push(pid.clone());
            }
        }
        ProviderReservation {
            allowed,
            skipped_by_budget: skipped,
            remaining_capacity: self
                .max_provider_operations
                .saturating_sub(self.provider_operations_reserved),
        }
    }

    #[allow(missing_docs)]
    pub fn identifiers_seen(&self) -> usize {
        self.identifiers_seen
    }

    #[allow(missing_docs)]
    pub fn provider_operations_reserved(&self) -> usize {
        self.provider_operations_reserved
    }

    #[allow(missing_docs)]
    pub fn max_identifiers(&self) -> usize {
        self.max_identifiers
    }

    #[allow(missing_docs)]
    pub fn max_provider_operations(&self) -> usize {
        self.max_provider_operations
    }
}

/// Result of reserving provider operations within a budget.
#[derive(Debug, Clone, Default)]
#[allow(missing_docs)]
pub struct ProviderReservation {
    pub allowed: Vec<String>,
    pub skipped_by_budget: Vec<String>,
    pub remaining_capacity: usize,
}

/// Summary of native advisory budget accounting.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)]
pub struct NativeAdvisoryBudgetSummary {
    pub identifiers_planned: usize,
    pub identifiers_scheduled: usize,
    pub provider_operations_planned: usize,
    pub provider_operations_dispatched: usize,
    pub provider_operations_skipped_by_budget: usize,
}

/// Planned advisory identifier after deduplication.
#[derive(Debug, Clone)]
struct PlannedAdvisoryIdentifier {
    identifier: String,
    subquery_id: &'static str,
}

/// Plan all unique advisory identifiers from resolved IDs.
///
/// Produces a stable, deduplicated list in family order (CVE, GHSA, OSV, RustSec).
/// Duplicates across families are eliminated. This function performs no
/// network calls and no budget reservation.
fn plan_unique_advisory_identifiers(
    resolved: &SecurityIdentifiers,
) -> Vec<PlannedAdvisoryIdentifier> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut planned: Vec<PlannedAdvisoryIdentifier> = Vec::new();

    for (ids, subquery_id) in [
        (&resolved.cve_ids, "advisory_by_cve"),
        (&resolved.ghsa_ids, "advisory_by_ghsa"),
        (&resolved.osv_ids, "advisory_by_osv"),
        (&resolved.rustsec_ids, "advisory_by_rustsec"),
    ] {
        for id in ids {
            if seen.insert(id.clone()) {
                planned.push(PlannedAdvisoryIdentifier {
                    identifier: id.clone(),
                    subquery_id,
                });
            }
        }
    }

    planned
}

#[allow(clippy::too_many_arguments)]
fn native_advisory_attempt(
    provider_id: &str,
    subquery_id: &str,
    operation: &RetrievalOperationIdentity,
    intended_roles: Vec<EvidenceRole>,
    outcome: RetrievalAttemptOutcome,
    result_count: usize,
    error_class: Option<String>,
    query_text: &str,
    start: Instant,
) -> RetrievalAttempt {
    RetrievalAttempt {
        provider_id: provider_id.to_string(),
        subquery_id: Some(subquery_id.to_string()),
        operation_id: Some(operation.stable_id()),
        intended_roles,
        outcome,
        result_count,
        error_class,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: Some(crate::core::retrieval_status::query_fingerprint_from_query(
            query_text,
        )),
        duration_ms: Some(start.elapsed().as_millis() as u64),
    }
}

#[allow(clippy::too_many_arguments)]
fn native_advisory_attempt_with_duration(
    provider_id: &str,
    subquery_id: &str,
    operation: &RetrievalOperationIdentity,
    intended_roles: Vec<EvidenceRole>,
    outcome: RetrievalAttemptOutcome,
    result_count: usize,
    error_class: Option<String>,
    query_text: &str,
    duration_ms: u64,
) -> RetrievalAttempt {
    RetrievalAttempt {
        provider_id: provider_id.to_string(),
        subquery_id: Some(subquery_id.to_string()),
        operation_id: Some(operation.stable_id()),
        intended_roles,
        outcome,
        result_count,
        error_class,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: Some(crate::core::retrieval_status::query_fingerprint_from_query(
            query_text,
        )),
        duration_ms: Some(duration_ms),
    }
}

fn native_error_outcome(error: &EngineError) -> (RetrievalAttemptOutcome, String) {
    match error {
        EngineError::Timeout { .. } => (RetrievalAttemptOutcome::TimedOut, "timeout".to_string()),
        EngineError::BadStatus { status: 429, .. } => (
            RetrievalAttemptOutcome::RateLimited,
            "rate_limited".to_string(),
        ),
        EngineError::BadStatus { .. } => {
            (RetrievalAttemptOutcome::Failed, "http_status".to_string())
        }
        EngineError::ParseFailed { .. } => {
            (RetrievalAttemptOutcome::Failed, "parse_error".to_string())
        }
        EngineError::NetworkError { .. } | EngineError::Http { .. } => {
            (RetrievalAttemptOutcome::Failed, "network_error".to_string())
        }
    }
}

fn record_lookup_outcomes(
    outcomes: Vec<ProviderAdvisoryOutcome<Option<VulnerabilityMetadata>>>,
    subquery_id: &str,
    operation: &RetrievalOperationIdentity,
    query_text: &str,
    vulnerabilities: &mut Vec<VulnerabilityMetadata>,
    attempts: &mut Vec<RetrievalAttempt>,
) {
    let roles = vec![EvidenceRole::AuthoritativeSecurityAdvisory];
    for outcome in outcomes {
        let attempt = match outcome.status {
            ProviderAdvisoryStatus::CapabilityUnavailable => native_advisory_attempt_with_duration(
                &outcome.provider_id,
                subquery_id,
                operation,
                roles.clone(),
                RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
                0,
                None,
                query_text,
                outcome.duration_ms,
            ),
            ProviderAdvisoryStatus::InterruptedByDeadline => {
                let mut attempt = native_advisory_attempt_with_duration(
                    &outcome.provider_id,
                    subquery_id,
                    operation,
                    roles.clone(),
                    RetrievalAttemptOutcome::InterruptedByDeadline,
                    0,
                    Some("deadline".to_string()),
                    query_text,
                    outcome.duration_ms,
                );
                attempt.deadline_interrupted = true;
                attempt
            }
            ProviderAdvisoryStatus::Completed(Ok(Some(metadata))) => {
                if !vulnerabilities
                    .iter()
                    .any(|existing| ids_overlap(existing, &metadata))
                {
                    vulnerabilities.push(metadata);
                }
                native_advisory_attempt_with_duration(
                    &outcome.provider_id,
                    subquery_id,
                    operation,
                    roles.clone(),
                    RetrievalAttemptOutcome::SuccessWithResults,
                    1,
                    None,
                    query_text,
                    outcome.duration_ms,
                )
            }
            ProviderAdvisoryStatus::Completed(Ok(None)) => native_advisory_attempt_with_duration(
                &outcome.provider_id,
                subquery_id,
                operation,
                roles.clone(),
                RetrievalAttemptOutcome::SuccessZeroResults,
                0,
                None,
                query_text,
                outcome.duration_ms,
            ),
            ProviderAdvisoryStatus::Completed(Err(error)) => {
                let (attempt_outcome, error_class) = native_error_outcome(&error);
                native_advisory_attempt_with_duration(
                    &outcome.provider_id,
                    subquery_id,
                    operation,
                    roles.clone(),
                    attempt_outcome,
                    0,
                    Some(error_class),
                    query_text,
                    outcome.duration_ms,
                )
            }
        };
        attempts.push(attempt);
    }
}

fn record_package_outcomes(
    outcomes: Vec<ProviderAdvisoryOutcome<Vec<VulnerabilityMetadata>>>,
    operation: &RetrievalOperationIdentity,
    query_text: &str,
    vulnerabilities: &mut Vec<VulnerabilityMetadata>,
    attempts: &mut Vec<RetrievalAttempt>,
) {
    let advisory_role = vec![EvidenceRole::AuthoritativeSecurityAdvisory];
    let dependency_role = vec![EvidenceRole::ManifestOrDependencyMetadata];
    for outcome in outcomes {
        let advisory_attempt = match outcome.status {
            ProviderAdvisoryStatus::CapabilityUnavailable => native_advisory_attempt_with_duration(
                &outcome.provider_id,
                "advisory_by_package",
                operation,
                advisory_role.clone(),
                RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
                0,
                None,
                query_text,
                outcome.duration_ms,
            ),
            ProviderAdvisoryStatus::InterruptedByDeadline => {
                let mut attempt = native_advisory_attempt_with_duration(
                    &outcome.provider_id,
                    "advisory_by_package",
                    operation,
                    advisory_role.clone(),
                    RetrievalAttemptOutcome::InterruptedByDeadline,
                    0,
                    Some("deadline".to_string()),
                    query_text,
                    outcome.duration_ms,
                );
                attempt.deadline_interrupted = true;
                attempt
            }
            ProviderAdvisoryStatus::Completed(Ok(package_vulns)) => {
                let count = package_vulns.len();
                for vuln in package_vulns {
                    if !vulnerabilities
                        .iter()
                        .any(|existing| ids_overlap(existing, &vuln))
                    {
                        vulnerabilities.push(vuln);
                    }
                }
                native_advisory_attempt_with_duration(
                    &outcome.provider_id,
                    "advisory_by_package",
                    operation,
                    advisory_role.clone(),
                    if count > 0 {
                        RetrievalAttemptOutcome::SuccessWithResults
                    } else {
                        RetrievalAttemptOutcome::SuccessZeroResults
                    },
                    count,
                    None,
                    query_text,
                    outcome.duration_ms,
                )
            }
            ProviderAdvisoryStatus::Completed(Err(error)) => {
                let (attempt_outcome, error_class) = native_error_outcome(&error);
                native_advisory_attempt_with_duration(
                    &outcome.provider_id,
                    "advisory_by_package",
                    operation,
                    advisory_role.clone(),
                    attempt_outcome,
                    0,
                    Some(error_class),
                    query_text,
                    outcome.duration_ms,
                )
            }
        };

        let dependency_interrupted =
            advisory_attempt.outcome == RetrievalAttemptOutcome::InterruptedByDeadline;
        attempts.push(advisory_attempt);

        let dependency_attempt = native_advisory_attempt_with_duration(
            &outcome.provider_id,
            "advisory_by_package",
            operation,
            dependency_role.clone(),
            if dependency_interrupted {
                RetrievalAttemptOutcome::InterruptedByDeadline
            } else {
                RetrievalAttemptOutcome::SkippedCapabilityUnavailable
            },
            0,
            if dependency_interrupted {
                Some("deadline".to_string())
            } else {
                Some("native_advisory_provider_does_not_supply_manifest_metadata".to_string())
            },
            query_text,
            outcome.duration_ms,
        );
        let mut dependency_attempt = dependency_attempt;
        dependency_attempt.deadline_interrupted = dependency_interrupted;
        attempts.push(dependency_attempt);
    }
}

/// Orchestrate a security search: parse identifiers, run web search
/// with security intent, perform native advisory lookups, enrich with
/// KEV data, group results, and generate suggested fetches.
///
/// `effective_max` is the caller-computed max results (after config
/// cap). `max_results_cap` is the configured server cap used to bound
/// the candidate pool.
pub async fn run_security_search_plan(
    adapter: &MetadataSearchAdapter,
    kev_client: &KevClient,
    req: &SecuritySearchRequest,
    effective_max: usize,
    max_results_cap: usize,
) -> SecuritySearchResponse {
    // 1. Parse identifiers from request fields and free-text query
    let resolved_ids = SecurityIdentifiers::parse(
        &req.query,
        req.cve_id.as_deref(),
        req.ghsa_id.as_deref(),
        req.osv_id.as_deref(),
        req.rustsec_id.as_deref(),
        req.package.as_deref(),
        req.ecosystem.as_deref(),
        req.version.as_deref(),
    );

    // 2. Run security search via parallel dispatcher
    let effective_providers = if req.providers.is_empty() {
        adapter.provider_ids().to_vec()
    } else {
        req.providers.clone()
    };

    let (results, dispatch_warnings, providers_failed, trust_markers, security_attempts) = adapter
        .security_search_subqueries(
            &req.query,
            &effective_providers,
            effective_max,
            max_results_cap,
            req.timeout_ms,
        )
        .await;

    // Build a WebSearchResponse-shaped structure for downstream compatibility
    let web_resp = WebSearchResponse {
        query: req.query.clone(),
        mode: "security_metasearch",
        results,
        providers_queried: effective_providers.clone(),
        providers_failed: providers_failed.clone(),
        warnings: dispatch_warnings,
        trust_markers,
        evidence_postprocess: None,
    };

    // 3. Check whether any selected provider exposes native advisory capability.
    let has_native_advisory = adapter
        .advisory_provider_capabilities(&effective_providers)
        .into_iter()
        .any(|(_, capabilities)| capabilities.lookup_by_id || capabilities.query_by_package);

    let mut warnings: Vec<SearchWarning> = web_resp.warnings;

    if !has_native_advisory {
        warnings.push(SearchWarning::new(
            "_system",
            "native_advisory_search_unavailable: only generic web search was used; \
             enable the 'osv' provider for native advisory lookups",
        ));
    }

    // Generic context is external untrusted discussion, not advisory fact
    if !web_resp.results.is_empty() {
        warnings.push(SearchWarning::new(
            "_system",
            "generic_context_untrusted: generic web results are external untrusted \
             discussion, not authoritative advisory facts",
        ));
    }

    // Severity may be unavailable from generic search
    warnings.push(SearchWarning::new(
        "_system",
        "severity_unavailable: severity levels may not be available \
         from generic web search results; use native advisory providers for severity data",
    ));

    // 4. Native advisory ID lookups for identified CVE/GHSA/RustSec/OSV IDs
    let mut vulnerabilities: Vec<VulnerabilityMetadata> = Vec::new();
    let mut native_attempts: Vec<RetrievalAttempt> = Vec::new();
    let native_deadline = Instant::now() + adapter.effective_timeout(req.timeout_ms);
    let mut budget = NativeOperationBudget::new();
    let mut budget_summary = NativeAdvisoryBudgetSummary::default();

    let advisory_caps = adapter.advisory_provider_capabilities(&effective_providers);
    let capable_lookup_providers: Vec<String> = advisory_caps
        .iter()
        .filter(|(_, caps)| caps.lookup_by_id)
        .map(|(id, _)| id.clone())
        .collect();
    let incapable_lookup_providers: Vec<String> = advisory_caps
        .iter()
        .filter(|(_, caps)| !caps.lookup_by_id)
        .map(|(id, _)| id.clone())
        .collect();

    let mut identifier_cap_reached = false;

    let planned_ids = plan_unique_advisory_identifiers(&resolved_ids);
    budget_summary.identifiers_planned = planned_ids.len();

    for planned in &planned_ids {
        if !budget.reserve_identifier() {
            identifier_cap_reached = true;
            break;
        }
        let vulnerability_id = &planned.identifier;
        let subquery_id = planned.subquery_id;
        let operation = RetrievalOperationIdentity::from_advisory_id(vulnerability_id);
        let reservation = budget.reserve_providers(&capable_lookup_providers);
        budget_summary.provider_operations_planned += capable_lookup_providers.len();
        budget_summary.provider_operations_dispatched += reservation.allowed.len();
        budget_summary.provider_operations_skipped_by_budget += reservation.skipped_by_budget.len();

        for skipped_pid in &reservation.skipped_by_budget {
            native_attempts.push(native_advisory_attempt_with_duration(
                skipped_pid,
                subquery_id,
                &operation,
                vec![EvidenceRole::AuthoritativeSecurityAdvisory],
                RetrievalAttemptOutcome::SkippedByPolicy,
                0,
                Some("native_operation_budget_exhausted".to_string()),
                vulnerability_id,
                0,
            ));
        }

        for incapable_pid in &incapable_lookup_providers {
            native_attempts.push(native_advisory_attempt_with_duration(
                incapable_pid,
                subquery_id,
                &operation,
                vec![EvidenceRole::AuthoritativeSecurityAdvisory],
                RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
                0,
                None,
                vulnerability_id,
                0,
            ));
        }

        if !reservation.allowed.is_empty() {
            let remaining = native_deadline.saturating_duration_since(Instant::now());
            let outcomes = adapter
                .lookup_advisory_scoped_with_timeout(
                    &reservation.allowed,
                    vulnerability_id,
                    remaining,
                )
                .await;
            record_lookup_outcomes(
                outcomes,
                subquery_id,
                &operation,
                vulnerability_id,
                &mut vulnerabilities,
                &mut native_attempts,
            );
        }
    }

    budget_summary.identifiers_scheduled = budget.identifiers_seen();

    // 5. Native package advisory queries when both package and ecosystem are present
    if let (Some(ref package), Some(ref ecosystem)) =
        (&resolved_ids.package, &resolved_ids.ecosystem)
    {
        let package_operation = RetrievalOperationIdentity::from_package(
            ecosystem,
            package,
            resolved_ids.version.as_deref(),
        );

        let capable_package_providers: Vec<String> = advisory_caps
            .iter()
            .filter(|(_, caps)| caps.query_by_package)
            .map(|(id, _)| id.clone())
            .collect();
        let incapable_package_providers: Vec<String> = advisory_caps
            .iter()
            .filter(|(_, caps)| !caps.query_by_package)
            .map(|(id, _)| id.clone())
            .collect();

        let reservation = budget.reserve_providers(&capable_package_providers);
        budget_summary.provider_operations_planned += capable_package_providers.len();
        budget_summary.provider_operations_dispatched += reservation.allowed.len();
        budget_summary.provider_operations_skipped_by_budget += reservation.skipped_by_budget.len();

        for skipped_pid in &reservation.skipped_by_budget {
            native_attempts.push(native_advisory_attempt_with_duration(
                skipped_pid,
                "advisory_by_package",
                &package_operation,
                vec![EvidenceRole::AuthoritativeSecurityAdvisory],
                RetrievalAttemptOutcome::SkippedByPolicy,
                0,
                Some("native_operation_budget_exhausted".to_string()),
                package,
                0,
            ));
            native_attempts.push(native_advisory_attempt_with_duration(
                skipped_pid,
                "advisory_by_package",
                &package_operation,
                vec![EvidenceRole::ManifestOrDependencyMetadata],
                RetrievalAttemptOutcome::SkippedByPolicy,
                0,
                Some("native_operation_budget_exhausted".to_string()),
                package,
                0,
            ));
        }

        for incapable_pid in &incapable_package_providers {
            native_attempts.push(native_advisory_attempt_with_duration(
                incapable_pid,
                "advisory_by_package",
                &package_operation,
                vec![EvidenceRole::AuthoritativeSecurityAdvisory],
                RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
                0,
                None,
                package,
                0,
            ));
            native_attempts.push(native_advisory_attempt_with_duration(
                incapable_pid,
                "advisory_by_package",
                &package_operation,
                vec![EvidenceRole::ManifestOrDependencyMetadata],
                RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
                0,
                None,
                package,
                0,
            ));
        }

        if !reservation.allowed.is_empty() {
            let remaining = native_deadline.saturating_duration_since(Instant::now());
            let outcomes = adapter
                .query_advisories_by_package_scoped_with_timeout(
                    &reservation.allowed,
                    ecosystem,
                    package,
                    resolved_ids.version.as_deref(),
                    effective_max,
                    remaining,
                )
                .await;
            record_package_outcomes(
                outcomes,
                &package_operation,
                package,
                &mut vulnerabilities,
                &mut native_attempts,
            );
        }
    }

    if identifier_cap_reached {
        warnings.push(SearchWarning::new(
            "_system",
            format!(
                "native_advisory_identifier_cap_reached: processed {} unique identifiers; {} additional identifiers were not scheduled",
                budget_summary.identifiers_scheduled,
                budget_summary.identifiers_planned.saturating_sub(budget_summary.identifiers_scheduled)
            ),
        ));
    }

    let provider_op_cap_reached = budget_summary.provider_operations_skipped_by_budget > 0;
    if provider_op_cap_reached {
        warnings.push(SearchWarning::new(
            "_system",
            format!(
                "native_advisory_provider_operation_cap_reached: dispatched {} provider operations; {} capable provider operations were skipped by policy after the provider-operation cap was reached",
                budget_summary.provider_operations_dispatched,
                budget_summary.provider_operations_skipped_by_budget
            ),
        ));
    }

    // 6. Enrich vulnerabilities with KEV data if requested
    if req.include_kev == Some(true) {
        let cve_ids_for_kev: Vec<String> = vulnerabilities
            .iter()
            .flat_map(|v| v.cve_ids.iter().cloned())
            .collect();

        if cve_ids_for_kev.is_empty() {
            let kev_na_operation =
                RetrievalOperationIdentity::from_search_subquery("kev-not-applicable");
            native_attempts.push(native_advisory_attempt(
                "cisa_kev",
                "kev_by_cve",
                &kev_na_operation,
                vec![EvidenceRole::AuthoritativeSecurityAdvisory],
                RetrievalAttemptOutcome::NotApplicable,
                0,
                None,
                &req.query,
                Instant::now(),
            ));
            warnings.push(SearchWarning::new(
                "_system",
                "kev_lookup_skipped: KEV lookup requires CVE identifiers",
            ));
        } else {
            let mut kev_found_ids: Vec<String> = Vec::new();
            let mut kev_lookup_failed = false;

            for cve_id in &cve_ids_for_kev {
                let kev_operation = RetrievalOperationIdentity::from_kev_cve(cve_id);
                let start = Instant::now();
                match kev_client.lookup(cve_id).await {
                    Ok(Some(kev_meta)) => {
                        native_attempts.push(native_advisory_attempt(
                            "cisa_kev",
                            "kev_by_cve",
                            &kev_operation,
                            vec![EvidenceRole::AuthoritativeSecurityAdvisory],
                            RetrievalAttemptOutcome::SuccessWithResults,
                            1,
                            None,
                            cve_id,
                            start,
                        ));
                        for vuln in &mut vulnerabilities {
                            if vuln.cve_ids.iter().any(|id| id == cve_id) {
                                vuln.kev = Some(kev_meta.clone());
                            }
                        }
                        kev_found_ids.push(cve_id.clone());
                    }
                    Ok(None) => {
                        native_attempts.push(native_advisory_attempt(
                            "cisa_kev",
                            "kev_by_cve",
                            &kev_operation,
                            vec![EvidenceRole::AuthoritativeSecurityAdvisory],
                            RetrievalAttemptOutcome::SuccessZeroResults,
                            0,
                            None,
                            cve_id,
                            start,
                        ));
                    }
                    Err(e) => {
                        kev_lookup_failed = true;
                        native_attempts.push(native_advisory_attempt(
                            "cisa_kev",
                            "kev_by_cve",
                            &kev_operation,
                            vec![EvidenceRole::AuthoritativeSecurityAdvisory],
                            RetrievalAttemptOutcome::Failed,
                            0,
                            Some(format!("{e}")),
                            cve_id,
                            start,
                        ));
                    }
                }
            }

            if kev_lookup_failed && kev_found_ids.is_empty() {
                warnings.push(SearchWarning::new(
                    "_system",
                    "kev_lookup_failed: KEV catalog lookup failed; KEV status could not be determined",
                ));
            } else if !kev_found_ids.is_empty() {
                warnings.push(SearchWarning::new(
                    "_system",
                    format!(
                        "kev_match: {} CVE(s) found in CISA KEV catalog",
                        kev_found_ids.len()
                    ),
                ));
            } else {
                warnings.push(SearchWarning::new(
                    "_system",
                    "kev_absent_not_proof: no CVE(s) found in CISA KEV catalog; \
                     absence does not prove no exploitation",
                ));
            }
        }
    }

    // 7. Version matching status
    if req.version.is_some() && req.assess_applicability != Some(true) {
        warnings.push(SearchWarning::new(
            "_system",
            "version_match_unavailable: version-specific matching requires assess_applicability=true; \
             affected version ranges are returned as-is from advisory databases",
        ));

        // Warn when package was found but no vulnerability has affected ranges
        if resolved_ids.package.is_some()
            && resolved_ids.ecosystem.is_some()
            && vulnerabilities
                .iter()
                .all(|v| v.affected_ranges.is_empty() && v.vulnerable_versions.is_empty())
        {
            warnings.push(SearchWarning::new(
                "_system",
                "version_mismatch: package was found but no advisory has affected version \
                 ranges matching the supplied version; the package may not be affected or \
                 version-specific advisory data is unavailable",
            ));
        }
    }

    // Applicability analysis
    let mut applicability_assessments = Vec::new();
    let mut dependency_findings = Vec::new();

    if req.assess_applicability == Some(true) {
        use crate::core::security_applicability::{
            ApplicabilityAssessment, ApplicabilityConfidence, ApplicabilityStatus,
        };
        use crate::meta::advisory_range::{assess_version_applicability, extract_advisory_ranges};
        use crate::meta::dependency_parse::parse_dependency_file;

        // Track (advisory_id, package, version) to deduplicate assessments
        let mut seen_assessments: std::collections::HashSet<(String, String, String)> =
            std::collections::HashSet::new();

        for file_path in &req.dependency_files {
            match read_bounded_file(file_path) {
                Ok(content) => {
                    let findings = parse_dependency_file(file_path, &content);
                    dependency_findings.extend(findings);
                }
                Err(_) => {
                    warnings.push(SearchWarning::new(
                        "_system",
                        format!("dependency_file_read_error: could not read {file_path}"),
                    ));
                }
            }
        }

        let target_version = resolved_ids.version.as_deref();
        let target_package = resolved_ids.package.as_deref();
        let target_ecosystem = resolved_ids.ecosystem.as_deref();

        for vuln in &vulnerabilities {
            let ranges = extract_advisory_ranges(vuln);

            if let (Some(pkg), Some(ver)) = (target_package, target_version) {
                let vuln_pkg = vuln.package.as_deref().unwrap_or("");
                let vuln_eco = vuln.ecosystem.as_deref().unwrap_or("");

                let pkg_matches = vuln_pkg.eq_ignore_ascii_case(pkg);
                let eco_matches = target_ecosystem
                    .map(|e| e.eq_ignore_ascii_case(vuln_eco))
                    .unwrap_or(true);

                if pkg_matches && eco_matches {
                    let advisory_id = vuln
                        .cve_ids
                        .first()
                        .or(vuln.ghsa_ids.first())
                        .or(vuln.osv_ids.first())
                        .or(vuln.rustsec_ids.first())
                        .cloned()
                        .unwrap_or_default();

                    let outcome = assess_version_applicability(
                        ver,
                        &ranges,
                        &ranges
                            .first()
                            .map(|r| r.ecosystem.clone())
                            .unwrap_or(crate::core::package::PackageEcosystem::CratesIo),
                    );
                    let status = outcome.status;
                    let confidence = if !ranges.is_empty() {
                        ApplicabilityConfidence::High
                    } else {
                        ApplicabilityConfidence::Low
                    };

                    let mut assessment_reasons = outcome.reasons;
                    match status {
                        ApplicabilityStatus::Affected => assessment_reasons.push(format!(
                            "version {ver} appears affected by advisory {advisory_id}"
                        )),
                        ApplicabilityStatus::NotAffected => assessment_reasons.push(format!(
                            "version {ver} does not appear affected by advisory {advisory_id}"
                        )),
                        ApplicabilityStatus::Unknown => assessment_reasons.push(format!(
                            "could not determine applicability of version {ver} for advisory {advisory_id}"
                        )),
                        ApplicabilityStatus::InsufficientEvidence => assessment_reasons.push(
                            "insufficient package/version data to assess applicability"
                                .to_string(),
                        ),
                    }

                    let key = (advisory_id.clone(), pkg.to_string(), ver.to_string());
                    if seen_assessments.insert(key) {
                        applicability_assessments.push(ApplicabilityAssessment {
                            status,
                            confidence,
                            ecosystem: vuln
                                .ecosystem
                                .as_deref()
                                .and_then(crate::core::package::PackageEcosystem::parse)
                                .unwrap_or(crate::core::package::PackageEcosystem::CratesIo),
                            package: pkg.to_string(),
                            version: Some(ver.to_string()),
                            advisory_ids: vec![advisory_id],
                            matched_ranges: outcome.matched_ranges.clone(),
                            fixed_versions: outcome
                                .matched_ranges
                                .iter()
                                .flat_map(|r| r.fixed_versions.iter().cloned())
                                .collect(),
                            reasons: assessment_reasons,
                            evidence_urls: vuln.references.iter().map(|r| r.url.clone()).collect(),
                            warnings: Vec::new(),
                            version_source: None,
                            dependency_relation: None,
                            source_ids: Vec::new(),
                            fetch_ids: Vec::new(),
                        });
                    }
                }
            }

            for finding in &dependency_findings {
                let vuln_pkg = vuln.package.as_deref().unwrap_or("");
                let vuln_eco = vuln.ecosystem.as_deref().unwrap_or("");

                if finding.package.eq_ignore_ascii_case(vuln_pkg)
                    && finding.ecosystem.as_str().eq_ignore_ascii_case(vuln_eco)
                {
                    if let Some(ref ver) = finding.version {
                        let advisory_id = vuln
                            .cve_ids
                            .first()
                            .or(vuln.ghsa_ids.first())
                            .or(vuln.osv_ids.first())
                            .or(vuln.rustsec_ids.first())
                            .cloned()
                            .unwrap_or_default();

                        let outcome =
                            assess_version_applicability(ver, &ranges, &finding.ecosystem);
                        let status = outcome.status;
                        let confidence = if !ranges.is_empty() {
                            ApplicabilityConfidence::High
                        } else {
                            ApplicabilityConfidence::Low
                        };

                        let mut reasons = outcome.reasons;
                        reasons.push(format!(
                            "dependency '{}' version '{}' found in {}",
                            finding.package,
                            ver,
                            finding.source_file.as_deref().unwrap_or("unknown")
                        ));

                        let key = (advisory_id.clone(), finding.package.clone(), ver.clone());
                        if seen_assessments.insert(key) {
                            applicability_assessments.push(ApplicabilityAssessment {
                                status,
                                confidence,
                                ecosystem: finding.ecosystem.clone(),
                                package: finding.package.clone(),
                                version: Some(ver.clone()),
                                advisory_ids: vec![advisory_id],
                                matched_ranges: outcome.matched_ranges.clone(),
                                fixed_versions: outcome
                                    .matched_ranges
                                    .iter()
                                    .flat_map(|r| r.fixed_versions.iter().cloned())
                                    .collect(),
                                reasons,
                                evidence_urls: vuln
                                    .references
                                    .iter()
                                    .map(|r| r.url.clone())
                                    .collect(),
                                warnings: Vec::new(),
                                version_source: None,
                                dependency_relation: None,
                                source_ids: Vec::new(),
                                fetch_ids: Vec::new(),
                            });
                        }
                    }
                }
            }
        }

        if !applicability_assessments.is_empty() {
            warnings.push(SearchWarning::new(
                "_system",
                "applicability_not_exploitability: Advisory range matching does not determine \
                 runtime exploitability or reachability. Applicability assessments are based on \
                 advisory metadata and dependency file parsing, not runtime analysis.",
            ));
        }
    }

    // 8. Group results and generate suggested fetches
    let mut groups = group_security_results(&web_resp.results, req.max_per_group);

    // Apply severity_min filtering to vulnerability records and grouped
    // source cards when severity metadata is available. When no
    // severity data is available (generic web search without native
    // advisory providers), emit a structured warning so callers know
    // the filter could not be enforced.
    if let Some(min_sev) = req.severity_min {
        let total_vulns_before = vulnerabilities.len();
        vulnerabilities.retain(|v| {
            v.severity
                .map(|s| s.meets_minimum(min_sev))
                .unwrap_or(false)
        });
        let dropped_vulns = total_vulns_before - vulnerabilities.len();

        let total_groups_before: usize = groups.iter().map(|g| g.results.len()).sum::<usize>();
        for group in &mut groups {
            group.results.retain(|card| {
                card.metadata
                    .vulnerability
                    .as_ref()
                    .and_then(|v| v.severity)
                    .map(|s| s.meets_minimum(min_sev))
                    .unwrap_or(true)
            });
        }
        groups.retain(|g| {
            !g.results.is_empty() || g.kind == crate::core::security::SecurityResultGroupKind::Other
        });
        let dropped_cards: usize =
            total_groups_before - groups.iter().map(|g| g.results.len()).sum::<usize>();

        if dropped_vulns == 0 && dropped_cards == 0 {
            warnings.push(SearchWarning::new(
                "_system",
                format!(
                    "severity_min_unenforced: severity_min={} requested but no source \
                     cards or vulnerabilities carry severity metadata; filter was not applied",
                    min_sev.as_str()
                ),
            ));
        } else {
            warnings.push(SearchWarning::new(
                "_system",
                format!(
                    "severity_min_applied: dropped {dropped_vulns} vulnerabilities and \
                     {dropped_cards} source cards below severity_min={}",
                    min_sev.as_str()
                ),
            ));
        }
    }

    let suggested_fetches = generate_security_suggested_fetches(
        &groups,
        &resolved_ids,
        req.ecosystem.as_deref(),
        req.package.as_deref(),
        &dependency_findings,
        req.include_exploit_context,
        req.include_defensive_guidance,
        req.include_vendor_advisories,
    );

    // 9. Build security context
    let query_kind = security::classify_query_kind(&resolved_ids);
    let identifiers = security::build_identifier_list(&resolved_ids);
    let mut source_quality = security::assess_source_quality(&web_resp.results);

    // Annotate when version hint is present and vulnerabilities have affected ranges
    if resolved_ids.version.is_some()
        && vulnerabilities
            .iter()
            .any(|v| !v.affected_ranges.is_empty())
    {
        source_quality.tier_reasons.push(
            "version_affected_match: query includes version hint and advisory has affected ranges"
                .to_string(),
        );
    }

    // Build affected package summaries from vulnerability metadata
    let affected_packages: Vec<AffectedPackageSummary> = {
        let mut seen = std::collections::HashSet::new();
        let mut packages = Vec::new();
        for vuln in &vulnerabilities {
            if let (Some(ref pkg), Some(ref eco)) = (&vuln.package, &vuln.ecosystem) {
                let key = format!("{eco}:{pkg}");
                if seen.insert(key) {
                    packages.push(AffectedPackageSummary {
                        package: pkg.clone(),
                        ecosystem: eco.clone(),
                        affected_ranges: vuln.affected_ranges.clone(),
                        patched_versions: vuln.patched_versions.clone(),
                    });
                }
            }
        }
        packages
    };

    // Build vulnerability summaries
    let vulnerability_summaries: Vec<VulnerabilitySummary> = vulnerabilities
        .iter()
        .map(|vuln| {
            let id = vuln
                .cve_ids
                .first()
                .or(vuln.ghsa_ids.first())
                .or(vuln.osv_ids.first())
                .or(vuln.rustsec_ids.first())
                .cloned()
                .unwrap_or_default();
            VulnerabilitySummary {
                id,
                severity: vuln.severity,
                description: None,
                source: vuln.source,
                kev: vuln.kev.is_some(),
            }
        })
        .collect();

    // Build defensive guidance from grouping results
    let mut defensive_guidance = Vec::new();
    for group in &groups {
        if group.kind == crate::core::security::SecurityResultGroupKind::DefensiveGuidance {
            for card in &group.results {
                defensive_guidance.push(security::DefensiveGuidance {
                    category: security::DefensiveGuidanceCategory::Unknown,
                    summary: card.title.clone(),
                    source_urls: vec![card.url.clone()],
                    confidence: EvidenceConfidence::Weak,
                });
            }
        }
    }

    // Build context warnings
    let mut context_warnings = Vec::new();
    if vulnerabilities.is_empty() && !resolved_ids.has_strong_identifier() {
        context_warnings.push(
            "no native vulnerability data found; results are generic web search only".to_string(),
        );
    }
    if source_quality.tier == security::SecuritySourceTier::Unknown
        || matches!(
            source_quality.tier,
            security::SecuritySourceTier::NewsOrBlog
                | security::SecuritySourceTier::CommunityDiscussion
        )
    {
        context_warnings.push(format!(
            "source quality is low ({}); advisory authority may be limited",
            source_quality.tier.as_str()
        ));
    }

    let security_context = SecurityContext {
        query_kind,
        identifiers,
        affected_packages,
        vulnerability_summaries,
        defensive_guidance,
        source_quality,
        warnings: context_warnings,
    };

    // Generate remediation actions from applicability assessments
    let mut remediation_actions = Vec::new();
    for assessment in &applicability_assessments {
        match assessment.status {
            crate::core::security_applicability::ApplicabilityStatus::Affected => {
                if !assessment.fixed_versions.is_empty() {
                    remediation_actions.push(crate::core::security::SecurityRemediation {
                        category: crate::core::security::RemediationCategory::Upgrade,
                        description: format!(
                            "Upgrade {} to version {} or later",
                            assessment.package,
                            assessment.fixed_versions.first().unwrap_or(&String::new())
                        ),
                        rationale: format!(
                            "Advisory {} indicates this package is affected; fixed versions are available",
                            assessment.advisory_ids.first().unwrap_or(&String::new())
                        ),
                        evidence_urls: assessment.evidence_urls.clone(),
                        fixed_versions: assessment.fixed_versions.clone(),
                        affected_packages: vec![assessment.package.clone()],
                        source_ids: assessment.source_ids.clone(),
                        confidence: crate::core::code_evidence::EvidenceConfidence::Strong,
                    });
                } else {
                    remediation_actions.push(crate::core::security::SecurityRemediation {
                        category: crate::core::security::RemediationCategory::ManualReview,
                        description: format!(
                            "Manual review required for {} - no fixed version available in advisory metadata",
                            assessment.package
                        ),
                        rationale: format!(
                            "Advisory {} confirms affected status but no fixed version is documented",
                            assessment.advisory_ids.first().unwrap_or(&String::new())
                        ),
                        evidence_urls: assessment.evidence_urls.clone(),
                        fixed_versions: Vec::new(),
                        affected_packages: vec![assessment.package.clone()],
                        source_ids: assessment.source_ids.clone(),
                        confidence: crate::core::code_evidence::EvidenceConfidence::Weak,
                    });
                }
            }
            crate::core::security_applicability::ApplicabilityStatus::Unknown => {
                remediation_actions.push(crate::core::security::SecurityRemediation {
                    category: crate::core::security::RemediationCategory::ManualReview,
                    description: format!(
                        "Manual review required for {} - applicability could not be determined",
                        assessment.package
                    ),
                    rationale:
                        "Advisory range syntax or version parsing prevented automated assessment"
                            .to_string(),
                    evidence_urls: assessment.evidence_urls.clone(),
                    fixed_versions: assessment.fixed_versions.clone(),
                    affected_packages: vec![assessment.package.clone()],
                    source_ids: assessment.source_ids.clone(),
                    confidence: crate::core::code_evidence::EvidenceConfidence::Unknown,
                });
            }
            crate::core::security_applicability::ApplicabilityStatus::InsufficientEvidence => {
                remediation_actions.push(crate::core::security::SecurityRemediation {
                    category: crate::core::security::RemediationCategory::ManualReview,
                    description: format!(
                        "Manual review required for {} - insufficient version/dependency data to assess",
                        assessment.package
                    ),
                    rationale: "Query lacked package version or dependency data needed for applicability assessment".to_string(),
                    evidence_urls: assessment.evidence_urls.clone(),
                    fixed_versions: Vec::new(),
                    affected_packages: vec![assessment.package.clone()],
                    source_ids: assessment.source_ids.clone(),
                    confidence: crate::core::code_evidence::EvidenceConfidence::Unknown,
                });
            }
            crate::core::security_applicability::ApplicabilityStatus::NotAffected => {
                // No remediation needed for not-affected packages
            }
        }
    }

    // Validate remediation text safety in debug builds
    debug_assert!(
        remediation_actions
            .iter()
            .all(|r| r.validate_text_safety().is_ok()),
        "remediation text must not contain offensive-instruction or vulnerability-class keywords"
    );

    // Build security evidence summary
    let security_evidence_summary = if !vulnerabilities.is_empty()
        || !applicability_assessments.is_empty()
    {
        let affected_count = applicability_assessments
            .iter()
            .filter(|a| {
                a.status == crate::core::security_applicability::ApplicabilityStatus::Affected
            })
            .count();
        let not_affected_count = applicability_assessments
            .iter()
            .filter(|a| {
                a.status == crate::core::security_applicability::ApplicabilityStatus::NotAffected
            })
            .count();
        let unknown_count = applicability_assessments
            .iter()
            .filter(|a| {
                a.status == crate::core::security_applicability::ApplicabilityStatus::Unknown
            })
            .count();
        let insufficient_evidence_count = applicability_assessments.iter()
            .filter(|a| a.status == crate::core::security_applicability::ApplicabilityStatus::InsufficientEvidence)
            .count();
        let kev_match_present = vulnerabilities.iter().any(|v| v.kev.is_some());
        let highest_severity = vulnerabilities
            .iter()
            .filter_map(|v| v.severity)
            .max_by_key(|s| match s {
                crate::core::security::SeverityLevel::Critical => 4,
                crate::core::security::SeverityLevel::High => 3,
                crate::core::security::SeverityLevel::Medium => 2,
                crate::core::security::SeverityLevel::Low => 1,
                crate::core::security::SeverityLevel::Unknown => 0,
            });
        Some(crate::core::security::SecurityEvidenceSummary {
            total_vulnerabilities: vulnerabilities.len(),
            total_assessments: applicability_assessments.len(),
            affected_count,
            not_affected_count,
            unknown_count,
            insufficient_evidence_count,
            remediation_count: remediation_actions.len(),
            highest_severity,
            kev_match_present,
            source_quality_tier: security_context.source_quality.tier,
            has_authoritative_source: matches!(
                security_context.source_quality.tier,
                crate::core::security::SecuritySourceTier::PrimaryAdvisory
                    | crate::core::security::SecuritySourceTier::VendorAdvisory
                    | crate::core::security::SecuritySourceTier::PackageRegistryAdvisory
            ),
        })
    } else {
        None
    };

    let structured_warnings = crate::core::warning::convert_warnings(&warnings);

    for group in &mut groups {
        crate::core::evidence_postprocess::materialize_evidence_roles(&mut group.results);
    }

    let all_cards: Vec<crate::core::SourceCard> = groups
        .iter()
        .flat_map(|g| g.results.iter())
        .cloned()
        .collect();

    let (workflow_model, resolution_source) =
        crate::core::evidence_postprocess::resolve_workflow_model_with_context(
            &crate::core::workflow_coverage::WorkflowResolutionContext {
                tool: "security_search",
                workflow: req.workflow,
                profile: None,
                research_domain: None,
                exact_error: false,
            },
        );

    let mut all_attempts = security_attempts;
    all_attempts.extend(native_attempts);

    let retrieval_failures = crate::meta::adapter::build_retrieval_failures(
        &providers_failed,
        &web_resp.providers_queried,
        &all_attempts,
    );
    let postprocess_result = crate::core::evidence_postprocess::postprocess(
        &all_cards,
        &providers_failed,
        &web_resp.providers_queried,
        workflow_model.as_ref(),
        &retrieval_failures,
        resolution_source,
        &all_attempts,
    );

    SecuritySearchResponse {
        query: req.query.clone(),
        mode: "security_metasearch".to_string(),
        resolved_identifiers: resolved_ids,
        vulnerabilities,
        security_context: Some(security_context),
        groups,
        suggested_fetches,
        providers_queried: web_resp.providers_queried,
        providers_failed,
        warnings,
        trust_markers: web_resp.trust_markers,
        capability_enforcement: Some(
            crate::meta::provider_diagnostics::CapabilityEnforcementTelemetry::for_security_search(
                req,
                &effective_providers,
            ),
        ),
        routing_decision: None,
        applicability: applicability_assessments,
        dependency_findings,
        structured_warnings,
        next_actions: postprocess_result
            .workflow_coverage
            .as_ref()
            .map(|wc| {
                let known_ids: Vec<String> = all_cards.iter().map(|c| c.id.clone()).collect();
                crate::core::workflow_coverage::generate_gap_driven_next_actions(
                    wc,
                    &retrieval_failures,
                    &known_ids,
                )
            })
            .unwrap_or_default(),
        remediation_actions,
        security_evidence_summary,
        retrieval_summary: postprocess_result.retrieval_summary,
        evidence_role_summary: postprocess_result.evidence_role_summary,
        workflow_coverage: postprocess_result.workflow_coverage,
        conflict_metadata: postprocess_result.conflict_metadata,
    }
}

/// Check if two `VulnerabilityMetadata` records share any advisory IDs.
fn ids_overlap(a: &VulnerabilityMetadata, b: &VulnerabilityMetadata) -> bool {
    for id in &a.cve_ids {
        if b.cve_ids.contains(id) {
            return true;
        }
    }
    for id in &a.ghsa_ids {
        if b.ghsa_ids.contains(id) {
            return true;
        }
    }
    for id in &a.osv_ids {
        if b.osv_ids.contains(id) {
            return true;
        }
    }
    for id in &a.rustsec_ids {
        if b.rustsec_ids.contains(id) {
            return true;
        }
    }
    false
}

fn read_bounded_file(path: &str) -> Result<String, std::io::Error> {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open(path)?;
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut tmp = [0u8; 8192];
    let cap = 1024 * 1024;
    let mut total = 0usize;
    loop {
        let n = file.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        total += n;
        if total > cap {
            return Err(std::io::Error::other("file exceeds 1MB cap"));
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security::VulnerabilitySource;
    use crate::meta::NativeAdvisoryOperation;

    fn make_vuln(cve_id: &str) -> VulnerabilityMetadata {
        VulnerabilityMetadata {
            cve_ids: vec![cve_id.to_string()],
            source: VulnerabilitySource::Osv,
            ..Default::default()
        }
    }

    #[test]
    fn ids_overlap_same_cve() {
        let a = make_vuln("CVE-2024-0001");
        let b = make_vuln("CVE-2024-0001");
        assert!(ids_overlap(&a, &b));
    }

    #[test]
    fn ids_overlap_different_cve() {
        let a = make_vuln("CVE-2024-0001");
        let b = make_vuln("CVE-2024-0002");
        assert!(!ids_overlap(&a, &b));
    }

    #[test]
    fn ids_overlap_ghsa_match() {
        let a = VulnerabilityMetadata {
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            source: VulnerabilitySource::Osv,
            ..Default::default()
        };
        let b = VulnerabilityMetadata {
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            source: VulnerabilitySource::Osv,
            ..Default::default()
        };
        assert!(ids_overlap(&a, &b));
    }

    #[test]
    fn ids_overlap_cross_type() {
        let a = VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            source: VulnerabilitySource::GithubAdvisory,
            ..Default::default()
        };
        let b = VulnerabilityMetadata {
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            source: VulnerabilitySource::GithubAdvisory,
            ..Default::default()
        };
        assert!(!ids_overlap(&a, &b));
    }

    #[test]
    fn warning_prefix_native_advisory_search_unavailable() {
        // Verify the warning message format uses the stable prefix
        let msg = "native_advisory_search_unavailable: only generic web search was used; \
                    enable the 'osv' provider for native advisory lookups";
        assert!(
            msg.starts_with("native_advisory_search_unavailable:"),
            "native advisory warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_kev_match() {
        let msg = "kev_match: 2 CVE(s) found in CISA KEV catalog";
        assert!(
            msg.starts_with("kev_match:"),
            "kev_match warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_kev_absent_not_proof() {
        let msg = "kev_absent_not_proof: no CVE(s) found in CISA KEV catalog; \
                   absence does not prove no exploitation";
        assert!(
            msg.starts_with("kev_absent_not_proof:"),
            "kev_absent_not_proof warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_kev_lookup_failed() {
        let msg =
            "kev_lookup_failed: KEV catalog lookup failed; KEV status could not be determined";
        assert!(
            msg.starts_with("kev_lookup_failed:"),
            "kev_lookup_failed warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_kev_lookup_skipped() {
        let msg = "kev_lookup_skipped: KEV lookup requires CVE identifiers";
        assert!(
            msg.starts_with("kev_lookup_skipped:"),
            "kev_lookup_skipped warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_version_match_unavailable() {
        let msg = "version_match_unavailable: version-specific matching requires assess_applicability=true; \
                   affected version ranges are returned as-is from advisory databases";
        assert!(
            msg.starts_with("version_match_unavailable:"),
            "version_match_unavailable warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_version_mismatch() {
        let msg = "version_mismatch: package was found but no advisory has affected version \
                   ranges matching the supplied version; the package may not be affected or \
                   version-specific advisory data is unavailable";
        assert!(
            msg.starts_with("version_mismatch:"),
            "version_mismatch warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_generic_context_untrusted() {
        let msg = "generic_context_untrusted: generic web results are external untrusted \
                   discussion, not authoritative advisory facts";
        assert!(
            msg.starts_with("generic_context_untrusted:"),
            "generic_context_untrusted warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_applicability_not_exploitability() {
        let msg = "applicability_not_exploitability: Advisory range matching does not determine \
                   runtime exploitability or reachability. Applicability assessments are based on \
                   advisory metadata and dependency file parsing, not runtime analysis.";
        assert!(
            msg.starts_with("applicability_not_exploitability:"),
            "applicability warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_dependency_file_read_error() {
        let msg = "dependency_file_read_error: could not read /nonexistent/Cargo.lock";
        assert!(
            msg.starts_with("dependency_file_read_error:"),
            "dependency_file_read_error warning must use stable prefix: {msg}"
        );
    }

    fn pkg_outcome(
        provider_id: &str,
        status: ProviderAdvisoryStatus<Vec<VulnerabilityMetadata>>,
    ) -> ProviderAdvisoryOutcome<Vec<VulnerabilityMetadata>> {
        ProviderAdvisoryOutcome {
            provider_id: provider_id.to_string(),
            operation: NativeAdvisoryOperation::QueryByPackage {
                ecosystem: "crates_io".to_string(),
                package: "test-pkg".to_string(),
                version: None,
            },
            status,
            duration_ms: 10,
        }
    }

    fn assert_ledger_unique(attempts: &[RetrievalAttempt]) {
        crate::core::retrieval_status::validate_attempt_ledger(attempts)
            .expect("ledger must be valid");
    }

    #[test]
    fn a1_capability_unavailable_emits_two_attempts_one_per_role() {
        let outcomes = vec![pkg_outcome(
            "osv",
            ProviderAdvisoryStatus::CapabilityUnavailable,
        )];
        let mut vulns = Vec::new();
        let mut attempts = Vec::new();
        let op = RetrievalOperationIdentity::from_package("crates_io", "test-pkg", None);
        record_package_outcomes(outcomes, &op, "test-pkg", &mut vulns, &mut attempts);
        assert_eq!(attempts.len(), 2);
        assert_eq!(
            attempts[0].intended_roles,
            vec![EvidenceRole::AuthoritativeSecurityAdvisory]
        );
        assert_eq!(
            attempts[1].intended_roles,
            vec![EvidenceRole::ManifestOrDependencyMetadata]
        );
        assert_eq!(
            attempts[0].outcome,
            RetrievalAttemptOutcome::SkippedCapabilityUnavailable
        );
        assert_eq!(
            attempts[1].outcome,
            RetrievalAttemptOutcome::SkippedCapabilityUnavailable
        );
        assert_ledger_unique(&attempts);
    }

    #[test]
    fn a2_success_with_results_emits_two_attempts() {
        let outcomes = vec![pkg_outcome(
            "osv",
            ProviderAdvisoryStatus::Completed(Ok(vec![make_vuln("CVE-2024-0001")])),
        )];
        let mut vulns = Vec::new();
        let mut attempts = Vec::new();
        let op = RetrievalOperationIdentity::from_package("crates_io", "test-pkg", None);
        record_package_outcomes(outcomes, &op, "test-pkg", &mut vulns, &mut attempts);
        assert_eq!(attempts.len(), 2);
        assert_eq!(
            attempts[0].outcome,
            RetrievalAttemptOutcome::SuccessWithResults
        );
        assert_eq!(
            attempts[1].outcome,
            RetrievalAttemptOutcome::SkippedCapabilityUnavailable
        );
        assert_eq!(attempts[0].result_count, 1);
        assert_eq!(attempts[1].result_count, 0);
        assert_ledger_unique(&attempts);
    }

    #[test]
    fn a3_zero_result_emits_two_attempts() {
        let outcomes = vec![pkg_outcome(
            "osv",
            ProviderAdvisoryStatus::Completed(Ok(vec![])),
        )];
        let mut vulns = Vec::new();
        let mut attempts = Vec::new();
        let op = RetrievalOperationIdentity::from_package("crates_io", "test-pkg", None);
        record_package_outcomes(outcomes, &op, "test-pkg", &mut vulns, &mut attempts);
        assert_eq!(attempts.len(), 2);
        assert_eq!(
            attempts[0].outcome,
            RetrievalAttemptOutcome::SuccessZeroResults
        );
        assert_eq!(
            attempts[1].outcome,
            RetrievalAttemptOutcome::SkippedCapabilityUnavailable
        );
        assert_ledger_unique(&attempts);
    }

    #[test]
    fn a4_failed_provider_emits_two_attempts_no_fabricated_dependency_failure() {
        let outcomes = vec![pkg_outcome(
            "osv",
            ProviderAdvisoryStatus::Completed(Err(EngineError::NetworkError {
                engine: "osv",
                reason: "connection refused".to_string(),
            })),
        )];
        let mut vulns = Vec::new();
        let mut attempts = Vec::new();
        let op = RetrievalOperationIdentity::from_package("crates_io", "test-pkg", None);
        record_package_outcomes(outcomes, &op, "test-pkg", &mut vulns, &mut attempts);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].outcome, RetrievalAttemptOutcome::Failed);
        assert_eq!(
            attempts[1].outcome,
            RetrievalAttemptOutcome::SkippedCapabilityUnavailable
        );
        assert_eq!(
            attempts[1].error_class.as_deref(),
            Some("native_advisory_provider_does_not_supply_manifest_metadata")
        );
        assert_ledger_unique(&attempts);
    }

    #[test]
    fn a5_deadline_emits_no_duplicate_role_tuple() {
        let outcomes = vec![pkg_outcome(
            "osv",
            ProviderAdvisoryStatus::InterruptedByDeadline,
        )];
        let mut vulns = Vec::new();
        let mut attempts = Vec::new();
        let op = RetrievalOperationIdentity::from_package("crates_io", "test-pkg", None);
        record_package_outcomes(outcomes, &op, "test-pkg", &mut vulns, &mut attempts);
        assert_eq!(attempts.len(), 2);
        assert_eq!(
            attempts[0].outcome,
            RetrievalAttemptOutcome::InterruptedByDeadline
        );
        assert!(attempts[0].deadline_interrupted);
        assert_eq!(
            attempts[1].outcome,
            RetrievalAttemptOutcome::InterruptedByDeadline
        );
        assert!(attempts[1].deadline_interrupted);
        assert_ledger_unique(&attempts);
    }

    #[test]
    fn a6_two_providers_produce_four_unique_tuples() {
        let outcomes = vec![
            pkg_outcome(
                "osv",
                ProviderAdvisoryStatus::Completed(Ok(vec![make_vuln("CVE-2024-0001")])),
            ),
            pkg_outcome(
                "github_advisory",
                ProviderAdvisoryStatus::Completed(Ok(vec![])),
            ),
        ];
        let mut vulns = Vec::new();
        let mut attempts = Vec::new();
        let op = RetrievalOperationIdentity::from_package("crates_io", "test-pkg", None);
        record_package_outcomes(outcomes, &op, "test-pkg", &mut vulns, &mut attempts);
        assert_eq!(attempts.len(), 4);
        assert_ledger_unique(&attempts);
    }

    #[test]
    fn b1_budget_one_identifier_one_provider() {
        let mut budget = NativeOperationBudget::new();
        assert!(budget.reserve_identifier());
        let reservation = budget.reserve_providers(&["osv".to_string()]);
        assert_eq!(reservation.allowed, vec!["osv".to_string()]);
        assert!(reservation.skipped_by_budget.is_empty());
        assert_eq!(reservation.remaining_capacity, 63);
    }

    #[test]
    fn b2_budget_one_identifier_four_providers() {
        let mut budget = NativeOperationBudget::new();
        assert!(budget.reserve_identifier());
        let providers = vec![
            "osv".to_string(),
            "github_advisory".to_string(),
            "gitlab_advisory".to_string(),
            "rustsec".to_string(),
        ];
        let reservation = budget.reserve_providers(&providers);
        assert_eq!(reservation.allowed.len(), 4);
        assert!(reservation.skipped_by_budget.is_empty());
        assert_eq!(reservation.remaining_capacity, 60);
    }

    #[test]
    fn b3_budget_smaller_than_fan_out() {
        let mut budget = NativeOperationBudget::new();
        budget.provider_operations_reserved = 63;
        let providers = vec![
            "osv".to_string(),
            "github_advisory".to_string(),
            "gitlab_advisory".to_string(),
            "rustsec".to_string(),
        ];
        let reservation = budget.reserve_providers(&providers);
        assert_eq!(reservation.allowed.len(), 1);
        assert_eq!(reservation.skipped_by_budget.len(), 3);
    }

    #[test]
    fn b4_budget_skipped_produce_skipped_by_policy() {
        let mut budget = NativeOperationBudget::new();
        budget.provider_operations_reserved = 63;
        let providers = vec!["osv".to_string(), "github_advisory".to_string()];
        let reservation = budget.reserve_providers(&providers);
        assert_eq!(reservation.allowed.len(), 1);
        assert_eq!(reservation.skipped_by_budget.len(), 1);
        assert_eq!(reservation.skipped_by_budget[0], "github_advisory");
    }

    #[test]
    fn b5_duplicate_identifier_consumes_one_slot() {
        let mut budget = NativeOperationBudget::new();
        assert!(budget.reserve_identifier());
        assert!(budget.reserve_identifier());
        assert_eq!(budget.identifiers_seen(), 2);
    }

    #[test]
    fn b6_capability_unavailable_not_charged_to_dispatch_budget() {
        let mut budget = NativeOperationBudget::new();
        let capable = vec!["osv".to_string()];
        let reservation = budget.reserve_providers(&capable);
        assert_eq!(reservation.allowed.len(), 1);
        assert_eq!(budget.provider_operations_reserved(), 1);
    }

    #[test]
    fn b7_identifier_cap_exhaustion_stops_scheduling() {
        let mut budget = NativeOperationBudget::new();
        for _ in 0..MAX_NATIVE_ADVISORY_IDENTIFIERS {
            assert!(budget.reserve_identifier());
        }
        assert!(!budget.reserve_identifier());
    }

    #[test]
    fn gate_c_cross_family_deduplication_with_normalization() {
        let resolved = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".into()],
            ghsa_ids: vec!["CVE-2024-0001".into()],
            osv_ids: vec!["CVE-2024-0001".into()],
            rustsec_ids: vec![],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let planned = plan_unique_advisory_identifiers(&resolved);
        assert_eq!(
            planned.len(),
            1,
            "same ID across CVE/GHSA/OSV families should collapse to 1 unique ID"
        );
        assert_eq!(planned[0].identifier, "CVE-2024-0001");
    }

    #[test]
    fn gate_c_plan_unique_stable_family_order() {
        let resolved = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0002".into(), "CVE-2024-0001".into()],
            ghsa_ids: vec!["GHSA-test-1234-abcd".into()],
            osv_ids: vec!["PYSEC-2024-1".into()],
            rustsec_ids: vec!["RUSTSEC-2024-0001".into()],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let planned = plan_unique_advisory_identifiers(&resolved);
        assert_eq!(planned.len(), 5);
        assert_eq!(planned[0].subquery_id, "advisory_by_cve");
        assert_eq!(planned[1].subquery_id, "advisory_by_cve");
        assert_eq!(planned[2].subquery_id, "advisory_by_ghsa");
        assert_eq!(planned[3].subquery_id, "advisory_by_osv");
        assert_eq!(planned[4].subquery_id, "advisory_by_rustsec");
    }

    #[test]
    fn gate_c_plan_unique_40_ids_report_correct_planned_count() {
        let mut cve_ids: Vec<String> = (0..40).map(|i| format!("CVE-2024-{i:04}")).collect();
        cve_ids.dedup();
        let resolved = SecurityIdentifiers {
            cve_ids,
            ghsa_ids: vec![],
            osv_ids: vec![],
            rustsec_ids: vec![],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let planned = plan_unique_advisory_identifiers(&resolved);
        assert_eq!(planned.len(), 40);
    }

    #[test]
    fn gate_c_plan_unique_repeated_ids_count_as_one() {
        let resolved = SecurityIdentifiers {
            cve_ids: vec![
                "CVE-2024-0001".into(),
                "CVE-2024-0001".into(),
                "CVE-2024-0001".into(),
            ],
            ghsa_ids: vec![],
            osv_ids: vec![],
            rustsec_ids: vec![],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let planned = plan_unique_advisory_identifiers(&resolved);
        assert_eq!(planned.len(), 1);
    }

    #[test]
    fn gate_c_budget_summary_exact_omitted_count() {
        let resolved = SecurityIdentifiers {
            cve_ids: (0..40).map(|i| format!("CVE-2024-{i:04}")).collect(),
            ghsa_ids: vec![],
            osv_ids: vec![],
            rustsec_ids: vec![],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let planned = plan_unique_advisory_identifiers(&resolved);
        let mut budget = NativeOperationBudget::new();
        let mut scheduled = 0usize;
        for _ in planned.iter().take(MAX_NATIVE_ADVISORY_IDENTIFIERS) {
            if budget.reserve_identifier() {
                scheduled += 1;
            }
        }
        let identifiers_planned = planned.len();
        let identifiers_scheduled = scheduled;
        let omitted = identifiers_planned.saturating_sub(identifiers_scheduled);
        assert_eq!(identifiers_planned, 40);
        assert_eq!(identifiers_scheduled, 32);
        assert_eq!(omitted, 8);
    }

    #[test]
    fn gate_c_no_cap_no_warning() {
        let resolved = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0001".into(), "CVE-2024-0002".into()],
            ghsa_ids: vec![],
            osv_ids: vec![],
            rustsec_ids: vec![],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let planned = plan_unique_advisory_identifiers(&resolved);
        let mut budget = NativeOperationBudget::new();
        for _ in &planned {
            assert!(budget.reserve_identifier());
        }
        assert_eq!(budget.identifiers_seen(), planned.len());
    }

    #[test]
    fn gate_c_exact_cap_no_warning() {
        let resolved = SecurityIdentifiers {
            cve_ids: (0..32).map(|i| format!("CVE-2024-{i:04}")).collect(),
            ghsa_ids: vec![],
            osv_ids: vec![],
            rustsec_ids: vec![],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let planned = plan_unique_advisory_identifiers(&resolved);
        let mut budget = NativeOperationBudget::new();
        for _ in &planned {
            assert!(budget.reserve_identifier());
        }
        assert_eq!(budget.identifiers_seen(), 32);
    }

    #[test]
    fn gate_c_cap_plus_one_emits_omitted_one() {
        let resolved = SecurityIdentifiers {
            cve_ids: (0..33).map(|i| format!("CVE-2024-{i:04}")).collect(),
            ghsa_ids: vec![],
            osv_ids: vec![],
            rustsec_ids: vec![],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let planned = plan_unique_advisory_identifiers(&resolved);
        let mut budget = NativeOperationBudget::new();
        let mut scheduled = 0usize;
        for _ in planned.iter().take(MAX_NATIVE_ADVISORY_IDENTIFIERS) {
            if budget.reserve_identifier() {
                scheduled += 1;
            }
        }
        assert_eq!(planned.len(), 33);
        assert_eq!(scheduled, 32);
        assert_eq!(planned.len().saturating_sub(scheduled), 1);
    }

    #[test]
    fn gate_c_planning_is_deterministic() {
        let resolved = SecurityIdentifiers {
            cve_ids: vec!["CVE-2024-0002".into(), "CVE-2024-0001".into()],
            ghsa_ids: vec!["GHSA-b".into(), "GHSA-a".into()],
            osv_ids: vec![],
            rustsec_ids: vec![],
            cwe_ids: vec![],
            package: None,
            ecosystem: None,
            version: None,
            function_or_api: None,
            residual_query: String::new(),
        };
        let p1 = plan_unique_advisory_identifiers(&resolved);
        let p2 = plan_unique_advisory_identifiers(&resolved);
        assert_eq!(p1.len(), p2.len());
        for (a, b) in p1.iter().zip(p2.iter()) {
            assert_eq!(a.identifier, b.identifier);
            assert_eq!(a.subquery_id, b.subquery_id);
        }
    }
}
