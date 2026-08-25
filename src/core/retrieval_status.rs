use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::evidence_role::EvidenceRole;
use crate::core::workflow_coverage::{RetrievalFailure, RetrievalFailureKind};

/// Compute a bounded, non-recoverable query fingerprint from raw query text.
///
/// Uses FNV-1a 64-bit hash formatted as a hex string. The fingerprint
/// preserves no recoverable query content (credentials, file paths, tokens,
/// or proprietary fragments are not leaked).
pub fn query_fingerprint_from_query(query: &str) -> String {
    let mut hasher = crate::core::identity::FnvHasher::new();
    hasher.write(query.as_bytes());
    format!("fp_{:016x}", hasher.finish())
}

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAbsenceKind {
    NoMatchingEvidenceFound,
    ProviderCapabilityUnavailable,
    ProviderSkippedByPolicy,
    ProviderFailed,
    DeadlinePreventedCompletion,
    ResultTruncatedByCap,
    EvidenceRoleNotRequested,
    EvidenceRoleRequestedButNotFound,
    EvidenceRoleIndeterminateBecauseRetrievalFailed,
    #[default]
    NotApplicable,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalDimensionStatus {
    pub evidence_role: EvidenceRole,
    pub absence_kind: EvidenceAbsenceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subquery_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_outcome: Option<RetrievalAttemptOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// Evidence supporting a confirmed or possible truncation signal.
    #[serde(default, skip_serializing_if = "is_none_truncation_evidence")]
    pub truncation_evidence: TruncationEvidence,
    /// Authoritative coarse terminal state for this dimension.
    ///
    /// This field is additive and optional for backward compatibility.
    /// Consumers should prefer `state` for coarse terminal interpretation,
    /// `attempt_outcome` for exact operation outcome, and
    /// `truncation_evidence` for completeness qualification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<RetrievalDimensionState>,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ResponseRetrievalSummary {
    pub dimensions: Vec<RetrievalDimensionStatus>,
    pub has_failures: bool,
    pub has_absences: bool,
    pub has_truncation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempted_job_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_job_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_job_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zero_result_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timed_out_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limited_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_skipped_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_skipped_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_interrupted_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reached_unknown_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles_attempted: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles_complete: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles_indeterminate: Option<usize>,
    /// Number of role-expanded dimensions (attempt level × role expansion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempted_dimension_count: Option<usize>,
    /// Dimensions whose attempt completed (success, zero-result, not-applicable, partial).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_dimension_count: Option<usize>,
    /// Dimensions whose attempt failed, timed out, or was rate-limited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_dimension_count: Option<usize>,
    /// Dimensions whose attempt outcome was `NotApplicable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_applicable_count: Option<usize>,
    /// Attempts whose outcome was `NotApplicable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_applicable_job_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectiveDimensionState {
    Satisfied,
    CompletedNoMatch,
    Failed,
    SkippedByPolicy,
    CapabilityUnavailable,
    Interrupted,
    Partial,
    NotApplicable,
    LegacyOtherAbsence,
}

fn dimension_state_or_legacy(d: &RetrievalDimensionStatus) -> EffectiveDimensionState {
    if let Some(state) = d.state {
        match state {
            RetrievalDimensionState::Satisfied => EffectiveDimensionState::Satisfied,
            RetrievalDimensionState::CompletedNoMatch => EffectiveDimensionState::CompletedNoMatch,
            RetrievalDimensionState::Failed => EffectiveDimensionState::Failed,
            RetrievalDimensionState::SkippedByPolicy => EffectiveDimensionState::SkippedByPolicy,
            RetrievalDimensionState::CapabilityUnavailable => {
                EffectiveDimensionState::CapabilityUnavailable
            }
            RetrievalDimensionState::Interrupted => EffectiveDimensionState::Interrupted,
            RetrievalDimensionState::Partial => EffectiveDimensionState::Partial,
            RetrievalDimensionState::NotApplicable => EffectiveDimensionState::NotApplicable,
        }
    } else {
        match d.absence_kind {
            EvidenceAbsenceKind::NotApplicable => EffectiveDimensionState::LegacyOtherAbsence,
            EvidenceAbsenceKind::NoMatchingEvidenceFound => {
                EffectiveDimensionState::CompletedNoMatch
            }
            EvidenceAbsenceKind::ProviderFailed => EffectiveDimensionState::Failed,
            EvidenceAbsenceKind::DeadlinePreventedCompletion => {
                EffectiveDimensionState::Interrupted
            }
            EvidenceAbsenceKind::ProviderSkippedByPolicy => {
                EffectiveDimensionState::SkippedByPolicy
            }
            EvidenceAbsenceKind::ProviderCapabilityUnavailable => {
                EffectiveDimensionState::CapabilityUnavailable
            }
            EvidenceAbsenceKind::ResultTruncatedByCap => EffectiveDimensionState::Partial,
            EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound => {
                EffectiveDimensionState::CompletedNoMatch
            }
            EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed => {
                EffectiveDimensionState::Failed
            }
            EvidenceAbsenceKind::EvidenceRoleNotRequested => EffectiveDimensionState::NotApplicable,
        }
    }
}

#[allow(missing_docs)]
pub fn summarize_retrieval(dimensions: Vec<RetrievalDimensionStatus>) -> ResponseRetrievalSummary {
    let mut has_failures = false;
    let mut has_absences = false;
    let mut has_truncation = false;

    let mut roles_seen = std::collections::HashSet::new();
    let mut roles_with_success = std::collections::HashSet::new();
    let mut roles_indeterminate = std::collections::HashSet::new();

    let mut completed_dimension_count = 0usize;
    let mut failed_dimension_count = 0usize;
    let mut not_applicable_count = 0usize;

    for d in &dimensions {
        roles_seen.insert(d.evidence_role);

        let effective_state = dimension_state_or_legacy(d);

        match effective_state {
            EffectiveDimensionState::Satisfied => {
                roles_with_success.insert(d.evidence_role);
                completed_dimension_count += 1;
            }
            EffectiveDimensionState::CompletedNoMatch => {
                roles_with_success.insert(d.evidence_role);
                completed_dimension_count += 1;
            }
            EffectiveDimensionState::Failed => {
                roles_indeterminate.insert(d.evidence_role);
                has_failures = true;
                failed_dimension_count += 1;
            }
            EffectiveDimensionState::SkippedByPolicy => {
                roles_indeterminate.insert(d.evidence_role);
            }
            EffectiveDimensionState::CapabilityUnavailable => {
                roles_indeterminate.insert(d.evidence_role);
            }
            EffectiveDimensionState::Interrupted => {
                roles_indeterminate.insert(d.evidence_role);
                has_failures = true;
                failed_dimension_count += 1;
            }
            EffectiveDimensionState::Partial => {
                roles_indeterminate.insert(d.evidence_role);
                has_truncation = true;
                completed_dimension_count += 1;
            }
            EffectiveDimensionState::NotApplicable => {
                not_applicable_count += 1;
                completed_dimension_count += 1;
            }
            EffectiveDimensionState::LegacyOtherAbsence => {
                completed_dimension_count += 1;
            }
        }

        match d.truncation_evidence {
            TruncationEvidence::LimitReachedUnknown => {}
            TruncationEvidence::ConfirmedByEggsearch | TruncationEvidence::ConfirmedByProvider => {
                has_truncation = true;
            }
            TruncationEvidence::None => {}
        }

        if d.absence_kind == EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound {
            has_absences = true;
        }
    }

    let dimension_count = dimensions.len();

    ResponseRetrievalSummary {
        dimensions,
        has_failures,
        has_absences,
        has_truncation,
        attempted_job_count: None,
        completed_job_count: None,
        failed_job_count: None,
        zero_result_count: None,
        timed_out_count: None,
        rate_limited_count: None,
        policy_skipped_count: None,
        capability_skipped_count: None,
        deadline_interrupted_count: None,
        truncated_count: None,
        limit_reached_unknown_count: None,
        roles_attempted: Some(roles_seen.len()),
        roles_complete: Some(roles_with_success.len()),
        roles_indeterminate: Some(roles_indeterminate.len()),
        attempted_dimension_count: Some(dimension_count),
        completed_dimension_count: Some(completed_dimension_count),
        failed_dimension_count: Some(failed_dimension_count),
        not_applicable_count: Some(not_applicable_count),
        not_applicable_job_count: None,
    }
}

/// Accumulator for attempt-level summary counts.
///
/// Attempt-level counts are derived directly from `&[RetrievalAttempt]`,
/// not from expanded role dimensions. This prevents multi-role attempts
/// from inflating job counts.
#[derive(Default, Clone, Debug)]
#[allow(missing_docs)]
pub struct AttemptSummaryCounts {
    pub attempted: usize,
    pub completed: usize,
    pub failed: usize,
    pub zero_result: usize,
    pub timed_out: usize,
    pub rate_limited: usize,
    pub policy_skipped: usize,
    pub capability_skipped: usize,
    pub deadline_interrupted: usize,
    pub confirmed_truncated: usize,
    pub limit_reached_unknown: usize,
    pub not_applicable: usize,
}

impl AttemptSummaryCounts {
    /// Compute attempt-level counts from a slice of retrieval attempts.
    pub fn from_attempts(attempts: &[RetrievalAttempt]) -> Self {
        let mut counts = AttemptSummaryCounts::default();
        for attempt in attempts {
            counts.attempted += 1;
            match attempt.outcome {
                RetrievalAttemptOutcome::SuccessWithResults => {
                    counts.completed += 1;
                }
                RetrievalAttemptOutcome::SuccessZeroResults => {
                    counts.completed += 1;
                    counts.zero_result += 1;
                }
                RetrievalAttemptOutcome::Failed => {
                    counts.failed += 1;
                }
                RetrievalAttemptOutcome::TimedOut => {
                    counts.failed += 1;
                    counts.timed_out += 1;
                }
                RetrievalAttemptOutcome::RateLimited => {
                    counts.failed += 1;
                    counts.rate_limited += 1;
                }
                RetrievalAttemptOutcome::InterruptedByDeadline => {
                    counts.failed += 1;
                    counts.deadline_interrupted += 1;
                }
                RetrievalAttemptOutcome::SkippedByPolicy => {
                    counts.policy_skipped += 1;
                }
                RetrievalAttemptOutcome::SkippedCapabilityUnavailable => {
                    counts.capability_skipped += 1;
                }
                RetrievalAttemptOutcome::NotApplicable => {
                    counts.completed += 1;
                    counts.not_applicable += 1;
                }
                RetrievalAttemptOutcome::TruncatedAfterPartialSuccess => {
                    counts.completed += 1;
                }
            }

            let eff_truncation = effective_truncation_evidence(attempt);
            if matches!(
                eff_truncation,
                TruncationEvidence::ConfirmedByEggsearch | TruncationEvidence::ConfirmedByProvider
            ) {
                counts.confirmed_truncated += 1;
            }
            if attempt.truncation_evidence == TruncationEvidence::LimitReachedUnknown {
                counts.limit_reached_unknown += 1;
            }
        }
        counts
    }
}

/// Summarize retrieval with both attempt-level and dimension-level counts.
///
/// This is the authoritative summary path used when retrieval attempts
/// are available. Job counts (`attempted_job_count`, `completed_job_count`,
/// `failed_job_count`) are derived from `&[RetrievalAttempt]`, while
/// dimension counts (`attempted_dimension_count`, etc.) are derived from
/// the role-expanded dimensions.
///
/// Invariants:
/// - `attempted_job_count == attempts.len()`
/// - `attempted_dimension_count == dimensions.len()`
/// - `attempted_dimension_count >= attempted_job_count`
/// - `attempted_job_count == completed + failed + policy_skipped + capability_skipped`
pub fn summarize_retrieval_with_attempts(
    attempts: &[RetrievalAttempt],
    dimensions: Vec<RetrievalDimensionStatus>,
) -> ResponseRetrievalSummary {
    let attempt_counts = AttemptSummaryCounts::from_attempts(attempts);

    let mut summary = summarize_retrieval(dimensions);

    summary.attempted_job_count = Some(attempt_counts.attempted);
    summary.completed_job_count = Some(attempt_counts.completed);
    summary.failed_job_count = Some(attempt_counts.failed);
    summary.zero_result_count = Some(attempt_counts.zero_result);
    summary.timed_out_count = Some(attempt_counts.timed_out);
    summary.rate_limited_count = Some(attempt_counts.rate_limited);
    summary.policy_skipped_count = Some(attempt_counts.policy_skipped);
    summary.capability_skipped_count = Some(attempt_counts.capability_skipped);
    summary.deadline_interrupted_count = Some(attempt_counts.deadline_interrupted);
    summary.truncated_count = Some(attempt_counts.confirmed_truncated);
    summary.limit_reached_unknown_count = Some(attempt_counts.limit_reached_unknown);
    summary.not_applicable_job_count = Some(attempt_counts.not_applicable);

    summary
}

#[allow(missing_docs)]
pub fn is_absence_only(summary: &ResponseRetrievalSummary) -> bool {
    summary.dimensions.iter().all(|d| {
        if let Some(ref state) = d.state {
            matches!(
                state,
                RetrievalDimensionState::CompletedNoMatch | RetrievalDimensionState::NotApplicable
            )
        } else {
            matches!(
                d.absence_kind,
                EvidenceAbsenceKind::NoMatchingEvidenceFound
                    | EvidenceAbsenceKind::EvidenceRoleNotRequested
                    | EvidenceAbsenceKind::NotApplicable
            )
        }
    })
}

/// Returns true when every applicable dimension is failed or interrupted.
///
/// An applicable dimension is one that is not NotApplicable or SkippedByPolicy.
/// Returns false if there are no applicable dimensions.
#[allow(missing_docs)]
pub fn is_failure_only(summary: &ResponseRetrievalSummary) -> bool {
    let mut has_applicable = false;
    for d in &summary.dimensions {
        let state = dimension_state_or_legacy(d);
        match state {
            EffectiveDimensionState::NotApplicable
            | EffectiveDimensionState::SkippedByPolicy
            | EffectiveDimensionState::CapabilityUnavailable => {}
            EffectiveDimensionState::Failed | EffectiveDimensionState::Interrupted => {
                has_applicable = true;
            }
            _ => return false,
        }
    }
    has_applicable
}

#[allow(missing_docs)]
pub fn has_indeterminate(summary: &ResponseRetrievalSummary) -> bool {
    summary.dimensions.iter().any(|d| {
        if d.state.is_some() {
            dimension_is_indeterminate(d)
        } else {
            d.absence_kind == EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed
        }
    })
}

#[allow(missing_docs)]
pub fn absent_roles(summary: &ResponseRetrievalSummary) -> Vec<EvidenceRole> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for d in &summary.dimensions {
        let is_absent = if let Some(ref state) = d.state {
            *state == RetrievalDimensionState::CompletedNoMatch
        } else {
            d.absence_kind == EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound
        };
        if is_absent && seen.insert(d.evidence_role) {
            result.push(d.evidence_role);
        }
    }

    result
}

