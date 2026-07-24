use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::retrieval_status::{
    attempts_to_failures, RetrievalAttempt, RetrievalAttemptOutcome,
};

fn attempt(
    provider_id: &str,
    outcome: RetrievalAttemptOutcome,
    roles: Vec<EvidenceRole>,
) -> RetrievalAttempt {
    RetrievalAttempt {
        provider_id: provider_id.to_string(),
        subquery_id: Some("sq_0".to_string()),
        intended_roles: roles,
        outcome,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: None,
        duration_ms: Some(100),
    }
}

#[test]
fn c8_01_complete_success_with_results() {
    let a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.dimensions.len(), 1);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::NotApplicable
    );
    assert_eq!(summary.dimensions[0].result_count, Some(0));
    assert!(!summary.has_failures);
}

#[test]
fn c8_02_complete_success_with_zero_results() {
    let a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessZeroResults,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.dimensions.len(), 1);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::NoMatchingEvidenceFound
    );
    assert!(!summary.has_failures);
}

#[test]
fn c8_03_provider_failure() {
    let a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ProviderFailed
    );
}

#[test]
fn c8_04_provider_local_timeout() {
    let a = attempt(
        "startpage",
        RetrievalAttemptOutcome::TimedOut,
        vec![EvidenceRole::OfficialDocumentation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::DeadlinePreventedCompletion
    );
}

#[test]
fn c8_05_http_429_rate_limit() {
    let a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::RateLimited,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ProviderFailed
    );
}

#[test]
fn c8_06_explicit_policy_exclusion() {
    let a = attempt(
        "brave",
        RetrievalAttemptOutcome::SkippedByPolicy,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(!summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ProviderSkippedByPolicy
    );
}

#[test]
fn c8_07_capability_exclusion() {
    let a = attempt(
        "brave",
        RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(!summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ProviderCapabilityUnavailable
    );
}

#[test]
fn c8_08_planner_not_applicable() {
    let a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::NotApplicable,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(!summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::NotApplicable
    );
}

#[test]
fn c8_09_pending_job_interrupted_by_global_deadline() {
    let mut a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::InterruptedByDeadline,
        vec![EvidenceRole::PrimaryImplementation],
    );
    a.deadline_interrupted = true;
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::DeadlinePreventedCompletion
    );
}

#[test]
fn c8_10_running_job_interrupted_by_global_deadline() {
    let mut a = attempt(
        "startpage",
        RetrievalAttemptOutcome::InterruptedByDeadline,
        vec![EvidenceRole::OfficialDocumentation],
    );
    a.deadline_interrupted = true;
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::DeadlinePreventedCompletion
    );
}

#[test]
fn c8_11_partial_results_truncated_by_candidate_cap() {
    let mut a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
        vec![EvidenceRole::PrimaryImplementation],
    );
    a.truncated = true;
    a.result_count = 5;
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_truncation);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ResultTruncatedByCap
    );
}

#[test]
fn c8_12_forge_partial_results_truncated_by_byte_budget() {
    let mut a = attempt(
        "gitea",
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
        vec![EvidenceRole::PrimaryImplementation],
    );
    a.truncated = true;
    a.result_count = 3;
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_truncation);
    assert!(summary.dimensions[0].truncated);
}

#[test]
fn c8_13_same_provider_multiple_subqueries_produce_distinct_attempts() {
    let a1 = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_0".to_string()),
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 5,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: None,
        duration_ms: Some(50),
    };
    let a2 = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_1".to_string()),
        intended_roles: vec![EvidenceRole::OfficialDocumentation],
        outcome: RetrievalAttemptOutcome::SuccessZeroResults,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: None,
        duration_ms: Some(30),
    };
    let summary = build_retrieval_summary_from_attempts(&[a1, a2]);
    assert_eq!(summary.dimensions.len(), 2);
    assert_ne!(
        summary.dimensions[0].subquery_id,
        summary.dimensions[1].subquery_id
    );
}

#[test]
fn c8_14_selected_job_produces_exactly_one_terminal_record() {
    let attempts = vec![
        attempt(
            "duckduckgo",
            RetrievalAttemptOutcome::SuccessWithResults,
            vec![EvidenceRole::PrimaryImplementation],
        ),
        attempt(
            "startpage",
            RetrievalAttemptOutcome::Failed,
            vec![EvidenceRole::OfficialDocumentation],
        ),
        attempt(
            "brave",
            RetrievalAttemptOutcome::RateLimited,
            vec![EvidenceRole::UsageExample],
        ),
    ];
    let summary = build_retrieval_summary_from_attempts(&attempts);
    assert_eq!(summary.dimensions.len(), 3);
    assert_eq!(summary.attempted_job_count, Some(3));
}

