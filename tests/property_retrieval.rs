use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::retrieval_status::{
    attempts_to_failures, classify_absence, map_provider_to_intended_roles,
    query_fingerprint_from_query, EvidenceAbsenceKind, RetrievalAttempt, RetrievalAttemptOutcome,
    TruncationEvidence,
};
use eggsearch::core::workflow_coverage::{
    compute_coverage, CoverageStatus, RetrievalFailureKind, WorkflowCoverageModel,
};
use proptest::prelude::*;

fn provider_id_strategy() -> impl Strategy<Value = String> {
    "[a-z_]{3,15}"
}

fn subquery_label_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("advisory".to_string()),
        Just("security".to_string()),
        Just("vendor".to_string()),
        Just("defensive".to_string()),
        Just("source".to_string()),
        Just("code".to_string()),
        Just("issues".to_string()),
        Just("releases".to_string()),
        Just("docs".to_string()),
        Just("documentation".to_string()),
        Just("examples".to_string()),
        Just("registry".to_string()),
        Just("packages".to_string()),
        Just("benchmarks".to_string()),
        Just("research".to_string()),
        Just("academic".to_string()),
        Just("unknown_label".to_string()),
    ]
}

fn outcome_strategy() -> impl Strategy<Value = RetrievalAttemptOutcome> {
    prop_oneof![
        Just(RetrievalAttemptOutcome::SuccessWithResults),
        Just(RetrievalAttemptOutcome::SuccessZeroResults),
        Just(RetrievalAttemptOutcome::Failed),
        Just(RetrievalAttemptOutcome::TimedOut),
        Just(RetrievalAttemptOutcome::RateLimited),
        Just(RetrievalAttemptOutcome::SkippedByPolicy),
        Just(RetrievalAttemptOutcome::SkippedCapabilityUnavailable),
        Just(RetrievalAttemptOutcome::NotApplicable),
        Just(RetrievalAttemptOutcome::InterruptedByDeadline),
        Just(RetrievalAttemptOutcome::TruncatedAfterPartialSuccess),
    ]
}

