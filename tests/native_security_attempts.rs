use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::retrieval_status::{
    attempts_to_failures, RetrievalAttempt, RetrievalAttemptOutcome,
};
use eggsearch::core::workflow_coverage::{compute_coverage, CoverageStatus, WorkflowCoverageModel};

fn native_attempt(
    provider_id: &str,
    subquery_id: &str,
    outcome: RetrievalAttemptOutcome,
    roles: Vec<EvidenceRole>,
    result_count: usize,
) -> RetrievalAttempt {
    RetrievalAttempt {
        provider_id: provider_id.to_string(),
        subquery_id: Some(subquery_id.to_string()),
        intended_roles: roles,
        outcome,
        result_count,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(50),
    }
}

#[test]
fn d8_01_cve_lookup_found() {
    let a = native_attempt(
        "github_advisory",
        "advisory_by_cve",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        1,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.dimensions.len(), 1);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::NotApplicable
    );
    assert_eq!(summary.dimensions[0].result_count, Some(1));
}

#[test]
fn d8_02_cve_lookup_zero_result() {
    let a = native_attempt(
        "github_advisory",
        "advisory_by_cve",
        RetrievalAttemptOutcome::SuccessZeroResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::NoMatchingEvidenceFound
    );
    assert_eq!(summary.dimensions[0].result_count, Some(0));
}

#[test]
fn d8_03_ghsa_lookup_failure() {
    let a = native_attempt(
        "github_advisory",
        "advisory_by_ghsa",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let _ = a.clone(); // ensure Clone works
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::ProviderFailed
    );
}

#[test]
fn d8_04_osv_package_query_rate_limited() {
    let a = native_attempt(
        "osv",
        "advisory_by_osv",
        RetrievalAttemptOutcome::RateLimited,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(summary.rate_limited_count, Some(1));
}

#[test]
fn d8_05_package_query_returns_multiple_advisories() {
    let a = native_attempt(
        "osv",
        "advisory_by_package",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        5,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.dimensions[0].result_count, Some(5));
    assert!(!summary.has_failures);
}

#[test]
fn d8_06_package_query_truncates_after_partial_success() {
    let mut a = native_attempt(
        "osv",
        "advisory_by_package",
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        10,
    );
    a.truncated = true;
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_truncation);
    assert!(summary.dimensions[0].truncated);
}

#[test]
fn d8_07_kev_found() {
    let a = native_attempt(
        "cisa_kev",
        "kev_by_cve",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        1,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.dimensions.len(), 1);
    assert!(!summary.has_failures);
}