#[allow(missing_docs)]
pub fn failed_providers(summary: &ResponseRetrievalSummary) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for d in &summary.dimensions {
        let is_failed = if let Some(ref state) = d.state {
            matches!(
                state,
                RetrievalDimensionState::Failed | RetrievalDimensionState::Interrupted
            )
        } else {
            matches!(
                d.absence_kind,
                EvidenceAbsenceKind::ProviderFailed
                    | EvidenceAbsenceKind::DeadlinePreventedCompletion
            )
        };
        if is_failed {
            if let Some(ref pid) = d.provider_id {
                if seen.insert(pid.clone()) {
                    result.push(pid.clone());
                }
            }
        }
    }

    result
}

#[allow(missing_docs)]
pub fn classify_absence(kind: EvidenceAbsenceKind) -> &'static str {
    match kind {
        EvidenceAbsenceKind::NoMatchingEvidenceFound => "no_matching_evidence_found",
        EvidenceAbsenceKind::ProviderCapabilityUnavailable => "provider_capability_unavailable",
        EvidenceAbsenceKind::ProviderSkippedByPolicy => "provider_skipped_by_policy",
        EvidenceAbsenceKind::ProviderFailed => "provider_failed",
        EvidenceAbsenceKind::DeadlinePreventedCompletion => "deadline_prevented_completion",
        EvidenceAbsenceKind::ResultTruncatedByCap => "result_truncated_by_cap",
        EvidenceAbsenceKind::EvidenceRoleNotRequested => "evidence_role_not_requested",
        EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound => {
            "evidence_role_requested_but_not_found"
        }
        EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed => {
            "evidence_role_indeterminate_because_retrieval_failed"
        }
        EvidenceAbsenceKind::NotApplicable => "not_applicable",
    }
}