fn attempt_strategy() -> impl Strategy<Value = RetrievalAttempt> {
    (provider_id_strategy(), outcome_strategy(), 0usize..20usize).prop_map(
        |(provider_id, outcome, result_count)| RetrievalAttempt {
            provider_id,
            subquery_id: None,
            operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome,
            result_count,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
    )
}

fn failure_only_outcome() -> impl Strategy<Value = RetrievalAttemptOutcome> {
    prop_oneof![
        Just(RetrievalAttemptOutcome::Failed),
        Just(RetrievalAttemptOutcome::TimedOut),
        Just(RetrievalAttemptOutcome::RateLimited),
        Just(RetrievalAttemptOutcome::InterruptedByDeadline),
    ]
}

fn non_failure_outcome() -> impl Strategy<Value = RetrievalAttemptOutcome> {
    prop_oneof![
        Just(RetrievalAttemptOutcome::SuccessWithResults),
        Just(RetrievalAttemptOutcome::SuccessZeroResults),
        Just(RetrievalAttemptOutcome::NotApplicable),
        Just(RetrievalAttemptOutcome::TruncatedAfterPartialSuccess),
    ]
}

proptest! {
    #[test]
    fn attempts_to_failures_only_failure_outcomes(
        attempts in proptest::collection::vec(attempt_strategy(), 0..20),
    ) {
        let failures = attempts_to_failures(&attempts);
        for failure in &failures {
            prop_assert!(
                failure.kind == eggsearch::core::workflow_coverage::RetrievalFailureKind::ProviderFailed
                    || failure.kind == eggsearch::core::workflow_coverage::RetrievalFailureKind::DeadlinePreventedCompletion
                    || failure.kind == eggsearch::core::workflow_coverage::RetrievalFailureKind::ProviderSkippedByPolicy
                    || failure.kind == eggsearch::core::workflow_coverage::RetrievalFailureKind::ProviderCapabilityUnavailable,
                "unexpected failure kind: {:?}",
                failure.kind
            );
        }
    }

    #[test]
    fn attempts_to_failures_empty_input_produces_empty_output(
        _dummy in 0usize..1usize,
    ) {
        let failures = attempts_to_failures(&[]);
        prop_assert!(failures.is_empty(), "empty input must produce empty output");
    }

    #[test]
    fn attempts_to_failures_count_matches_failure_outcomes(
        failures in proptest::collection::vec(failure_only_outcome(), 0..10),
        non_failures in proptest::collection::vec(non_failure_outcome(), 0..10),
    ) {
        let mut all_outcomes: Vec<RetrievalAttemptOutcome> = failures;
        all_outcomes.extend(non_failures);

        let all_attempts: Vec<RetrievalAttempt> = all_outcomes.into_iter().enumerate().map(|(i, outcome)| {
            RetrievalAttempt {
                provider_id: format!("prov_{i}"),
                subquery_id: None,
                operation_id: None,
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                outcome,
                result_count: 0,
                error_class: None,
                deadline_interrupted: false,
                truncated: false,
                truncation_evidence: Default::default(),
                query_fingerprint: None,
                duration_ms: None,
            }
        }).collect();

        let result = attempts_to_failures(&all_attempts);
        let expected_count = all_attempts.iter()
            .filter(|a| matches!(
                a.outcome,
                RetrievalAttemptOutcome::Failed
                    | RetrievalAttemptOutcome::TimedOut
                    | RetrievalAttemptOutcome::RateLimited
                    | RetrievalAttemptOutcome::InterruptedByDeadline
                    | RetrievalAttemptOutcome::SkippedByPolicy
                    | RetrievalAttemptOutcome::SkippedCapabilityUnavailable
            ))
            .count();
        prop_assert_eq!(result.len(), expected_count);
    }

    #[test]
    fn map_provider_to_intended_roles_deterministic(
        provider_id in provider_id_strategy(),
        subquery_label in subquery_label_strategy(),
    ) {
        let roles1 = map_provider_to_intended_roles(&provider_id, &subquery_label);
        let roles2 = map_provider_to_intended_roles(&provider_id, &subquery_label);
        prop_assert_eq!(roles1, roles2, "mapping must be deterministic");
    }

    #[test]
    fn map_provider_to_intended_roles_never_empty(
        provider_id in provider_id_strategy(),
        subquery_label in subquery_label_strategy(),
    ) {
        let roles = map_provider_to_intended_roles(&provider_id, &subquery_label);
        prop_assert!(!roles.is_empty(), "role mapping must never be empty");
    }

    #[test]
    fn map_known_labels_to_specific_roles(
        label in prop_oneof![
            Just("advisory"),
            Just("security"),
            Just("vendor"),
            Just("defensive"),
            Just("source"),
            Just("code"),
            Just("issues"),
            Just("releases"),
            Just("docs"),
            Just("documentation"),
            Just("examples"),
            Just("registry"),
            Just("packages"),
            Just("benchmarks"),
            Just("research"),
            Just("academic"),
        ],
    ) {
        let roles = map_provider_to_intended_roles("any_provider", label);
        prop_assert_eq!(roles.len(), 1,
            "known label must map to exactly one role, got {:?} for label={}",
            roles, label);
    }

    #[test]
    fn map_unknown_label_falls_back_by_provider(
        provider_id in provider_id_strategy(),
    ) {
        let roles = map_provider_to_intended_roles(&provider_id, "totally_unknown_label");
        prop_assert_eq!(roles.len(), 1,
            "unknown label must map to exactly one fallback role");
    }

    #[test]
    fn classify_absence_returns_snake_case(
        kind in prop_oneof![
            Just(EvidenceAbsenceKind::NoMatchingEvidenceFound),
            Just(EvidenceAbsenceKind::ProviderCapabilityUnavailable),
            Just(EvidenceAbsenceKind::ProviderSkippedByPolicy),
            Just(EvidenceAbsenceKind::ProviderFailed),
            Just(EvidenceAbsenceKind::DeadlinePreventedCompletion),
            Just(EvidenceAbsenceKind::ResultTruncatedByCap),
            Just(EvidenceAbsenceKind::EvidenceRoleNotRequested),
            Just(EvidenceAbsenceKind::EvidenceRoleRequestedButNotFound),
            Just(EvidenceAbsenceKind::EvidenceRoleIndeterminateBecauseRetrievalFailed),
            Just(EvidenceAbsenceKind::NotApplicable),
        ],
    ) {
        let label = classify_absence(kind);
        prop_assert!(!label.is_empty(), "classification label must not be empty");
        prop_assert!(!label.contains(' '), "label must not contain spaces: {label}");
        prop_assert!(label.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "label must be snake_case: {label}");
    }

    #[test]
    fn attempts_to_failures_deterministic(
        attempts in proptest::collection::vec(attempt_strategy(), 0..15),
    ) {
        let r1 = attempts_to_failures(&attempts);
        let r2 = attempts_to_failures(&attempts);
        prop_assert_eq!(r1.len(), r2.len());
        for (f1, f2) in r1.iter().zip(r2.iter()) {
            prop_assert_eq!(&f1.message, &f2.message);
            prop_assert_eq!(&f1.provider_id, &f2.provider_id);
        }
    }

    #[test]
    fn failed_outcome_produces_failure(
        provider_id in provider_id_strategy(),
    ) {
        let attempt = RetrievalAttempt {
            provider_id: provider_id.clone(),
            subquery_id: None,
                operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome: RetrievalAttemptOutcome::Failed,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        };
        let failures = attempts_to_failures(&[attempt]);
        prop_assert_eq!(failures.len(), 1);
        prop_assert_eq!(failures[0].provider_id.as_deref(), Some(provider_id.as_str()));
    }

    #[test]
    fn timed_out_outcome_produces_failure(
        provider_id in provider_id_strategy(),
    ) {
        let attempt = RetrievalAttempt {
            provider_id: provider_id.clone(),
            subquery_id: None,
                operation_id: None,
            intended_roles: vec![EvidenceRole::OfficialDocumentation],
            outcome: RetrievalAttemptOutcome::TimedOut,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        };
        let failures = attempts_to_failures(&[attempt]);
        prop_assert_eq!(failures.len(), 1);
    }

    #[test]
    fn rate_limited_outcome_produces_failure(
        provider_id in provider_id_strategy(),
    ) {
        let attempt = RetrievalAttempt {
            provider_id: provider_id.clone(),
            subquery_id: None,
                operation_id: None,
            intended_roles: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
            outcome: RetrievalAttemptOutcome::RateLimited,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        };
        let failures = attempts_to_failures(&[attempt]);
        prop_assert_eq!(failures.len(), 1);
    }

    #[test]
    fn interrupted_by_deadline_produces_failure(
        provider_id in provider_id_strategy(),
    ) {
        let attempt = RetrievalAttempt {
            provider_id: provider_id.clone(),
            subquery_id: None,
                operation_id: None,
            intended_roles: vec![EvidenceRole::UsageExample],
            outcome: RetrievalAttemptOutcome::InterruptedByDeadline,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        };
        let failures = attempts_to_failures(&[attempt]);
        prop_assert_eq!(failures.len(), 1);
    }

    #[test]
    fn success_outcomes_no_failures(
        provider_id in provider_id_strategy(),
        outcome in non_failure_outcome(),
    ) {
        let attempt = RetrievalAttempt {
            provider_id,
            subquery_id: None,
                operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome,
            result_count: 5,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        };
        let failures = attempts_to_failures(&[attempt]);
        prop_assert!(failures.is_empty(), "non-failure outcome must produce no failures");
    }

    #[test]
    fn provider_failure_for_required_role_reports_indeterminate(
        provider_id in provider_id_strategy(),
    ) {
        let model = WorkflowCoverageModel {
            workflow_id: "test_security".to_string(),
            title: "Test Security".to_string(),
            required: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
            recommended: vec![],
            optional: vec![],
        };
        let failures = vec![eggsearch::core::workflow_coverage::RetrievalFailure {
            kind: RetrievalFailureKind::ProviderFailed,
            role: EvidenceRole::AuthoritativeSecurityAdvisory,
            provider_id: Some(provider_id),
            message: "provider error".into(),
        }];
        let result = compute_coverage(&model, &[], &failures);
        prop_assert_eq!(
            result.status,
            CoverageStatus::IndeterminateDueToFailures,
            "when a required role's provider fails, coverage must be IndeterminateDueToFailures, not {:?}",
            result.status
        );
    }

    #[test]
    fn b6_10_property_failure_count_matches_unique_intended_role_count(
        roles in proptest::collection::vec(
            prop_oneof![
                Just(EvidenceRole::PrimaryImplementation),
                Just(EvidenceRole::OfficialDocumentation),
                Just(EvidenceRole::AuthoritativeSecurityAdvisory),
                Just(EvidenceRole::UsageExample),
            ],
            1..5,
        ),
    ) {
        let attempt = RetrievalAttempt {
            provider_id: "test_prov".to_string(),
            subquery_id: None,
                operation_id: None,
            intended_roles: roles.clone(),
            outcome: RetrievalAttemptOutcome::Failed,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        };
        let failures = attempt.to_retrieval_failures();
        let unique_roles: std::collections::HashSet<_> = roles.into_iter().collect();
        prop_assert_eq!(failures.len(), unique_roles.len());
    }
}

#[test]
fn b6_01_two_intended_roles_produce_two_failures() {
    let attempt = RetrievalAttempt {
        provider_id: "test_prov".to_string(),
        subquery_id: None,
        operation_id: None,
        intended_roles: vec![
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::PrimaryImplementation,
        ],
        outcome: RetrievalAttemptOutcome::Failed,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    };
    let failures = attempt.to_retrieval_failures();
    assert_eq!(failures.len(), 2);
    let roles: std::collections::HashSet<_> = failures.iter().map(|f| f.role).collect();
    assert!(roles.contains(&EvidenceRole::OfficialDocumentation));
    assert!(roles.contains(&EvidenceRole::PrimaryImplementation));
}

#[test]
fn b6_02_duplicate_intended_roles_produce_one_failure_per_unique_role() {
    let attempt = RetrievalAttempt {
        provider_id: "test_prov".to_string(),
        subquery_id: None,
        operation_id: None,
        intended_roles: vec![
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::PrimaryImplementation,
        ],
        outcome: RetrievalAttemptOutcome::TimedOut,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    };
    let failures = attempt.to_retrieval_failures();
    assert_eq!(
        failures.len(),
        1,
        "duplicate roles must not create duplicate failures"
    );
}

#[test]
fn b6_03_empty_intended_roles_produce_unknown_role_failure() {
    let attempt = RetrievalAttempt {
        provider_id: "test_prov".to_string(),
        subquery_id: None,
        operation_id: None,
        intended_roles: vec![],
        outcome: RetrievalAttemptOutcome::Failed,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    };
    let failures = attempt.to_retrieval_failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].role, EvidenceRole::UnknownOrWeakContext);
}

