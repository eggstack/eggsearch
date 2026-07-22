use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::retrieval_status::{
    attempts_to_failures, classify_absence, map_provider_to_intended_roles, EvidenceAbsenceKind,
    RetrievalAttempt, RetrievalAttemptOutcome,
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
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome,
            result_count,
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
        Just(RetrievalAttemptOutcome::SkippedByPolicy),
        Just(RetrievalAttemptOutcome::SkippedCapabilityUnavailable),
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
                    || failure.kind == eggsearch::core::workflow_coverage::RetrievalFailureKind::DeadlinePreventedCompletion,
                "failure kind must be ProviderFailed or DeadlinePreventedCompletion, got {:?}",
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
                intended_roles: vec![EvidenceRole::PrimaryImplementation],
                outcome,
                result_count: 0,
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
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome: RetrievalAttemptOutcome::Failed,
            result_count: 0,
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
            intended_roles: vec![EvidenceRole::OfficialDocumentation],
            outcome: RetrievalAttemptOutcome::TimedOut,
            result_count: 0,
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
            intended_roles: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
            outcome: RetrievalAttemptOutcome::RateLimited,
            result_count: 0,
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
            intended_roles: vec![EvidenceRole::UsageExample],
            outcome: RetrievalAttemptOutcome::InterruptedByDeadline,
            result_count: 0,
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
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome,
            result_count: 5,
        };
        let failures = attempts_to_failures(&[attempt]);
        prop_assert!(failures.is_empty(), "non-failure outcome must produce no failures");
    }
}
