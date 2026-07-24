use eggsearch::core::evidence_postprocess::build_retrieval_summary_from_attempts;
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::retrieval_status::{
    ResponseRetrievalSummary, RetrievalAttempt, RetrievalAttemptOutcome,
};

fn make_attempt(
    provider: &str,
    outcome: RetrievalAttemptOutcome,
    roles: Vec<EvidenceRole>,
    result_count: usize,
) -> RetrievalAttempt {
    RetrievalAttempt {
        provider_id: provider.to_string(),
        subquery_id: Some("codegg_sq_0".to_string()),
        intended_roles: roles,
        outcome,
        result_count,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: Some("fp_abc123".to_string()),
        duration_ms: Some(150),
    }
}

#[test]
fn codegg_consumes_multi_role_attempt_dimensions() {
    let a = make_attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::OfficialDocumentation,
        ],
        5,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.dimensions.len(), 2);
    let roles: Vec<_> = summary.dimensions.iter().map(|d| d.evidence_role).collect();
    assert!(roles.contains(&EvidenceRole::PrimaryImplementation));
    assert!(roles.contains(&EvidenceRole::OfficialDocumentation));
}

#[test]
fn codegg_consumes_zero_result_dimension() {
    let a = make_attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessZeroResults,
        vec![EvidenceRole::PrimaryImplementation],
        0,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.dimensions[0].result_count, Some(0));
    assert_eq!(
        summary.dimensions[0].attempt_outcome,
        Some(RetrievalAttemptOutcome::SuccessZeroResults)
    );
}

#[test]
fn codegg_consumes_failure_dimension() {
    let a = make_attempt(
        "startpage",
        RetrievalAttemptOutcome::Failed,
        vec![EvidenceRole::OfficialDocumentation],
        0,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_failures);
    assert_eq!(
        summary.dimensions[0].attempt_outcome,
        Some(RetrievalAttemptOutcome::Failed)
    );
}

#[test]
fn codegg_consumes_rate_limit_dimension() {
    let a = make_attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::RateLimited,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        0,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.rate_limited_count, Some(1));
    assert_eq!(
        summary.dimensions[0].attempt_outcome,
        Some(RetrievalAttemptOutcome::RateLimited)
    );
}

#[test]
fn codegg_consumes_deadline_dimension() {
    let mut a = make_attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::InterruptedByDeadline,
        vec![EvidenceRole::PrimaryImplementation],
        0,
    );
    a.deadline_interrupted = true;
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(summary.deadline_interrupted_count, Some(1));
}

#[test]
fn codegg_consumes_truncation_dimension() {
    let mut a = make_attempt(
        "gitea",
        RetrievalAttemptOutcome::TruncatedAfterPartialSuccess,
        vec![EvidenceRole::PrimaryImplementation],
        8,
    );
    a.truncated = true;
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert!(summary.has_truncation);
    assert!(summary.dimensions[0].truncated);
}

#[test]
fn codegg_consumes_conflict_source_ids() {
    use eggsearch::core::conflict::detect_entity_scoped_conflicts;
    use eggsearch::core::security::VulnerabilityMetadata;
    use eggsearch::core::source_card::{SourceCard, SourceKind, SourceMetadata};

    let cards = vec![
        SourceCard {
            id: "c1".to_string(),
            stable_id: Some("c1".to_string()),
            title: "advisory".to_string(),
            url: "https://example.com/cve-1".to_string(),
            providers: vec!["test".to_string()],
            score: Some(1.0),
            trust: eggsearch::core::result::TrustLevel::ExternalUntrusted,
            fetched: false,
            snippet: None,
            trust_markers: eggsearch::core::sanitize::TrustMarkers::default(),
            metadata: SourceMetadata {
                source_kind: SourceKind::SecurityAdvisory,
                vulnerability: Some(Box::new(VulnerabilityMetadata {
                    cve_ids: vec!["CVE-2024-9999".to_string()],
                    package: Some("test-pkg".to_string()),
                    patched_versions: vec![">=1.0.0".to_string()],
                    ..Default::default()
                })),
                ..Default::default()
            },
            quality: None,
        },
        SourceCard {
            id: "c2".to_string(),
            stable_id: Some("c2".to_string()),
            title: "advisory".to_string(),
            url: "https://example.com/cve-1".to_string(),
            providers: vec!["test".to_string()],
            score: Some(1.0),
            trust: eggsearch::core::result::TrustLevel::ExternalUntrusted,
            fetched: false,
            snippet: None,
            trust_markers: eggsearch::core::sanitize::TrustMarkers::default(),
            metadata: SourceMetadata {
                source_kind: SourceKind::SecurityAdvisory,
                vulnerability: Some(Box::new(VulnerabilityMetadata {
                    cve_ids: vec!["CVE-2024-9999".to_string()],
                    package: Some("test-pkg".to_string()),
                    patched_versions: vec![">=2.0.0".to_string()],
                    ..Default::default()
                })),
                ..Default::default()
            },
            quality: None,
        },
    ];
    let conflicts = detect_entity_scoped_conflicts(&cards);
    for c in &conflicts {
        assert!(
            c.source_ids.len() >= 2,
            "conflict must name at least 2 disagreeing sources"
        );
    }
}