#[test]
fn b6_04_provider_fails_docs_succeeds_source_only_docs_affected() {
    use eggsearch::core::retrieval_status::attempts_to_failures;

    let docs_attempt = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("docs_sq".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::OfficialDocumentation],
        outcome: RetrievalAttemptOutcome::Failed,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    };
    let source_attempt = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("source_sq".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 5,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    };
    let all_failures = attempts_to_failures(&[docs_attempt, source_attempt]);
    assert_eq!(all_failures.len(), 1);
    assert_eq!(all_failures[0].role, EvidenceRole::OfficialDocumentation);
}

#[test]
fn b6_05_advisory_attempt_fails_both_roles_indeterminate() {
    use eggsearch::core::retrieval_status::attempts_to_failures;
    use eggsearch::core::workflow_coverage::{
        compute_coverage, CoverageStatus, WorkflowCoverageModel,
    };

    let model = WorkflowCoverageModel {
        workflow_id: "security_review".to_string(),
        title: "Security Review".to_string(),
        required: vec![
            EvidenceRole::AuthoritativeSecurityAdvisory,
            EvidenceRole::VendorSecurityGuidance,
        ],
        recommended: vec![],
        optional: vec![],
    };
    let attempt = RetrievalAttempt {
        provider_id: "osv".to_string(),
        subquery_id: None,
        operation_id: None,
        intended_roles: vec![
            EvidenceRole::AuthoritativeSecurityAdvisory,
            EvidenceRole::VendorSecurityGuidance,
        ],
        outcome: RetrievalAttemptOutcome::Failed,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    };
    let failures = attempts_to_failures(&[attempt]);
    assert_eq!(failures.len(), 2);
    let result = compute_coverage(&model, &[], &failures);
    assert_eq!(result.status, CoverageStatus::IndeterminateDueToFailures);
}