#[test]
fn c8_15_query_fingerprint_does_not_expose_raw_text() {
    let mut a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::PrimaryImplementation],
    );
    a.query_fingerprint = Some("abc123def456".to_string());
    let summary = build_retrieval_summary_from_attempts(&[a]);
    let q = &summary.dimensions[0].query;
    assert!(q
        .as_ref()
        .is_none_or(|f| !f.contains("password") && !f.contains("token")));
}

#[test]
fn c8_16_attempt_ordering_deterministic() {
    let attempts = vec![
        attempt(
            "c_provider",
            RetrievalAttemptOutcome::Failed,
            vec![EvidenceRole::PrimaryImplementation],
        ),
        attempt(
            "a_provider",
            RetrievalAttemptOutcome::SuccessWithResults,
            vec![EvidenceRole::OfficialDocumentation],
        ),
        attempt(
            "b_provider",
            RetrievalAttemptOutcome::RateLimited,
            vec![EvidenceRole::UsageExample],
        ),
    ];
    let s1 = build_retrieval_summary_from_attempts(&attempts);
    let s2 = build_retrieval_summary_from_attempts(&attempts);
    for (d1, d2) in s1.dimensions.iter().zip(s2.dimensions.iter()) {
        assert_eq!(d1.provider_id, d2.provider_id);
        assert_eq!(d1.absence_kind, d2.absence_kind);
    }
}

#[test]
fn c8_17_provider_panic_yields_failed_attempt() {
    let a = RetrievalAttempt {
        provider_id: "panicking_provider".to_string(),
        subquery_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::Failed,
        result_count: 0,
        error_class: Some("panic".to_string()),
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: None,
        duration_ms: None,
    };
    let failures = attempts_to_failures(&[a]);
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].provider_id.as_deref(),
        Some("panicking_provider")
    );
}

#[test]
fn c8_18_property_selected_job_has_one_terminal_outcome() {
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
        let a = attempt(
            "test_provider",
            outcome.clone(),
            vec![EvidenceRole::PrimaryImplementation],
        );
        let summary = build_retrieval_summary_from_attempts(&[a]);
        assert_eq!(
            summary.dimensions.len(),
            1,
            "each selected job produces exactly one dimension for outcome {outcome:?}"
        );
    }
}

#[test]
fn b6_attempt_ledger_completeness() {
    let attempts = vec![
        attempt(
            "duckduckgo",
            RetrievalAttemptOutcome::SuccessWithResults,
            vec![EvidenceRole::PrimaryImplementation],
        ),
        attempt(
            "startpage",
            RetrievalAttemptOutcome::Failed,
            vec![EvidenceRole::OfficialDocumentation],
        ),
        attempt(
            "brave",
            RetrievalAttemptOutcome::TimedOut,
            vec![EvidenceRole::UsageExample],
        ),
        attempt(
            "osv",
            RetrievalAttemptOutcome::SuccessZeroResults,
            vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        ),
    ];
    let summary = build_retrieval_summary_from_attempts(&attempts);
    assert_eq!(
        summary.dimensions.len(),
        4,
        "every attempt must produce a dimension"
    );
    assert_eq!(summary.attempted_job_count, Some(4));
}

#[test]
fn b7_attempt_ledger_deadline_interrupted() {
    let mut a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::InterruptedByDeadline,
        vec![EvidenceRole::PrimaryImplementation],
    );
    a.deadline_interrupted = true;
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(summary.deadline_interrupted_count, Some(1));
    assert_eq!(
        summary.dimensions[0].attempt_outcome,
        Some(RetrievalAttemptOutcome::InterruptedByDeadline)
    );
}

#[test]
fn b8_attempt_ledger_early_termination() {
    let a1 = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let mut a2 = attempt(
        "startpage",
        RetrievalAttemptOutcome::InterruptedByDeadline,
        vec![EvidenceRole::OfficialDocumentation],
    );
    a2.deadline_interrupted = true;
    let summary = build_retrieval_summary_from_attempts(&[a1, a2]);
    assert_eq!(summary.dimensions.len(), 2);
    let success_dim = summary
        .dimensions
        .iter()
        .find(|d| d.provider_id.as_deref() == Some("duckduckgo"))
        .unwrap();
    let deadline_dim = summary
        .dimensions
        .iter()
        .find(|d| d.provider_id.as_deref() == Some("startpage"))
        .unwrap();
    assert_eq!(
        success_dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::SuccessWithResults)
    );
    assert_eq!(
        deadline_dim.attempt_outcome,
        Some(RetrievalAttemptOutcome::InterruptedByDeadline)
    );
}

#[test]
fn b9_attempt_ledger_not_applicable() {
    let a = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::NotApplicable,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(!summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::NotApplicable
    );
    assert_eq!(summary.policy_skipped_count, Some(0));
}