/// Outcome of a single retrieval attempt.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalAttemptOutcome {
    /// Provider returned results.
    SuccessWithResults,
    /// Provider returned successfully but zero results.
    SuccessZeroResults,
    /// Provider returned an error.
    Failed,
    /// Provider timed out.
    TimedOut,
    /// Provider was rate-limited.
    RateLimited,
    /// Provider was skipped by operator policy.
    SkippedByPolicy,
    /// Provider capability was unavailable.
    SkippedCapabilityUnavailable,
    /// Not applicable to this query.
    NotApplicable,
    /// Interrupted by global deadline.
    InterruptedByDeadline,
    /// Truncated after partial success.
    TruncatedAfterPartialSuccess,
}

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TruncationEvidence {
    #[default]
    None,
    LimitReachedUnknown,
    ConfirmedByEggsearch,
    ConfirmedByProvider,
}

fn is_none_truncation_evidence(value: &TruncationEvidence) -> bool {
    *value == TruncationEvidence::None
}

/// Compute the effective truncation evidence for an attempt.
///
/// If the attempt explicitly carries truncation evidence, use it.
/// Otherwise, infer from the `truncated` flag or
/// `TruncatedAfterPartialSuccess` outcome.
pub fn effective_truncation_evidence(attempt: &RetrievalAttempt) -> TruncationEvidence {
    if attempt.truncation_evidence != TruncationEvidence::None {
        return attempt.truncation_evidence;
    }
    if attempt.truncated || attempt.outcome == RetrievalAttemptOutcome::TruncatedAfterPartialSuccess
    {
        TruncationEvidence::ConfirmedByEggsearch
    } else {
        TruncationEvidence::None
    }
}

/// Coarse terminal state for a single evidence dimension.
///
/// This is the authoritative field for terminal interpretation.
/// `absence_kind` describes absence/failure context and is not a
/// complete success-state enum. Consumers should prefer `state` for
/// coarse terminal status, `attempt_outcome` for exact operation
/// outcome, and `truncation_evidence` for completeness qualification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalDimensionState {
    /// Retrieval completed and evidence was found.
    Satisfied,
    /// Retrieval completed but no evidence was found.
    CompletedNoMatch,
    /// The provider operation failed, timed out, or was rate-limited.
    Failed,
    /// The provider was deliberately suppressed by operator policy.
    SkippedByPolicy,
    /// The provider cannot perform the requested operation.
    CapabilityUnavailable,
    /// The global deadline prevented completion.
    Interrupted,
    /// Results were truncated; the dimension is partially satisfied.
    Partial,
    /// The operation did not apply to this request.
    NotApplicable,
}

/// Map a retrieval attempt outcome to its authoritative dimension state.
///
/// Confirmed truncation (via `truncation_evidence` or `truncated` flag)
/// takes precedence and maps to `Partial`. Uncertain limit reached
/// (`LimitReachedUnknown`) leaves the state as `Satisfied` because the
/// provider call completed and returned evidence.
pub fn attempt_outcome_to_dimension_state(attempt: &RetrievalAttempt) -> RetrievalDimensionState {
    let truncation_evidence = effective_truncation_evidence(attempt);
    if matches!(
        truncation_evidence,
        TruncationEvidence::ConfirmedByEggsearch | TruncationEvidence::ConfirmedByProvider
    ) {
        return RetrievalDimensionState::Partial;
    }
    match attempt.outcome {
        RetrievalAttemptOutcome::SuccessWithResults => RetrievalDimensionState::Satisfied,
        RetrievalAttemptOutcome::SuccessZeroResults => RetrievalDimensionState::CompletedNoMatch,
        RetrievalAttemptOutcome::Failed
        | RetrievalAttemptOutcome::TimedOut
        | RetrievalAttemptOutcome::RateLimited => RetrievalDimensionState::Failed,
        RetrievalAttemptOutcome::SkippedByPolicy => RetrievalDimensionState::SkippedByPolicy,
        RetrievalAttemptOutcome::SkippedCapabilityUnavailable => {
            RetrievalDimensionState::CapabilityUnavailable
        }
        RetrievalAttemptOutcome::InterruptedByDeadline => RetrievalDimensionState::Interrupted,
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess => RetrievalDimensionState::Partial,
        RetrievalAttemptOutcome::NotApplicable => RetrievalDimensionState::NotApplicable,
    }
}

#[allow(dead_code)]
fn dimension_is_satisfied(d: &RetrievalDimensionStatus) -> bool {
    d.state == Some(RetrievalDimensionState::Satisfied)
}

#[allow(dead_code)]
fn dimension_is_completed_no_match(d: &RetrievalDimensionStatus) -> bool {
    d.state == Some(RetrievalDimensionState::CompletedNoMatch)
}

#[allow(dead_code)]
fn dimension_is_not_applicable(d: &RetrievalDimensionStatus) -> bool {
    d.state == Some(RetrievalDimensionState::NotApplicable)
}

#[allow(dead_code)]
fn dimension_is_failed_or_interrupted(d: &RetrievalDimensionStatus) -> bool {
    matches!(
        d.state,
        Some(RetrievalDimensionState::Failed | RetrievalDimensionState::Interrupted)
    )
}

fn dimension_is_indeterminate(d: &RetrievalDimensionStatus) -> bool {
    matches!(
        d.state,
        Some(
            RetrievalDimensionState::Failed
                | RetrievalDimensionState::SkippedByPolicy
                | RetrievalDimensionState::CapabilityUnavailable
                | RetrievalDimensionState::Interrupted
                | RetrievalDimensionState::Partial
        )
    )
}

/// Internal operation identity used for ledger deduplication.
///
/// This is a deterministic, provider-independent identifier for a
/// retrieval operation. It is used internally for deduplication and
/// testing. It does not contain raw query text.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[allow(missing_docs)]
pub enum RetrievalOperationIdentity {
    /// A generic search subquery.
    SearchSubquery { subquery_id: String },
    /// A native advisory identifier lookup.
    AdvisoryLookupById {
        vulnerability_id_fingerprint: String,
    },
    /// A native advisory package query.
    AdvisoryQueryByPackage {
        ecosystem: String,
        package_fingerprint: String,
        version_fingerprint: Option<String>,
    },
    /// A CISA KEV lookup by CVE ID.
    KevLookup { cve_id_fingerprint: String },
}

impl RetrievalOperationIdentity {
    /// Compute a bounded, non-recoverable fingerprint for an identifier.
    fn fingerprint(value: &str) -> String {
        crate::core::retrieval_status::query_fingerprint_from_query(value)
    }

    /// Build an operation identity from a subquery label and ID.
    pub fn from_search_subquery(subquery_id: &str) -> Self {
        RetrievalOperationIdentity::SearchSubquery {
            subquery_id: subquery_id.to_string(),
        }
    }

    /// Build an operation identity from an advisory identifier.
    pub fn from_advisory_id(vulnerability_id: &str) -> Self {
        RetrievalOperationIdentity::AdvisoryLookupById {
            vulnerability_id_fingerprint: Self::fingerprint(vulnerability_id),
        }
    }

    /// Build an operation identity from a package coordinate.
    pub fn from_package(ecosystem: &str, package: &str, version: Option<&str>) -> Self {
        RetrievalOperationIdentity::AdvisoryQueryByPackage {
            ecosystem: ecosystem.to_lowercase(),
            package_fingerprint: Self::fingerprint(package),
            version_fingerprint: version.map(Self::fingerprint),
        }
    }

    /// Build an operation identity from a CVE ID for KEV lookup.
    pub fn from_kev_cve(cve_id: &str) -> Self {
        RetrievalOperationIdentity::KevLookup {
            cve_id_fingerprint: Self::fingerprint(cve_id),
        }
    }

    /// Deterministic bounded identifier string for this operation instance.
    ///
    /// Distinguishes multiple operations sharing one subquery label.
    /// Does not contain raw query text, tokens, file contents, or secrets.
    pub fn stable_id(&self) -> String {
        match self {
            RetrievalOperationIdentity::SearchSubquery { subquery_id } => {
                format!("search:{subquery_id}")
            }
            RetrievalOperationIdentity::AdvisoryLookupById {
                vulnerability_id_fingerprint,
            } => {
                format!("advisory-id:{vulnerability_id_fingerprint}")
            }
            RetrievalOperationIdentity::AdvisoryQueryByPackage {
                ecosystem,
                package_fingerprint,
                version_fingerprint,
            } => match version_fingerprint {
                Some(vf) => format!("advisory-package:{ecosystem}:{package_fingerprint}:{vf}"),
                None => format!("advisory-package:{ecosystem}:{package_fingerprint}:none"),
            },
            RetrievalOperationIdentity::KevLookup { cve_id_fingerprint } => {
                format!("kev:{cve_id_fingerprint}")
            }
        }
    }
}