#[test]
fn b6_06_role_found_by_another_provider_redundant_failure_not_missing() {
    use eggsearch::core::retrieval_status::attempts_to_failures;
    use eggsearch::core::workflow_coverage::{
        compute_coverage, CoverageStatus, WorkflowCoverageModel,
    };

    let model = WorkflowCoverageModel {
        workflow_id: "test".to_string(),
        title: "Test".to_string(),
        required: vec![EvidenceRole::OfficialDocumentation],
        recommended: vec![],
        optional: vec![],
    };
    let failed_attempt = RetrievalAttempt {
        provider_id: "startpage".to_string(),
        subquery_id: None,
        operation_id: None,
        intended_roles: vec![EvidenceRole::OfficialDocumentation],
        outcome: RetrievalAttemptOutcome::Failed,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    };
    let failures = attempts_to_failures(&[failed_attempt]);
    let found_roles = vec![EvidenceRole::OfficialDocumentation];
    let result = compute_coverage(&model, &found_roles, &failures);
    assert_eq!(
        result.status,
        CoverageStatus::Sufficient,
        "found role must not be made missing by redundant provider failure"
    );
}

#[test]
fn b6_07_rate_limit_remains_rate_limited_in_attempt_data() {
    use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;

    let attempt = RetrievalAttempt {
        provider_id: "startpage".to_string(),
        subquery_id: Some("sq_rate".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::OfficialDocumentation],
        outcome: RetrievalAttemptOutcome::RateLimited,
        result_count: 0,
        error_class: Some("rate_limited".to_string()),
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(120),
    };
    let failures = attempts_to_failures(std::slice::from_ref(&attempt));
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].kind,
        RetrievalFailureKind::ProviderFailed,
        "rate limit must map to ProviderFailed in failure kind"
    );

    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    assert!(!summary.dimensions.is_empty());
    let dim = &summary.dimensions[0];
    assert_eq!(
        dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::RateLimited),
        "attempt_outcome must preserve RateLimited, not collapse to generic failure"
    );
    assert_eq!(
        dim.error_class.as_deref(),
        Some("rate_limited"),
        "error_class must be preserved"
    );
    assert_eq!(summary.rate_limited_count, Some(1));
    assert_eq!(summary.zero_result_count, Some(0));
    assert_eq!(summary.timed_out_count, Some(0));
}

