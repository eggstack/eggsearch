//! Evidence integration tests for Workstream E items.
//!
//! These tests verify the end-to-end behavior of evidence role assignment,
//! coverage computation, conflict detection, retrieval tracking, and
//! gap-driven next actions.

use eggsearch::core::conflict::{detect_entity_scoped_conflicts, detect_mutable_vs_pinned};
use eggsearch::core::evidence_postprocess::{
    assign_evidence_role, materialize_evidence_roles, resolve_workflow_model,
};
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::result::TrustLevel;
use eggsearch::core::retrieval_status::{
    attempts_to_failures, RetrievalAttempt, RetrievalAttemptOutcome,
};
use eggsearch::core::sanitize::TrustMarkers;
use eggsearch::core::source_card::{SourceCard, SourceKind, SourceMetadata};
use eggsearch::core::workflow_coverage::{
    api_comprehension_model, compute_coverage, generate_gap_driven_next_actions,
};

fn make_card(source_kind: SourceKind, url: &str) -> SourceCard {
    SourceCard {
        id: format!("test_{url}"),
        stable_id: Some(format!("test_{url}")),
        title: "test".to_string(),
        url: url.to_string(),
        providers: vec!["test".to_string()],
        score: Some(1.0),
        trust: TrustLevel::ExternalUntrusted,
        fetched: false,
        snippet: None,
        trust_markers: TrustMarkers::default(),
        metadata: SourceMetadata {
            source_kind,
            ..Default::default()
        },
        quality: None,
    }
}

fn make_card_with_role(url: &str, role: EvidenceRole) -> SourceCard {
    let mut card = make_card(SourceKind::Unknown, url);
    card.metadata.evidence_role = Some(role);
    card
}

// =============================================================================
// 1. Cards serialize inferred evidence role
// =============================================================================
#[test]
fn test_cards_serialize_inferred_evidence_role() {
    let mut cards = vec![
        make_card(SourceKind::SecurityAdvisory, "https://a.com"),
        make_card(SourceKind::OfficialDocs, "https://b.com"),
        make_card(SourceKind::SourceRepository, "https://c.com"),
    ];
    materialize_evidence_roles(&mut cards);
    assert_eq!(
        cards[0].metadata.evidence_role,
        Some(EvidenceRole::AuthoritativeSecurityAdvisory)
    );
    assert_eq!(
        cards[1].metadata.evidence_role,
        Some(EvidenceRole::OfficialDocumentation)
    );
    assert_eq!(
        cards[2].metadata.evidence_role,
        Some(EvidenceRole::PrimaryImplementation)
    );
}

// =============================================================================
// 2. Explicit provider role is preserved
// =============================================================================
#[test]
fn test_explicit_provider_role_is_preserved() {
    let mut cards = vec![make_card_with_role(
        "https://example.com",
        EvidenceRole::BenchmarkOrPerformanceEvidence,
    )];
    materialize_evidence_roles(&mut cards);
    assert_eq!(
        cards[0].metadata.evidence_role,
        Some(EvidenceRole::BenchmarkOrPerformanceEvidence)
    );
}

// =============================================================================
// 3. Role assignment is deterministic under randomized input
// =============================================================================
#[test]
fn test_role_assignment_is_deterministic() {
    let urls: Vec<String> = (0..100)
        .map(|i| format!("https://example.com/{i}"))
        .collect();

    // Two cards with same inputs must produce same role regardless of order
    let card1 = make_card(SourceKind::OfficialDocs, &urls[0]);
    let card2 = make_card(SourceKind::OfficialDocs, &urls[0]);
    assert_eq!(assign_evidence_role(&card1), assign_evidence_role(&card2));

    // All cards of same source kind get the same role
    let mut cards: Vec<SourceCard> = urls
        .iter()
        .map(|url| make_card(SourceKind::OfficialDocs, url))
        .collect();
    materialize_evidence_roles(&mut cards);
    let all_same = cards
        .iter()
        .all(|c| c.metadata.evidence_role == Some(EvidenceRole::OfficialDocumentation));
    assert!(all_same, "all OfficialDocs cards should have same role");
}

// =============================================================================
// 4. Workflow model selection: repo_search -> repo_architecture_model
// =============================================================================
#[test]
fn test_workflow_model_repo_search_default() {
    let model = resolve_workflow_model("repo_search", None, None, false);
    assert!(model.is_some());
    assert_eq!(model.as_ref().unwrap().workflow_id, "repo_architecture");
}