#[test]
fn b10_attempt_ledger_health_degradation() {
    let a1 = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let mut a2 = attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::OfficialDocumentation],
    );
    a2.error_class = Some("connection_refused".to_string());
    let summary = build_retrieval_summary_from_attempts(&[a1, a2]);
    assert!(summary.has_failures);
    assert_eq!(summary.failed_job_count, Some(1));
    let failed_dim = summary
        .dimensions
        .iter()
        .find(|d| {
            d.absence_kind == eggsearch::core::retrieval_status::EvidenceAbsenceKind::ProviderFailed
        })
        .unwrap();
    assert_eq!(
        failed_dim.error_class.as_deref(),
        Some("connection_refused")
    );
}

#[test]
fn b11_attempt_ledger_concurrency_limit() {
    let a = attempt(
        "rate_limited_provider",
        RetrievalAttemptOutcome::RateLimited,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(summary.rate_limited_count, Some(1));
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ProviderFailed
    );
}

#[test]
fn b12_attempt_ledger_network_error() {
    let mut a = attempt(
        "failing_provider",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::PrimaryImplementation],
    );
    a.error_class = Some("dns_resolution_failed".to_string());
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(summary.failed_job_count, Some(1));
    assert_eq!(
        summary.dimensions[0].error_class.as_deref(),
        Some("dns_resolution_failed")
    );
}

#[test]
fn b13_attempt_ledger_no_error_suppression() {
    let a1 = attempt(
        "provider_a",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let a2 = attempt(
        "provider_b",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::OfficialDocumentation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a1, a2]);
    assert!(
        summary.has_failures,
        "failure must not be suppressed by success of another provider"
    );
    assert_eq!(summary.failed_job_count, Some(1));
    assert_eq!(summary.completed_job_count, Some(1));
}

#[test]
fn b14_attempt_ledger_metadata_only() {
    let a = attempt(
        "provider_meta",
        RetrievalAttemptOutcome::SuccessZeroResults,
        vec![EvidenceRole::OfficialDocumentation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(!summary.has_failures);
    assert_eq!(summary.zero_result_count, Some(1));
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::NoMatchingEvidenceFound
    );
}

#[test]
fn b15_attempt_ledger_empty_query() {
    let a = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_empty".to_string()),
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::SuccessZeroResults,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: Some(
            eggsearch::core::retrieval_status::query_fingerprint_from_query(""),
        ),
        duration_ms: Some(10),
    };
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.dimensions.len(), 1);
    let dim = &summary.dimensions[0];
    assert!(
        dim.query.is_some(),
        "empty query must still produce a fingerprint"
    );
}

#[test]
fn b16_attempt_ledger_skipped_providers_not_in_attempted() {
    let a1 = attempt(
        "selected_provider",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let a2 = attempt(
        "skipped_provider",
        RetrievalAttemptOutcome::SkippedByPolicy,
        vec![EvidenceRole::OfficialDocumentation],
    );
    let summary = build_retrieval_summary_from_attempts(&[a1, a2]);
    assert_eq!(summary.dimensions.len(), 2);
    assert_eq!(summary.policy_skipped_count, Some(1));
    assert_eq!(summary.completed_job_count, Some(1));
    let skipped_dim = summary
        .dimensions
        .iter()
        .find(|d| d.provider_id.as_deref() == Some("skipped_provider"))
        .unwrap();
    assert_eq!(
        skipped_dim.absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ProviderSkippedByPolicy
    );
}

#[test]
fn b17_attempt_ledger_deadline_prevents_all_queries() {
    let a1 = attempt(
        "provider_a",
        RetrievalAttemptOutcome::InterruptedByDeadline,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let mut a2 = attempt(
        "provider_b",
        RetrievalAttemptOutcome::InterruptedByDeadline,
        vec![EvidenceRole::OfficialDocumentation],
    );
    a2.deadline_interrupted = true;
    let summary = build_retrieval_summary_from_attempts(&[a1, a2]);
    assert!(summary.has_failures);
    assert_eq!(summary.deadline_interrupted_count, Some(2));
    assert_eq!(summary.completed_job_count, Some(0));
}

#[test]
fn b18_attempt_ledger_partial_completion() {
    let a1 = attempt(
        "provider_a",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::PrimaryImplementation],
    );
    let mut a2 = attempt(
        "provider_b",
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
        vec![EvidenceRole::OfficialDocumentation],
    );
    a2.truncated = true;
    a2.result_count = 3;
    let summary = build_retrieval_summary_from_attempts(&[a1, a2]);
    assert!(summary.has_truncation);
    assert_eq!(summary.truncated_count, Some(1));
    assert_eq!(summary.completed_job_count, Some(1));
    let trunc_dim = summary.dimensions.iter().find(|d| d.truncated).unwrap();
    assert_eq!(
        trunc_dim.absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ResultTruncatedByCap
    );
}