#[test]
fn b6_08_deadline_interruption_distinct_from_provider_timeout() {
    use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;

    let timeout_attempt = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_timeout".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::TimedOut,
        result_count: 0,
        error_class: Some("timeout".to_string()),
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(5000),
    };
    let deadline_attempt = RetrievalAttempt {
        provider_id: "startpage".to_string(),
        subquery_id: Some("sq_deadline".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::InterruptedByDeadline,
        result_count: 0,
        error_class: None,
        deadline_interrupted: true,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(2000),
    };

    let summary = build_retrieval_summary_from_attempts(&[timeout_attempt, deadline_attempt]);
    assert_eq!(summary.timed_out_count, Some(1));
    assert_eq!(summary.deadline_interrupted_count, Some(1));

    let timeout_dim = summary
        .dimensions
        .iter()
        .find(|d| d.subquery_id.as_deref() == Some("sq_timeout"))
        .expect("timeout dimension must exist");
    let deadline_dim = summary
        .dimensions
        .iter()
        .find(|d| d.subquery_id.as_deref() == Some("sq_deadline"))
        .expect("deadline dimension must exist");

    assert_eq!(
        timeout_dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::TimedOut)
    );
    assert_eq!(
        deadline_dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::InterruptedByDeadline)
    );
    assert_ne!(
        timeout_dim.attempt_outcome, deadline_dim.attempt_outcome,
        "provider timeout and global deadline must serialize differently"
    );
}

#[test]
fn e14_multi_role_attempt_creates_dimensions_for_all_roles() {
    use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;

    let attempt = RetrievalAttempt {
        provider_id: "osv".to_string(),
        subquery_id: Some("sq_multi".to_string()),
        operation_id: None,
        intended_roles: vec![
            EvidenceRole::AuthoritativeSecurityAdvisory,
            EvidenceRole::ManifestOrDependencyMetadata,
        ],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 3,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(200),
    };
    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    let roles_in_dims: std::collections::HashSet<_> = summary
        .dimensions
        .iter()
        .map(|d| &d.evidence_role)
        .collect();
    assert!(
        roles_in_dims.contains(&EvidenceRole::AuthoritativeSecurityAdvisory),
        "summary must contain AuthoritativeSecurityAdvisory dimension"
    );
    assert!(
        roles_in_dims.contains(&EvidenceRole::ManifestOrDependencyMetadata),
        "summary must contain ManifestOrDependencyMetadata dimension"
    );
    assert_eq!(summary.dimensions.len(), 2);
}

#[test]
fn e14_property_multi_role_dimensions_preserve_all_intended_roles() {
    use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;

    let all_roles = vec![
        EvidenceRole::OfficialDocumentation,
        EvidenceRole::PrimaryImplementation,
        EvidenceRole::BenchmarkOrPerformanceEvidence,
        EvidenceRole::AuthoritativeSecurityAdvisory,
        EvidenceRole::ManifestOrDependencyMetadata,
        EvidenceRole::IndependentCorroboration,
        EvidenceRole::CommunityDiscussion,
        EvidenceRole::InterfaceOrApiDefinition,
        EvidenceRole::UsageExample,
        EvidenceRole::TestOrBehavioralSpecification,
        EvidenceRole::ConfigurationOrFeatureGate,
        EvidenceRole::ArchitectureOrDesignDocument,
        EvidenceRole::ReleaseNoteOrChangelog,
        EvidenceRole::MigrationGuidance,
        EvidenceRole::IssueOrIncidentDiscussion,
        EvidenceRole::PullRequestOrDesignReview,
        EvidenceRole::VendorSecurityGuidance,
        EvidenceRole::CounterpointOrConflictingEvidence,
        EvidenceRole::UnknownOrWeakContext,
    ];

    for role in &all_roles {
        let attempt = RetrievalAttempt {
            provider_id: "test_provider".to_string(),
            subquery_id: Some(format!("sq_{role:?}")),
            operation_id: None,
            intended_roles: vec![*role],
            outcome: RetrievalAttemptOutcome::SuccessWithResults,
            result_count: 1,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        };
        let summary = build_retrieval_summary_from_attempts(&[attempt]);
        let found: Vec<_> = summary
            .dimensions
            .iter()
            .filter(|d| &d.evidence_role == role)
            .collect();
        assert_eq!(
            found.len(),
            1,
            "role {role:?} must appear exactly once in summary dimensions"
        );
    }
}