/// Violation detected by [`validate_attempt_ledger`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum AttemptLedgerViolation {
    /// A provider/operation/role tuple appeared more than once.
    DuplicateProviderOperationRole {
        provider_id: String,
        operation_id: Option<String>,
        subquery_id: Option<String>,
        role: String,
    },
    /// An attempt has an empty provider ID.
    EmptyProviderId,
    /// An attempt's role vector contains a duplicate role.
    DuplicateRoleInAttempt { provider_id: String, role: String },
    /// A failure or skip outcome has a nonzero result count.
    ResultCountWithFailure {
        provider_id: String,
        result_count: usize,
    },
    /// `SuccessZeroResults` has a nonzero result count.
    ZeroResultWithCount {
        provider_id: String,
        result_count: usize,
    },
    /// `SuccessWithResults` has a zero result count.
    SuccessWithZeroResults { provider_id: String },
    /// A deadline outcome lacks `deadline_interrupted = true`.
    DeadlineWithoutFlag { provider_id: String },
    /// Confirmed truncation without a success/partial-success outcome.
    TruncationWithoutSuccess { provider_id: String },
    /// A capability skip has an empty role set.
    CapabilitySkipWithEmptyRoles { provider_id: String },
}

/// Validate the terminal-attempt ledger invariant.
///
/// The canonical invariant is: for a given request, there may be at most
/// one terminal retrieval attempt for each distinct
/// `(provider_id, operation identity, evidence role)` tuple.
///
/// This is a pure validation helper used by tests and optionally by
/// debug assertions. It does not fail production responses.
pub fn validate_attempt_ledger(
    attempts: &[RetrievalAttempt],
) -> Result<(), AttemptLedgerViolation> {
    let mut seen: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();

    for attempt in attempts {
        if attempt.provider_id.is_empty() {
            return Err(AttemptLedgerViolation::EmptyProviderId);
        }

        let mut role_seen: std::collections::HashSet<EvidenceRole> =
            std::collections::HashSet::new();
        for role in &attempt.intended_roles {
            if !role_seen.insert(*role) {
                return Err(AttemptLedgerViolation::DuplicateRoleInAttempt {
                    provider_id: attempt.provider_id.clone(),
                    role: role.label().to_string(),
                });
            }
        }

        if attempt.intended_roles.is_empty()
            && matches!(
                attempt.outcome,
                RetrievalAttemptOutcome::SkippedCapabilityUnavailable
            )
        {
            return Err(AttemptLedgerViolation::CapabilitySkipWithEmptyRoles {
                provider_id: attempt.provider_id.clone(),
            });
        }

        let roles: Vec<EvidenceRole> = if attempt.intended_roles.is_empty() {
            vec![EvidenceRole::UnknownOrWeakContext]
        } else {
            attempt.intended_roles.clone()
        };

        let operation_id = attempt
            .operation_id
            .clone()
            .or_else(|| {
                attempt
                    .subquery_id
                    .as_ref()
                    .map(|s| format!("legacy-subquery:{s}"))
            })
            .unwrap_or_else(|| "legacy-unknown".to_string());

        for role in &roles {
            let key = (
                attempt.provider_id.clone(),
                operation_id.clone(),
                role.label().to_string(),
            );
            if !seen.insert(key) {
                return Err(AttemptLedgerViolation::DuplicateProviderOperationRole {
                    provider_id: attempt.provider_id.clone(),
                    operation_id: attempt.operation_id.clone(),
                    subquery_id: attempt.subquery_id.clone(),
                    role: role.label().to_string(),
                });
            }
        }

        match attempt.outcome {
            RetrievalAttemptOutcome::Failed
            | RetrievalAttemptOutcome::TimedOut
            | RetrievalAttemptOutcome::RateLimited
            | RetrievalAttemptOutcome::SkippedByPolicy
            | RetrievalAttemptOutcome::SkippedCapabilityUnavailable
            | RetrievalAttemptOutcome::InterruptedByDeadline => {
                if attempt.result_count > 0 {
                    return Err(AttemptLedgerViolation::ResultCountWithFailure {
                        provider_id: attempt.provider_id.clone(),
                        result_count: attempt.result_count,
                    });
                }
            }
            RetrievalAttemptOutcome::SuccessZeroResults => {
                if attempt.result_count > 0 {
                    return Err(AttemptLedgerViolation::ZeroResultWithCount {
                        provider_id: attempt.provider_id.clone(),
                        result_count: attempt.result_count,
                    });
                }
            }
            RetrievalAttemptOutcome::SuccessWithResults => {
                if attempt.result_count == 0 {
                    return Err(AttemptLedgerViolation::SuccessWithZeroResults {
                        provider_id: attempt.provider_id.clone(),
                    });
                }
            }
            RetrievalAttemptOutcome::NotApplicable
            | RetrievalAttemptOutcome::TruncatedAfterPartialSuccess => {}
        }

        if attempt.outcome == RetrievalAttemptOutcome::InterruptedByDeadline
            && !attempt.deadline_interrupted
        {
            return Err(AttemptLedgerViolation::DeadlineWithoutFlag {
                provider_id: attempt.provider_id.clone(),
            });
        }

        let truncation_evidence = effective_truncation_evidence(attempt);
        if matches!(
            truncation_evidence,
            TruncationEvidence::ConfirmedByEggsearch | TruncationEvidence::ConfirmedByProvider
        ) && !matches!(
            attempt.outcome,
            RetrievalAttemptOutcome::SuccessWithResults
                | RetrievalAttemptOutcome::TruncatedAfterPartialSuccess
        ) {
            return Err(AttemptLedgerViolation::TruncationWithoutSuccess {
                provider_id: attempt.provider_id.clone(),
            });
        }
    }

    Ok(())
}

/// Validate a complete attempt ledger in debug/test builds.
///
/// Panics in debug and test builds if the ledger violates invariants.
/// No-op in release builds.
#[inline]
pub fn debug_validate_attempt_ledger(context: &str, attempts: &[RetrievalAttempt]) {
    if cfg!(debug_assertions) || cfg!(test) {
        if let Err(err) = validate_attempt_ledger(attempts) {
            panic!(
                "{context}: assembled retrieval attempt ledger must satisfy invariants: {err:?}"
            );
        }
    }
}

/// Record of a single provider/subquery retrieval attempt.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalAttempt {
    /// The provider that was attempted.
    pub provider_id: String,
    /// Optional subquery identifier for multi-query workflows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subquery_id: Option<String>,
    /// Deterministic bounded identifier for this operation instance.
    ///
    /// Distinguishes multiple operations sharing one subquery label.
    /// It must not contain raw query text, tokens, file contents, or secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Evidence roles this attempt was intended to produce.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intended_roles: Vec<EvidenceRole>,
    /// The outcome of the retrieval attempt.
    pub outcome: RetrievalAttemptOutcome,
    /// Number of results returned (0 if failed or timed out).
    pub result_count: usize,
    /// Coarse error classification if the attempt failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// Whether the global deadline interrupted this attempt.
    #[serde(default)]
    pub deadline_interrupted: bool,
    /// Whether results or response were truncated by a cap.
    #[serde(default)]
    pub truncated: bool,
    /// Evidence supporting a confirmed or possible truncation signal.
    #[serde(default, skip_serializing_if = "is_none_truncation_evidence")]
    pub truncation_evidence: TruncationEvidence,
    /// Bounded query fingerprint or label for the query that was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_fingerprint: Option<String>,
    /// Duration of the attempt in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl RetrievalAttempt {
    #[allow(missing_docs)]
    pub fn to_retrieval_failures(&self) -> Vec<RetrievalFailure> {
        match self.outcome {
            RetrievalAttemptOutcome::Failed
            | RetrievalAttemptOutcome::TimedOut
            | RetrievalAttemptOutcome::RateLimited
            | RetrievalAttemptOutcome::InterruptedByDeadline
            | RetrievalAttemptOutcome::SkippedByPolicy
            | RetrievalAttemptOutcome::SkippedCapabilityUnavailable => {}
            _ => return Vec::new(),
        }

        let (kind, base_msg) = match self.outcome {
            RetrievalAttemptOutcome::Failed => {
                let msg = match &self.error_class {
                    Some(cls) => format!("[{}] provider {} failed", cls, self.provider_id),
                    None => format!("provider {} failed", self.provider_id),
                };
                (RetrievalFailureKind::ProviderFailed, msg)
            }
            RetrievalAttemptOutcome::TimedOut => {
                let msg = match &self.error_class {
                    Some(cls) => format!("[{}] provider {} timed out", cls, self.provider_id),
                    None => format!("provider {} timed out", self.provider_id),
                };
                (RetrievalFailureKind::DeadlinePreventedCompletion, msg)
            }
            RetrievalAttemptOutcome::RateLimited => {
                let msg = match &self.error_class {
                    Some(cls) => {
                        format!("[{}] provider {} rate limited", cls, self.provider_id)
                    }
                    None => format!("provider {} rate limited", self.provider_id),
                };
                (RetrievalFailureKind::ProviderFailed, msg)
            }
            RetrievalAttemptOutcome::InterruptedByDeadline => {
                let msg = format!(
                    "provider {} interrupted by global deadline",
                    self.provider_id
                );
                (RetrievalFailureKind::DeadlinePreventedCompletion, msg)
            }
            RetrievalAttemptOutcome::SkippedByPolicy => (
                RetrievalFailureKind::ProviderSkippedByPolicy,
                format!("provider {} skipped by policy", self.provider_id),
            ),
            RetrievalAttemptOutcome::SkippedCapabilityUnavailable => (
                RetrievalFailureKind::ProviderCapabilityUnavailable,
                format!(
                    "provider {} lacks the requested capability",
                    self.provider_id
                ),
            ),
            _ => unreachable!(),
        };

        let roles: Vec<EvidenceRole> = if self.intended_roles.is_empty() {
            vec![EvidenceRole::UnknownOrWeakContext]
        } else {
            let mut seen = std::collections::HashSet::new();
            self.intended_roles
                .iter()
                .copied()
                .filter(|r| seen.insert(*r))
                .collect()
        };

        let provider_id = Some(self.provider_id.clone());
        roles
            .into_iter()
            .map(|role| RetrievalFailure {
                kind,
                role,
                message: base_msg.clone(),
                provider_id: provider_id.clone(),
            })
            .collect()
    }
}