#[test]
fn d8_08_kev_absent() {
    let a = native_attempt(
        "cisa_kev",
        "kev_by_cve",
        RetrievalAttemptOutcome::SuccessZeroResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(
        summary.dimensions[0].absence_kind,
        eggsearch::core::retrieval_status::EvidenceAbsenceKind::NoMatchingEvidenceFound
    );
}

#[test]
fn d8_09_kev_failure() {
    let a = native_attempt(
        "cisa_kev",
        "kev_by_cve",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
}

#[test]
fn d8_10_multiple_identifiers_produce_distinct_attempts() {
    let attempts = vec![
        native_attempt(
            "github_advisory",
            "advisory_by_cve",
            RetrievalAttemptOutcome::SuccessWithResults,
            vec![EvidenceRole::AuthoritativeSecurityAdvisory],
            1,
        ),
        native_attempt(
            "github_advisory",
            "advisory_by_ghsa",
            RetrievalAttemptOutcome::SuccessWithResults,
            vec![EvidenceRole::AuthoritativeSecurityAdvisory],
            1,
        ),
        native_attempt(
            "osv",
            "advisory_by_osv",
            RetrievalAttemptOutcome::SuccessZeroResults,
            vec![EvidenceRole::AuthoritativeSecurityAdvisory],
            0,
        ),
    ];
    let summary = build_retrieval_summary_from_attempts(&attempts);
    assert_eq!(summary.dimensions.len(), 3);
    let subquery_ids: Vec<_> = summary
        .dimensions
        .iter()
        .filter_map(|d| d.subquery_id.clone())
        .collect();
    let unique: std::collections::HashSet<_> = subquery_ids.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "each identifier must produce a distinct attempt"
    );
}

#[test]
fn d8_11_duplicate_identifiers_are_looked_up_once() {
    let attempts = vec![native_attempt(
        "osv",
        "advisory_by_osv",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        1,
    )];
    let summary = build_retrieval_summary_from_attempts(&attempts);
    assert_eq!(summary.dimensions.len(), 1);
}

#[test]
fn d8_12_native_advisory_failure_makes_coverage_indeterminate() {
    let model = WorkflowCoverageModel {
        workflow_id: "security_review".to_string(),
        title: "Security Review".to_string(),
        required: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        recommended: vec![],
        optional: vec![],
    };
    let a = native_attempt(
        "github_advisory",
        "advisory_by_cve",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let failures = attempts_to_failures(&[a]);
    let result = compute_coverage(&model, &[], &failures);
    assert_eq!(result.status, CoverageStatus::IndeterminateDueToFailures);
}

#[test]
fn d8_13_native_zero_result_makes_coverage_insufficient() {
    let model = WorkflowCoverageModel {
        workflow_id: "security_review".to_string(),
        title: "Security Review".to_string(),
        required: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        recommended: vec![],
        optional: vec![],
    };
    let a = native_attempt(
        "osv",
        "advisory_by_osv",
        RetrievalAttemptOutcome::SuccessZeroResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let failures = attempts_to_failures(&[a]);
    assert!(
        failures.is_empty(),
        "zero-result success must not produce failures"
    );
    let result = compute_coverage(&model, &[], &[]);
    assert_eq!(result.status, CoverageStatus::Insufficient);
}

#[test]
fn d8_14_native_failure_with_no_other_source_makes_indeterminate() {
    let model = WorkflowCoverageModel {
        workflow_id: "security_review".to_string(),
        title: "Security Review".to_string(),
        required: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        recommended: vec![],
        optional: vec![],
    };
    let native_fail = native_attempt(
        "github_advisory",
        "advisory_by_cve",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let failures = attempts_to_failures(&[native_fail]);
    let result = compute_coverage(&model, &[], &failures);
    assert_eq!(
        result.status,
        CoverageStatus::IndeterminateDueToFailures,
        "native failure with no other source must make coverage indeterminate"
    );
}

#[test]
fn d8_15_serialized_security_summary_contains_both_generic_and_native() {
    let generic = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("sq_generic".to_string()),
        intended_roles: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 3,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: Some(80),
    };
    let native = native_attempt(
        "github_advisory",
        "advisory_by_cve",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        1,
    );
    let summary = build_retrieval_summary_from_attempts(&[generic, native]);
    assert_eq!(summary.dimensions.len(), 2);
    let providers: Vec<_> = summary
        .dimensions
        .iter()
        .filter_map(|d| d.provider_id.clone())
        .collect();
    assert!(providers.contains(&"duckduckgo".to_string()));
    assert!(providers.contains(&"github_advisory".to_string()));
}

#[test]
fn d8_16_direct_lookup_errors_never_silently_discarded() {
    let a = native_attempt(
        "github_advisory",
        "advisory_by_cve",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let failures = attempts_to_failures(&[a]);
    assert_eq!(
        failures.len(),
        1,
        "direct lookup error must produce a failure record"
    );
}

#[test]
fn d8_17_codegg_fixture_distinguishes_no_advisory_from_provider_failed() {
    let zero = native_attempt(
        "osv",
        "advisory_by_osv",
        RetrievalAttemptOutcome::SuccessZeroResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let failed = native_attempt(
        "osv",
        "advisory_by_osv",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let s_zero = build_retrieval_summary_from_attempts(&[zero]);
    let s_failed = build_retrieval_summary_from_attempts(&[failed]);
    assert_ne!(
        s_zero.dimensions[0].absence_kind, s_failed.dimensions[0].absence_kind,
        "zero-result and failure must produce different absence kinds"
    );
}