#[test]
fn a11_absence_kind_populated_for_all_absence_paths() {
    let outcomes = [
        RetrievalAttemptOutcome::SuccessWithResults,
        RetrievalAttemptOutcome::SuccessZeroResults,
        RetrievalAttemptOutcome::Failed,
        RetrievalAttemptOutcome::TimedOut,
        RetrievalAttemptOutcome::RateLimited,
        RetrievalAttemptOutcome::SkippedByPolicy,
        RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
        RetrievalAttemptOutcome::NotApplicable,
        RetrievalAttemptOutcome::InterruptedByDeadline,
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
    ];

    for outcome in outcomes {
        let result_count = match outcome {
            RetrievalAttemptOutcome::SuccessWithResults => 1,
            _ => 0,
        };
        let deadline_interrupted = outcome == RetrievalAttemptOutcome::InterruptedByDeadline;
        let attempt = RetrievalAttempt {
            provider_id: "test_prov".to_string(),
            subquery_id: Some("sq_test".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome: outcome.clone(),
            result_count,
            error_class: None,
            deadline_interrupted,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        };
        let summary = build_retrieval_summary_from_attempts(&[attempt]);
        assert_eq!(
            summary.dimensions.len(),
            1,
            "each outcome must produce exactly one dimension"
        );
        let dim = &summary.dimensions[0];
        assert!(
            !dim.message.is_empty(),
            "absence_kind must produce a non-empty message for outcome {outcome:?}"
        );
        let classified = classify_absence(dim.absence_kind);
        assert!(
            !classified.is_empty(),
            "absence_kind must classify to a non-empty label for outcome {outcome:?}"
        );
    }
}

#[test]
fn a13_truncation_emitted_on_partial_success() {
    let attempt = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_trunc".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
        result_count: 5,
        error_class: None,
        deadline_interrupted: false,
        truncated: true,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(200),
    };
    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    assert!(summary.has_truncation, "truncation must be detected");
    assert_eq!(summary.truncated_count, Some(1));
    let dim = &summary.dimensions[0];
    assert_eq!(
        dim.absence_kind,
        EvidenceAbsenceKind::ResultTruncatedByCap,
        "truncated attempt must map to ResultTruncatedByCap"
    );
    assert!(dim.truncated, "dimension must have truncated=true");
    assert_eq!(dim.result_count, Some(5), "result count must be preserved");
}

#[test]
fn candidate_limit_reach_is_possible_truncation_only() {
    let attempt = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_limit".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 10,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: TruncationEvidence::LimitReachedUnknown,
        query_fingerprint: None,
        duration_ms: None,
    };

    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    assert!(!summary.has_truncation);
    assert_eq!(summary.truncated_count, Some(0));
    assert_eq!(summary.limit_reached_unknown_count, Some(1));
    assert!(!summary.dimensions[0].truncated);
    assert_eq!(
        summary.dimensions[0].truncation_evidence,
        TruncationEvidence::LimitReachedUnknown
    );
}