/// Convert a slice of `RetrievalAttempt` records into `RetrievalFailure` records
/// for failure/timed-out/rate-limited attempts.
pub fn attempts_to_failures(attempts: &[RetrievalAttempt]) -> Vec<RetrievalFailure> {
    attempts
        .iter()
        .flat_map(RetrievalAttempt::to_retrieval_failures)
        .collect()
}

/// Map a provider ID and subquery label to intended evidence roles.
///
/// The mapping is deterministic and based on provider capabilities and
/// the subquery's purpose within the search plan.
pub fn map_provider_to_intended_roles(
    provider_id: &str,
    subquery_label: &str,
) -> Vec<EvidenceRole> {
    match subquery_label {
        "advisory" | "security" => {
            vec![EvidenceRole::AuthoritativeSecurityAdvisory]
        }
        "vendor" => {
            vec![EvidenceRole::VendorSecurityGuidance]
        }
        "defensive" => {
            vec![EvidenceRole::ConfigurationOrFeatureGate]
        }
        "source" | "code" => {
            vec![EvidenceRole::PrimaryImplementation]
        }
        "issues" => {
            vec![EvidenceRole::IssueOrIncidentDiscussion]
        }
        "releases" => {
            vec![EvidenceRole::ReleaseNoteOrChangelog]
        }
        "docs" | "documentation" => {
            vec![EvidenceRole::OfficialDocumentation]
        }
        "examples" => {
            vec![EvidenceRole::UsageExample]
        }
        "registry" | "packages" => {
            vec![EvidenceRole::ManifestOrDependencyMetadata]
        }
        "benchmarks" => {
            vec![EvidenceRole::BenchmarkOrPerformanceEvidence]
        }
        "research" | "academic" => {
            vec![EvidenceRole::IndependentCorroboration]
        }
        "exact_phrase" | "error_exact" => {
            vec![EvidenceRole::PrimaryImplementation]
        }
        "error_code" => {
            vec![EvidenceRole::IssueOrIncidentDiscussion]
        }
        "error_package" => {
            vec![EvidenceRole::ManifestOrDependencyMetadata]
        }
        "error_issues" => {
            vec![EvidenceRole::IssueOrIncidentDiscussion]
        }
        "error_releases" => {
            vec![EvidenceRole::ReleaseNoteOrChangelog]
        }
        "error_docs" => {
            vec![EvidenceRole::OfficialDocumentation]
        }
        _ => {
            // Provider-specific fallback for generic subqueries
            if provider_id == "osv" || provider_id.contains("advisory") {
                vec![EvidenceRole::AuthoritativeSecurityAdvisory]
            } else if provider_id.contains("code") || provider_id.contains("repo") {
                vec![EvidenceRole::PrimaryImplementation]
            } else {
                vec![EvidenceRole::UnknownOrWeakContext]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(
        evidence_role: EvidenceRole,
        absence_kind: EvidenceAbsenceKind,
        message: &str,
    ) -> RetrievalDimensionStatus {
        RetrievalDimensionStatus {
            evidence_role,
            absence_kind,
            message: message.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn all_variants_serialize_deserialize() {
        let variants = [
            EvidenceAbsenceKind::NoMatchingEvidenceFound,
            EvidenceAbsenceKind::ProviderCapabilityUnavailable,
            EvidenceAbsenceKind::ProviderSkippedByPolicy,
            EvidenceAbsenceKind::ProviderFailed,
            EvidenceAbsenceKind::DeadlinePreventedCompletion,
            EvidenceAbsenceKind::ResultTruncatedByCap,
            EvidenceAbsenceKind::EvidenceRoleNotRequested,
            EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
            EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed,
            EvidenceAbsenceKind::NotApplicable,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let parsed: EvidenceAbsenceKind = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, v, "roundtrip failed for {v:?}");
        }
    }

    #[test]
    fn default_is_not_applicable() {
        assert_eq!(
            EvidenceAbsenceKind::default(),
            EvidenceAbsenceKind::NotApplicable
        );
    }

    #[test]
    fn is_absence_only_true_when_only_absences() {
        let summary = summarize_retrieval(vec![
            dim(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NoMatchingEvidenceFound,
                "none",
            ),
            dim(
                EvidenceRole::OfficialDocumentation,
                EvidenceAbsenceKind::EvidenceRoleNotRequested,
                "not requested",
            ),
        ]);
        assert!(is_absence_only(&summary));
    }

    #[test]
    fn is_absence_only_false_when_failures_present() {
        let summary = summarize_retrieval(vec![
            dim(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NoMatchingEvidenceFound,
                "none",
            ),
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed".into(),
                ..Default::default()
            },
        ]);
        assert!(!is_absence_only(&summary));
    }

    #[test]
    fn is_failure_only_true_when_all_applicable_are_failures() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed".into(),
                ..Default::default()
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::DeadlinePreventedCompletion,
                provider_id: Some("startpage".into()),
                message: "timeout".into(),
                ..Default::default()
            },
        ]);
        assert!(is_failure_only(&summary));
    }

    #[test]
    fn is_failure_only_false_when_mixed_success_and_failure() {
        let summary = summarize_retrieval(vec![
            dim(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NoMatchingEvidenceFound,
                "none",
            ),
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed".into(),
                ..Default::default()
            },
        ]);
        assert!(!is_failure_only(&summary));
    }

    #[test]
    fn is_failure_only_false_when_no_failures() {
        let summary = summarize_retrieval(vec![dim(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::NoMatchingEvidenceFound,
            "none",
        )]);
        assert!(!is_failure_only(&summary));
    }

    #[test]
    fn is_failure_only_ignores_skipped_dimensions() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed".into(),
                ..Default::default()
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::ProviderSkippedByPolicy,
                provider_id: Some("startpage".into()),
                message: "skipped".into(),
                ..Default::default()
            },
        ]);
        assert!(is_failure_only(&summary));
    }

    #[test]
    fn has_indeterminate_works() {
        let summary = summarize_retrieval(vec![dim(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed,
            "indeterminate",
        )]);
        assert!(has_indeterminate(&summary));

        let summary2 = summarize_retrieval(vec![dim(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::NoMatchingEvidenceFound,
            "none",
        )]);
        assert!(!has_indeterminate(&summary2));
    }

    #[test]
    fn absent_roles_returns_only_requested_but_not_found() {
        let summary = summarize_retrieval(vec![
            dim(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
                "not found",
            ),
            dim(
                EvidenceRole::OfficialDocumentation,
                EvidenceAbsenceKind::NoMatchingEvidenceFound,
                "none",
            ),
            dim(
                EvidenceRole::UsageExample,
                EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
                "not found",
            ),
        ]);
        let roles = absent_roles(&summary);
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&EvidenceRole::PrimaryImplementation));
        assert!(roles.contains(&EvidenceRole::UsageExample));
        assert!(!roles.contains(&EvidenceRole::OfficialDocumentation));
    }

    #[test]
    fn failed_providers_returns_unique_ids() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed".into(),
                ..Default::default()
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: Some("duckduckgo".into()),
                message: "failed again".into(),
                ..Default::default()
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::UsageExample,
                absence_kind: EvidenceAbsenceKind::DeadlinePreventedCompletion,
                provider_id: Some("startpage".into()),
                message: "timeout".into(),
                ..Default::default()
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::BenchmarkOrPerformanceEvidence,
                absence_kind: EvidenceAbsenceKind::ProviderFailed,
                provider_id: None,
                message: "no provider".into(),
                ..Default::default()
            },
        ]);
        let providers = failed_providers(&summary);
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"duckduckgo".to_string()));
        assert!(providers.contains(&"startpage".to_string()));
    }

    #[test]
    fn classify_absence_returns_correct_labels() {
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::NoMatchingEvidenceFound),
            "no_matching_evidence_found"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::ProviderCapabilityUnavailable),
            "provider_capability_unavailable"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::ProviderSkippedByPolicy),
            "provider_skipped_by_policy"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::ProviderFailed),
            "provider_failed"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::DeadlinePreventedCompletion),
            "deadline_prevented_completion"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::ResultTruncatedByCap),
            "result_truncated_by_cap"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::EvidenceRoleNotRequested),
            "evidence_role_not_requested"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound),
            "evidence_role_requested_but_not_found"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed),
            "evidence_role_indeterminate_because_retrieval_failed"
        );
        assert_eq!(
            classify_absence(EvidenceAbsenceKind::NotApplicable),
            "not_applicable"
        );
    }

    #[test]
    fn serde_roundtrip_retrieval_dimension_status() {
        let status = RetrievalDimensionStatus {
            evidence_role: EvidenceRole::PrimaryImplementation,
            absence_kind: EvidenceAbsenceKind::ProviderFailed,
            provider_id: Some("duckduckgo".into()),
            message: "connection refused".into(),
            query: Some("rust async runtime".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: RetrievalDimensionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.evidence_role, EvidenceRole::PrimaryImplementation);
        assert_eq!(parsed.absence_kind, EvidenceAbsenceKind::ProviderFailed);
        assert_eq!(parsed.provider_id.as_deref(), Some("duckduckgo"));
        assert_eq!(parsed.message, "connection refused");
        assert_eq!(parsed.query.as_deref(), Some("rust async runtime"));
    }

    #[test]
    fn serde_roundtrip_response_retrieval_summary() {
        let summary = ResponseRetrievalSummary {
            dimensions: vec![
                RetrievalDimensionStatus {
                    evidence_role: EvidenceRole::PrimaryImplementation,
                    absence_kind: EvidenceAbsenceKind::ProviderFailed,
                    provider_id: Some("duckduckgo".into()),
                    message: "failed".into(),
                    ..Default::default()
                },
                RetrievalDimensionStatus {
                    evidence_role: EvidenceRole::OfficialDocumentation,
                    absence_kind: EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
                    provider_id: None,
                    message: "not found".into(),
                    ..Default::default()
                },
            ],
            has_failures: true,
            has_absences: true,
            has_truncation: false,
            attempted_job_count: Some(2),
            completed_job_count: Some(0),
            failed_job_count: Some(1),
            ..Default::default()
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ResponseRetrievalSummary = serde_json::from_str(&json).unwrap();
        assert!(parsed.has_failures);
        assert!(parsed.has_absences);
        assert!(!parsed.has_truncation);
        assert_eq!(parsed.dimensions.len(), 2);
    }

    #[test]
    fn serde_deserializes_snake_case_enum() {
        let kind: EvidenceAbsenceKind =
            serde_json::from_str("\"evidence_role_requested_but_not_found\"").unwrap();
        assert_eq!(kind, EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound);

        let kind: EvidenceAbsenceKind =
            serde_json::from_str("\"deadline_prevented_completion\"").unwrap();
        assert_eq!(kind, EvidenceAbsenceKind::DeadlinePreventedCompletion);
    }

    #[test]
    fn summary_default_has_empty_dimensions() {
        let summary = ResponseRetrievalSummary::default();
        assert!(summary.dimensions.is_empty());
        assert!(!summary.has_failures);
        assert!(!summary.has_absences);
        assert!(!summary.has_truncation);
    }

    #[test]
    fn summarize_retrieval_populates_flags_correctly() {
        let summary = summarize_retrieval(vec![
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::PrimaryImplementation,
                absence_kind: EvidenceAbsenceKind::ResultTruncatedByCap,
                provider_id: None,
                message: "truncated".into(),
                ..Default::default()
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::OfficialDocumentation,
                absence_kind: EvidenceAbsenceKind::DeadlinePreventedCompletion,
                provider_id: Some("startpage".into()),
                message: "timeout".into(),
                ..Default::default()
            },
            RetrievalDimensionStatus {
                evidence_role: EvidenceRole::UsageExample,
                absence_kind: EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound,
                provider_id: None,
                message: "missing".into(),
                ..Default::default()
            },
        ]);
        assert!(summary.has_failures);
        assert!(summary.has_absences);
        assert!(summary.has_truncation);
    }

    fn dim_with_state(
        evidence_role: EvidenceRole,
        absence_kind: EvidenceAbsenceKind,
        state: RetrievalDimensionState,
        message: &str,
    ) -> RetrievalDimensionStatus {
        RetrievalDimensionStatus {
            evidence_role,
            absence_kind,
            state: Some(state),
            message: message.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn gate_a_success_not_counted_as_non_applicable() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::NotApplicable,
            RetrievalDimensionState::Satisfied,
            "success",
        )]);
        assert_eq!(summary.not_applicable_count, Some(0));
        assert_eq!(summary.completed_dimension_count, Some(1));
    }

    #[test]
    fn gate_a_one_success_one_na_gives_correct_counts() {
        let summary = summarize_retrieval(vec![
            dim_with_state(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::Satisfied,
                "success",
            ),
            dim_with_state(
                EvidenceRole::OfficialDocumentation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::NotApplicable,
                "na",
            ),
        ]);
        assert_eq!(summary.not_applicable_count, Some(1));
        assert_eq!(summary.completed_dimension_count, Some(2));
    }

    #[test]
    fn gate_a_na_only_role_not_complete() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::NotApplicable,
            RetrievalDimensionState::NotApplicable,
            "na",
        )]);
        assert!(!roles_with_success_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert!(
            !roles_attempted_from_summary(&summary).contains(&EvidenceRole::PrimaryImplementation)
        );
    }

    #[test]
    fn gate_a_satisfied_role_is_complete() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::NotApplicable,
            RetrievalDimensionState::Satisfied,
            "found",
        )]);
        assert!(roles_with_success_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
    }

    #[test]
    fn gate_a_zero_result_role_is_complete_but_absent() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::NoMatchingEvidenceFound,
            RetrievalDimensionState::CompletedNoMatch,
            "none",
        )]);
        assert!(roles_with_success_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert_eq!(summary.completed_dimension_count, Some(1));
    }

    #[test]
    fn gate_a_partial_role_is_indeterminate() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::ResultTruncatedByCap,
            RetrievalDimensionState::Partial,
            "partial",
        )]);
        assert!(roles_indeterminate_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert!(!roles_with_success_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
    }

    #[test]
    fn gate_a_capability_skip_is_indeterminate_not_failed() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::ProviderCapabilityUnavailable,
            RetrievalDimensionState::CapabilityUnavailable,
            "unavailable",
        )]);
        assert!(roles_indeterminate_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert!(!roles_with_success_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert_eq!(summary.failed_dimension_count, Some(0));
    }

    #[test]
    fn gate_a_policy_skip_is_indeterminate_not_failed() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::ProviderSkippedByPolicy,
            RetrievalDimensionState::SkippedByPolicy,
            "skipped",
        )]);
        assert!(roles_indeterminate_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert_eq!(summary.failed_dimension_count, Some(0));
    }

    #[test]
    fn gate_a_failed_dimension_is_failed_and_indeterminate() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::ProviderFailed,
            RetrievalDimensionState::Failed,
            "error",
        )]);
        assert!(roles_indeterminate_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert_eq!(summary.failed_dimension_count, Some(1));
        assert!(summary.has_failures);
    }

    #[test]
    fn gate_a_interrupted_dimension_is_failed_and_indeterminate() {
        let summary = summarize_retrieval(vec![dim_with_state(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::DeadlinePreventedCompletion,
            RetrievalDimensionState::Interrupted,
            "deadline",
        )]);
        assert!(roles_indeterminate_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert_eq!(summary.failed_dimension_count, Some(1));
    }

    #[test]
    fn gate_a_mixed_satisfied_and_failed_preserve_both() {
        let summary = summarize_retrieval(vec![
            dim_with_state(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::Satisfied,
                "ok",
            ),
            dim_with_state(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::ProviderFailed,
                RetrievalDimensionState::Failed,
                "error",
            ),
        ]);
        assert!(roles_with_success_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert!(roles_indeterminate_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert_eq!(summary.not_applicable_count, Some(0));
        assert_eq!(summary.failed_dimension_count, Some(1));
    }

    #[test]
    fn gate_a_legacy_dimension_without_state_compatible() {
        let summary = summarize_retrieval(vec![dim(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::NoMatchingEvidenceFound,
            "none",
        )]);
        assert!(roles_with_success_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
        assert_eq!(summary.completed_dimension_count, Some(1));
    }

    #[test]
    fn gate_a_state_overrides_contradictory_legacy() {
        let summary = summarize_retrieval(vec![RetrievalDimensionStatus {
            evidence_role: EvidenceRole::PrimaryImplementation,
            absence_kind: EvidenceAbsenceKind::NotApplicable,
            state: Some(RetrievalDimensionState::Satisfied),
            message: "success".into(),
            ..Default::default()
        }]);
        assert_eq!(summary.not_applicable_count, Some(0));
        assert!(roles_with_success_from_summary(&summary)
            .contains(&EvidenceRole::PrimaryImplementation));
    }

    #[test]
    fn gate_a_multi_role_na_attempt_gives_correct_dimension_and_job_count() {
        let summary = summarize_retrieval_with_attempts(
            &[RetrievalAttempt {
                provider_id: "duckduckgo".into(),
                subquery_id: Some("q1".into()),
                operation_id: Some("op1".into()),
                intended_roles: vec![
                    EvidenceRole::PrimaryImplementation,
                    EvidenceRole::OfficialDocumentation,
                ],
                outcome: RetrievalAttemptOutcome::NotApplicable,
                result_count: 0,
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                query_fingerprint: None,
                duration_ms: None,
            }],
            vec![
                dim_with_state(
                    EvidenceRole::PrimaryImplementation,
                    EvidenceAbsenceKind::NotApplicable,
                    RetrievalDimensionState::NotApplicable,
                    "na",
                ),
                dim_with_state(
                    EvidenceRole::OfficialDocumentation,
                    EvidenceAbsenceKind::NotApplicable,
                    RetrievalDimensionState::NotApplicable,
                    "na",
                ),
            ],
        );
        assert_eq!(summary.attempted_dimension_count, Some(2));
        assert_eq!(summary.not_applicable_count, Some(2));
        assert_eq!(summary.attempted_job_count, Some(1));
        assert_eq!(summary.not_applicable_job_count, Some(1));
    }

    #[test]
    fn gate_a_single_role_multi_attempt_attempted_count() {
        let summary = summarize_retrieval_with_attempts(
            &[
                RetrievalAttempt {
                    provider_id: "a".into(),
                    subquery_id: Some("q1".into()),
                    operation_id: Some("op1".into()),
                    intended_roles: vec![EvidenceRole::PrimaryImplementation],
                    outcome: RetrievalAttemptOutcome::SuccessWithResults,
                    result_count: 1,
                    error_class: None,
                    deadline_interrupted: false,
                    truncated: false,
                    truncation_evidence: TruncationEvidence::None,
                    query_fingerprint: None,
                    duration_ms: None,
                },
                RetrievalAttempt {
                    provider_id: "b".into(),
                    subquery_id: Some("q1".into()),
                    operation_id: Some("op2".into()),
                    intended_roles: vec![EvidenceRole::PrimaryImplementation],
                    outcome: RetrievalAttemptOutcome::Failed,
                    result_count: 0,
                    error_class: None,
                    deadline_interrupted: false,
                    truncated: false,
                    truncation_evidence: TruncationEvidence::None,
                    query_fingerprint: None,
                    duration_ms: None,
                },
            ],
            vec![
                dim_with_state(
                    EvidenceRole::PrimaryImplementation,
                    EvidenceAbsenceKind::NotApplicable,
                    RetrievalDimensionState::Satisfied,
                    "ok",
                ),
                dim_with_state(
                    EvidenceRole::PrimaryImplementation,
                    EvidenceAbsenceKind::ProviderFailed,
                    RetrievalDimensionState::Failed,
                    "error",
                ),
            ],
        );
        assert_eq!(summary.attempted_job_count, Some(2));
        assert_eq!(summary.attempted_dimension_count, Some(2));
        assert_eq!(roles_attempted_from_summary(&summary).len(), 1);
    }

    fn roles_with_success_from_summary(
        s: &ResponseRetrievalSummary,
    ) -> std::collections::HashSet<EvidenceRole> {
        let mut set = std::collections::HashSet::new();
        for d in &s.dimensions {
            let state = dimension_state_or_legacy(d);
            match state {
                EffectiveDimensionState::Satisfied | EffectiveDimensionState::CompletedNoMatch => {
                    set.insert(d.evidence_role);
                }
                _ => {}
            }
        }
        set
    }

    fn roles_indeterminate_from_summary(
        s: &ResponseRetrievalSummary,
    ) -> std::collections::HashSet<EvidenceRole> {
        let mut set = std::collections::HashSet::new();
        for d in &s.dimensions {
            let state = dimension_state_or_legacy(d);
            match state {
                EffectiveDimensionState::Failed
                | EffectiveDimensionState::SkippedByPolicy
                | EffectiveDimensionState::CapabilityUnavailable
                | EffectiveDimensionState::Interrupted
                | EffectiveDimensionState::Partial => {
                    set.insert(d.evidence_role);
                }
                _ => {}
            }
        }
        set
    }

    fn roles_attempted_from_summary(
        s: &ResponseRetrievalSummary,
    ) -> std::collections::HashSet<EvidenceRole> {
        let mut set = std::collections::HashSet::new();
        for d in &s.dimensions {
            let state = dimension_state_or_legacy(d);
            if state != EffectiveDimensionState::NotApplicable {
                set.insert(d.evidence_role);
            }
        }
        set
    }

    fn test_attempt_to_absence_kind(attempt: &RetrievalAttempt) -> EvidenceAbsenceKind {
        match attempt.outcome {
            RetrievalAttemptOutcome::SuccessWithResults => EvidenceAbsenceKind::NotApplicable,
            RetrievalAttemptOutcome::SuccessZeroResults => {
                EvidenceAbsenceKind::NoMatchingEvidenceFound
            }
            RetrievalAttemptOutcome::Failed => EvidenceAbsenceKind::ProviderFailed,
            RetrievalAttemptOutcome::TimedOut => EvidenceAbsenceKind::DeadlinePreventedCompletion,
            RetrievalAttemptOutcome::RateLimited => EvidenceAbsenceKind::ProviderFailed,
            RetrievalAttemptOutcome::SkippedByPolicy => {
                EvidenceAbsenceKind::ProviderSkippedByPolicy
            }
            RetrievalAttemptOutcome::SkippedCapabilityUnavailable => {
                EvidenceAbsenceKind::ProviderCapabilityUnavailable
            }
            RetrievalAttemptOutcome::NotApplicable => EvidenceAbsenceKind::NotApplicable,
            RetrievalAttemptOutcome::InterruptedByDeadline => {
                EvidenceAbsenceKind::DeadlinePreventedCompletion
            }
            RetrievalAttemptOutcome::TruncatedAfterPartialSuccess => {
                EvidenceAbsenceKind::NotApplicable
            }
        }
    }

    #[test]
    fn gate_b_dimension_only_has_no_job_counters() {
        let summary = summarize_retrieval(vec![
            dim(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NoMatchingEvidenceFound,
                "none",
            ),
            dim(
                EvidenceRole::OfficialDocumentation,
                EvidenceAbsenceKind::ProviderFailed,
                "failed",
            ),
        ]);
        assert!(summary.attempted_job_count.is_none());
        assert!(summary.completed_job_count.is_none());
        assert!(summary.failed_job_count.is_none());
        assert!(summary.zero_result_count.is_none());
        assert!(summary.timed_out_count.is_none());
        assert!(summary.rate_limited_count.is_none());
        assert!(summary.policy_skipped_count.is_none());
        assert!(summary.capability_skipped_count.is_none());
        assert!(summary.deadline_interrupted_count.is_none());
        assert!(summary.truncated_count.is_none());
        assert!(summary.limit_reached_unknown_count.is_none());
        assert!(summary.not_applicable_job_count.is_none());
    }

    #[test]
    fn gate_b_dimension_only_still_has_dimension_counters() {
        let summary = summarize_retrieval(vec![
            dim_with_state(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::Satisfied,
                "ok",
            ),
            dim_with_state(
                EvidenceRole::OfficialDocumentation,
                EvidenceAbsenceKind::ProviderFailed,
                RetrievalDimensionState::Failed,
                "error",
            ),
        ]);
        assert_eq!(summary.attempted_dimension_count, Some(2));
        assert_eq!(summary.completed_dimension_count, Some(1));
        assert_eq!(summary.failed_dimension_count, Some(1));
    }

    #[test]
    fn gate_b_attempt_derived_populates_job_counters() {
        let attempts = vec![
            RetrievalAttempt {
                provider_id: "duckduckgo".into(),
                subquery_id: Some("q1".into()),
                operation_id: Some("op1".into()),
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                outcome: RetrievalAttemptOutcome::SuccessWithResults,
                result_count: 3,
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                query_fingerprint: None,
                duration_ms: None,
            },
            RetrievalAttempt {
                provider_id: "startpage".into(),
                subquery_id: Some("q1".into()),
                operation_id: Some("op2".into()),
                intended_roles: vec![
                    EvidenceRole::PrimaryImplementation,
                    EvidenceRole::OfficialDocumentation,
                ],
                outcome: RetrievalAttemptOutcome::Failed,
                result_count: 0,
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                query_fingerprint: None,
                duration_ms: None,
            },
        ];
        let summary = summarize_retrieval_with_attempts(
            &attempts,
            vec![
                dim_with_state(
                    EvidenceRole::PrimaryImplementation,
                    EvidenceAbsenceKind::NotApplicable,
                    RetrievalDimensionState::Satisfied,
                    "ok",
                ),
                dim_with_state(
                    EvidenceRole::PrimaryImplementation,
                    EvidenceAbsenceKind::ProviderFailed,
                    RetrievalDimensionState::Failed,
                    "error",
                ),
                dim_with_state(
                    EvidenceRole::OfficialDocumentation,
                    EvidenceAbsenceKind::ProviderFailed,
                    RetrievalDimensionState::Failed,
                    "error",
                ),
            ],
        );
        assert_eq!(summary.attempted_job_count, Some(2));
        assert_eq!(summary.completed_job_count, Some(1));
        assert_eq!(summary.failed_job_count, Some(1));
        assert_eq!(summary.attempted_dimension_count, Some(3));
    }

    #[test]
    fn gate_b_attempt_partition_invariant() {
        let attempts = vec![
            RetrievalAttempt {
                provider_id: "a".into(),
                outcome: RetrievalAttemptOutcome::SuccessWithResults,
                result_count: 1,
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                subquery_id: None,
                operation_id: None,
                query_fingerprint: None,
                duration_ms: None,
            },
            RetrievalAttempt {
                provider_id: "b".into(),
                outcome: RetrievalAttemptOutcome::SkippedByPolicy,
                result_count: 0,
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                subquery_id: None,
                operation_id: None,
                query_fingerprint: None,
                duration_ms: None,
            },
            RetrievalAttempt {
                provider_id: "c".into(),
                outcome: RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
                result_count: 0,
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                subquery_id: None,
                operation_id: None,
                query_fingerprint: None,
                duration_ms: None,
            },
            RetrievalAttempt {
                provider_id: "d".into(),
                outcome: RetrievalAttemptOutcome::Failed,
                result_count: 0,
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                subquery_id: None,
                operation_id: None,
                query_fingerprint: None,
                duration_ms: None,
            },
        ];
        let dims: Vec<_> = attempts
            .iter()
            .flat_map(|a| {
                let state = crate::core::retrieval_status::attempt_outcome_to_dimension_state(a);
                let absence = test_attempt_to_absence_kind(a);
                vec![RetrievalDimensionStatus {
                    evidence_role: EvidenceRole::PrimaryImplementation,
                    absence_kind: absence,
                    provider_id: Some(a.provider_id.clone()),
                    state: Some(state),
                    ..Default::default()
                }]
            })
            .collect();
        let summary = summarize_retrieval_with_attempts(&attempts, dims);
        let attempted = summary.attempted_job_count.unwrap();
        let completed = summary.completed_job_count.unwrap();
        let failed = summary.failed_job_count.unwrap();
        let policy_skipped = summary.policy_skipped_count.unwrap();
        let cap_skipped = summary.capability_skipped_count.unwrap();
        assert_eq!(attempted, completed + failed + policy_skipped + cap_skipped);
    }

    #[test]
    fn gate_b_serde_omits_none_job_counters() {
        let summary = summarize_retrieval(vec![dim(
            EvidenceRole::PrimaryImplementation,
            EvidenceAbsenceKind::NoMatchingEvidenceFound,
            "none",
        )]);
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("attempted_job_count"));
        assert!(!json.contains("completed_job_count"));
        assert!(!json.contains("failed_job_count"));
        assert!(json.contains("attempted_dimension_count"));
    }

    #[test]
    fn gate_b_one_multi_role_attempt_produces_one_job_and_multiple_dims() {
        let attempts = vec![RetrievalAttempt {
            provider_id: "duckduckgo".into(),
            subquery_id: Some("q1".into()),
            operation_id: Some("op1".into()),
            intended_roles: vec![
                EvidenceRole::PrimaryImplementation,
                EvidenceRole::OfficialDocumentation,
            ],
            outcome: RetrievalAttemptOutcome::SuccessWithResults,
            result_count: 3,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: TruncationEvidence::None,
            query_fingerprint: None,
            duration_ms: None,
        }];
        let dims = vec![
            dim_with_state(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::Satisfied,
                "ok",
            ),
            dim_with_state(
                EvidenceRole::OfficialDocumentation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::Satisfied,
                "ok",
            ),
        ];
        let summary = summarize_retrieval_with_attempts(&attempts, dims);
        assert_eq!(summary.attempted_job_count, Some(1));
        assert_eq!(summary.attempted_dimension_count, Some(2));
    }

    #[test]
    fn gate_b_two_attempts_shared_role_produce_two_jobs_and_two_dims() {
        let attempts = vec![
            RetrievalAttempt {
                provider_id: "a".into(),
                subquery_id: Some("q1".into()),
                operation_id: Some("op1".into()),
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                outcome: RetrievalAttemptOutcome::SuccessWithResults,
                result_count: 1,
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                query_fingerprint: None,
                duration_ms: None,
            },
            RetrievalAttempt {
                provider_id: "b".into(),
                subquery_id: Some("q1".into()),
                operation_id: Some("op2".into()),
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                outcome: RetrievalAttemptOutcome::SuccessWithResults,
                result_count: 1,
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: TruncationEvidence::None,
                query_fingerprint: None,
                duration_ms: None,
            },
        ];
        let dims = vec![
            dim_with_state(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::Satisfied,
                "ok",
            ),
            dim_with_state(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::Satisfied,
                "ok",
            ),
        ];
        let summary = summarize_retrieval_with_attempts(&attempts, dims);
        assert_eq!(summary.attempted_job_count, Some(2));
        assert_eq!(summary.attempted_dimension_count, Some(2));
    }

    #[test]
    fn gate_b_nonapplicable_multi_role_attempt_gives_correct_counts() {
        let attempts = vec![RetrievalAttempt {
            provider_id: "github_code".into(),
            subquery_id: Some("q1".into()),
            operation_id: Some("op1".into()),
            intended_roles: vec![
                EvidenceRole::PrimaryImplementation,
                EvidenceRole::OfficialDocumentation,
            ],
            outcome: RetrievalAttemptOutcome::NotApplicable,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: TruncationEvidence::None,
            query_fingerprint: None,
            duration_ms: None,
        }];
        let dims = vec![
            dim_with_state(
                EvidenceRole::PrimaryImplementation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::NotApplicable,
                "na",
            ),
            dim_with_state(
                EvidenceRole::OfficialDocumentation,
                EvidenceAbsenceKind::NotApplicable,
                RetrievalDimensionState::NotApplicable,
                "na",
            ),
        ];
        let summary = summarize_retrieval_with_attempts(&attempts, dims);
        assert_eq!(summary.not_applicable_job_count, Some(1));
        assert_eq!(summary.not_applicable_count, Some(2));
        assert_eq!(summary.attempted_job_count, Some(1));
        assert_eq!(summary.completed_job_count, Some(1));
    }
}