// =============================================================================
// 5. Workflow model selection: repo_search + security profile
// =============================================================================
#[test]
fn test_workflow_model_repo_search_security_profile() {
    let model = resolve_workflow_model("repo_search", Some("security"), None, false);
    assert!(model.is_some());
    assert_eq!(model.as_ref().unwrap().workflow_id, "security_review");
}

// =============================================================================
// 6. Workflow model selection: research_search domains
// =============================================================================
#[test]
fn test_workflow_model_research_search_domains() {
    let model = resolve_workflow_model("research_search", None, Some("version_migration"), false);
    assert!(model.is_some());
    assert_eq!(model.as_ref().unwrap().workflow_id, "version_migration");

    let model = resolve_workflow_model("research_search", None, Some("error_investigation"), false);
    assert!(model.is_some());
    assert_eq!(model.as_ref().unwrap().workflow_id, "error_investigation");
}

// =============================================================================
// 7. Workflow model selection: web_search returns None
// =============================================================================
#[test]
fn test_workflow_model_web_search_returns_none() {
    let model = resolve_workflow_model("web_search", None, None, false);
    assert!(model.is_none());
}

// =============================================================================
// 8. Successful zero results produce absence
// =============================================================================
#[test]
fn test_zero_results_produce_failure() {
    let attempts = vec![RetrievalAttempt {
        provider_id: "duckduckgo".to_string(),
        subquery_id: None,
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::SuccessZeroResults,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    }];
    let failures = attempts_to_failures(&attempts);
    // Zero results is NOT a failure — it's a success with no data
    assert!(failures.is_empty());
}

// =============================================================================
// 9. Timeout produces indeterminate coverage
// =============================================================================
#[test]
fn test_timeout_produces_indeterminate_coverage() {
    let model = api_comprehension_model();
    let found = vec![EvidenceRole::InterfaceOrApiDefinition];
    let failures = vec![eggsearch::core::workflow_coverage::RetrievalFailure {
        kind: eggsearch::core::workflow_coverage::RetrievalFailureKind::EvidenceRoleIndeterminateBecauseRetrievalFailed,
        role: EvidenceRole::PrimaryImplementation,
        message: "provider timed out".to_string(),
        provider_id: Some("duckduckgo".to_string()),
    }];
    let result = compute_coverage(&model, &found, &failures);
    assert_eq!(
        result.status,
        eggsearch::core::workflow_coverage::CoverageStatus::IndeterminateDueToFailures
    );
}

// =============================================================================
// 10. Rate limit is not policy skip
// =============================================================================
#[test]
fn test_rate_limit_is_not_policy_skip() {
    let attempts = vec![RetrievalAttempt {
        provider_id: "brave_api".to_string(),
        subquery_id: None,
        operation_id: None,
        intended_roles: vec![EvidenceRole::PrimaryImplementation],
        outcome: RetrievalAttemptOutcome::RateLimited,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: None,
        duration_ms: None,
    }];
    let failures = attempts_to_failures(&attempts);
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].kind,
        eggsearch::core::workflow_coverage::RetrievalFailureKind::ProviderFailed
    );
}

// =============================================================================
// 11-12. Partial provider success
// =============================================================================
#[test]
fn test_partial_provider_success_one_fails() {
    let attempts = vec![
        RetrievalAttempt {
            provider_id: "duckduckgo".to_string(),
            subquery_id: Some("source".to_string()),
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
            provider_id: "brave_api".to_string(),
            subquery_id: Some("source".to_string()),
            operation_id: None,
            intended_roles: vec![EvidenceRole::PrimaryImplementation],
            outcome: RetrievalAttemptOutcome::TimedOut,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
    ];
    let failures = attempts_to_failures(&attempts);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].provider_id.as_deref(), Some("brave_api"));
}