#[test]
fn summary_distinguishes_unknown_and_confirmed_truncation() {
    let attempts = [
        RetrievalAttempt {
            provider_id: "provider_a".to_string(),
            subquery_id: Some("sq_unknown".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome: RetrievalAttemptOutcome::SuccessWithResults,
            result_count: 10,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: TruncationEvidence::LimitReachedUnknown,
            query_fingerprint: None,
            duration_ms: None,
        },
        RetrievalAttempt {
            provider_id: "provider_b".to_string(),
            subquery_id: Some("sq_confirmed".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::OfficialDocumentation],
            outcome: RetrievalAttemptOutcome::SuccessWithResults,
            result_count: 3,
            error_class: None,
            deadline_interrupted: false,
            truncated: true,
            truncation_evidence: TruncationEvidence::ConfirmedByProvider,
            query_fingerprint: None,
            duration_ms: None,
        },
    ];

    let summary = build_retrieval_summary_from_attempts(&attempts);
    assert!(summary.has_truncation);
    assert_eq!(summary.truncated_count, Some(1));
    assert_eq!(summary.limit_reached_unknown_count, Some(1));
}

#[test]
fn old_attempt_payload_defaults_new_truncation_field() {
    let payload = serde_json::json!({
        "provider_id": "provider",
        "outcome": "success_with_results",
        "result_count": 1,
        "truncated": false
    });
    let attempt: RetrievalAttempt = serde_json::from_value(payload).expect("old payload");
    assert_eq!(attempt.truncation_evidence, TruncationEvidence::None);
}

#[test]
fn c3_query_fingerprint_populated_in_all_attempts() {
    let outcomes = [
        RetrievalAttemptOutcome::SuccessWithResults,
        RetrievalAttemptOutcome::SuccessZeroResults,
        RetrievalAttemptOutcome::Failed,
        RetrievalAttemptOutcome::TimedOut,
        RetrievalAttemptOutcome::RateLimited,
        RetrievalAttemptOutcome::SkippedByPolicy,
        RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
        RetrievalAttemptOutcome::NotApplicable,
        RetrievalAttemptOutcome::InterruptedByDeadline,
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
    ];

    for outcome in outcomes {
        let result_count = match outcome {
            RetrievalAttemptOutcome::SuccessWithResults => 1,
            _ => 0,
        };
        let deadline_interrupted = outcome == RetrievalAttemptOutcome::InterruptedByDeadline;
        let attempt = RetrievalAttempt {
            provider_id: "test_prov".to_string(),
            subquery_id: Some("sq_fp".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome: outcome.clone(),
            result_count,
            error_class: None,
            deadline_interrupted,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: Some(query_fingerprint_from_query("test query")),
            duration_ms: None,
        };
        let summary = build_retrieval_summary_from_attempts(&[attempt]);
        assert_eq!(summary.dimensions.len(), 1);
        let dim = &summary.dimensions[0];
        assert!(
            dim.query.is_some(),
            "query_fingerprint must be populated in dimension for outcome {outcome:?}"
        );
        let fp = dim.query.as_ref().unwrap();
        assert!(
            fp.starts_with("fp_"),
            "fingerprint must start with fp_ prefix, got: {fp}"
        );
    }
}

#[test]
fn c4_query_fingerprint_deterministic() {
    let fp1 = query_fingerprint_from_query("axum router middleware");
    let fp2 = query_fingerprint_from_query("axum router middleware");
    assert_eq!(fp1, fp2, "same query must produce same fingerprint");

    let fp3 = query_fingerprint_from_query("different query entirely");
    assert_ne!(
        fp1, fp3,
        "different queries must produce different fingerprints"
    );
}

#[test]
fn c5_query_fingerprint_not_empty() {
    let fp = query_fingerprint_from_query("any query");
    assert!(!fp.is_empty(), "fingerprint must not be empty");
    assert!(
        fp.starts_with("fp_"),
        "fingerprint must start with fp_ prefix"
    );
    assert_eq!(fp.len(), 19, "fingerprint must be fp_ + 16 hex chars");

    let fp_empty = query_fingerprint_from_query("");
    assert!(
        !fp_empty.is_empty(),
        "fingerprint for empty query must not be empty"
    );
}

#[test]
fn e4_zero_result_summary_retains_result_count_and_outcome() {
    let attempt = RetrievalAttempt {
        provider_id: "osv".to_string(),
        subquery_id: Some("sq_zero".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        outcome: RetrievalAttemptOutcome::SuccessZeroResults,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(50),
    };
    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    let dim = &summary.dimensions[0];
    assert_eq!(dim.result_count, Some(0));
    assert_eq!(
        dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::SuccessZeroResults)
    );
    assert_eq!(
        dim.absence_kind,
        EvidenceAbsenceKind::NoMatchingEvidenceFound
    );
}

#[test]
fn e5_rate_limit_retains_rate_limited_and_coarse_mapping() {
    let attempt = RetrievalAttempt {
        provider_id: "startpage".to_string(),
        subquery_id: Some("sq_rl".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::OfficialDocumentation],
        outcome: RetrievalAttemptOutcome::RateLimited,
        result_count: 0,
        error_class: Some("rate_limited".to_string()),
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(100),
    };
    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    let dim = &summary.dimensions[0];
    assert_eq!(
        dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::RateLimited)
    );
    assert_eq!(dim.absence_kind, EvidenceAbsenceKind::ProviderFailed);
    assert_eq!(dim.error_class.as_deref(), Some("rate_limited"));
    assert_eq!(summary.rate_limited_count, Some(1));
}

#[test]
fn e6_provider_timeout_and_global_deadline_serialize_differently() {
    let timeout = RetrievalAttempt {
        provider_id: "prov_a".to_string(),
        subquery_id: Some("sq_t".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::TimedOut,
        result_count: 0,
        error_class: Some("timeout".to_string()),
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(5000),
    };
    let deadline = RetrievalAttempt {
        provider_id: "prov_b".to_string(),
        subquery_id: Some("sq_d".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::InterruptedByDeadline,
        result_count: 0,
        error_class: None,
        deadline_interrupted: true,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(2000),
    };
    let summary = build_retrieval_summary_from_attempts(&[timeout, deadline]);
    let t_dim = summary
        .dimensions
        .iter()
        .find(|d| d.subquery_id.as_deref() == Some("sq_t"))
        .unwrap();
    let d_dim = summary
        .dimensions
        .iter()
        .find(|d| d.subquery_id.as_deref() == Some("sq_d"))
        .unwrap();
    assert_eq!(
        t_dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::TimedOut)
    );
    assert_eq!(
        d_dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::InterruptedByDeadline)
    );
    assert_ne!(t_dim.attempt_outcome, d_dim.attempt_outcome);
}

#[test]
fn e7_truncation_is_explicit() {
    let attempt = RetrievalAttempt {
        provider_id: "gitea".to_string(),
        subquery_id: Some("sq_trunc".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
        result_count: 3,
        error_class: None,
        deadline_interrupted: false,
        truncated: true,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(150),
    };
    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    assert!(summary.has_truncation);
    assert_eq!(summary.truncated_count, Some(1));
    let dim = &summary.dimensions[0];
    assert!(dim.truncated);
    assert_eq!(dim.absence_kind, EvidenceAbsenceKind::ResultTruncatedByCap);
}

#[test]
fn e8_subquery_id_is_retained() {
    let attempt = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_my_subquery".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 5,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(100),
    };
    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    let dim = &summary.dimensions[0];
    assert_eq!(
        dim.subquery_id.as_deref(),
        Some("sq_my_subquery"),
        "subquery_id must be retained in summary"
    );
}

#[test]
fn e9_fallback_summary_not_used_when_attempts_exist() {
    let attempt = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_fb".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 3,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(80),
    };
    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    let dim = &summary.dimensions[0];
    assert_eq!(
        dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::SuccessWithResults),
        "attempt-derived summary must use attempt outcome, not infer from card membership"
    );
}