#[test]
fn codegg_summary_json_is_additive_compatible() {
    let a = make_attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::PrimaryImplementation],
        3,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    let json = serde_json::to_string(&summary).unwrap();
    let parsed: ResponseRetrievalSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.dimensions.len(), 1);
    assert_eq!(parsed.attempted_job_count, Some(1));
    assert_eq!(parsed.completed_job_count, Some(1));
    assert!(parsed.zero_result_count.is_some());
    assert!(parsed.roles_attempted.is_some());
}

#[test]
fn codegg_old_client_ignores_additive_fields() {
    let a = make_attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::PrimaryImplementation],
        3,
    );
    let summary = build_retrieval_summary_from_attempts(&[a]);
    let json = serde_json::to_string(&summary).unwrap();

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct OldSummary {
        dimensions: Vec<OldDimension>,
        has_failures: bool,
        has_absences: bool,
        has_truncation: bool,
    }

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct OldDimension {
        evidence_role: EvidenceRole,
        absence_kind: eggsearch::core::retrieval_status::EvidenceAbsenceKind,
        message: String,
    }

    let old: OldSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(old.dimensions.len(), 1);
    assert!(!old.has_failures);
    assert_eq!(
        old.dimensions[0].evidence_role,
        EvidenceRole::PrimaryImplementation
    );
}

#[test]
fn codegg_subquery_id_preserved() {
    let a = RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: Some("codegg_sub_42".to_string()),
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 2,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: None,
        duration_ms: None,
    };
    let summary = build_retrieval_summary_from_attempts(&[a]);
    assert_eq!(
        summary.dimensions[0].subquery_id.as_deref(),
        Some("codegg_sub_42")
    );
}

#[test]
fn codegg_aggregate_counts_match_dimensions() {
    let attempts = vec![
        make_attempt(
            "p1",
            RetrievalAttemptOutcome::SuccessWithResults,
            vec![EvidenceRole::PrimaryImplementation],
            5,
        ),
        make_attempt(
            "p2",
            RetrievalAttemptOutcome::Failed,
            vec![EvidenceRole::OfficialDocumentation],
            0,
        ),
        make_attempt(
            "p3",
            RetrievalAttemptOutcome::SuccessZeroResults,
            vec![EvidenceRole::UsageExample],
            0,
        ),
    ];
    let summary = build_retrieval_summary_from_attempts(&attempts);
    assert_eq!(summary.attempted_job_count, Some(3));
    assert_eq!(summary.completed_job_count, Some(1));
    assert_eq!(summary.failed_job_count, Some(1));
    assert_eq!(summary.zero_result_count, Some(1));
}

#[test]
fn codegg_native_security_attempt_dimensions_consumed() {
    let native_attempt = RetrievalAttempt {
        provider_id: "osv".to_string(),
        subquery_id: Some("advisory_by_cve".to_string()),
        intended_roles: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 1,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: Some("fp_native_abc".to_string()),
        duration_ms: Some(50),
    };
    let kev_attempt = RetrievalAttempt {
        provider_id: "cisa_kev".to_string(),
        subquery_id: Some("kev_by_cve".to_string()),
        intended_roles: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        outcome: RetrievalAttemptOutcome::SuccessWithResults,
        result_count: 1,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        query_fingerprint: Some("fp_kev_abc".to_string()),
        duration_ms: Some(30),
    };
    let web_attempt = make_attempt(
        "duckduckgo",
        RetrievalAttemptOutcome::SuccessWithResults,
        vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        3,
    );
    let summary =
        build_retrieval_summary_from_attempts(&[native_attempt, kev_attempt, web_attempt]);

    let native_dims: Vec<_> = summary
        .dimensions
        .iter()
        .filter(|d| d.provider_id.as_deref() == Some("osv"))
        .collect();
    assert_eq!(
        native_dims.len(),
        1,
        "native OSV attempt must produce a dimension"
    );
    assert_eq!(
        native_dims[0].subquery_id.as_deref(),
        Some("advisory_by_cve")
    );

    let kev_dims: Vec<_> = summary
        .dimensions
        .iter()
        .filter(|d| d.provider_id.as_deref() == Some("cisa_kev"))
        .collect();
    assert_eq!(kev_dims.len(), 1, "KEV attempt must produce a dimension");
    assert_eq!(kev_dims[0].subquery_id.as_deref(), Some("kev_by_cve"));

    assert_eq!(summary.attempted_job_count, Some(3));
    assert_eq!(summary.completed_job_count, Some(3));
}