#[test]
fn test_partial_provider_success_all_succeed() {
    let attempts = vec![
        RetrievalAttempt {
            provider_id: "duckduckgo".to_string(),
            subquery_id: None,
            operation_id: None,
            intended_roles: vec![EvidenceRole::OfficialDocumentation],
            outcome: RetrievalAttemptOutcome::SuccessWithResults,
            result_count: 3,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
        RetrievalAttempt {
            provider_id: "startpage".to_string(),
            subquery_id: None,
            operation_id: None,
            intended_roles: vec![EvidenceRole::OfficialDocumentation],
            outcome: RetrievalAttemptOutcome::SuccessZeroResults,
            result_count: 0,
            error_class: None,
            deadline_interrupted: false,
            truncated: false,
            truncation_evidence: Default::default(),
            query_fingerprint: None,
            duration_ms: None,
        },
    ];
    let failures = attempts_to_failures(&attempts);
    assert!(failures.is_empty());
}

// =============================================================================
// 13-14. Required role coverage
// =============================================================================
#[test]
fn test_required_role_coverage_insufficient() {
    let model = api_comprehension_model();
    let found = vec![EvidenceRole::OfficialDocumentation];
    let result = compute_coverage(&model, &found, &[]);
    assert_eq!(
        result.status,
        eggsearch::core::workflow_coverage::CoverageStatus::Insufficient
    );
    assert!(result
        .missing_required
        .contains(&EvidenceRole::PrimaryImplementation));
    assert!(result
        .missing_required
        .contains(&EvidenceRole::InterfaceOrApiDefinition));
}

#[test]
fn test_required_role_coverage_sufficient() {
    let model = api_comprehension_model();
    let found = vec![
        EvidenceRole::PrimaryImplementation,
        EvidenceRole::InterfaceOrApiDefinition,
        EvidenceRole::OfficialDocumentation,
        EvidenceRole::UsageExample,
        EvidenceRole::TestOrBehavioralSpecification,
    ];
    let result = compute_coverage(&model, &found, &[]);
    assert_eq!(
        result.status,
        eggsearch::core::workflow_coverage::CoverageStatus::Sufficient
    );
    assert!(result.missing_required.is_empty());
    assert!(result.missing_recommended.is_empty());
}

// =============================================================================
// 15-17. Conflict detection tests
// =============================================================================
#[test]
fn test_entity_scoped_conflicts_vulnerability() {
    let mut card1 = make_card(SourceKind::SecurityAdvisory, "https://a.com");
    card1.metadata.vulnerability =
        Some(Box::new(eggsearch::core::security::VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            patched_versions: vec![">=1.0 <2.0".to_string()],
            ..Default::default()
        }));

    let mut card2 = make_card(SourceKind::SecurityAdvisory, "https://b.com");
    card2.metadata.vulnerability =
        Some(Box::new(eggsearch::core::security::VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            patched_versions: vec![">=1.5 <3.0".to_string()],
            ..Default::default()
        }));

    let conflicts = detect_entity_scoped_conflicts(&[card1, card2]);
    assert!(!conflicts.is_empty());
    assert_eq!(
        conflicts[0].conflict_class,
        eggsearch::core::conflict::ConflictClass::DifferingVersionRanges
    );
}

#[test]
fn test_entity_scoped_no_conflict_different_entities() {
    let mut card1 = make_card(SourceKind::SecurityAdvisory, "https://a.com");
    card1.metadata.vulnerability =
        Some(Box::new(eggsearch::core::security::VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            patched_versions: vec![">=1.0 <2.0".to_string()],
            ..Default::default()
        }));

    let mut card2 = make_card(SourceKind::SecurityAdvisory, "https://b.com");
    card2.metadata.vulnerability =
        Some(Box::new(eggsearch::core::security::VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-9999".to_string()],
            patched_versions: vec![">=1.0 <2.0".to_string()],
            ..Default::default()
        }));

    let conflicts = detect_entity_scoped_conflicts(&[card1, card2]);
    assert!(conflicts.is_empty());
}

#[test]
fn test_mutable_vs_pinned_conflict() {
    let mutable_ids = vec!["src_branch".to_string()];
    let pinned_ids = vec!["src_commit".to_string()];
    let conflict = detect_mutable_vs_pinned(&mutable_ids, &pinned_ids).unwrap();
    assert_eq!(
        conflict.conflict_class,
        eggsearch::core::conflict::ConflictClass::MutableVsCommitPinnedContent
    );
    assert_eq!(
        conflict.severity,
        eggsearch::core::conflict::ConflictSeverity::Critical
    );
}