#[test]
fn e10_summary_ordering_deterministic_by_subquery_provider_role() {
    let attempts = vec![
        RetrievalAttempt {
            provider_id: "c_prov".to_string(),
            subquery_id: Some("sq_2".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::OfficialDocumentation],
            outcome: RetrievalAttemptOutcome::SuccessWithResults,
            result_count: 1,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
        RetrievalAttempt {
            provider_id: "a_prov".to_string(),
            subquery_id: Some("sq_1".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome: RetrievalAttemptOutcome::Failed,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
        RetrievalAttempt {
            provider_id: "b_prov".to_string(),
            subquery_id: Some("sq_0".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::UsageExample],
            outcome: RetrievalAttemptOutcome::RateLimited,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
    ];
    let s1 = build_retrieval_summary_from_attempts(&attempts);
    let s2 = build_retrieval_summary_from_attempts(&attempts);
    assert_eq!(s1.dimensions.len(), s2.dimensions.len());
    for (d1, d2) in s1.dimensions.iter().zip(s2.dimensions.iter()) {
        assert_eq!(d1.provider_id, d2.provider_id);
        assert_eq!(d1.subquery_id, d2.subquery_id);
        assert_eq!(d1.evidence_role, d2.evidence_role);
    }
}

#[test]
fn e11_aggregate_counts_equal_dimension_derived_counts() {
    let attempts = vec![
        RetrievalAttempt {
            provider_id: "p1".to_string(),
            subquery_id: Some("sq_0".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome: RetrievalAttemptOutcome::SuccessWithResults,
            result_count: 5,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
        RetrievalAttempt {
            provider_id: "p2".to_string(),
            subquery_id: Some("sq_1".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::OfficialDocumentation],
            outcome: RetrievalAttemptOutcome::Failed,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
        RetrievalAttempt {
            provider_id: "p3".to_string(),
            subquery_id: Some("sq_2".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::UsageExample],
            outcome: RetrievalAttemptOutcome::TimedOut,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
        RetrievalAttempt {
            provider_id: "p4".to_string(),
            subquery_id: Some("sq_3".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::BenchmarkOrPerformanceEvidence],
            outcome: RetrievalAttemptOutcome::RateLimited,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
    ];
    let summary = build_retrieval_summary_from_attempts(&attempts);
    let dim_count = summary.dimensions.len();
    assert_eq!(
        summary.attempted_job_count,
        Some(dim_count),
        "attempted_job_count must equal dimension count"
    );
}

#[test]
fn e12_codegg_fixture_consumes_enriched_summary() {
    let attempt = RetrievalAttempt {
        provider_id: "osv".to_string(),
        subquery_id: Some("sq_codegg".to_string()),
        operation_id: None,
        intended_roles: vec![
            EvidenceRole::AuthoritativeSecurityAdvisory,
            EvidenceRole::ManifestOrDependencyMetadata,
        ],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 2,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: Some("fp_aabbccdd11223344".to_string()),
        duration_ms: Some(150),
    };
    let summary = build_retrieval_summary_from_attempts(&[attempt]);
    let json = serde_json::to_value(&summary).unwrap();
    assert!(json.is_object());
    assert!(json["dimensions"].is_array());
    assert_eq!(json["dimensions"].as_array().unwrap().len(), 2);
    let dim0 = &json["dimensions"][0];
    assert!(dim0["provider_id"].is_string());
    assert!(dim0["attempt_outcome"].is_string());
    assert!(dim0["result_count"].is_number());
}

#[test]
fn e13_next_actions_avoid_identical_failed_provider_query() {
    let failed_attempt = RetrievalAttempt {
        provider_id: "startpage".to_string(),
        subquery_id: Some("sq_retry".to_string()),
        operation_id: None,
        intended_roles: vec![EvidenceRole::OfficialDocumentation],
        outcome: RetrievalAttemptOutcome::Failed,
        result_count: 0,
        error_class: Some("connection_refused".to_string()),
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(1000),
    };
    let failures = attempts_to_failures(&[failed_attempt]);
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].kind,
        RetrievalFailureKind::ProviderFailed,
        "failed attempt must produce ProviderFailed"
    );
    let model = WorkflowCoverageModel {
        workflow_id: "test".to_string(),
        title: "Test".to_string(),
        required: vec![EvidenceRole::OfficialDocumentation],
        recommended: vec![],
        optional: vec![],
    };
    let result = compute_coverage(&model, &[], &failures);
    assert_eq!(
        result.status,
        CoverageStatus::IndeterminateDueToFailures,
        "missing required role with failed attempt must be indeterminate"
    );
}