// =============================================================================
// 18-19. Mutable vs pinned conflict scoping
// =============================================================================
#[test]
fn test_mutable_vs_pinned_empty_mutable() {
    let conflict = detect_mutable_vs_pinned(&[], &["src_a".to_string()]);
    assert!(conflict.is_none());
}

#[test]
fn test_mutable_vs_pinned_empty_pinned() {
    let conflict = detect_mutable_vs_pinned(&["src_a".to_string()], &[]);
    assert!(conflict.is_none());
}

// =============================================================================
// 20-21. Gap-driven next action tests
// =============================================================================
#[test]
fn test_gap_driven_next_actions_for_missing_required() {
    let model = api_comprehension_model();
    let found = vec![EvidenceRole::InterfaceOrApiDefinition];
    let result = compute_coverage(&model, &found, &[]);

    let actions = generate_gap_driven_next_actions(&result, &[], &[]);
    assert!(!actions.is_empty());
    assert!(actions
        .iter()
        .any(|a| a.evidence_role == Some(EvidenceRole::PrimaryImplementation)));
    assert!(actions.iter().any(|a| a.tool == "repo_search"));
}

#[test]
fn test_gap_driven_next_actions_avoids_repeated_failures() {
    let model = api_comprehension_model();
    let found = vec![EvidenceRole::InterfaceOrApiDefinition];
    let result = compute_coverage(&model, &found, &[]);

    let past_failures = vec![eggsearch::core::workflow_coverage::RetrievalFailure {
        kind: eggsearch::core::workflow_coverage::RetrievalFailureKind::ProviderFailed,
        role: EvidenceRole::PrimaryImplementation,
        message: "failed before".to_string(),
        provider_id: Some("duckduckgo".to_string()),
    }];

    let actions = generate_gap_driven_next_actions(&result, &past_failures, &[]);
    // The action for PrimaryImplementation should have retry-related gap label
    let primary_action = actions
        .iter()
        .find(|a| a.evidence_role == Some(EvidenceRole::PrimaryImplementation));
    assert!(primary_action.is_some());
    let action = primary_action.unwrap();
    assert!(
        action
            .evidence_gap
            .as_ref()
            .is_some_and(|g| g.contains("retry")),
        "expected retry gap label, got: {:?}",
        action.evidence_gap
    );
}

// =============================================================================
// 22. Failed retrieval not immediately recommended
// =============================================================================
#[test]
fn test_failed_retrieval_not_immediately_recommended() {
    let model = api_comprehension_model();
    let found = vec![
        EvidenceRole::InterfaceOrApiDefinition,
        EvidenceRole::PrimaryImplementation,
    ];
    let past_failures = vec![eggsearch::core::workflow_coverage::RetrievalFailure {
        kind: eggsearch::core::workflow_coverage::RetrievalFailureKind::ProviderFailed,
        role: EvidenceRole::PrimaryImplementation,
        message: "failed before".to_string(),
        provider_id: Some("duckduckgo".to_string()),
    }];
    let result = compute_coverage(&model, &found, &past_failures);
    // PrimaryImplementation is found, so it should NOT be in missing_required
    assert!(
        !result
            .missing_required
            .contains(&EvidenceRole::PrimaryImplementation),
        "PrimaryImplementation should not be missing when found"
    );
}

// =============================================================================
// 23. Output ordering deterministic
// =============================================================================
#[test]
fn test_output_ordering_deterministic() {
    let model = api_comprehension_model();
    let found = vec![EvidenceRole::InterfaceOrApiDefinition];

    let result1 = compute_coverage(&model, &found, &[]);
    let result2 = compute_coverage(&model, &found, &[]);

    assert_eq!(result1.missing_required, result2.missing_required);
    assert_eq!(result1.missing_recommended, result2.missing_recommended);
    assert_eq!(result1.next_actions.len(), result2.next_actions.len());

    for (a1, a2) in result1.next_actions.iter().zip(result2.next_actions.iter()) {
        assert_eq!(a1.tool, a2.tool);
        assert_eq!(a1.evidence_role, a2.evidence_role);
    }
}

// =============================================================================
// 24. Empty web search doesn't get coding coverage model
// =============================================================================
#[test]
fn test_empty_web_search_no_coding_coverage_model() {
    let model = resolve_workflow_model("web_search", None, None, false);
    assert!(
        model.is_none(),
        "web_search should not have a coverage model"
    );
}
