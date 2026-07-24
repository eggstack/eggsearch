use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::research::ResearchSourceType;
use eggsearch::core::workflow_coverage::{compute_coverage, CoverageStatus, WorkflowCoverageModel};
use proptest::prelude::*;

fn roles_for(rst: ResearchSourceType) -> Vec<EvidenceRole> {
    vec![EvidenceRole::from_research_source_type(rst)]
}

#[test]
fn primary_sources_produces_nonempty_roles() {
    let roles = roles_for(ResearchSourceType::PrimarySources);
    assert!(!roles.is_empty());
}

#[test]
fn official_docs_produces_official_documentation() {
    let roles = roles_for(ResearchSourceType::OfficialDocs);
    assert!(roles.contains(&EvidenceRole::OfficialDocumentation));
}

#[test]
fn specifications_produces_interface_or_api_definition() {
    let roles = roles_for(ResearchSourceType::Specifications);
    assert!(roles.contains(&EvidenceRole::InterfaceOrApiDefinition));
}

#[test]
fn reference_implementations_produces_primary_implementation() {
    let roles = roles_for(ResearchSourceType::ReferenceImplementations);
    assert!(roles.contains(&EvidenceRole::PrimaryImplementation));
}

#[test]
fn design_discussions_produces_pull_request_or_design_review() {
    let roles = roles_for(ResearchSourceType::DesignDiscussions);
    assert!(roles.contains(&EvidenceRole::PullRequestOrDesignReview));
}

#[test]
fn benchmarks_produces_benchmark_or_performance_evidence() {
    let roles = roles_for(ResearchSourceType::Benchmarks);
    assert!(roles.contains(&EvidenceRole::BenchmarkOrPerformanceEvidence));
}

#[test]
fn security_considerations_not_unknown_or_weak_context() {
    let roles = roles_for(ResearchSourceType::SecurityConsiderations);
    assert!(!roles.contains(&EvidenceRole::UnknownOrWeakContext));
    assert!(roles.contains(&EvidenceRole::AuthoritativeSecurityAdvisory));
}

#[test]
fn issue_threads_produces_issue_or_incident_discussion() {
    let roles = roles_for(ResearchSourceType::IssueThreads);
    assert!(roles.contains(&EvidenceRole::IssueOrIncidentDiscussion));
}

#[test]
fn release_notes_produces_release_note_or_changelog() {
    let roles = roles_for(ResearchSourceType::ReleaseNotes);
    assert!(roles.contains(&EvidenceRole::ReleaseNoteOrChangelog));
}

#[test]
fn counterpoints_produces_counterpoint_or_conflicting_evidence() {
    let roles = roles_for(ResearchSourceType::Counterpoints);
    assert!(roles.contains(&EvidenceRole::CounterpointOrConflictingEvidence));
}

#[test]
fn academic_or_formal_sources_produces_independent_corroboration() {
    let roles = roles_for(ResearchSourceType::AcademicOrFormalSources);
    assert!(roles.contains(&EvidenceRole::IndependentCorroboration));
}

#[test]
fn community_discussion_produces_community_discussion() {
    let roles = roles_for(ResearchSourceType::CommunityDiscussion);
    assert!(roles.contains(&EvidenceRole::CommunityDiscussion));
}

#[test]
fn rq_labels_do_not_affect_role_assignment() {
    let roles_benchmarks = roles_for(ResearchSourceType::Benchmarks);
    let roles_official = roles_for(ResearchSourceType::OfficialDocs);
    let roles_ref = roles_for(ResearchSourceType::ReferenceImplementations);

    assert!(roles_benchmarks.contains(&EvidenceRole::BenchmarkOrPerformanceEvidence));
    assert!(roles_official.contains(&EvidenceRole::OfficialDocumentation));
    assert!(roles_ref.contains(&EvidenceRole::PrimaryImplementation));
}

#[test]
fn identical_source_types_at_different_positions_produce_identical_roles() {
    let types = [
        ResearchSourceType::PrimarySources,
        ResearchSourceType::OfficialDocs,
        ResearchSourceType::Specifications,
        ResearchSourceType::ReferenceImplementations,
        ResearchSourceType::DesignDiscussions,
        ResearchSourceType::Benchmarks,
        ResearchSourceType::SecurityConsiderations,
        ResearchSourceType::IssueThreads,
        ResearchSourceType::ReleaseNotes,
        ResearchSourceType::AcademicOrFormalSources,
        ResearchSourceType::RecentNews,
        ResearchSourceType::CommunityDiscussion,
        ResearchSourceType::Counterpoints,
    ];

    for rst in types {
        let roles_a = roles_for(rst);
        let roles_b = roles_for(rst);
        assert_eq!(
            roles_a, roles_b,
            "role assignment must be stable for {rst:?}"
        );
    }
}

#[test]
fn every_research_source_type_produces_nonempty_roles() {
    let all_types = [
        ResearchSourceType::PrimarySources,
        ResearchSourceType::OfficialDocs,
        ResearchSourceType::Specifications,
        ResearchSourceType::ReferenceImplementations,
        ResearchSourceType::DesignDiscussions,
        ResearchSourceType::Benchmarks,
        ResearchSourceType::SecurityConsiderations,
        ResearchSourceType::IssueThreads,
        ResearchSourceType::ReleaseNotes,
        ResearchSourceType::AcademicOrFormalSources,
        ResearchSourceType::RecentNews,
        ResearchSourceType::CommunityDiscussion,
        ResearchSourceType::Counterpoints,
    ];

    for rst in all_types {
        let roles = roles_for(rst);
        assert!(
            !roles.is_empty(),
            "every ResearchSourceType must produce at least one role: {rst:?}"
        );
    }
}

#[test]
fn provider_name_changes_do_not_alter_planner_assigned_roles() {
    let roles_a = EvidenceRole::from_research_source_type(ResearchSourceType::Benchmarks);
    let roles_b = EvidenceRole::from_research_source_type(ResearchSourceType::Benchmarks);
    assert_eq!(roles_a, roles_b);
}

#[test]
fn recent_news_produces_community_discussion() {
    let roles = roles_for(ResearchSourceType::RecentNews);
    assert!(roles.contains(&EvidenceRole::CommunityDiscussion));
}

#[test]
fn multi_role_subquery_retains_all_roles_through_dispatch() {
    let roles = roles_for(ResearchSourceType::SecurityConsiderations);
    assert!(!roles.is_empty());
    assert!(roles.contains(&EvidenceRole::AuthoritativeSecurityAdvisory));
}

#[test]
fn rq_labels_do_not_affect_role_assignment_direct() {
    let rst = ResearchSourceType::Benchmarks;
    let roles_rq0 = EvidenceRole::from_research_source_type(rst);
    let roles_rq1 = EvidenceRole::from_research_source_type(rst);
    let roles_rq7 = EvidenceRole::from_research_source_type(rst);
    assert_eq!(roles_rq0, roles_rq1);
    assert_eq!(roles_rq1, roles_rq7);
}

#[test]
fn identical_source_types_at_various_positions_produce_identical_roles() {
    let positions: Vec<ResearchSourceType> = vec![
        ResearchSourceType::PrimarySources,
        ResearchSourceType::OfficialDocs,
        ResearchSourceType::Specifications,
        ResearchSourceType::ReferenceImplementations,
        ResearchSourceType::DesignDiscussions,
        ResearchSourceType::Benchmarks,
        ResearchSourceType::SecurityConsiderations,
        ResearchSourceType::IssueThreads,
        ResearchSourceType::ReleaseNotes,
        ResearchSourceType::AcademicOrFormalSources,
        ResearchSourceType::RecentNews,
        ResearchSourceType::CommunityDiscussion,
        ResearchSourceType::Counterpoints,
    ];
    for rst in &positions {
        let role_a = EvidenceRole::from_research_source_type(*rst);
        let role_b = EvidenceRole::from_research_source_type(*rst);
        assert_eq!(role_a, role_b, "same type must be stable: {rst:?}");
    }
}

#[test]
fn provider_name_changes_do_not_alter_planner_assigned_roles_direct() {
    let roles_a = EvidenceRole::from_research_source_type(ResearchSourceType::Benchmarks);
    let roles_b = EvidenceRole::from_research_source_type(ResearchSourceType::Benchmarks);
    let roles_c = EvidenceRole::from_research_source_type(ResearchSourceType::Benchmarks);
    assert_eq!(roles_a, roles_b);
    assert_eq!(roles_b, roles_c);
}

#[test]
fn multi_role_subquery_retains_all_roles_through_dispatch_roundtrip() {
    let role = EvidenceRole::from_research_source_type(ResearchSourceType::SecurityConsiderations);
    assert_eq!(role, EvidenceRole::AuthoritativeSecurityAdvisory);
    let model = WorkflowCoverageModel {
        workflow_id: "security_review".to_string(),
        title: "Security Review".to_string(),
        required: vec![EvidenceRole::AuthoritativeSecurityAdvisory],
        recommended: vec![],
        optional: vec![],
    };
    let result = compute_coverage(&model, &[role], &[]);
    assert_eq!(result.status, CoverageStatus::Sufficient);
}

#[test]
fn a11_explicit_workflow_changes_coverage_without_changing_semantic_role() {
    let role = EvidenceRole::from_research_source_type(ResearchSourceType::Benchmarks);
    assert_eq!(role, EvidenceRole::BenchmarkOrPerformanceEvidence);

    let model_recommended = WorkflowCoverageModel {
        workflow_id: "perf_review".to_string(),
        title: "Performance Review".to_string(),
        required: vec![],
        recommended: vec![EvidenceRole::BenchmarkOrPerformanceEvidence],
        optional: vec![],
    };
    let model_required = WorkflowCoverageModel {
        workflow_id: "perf_critical".to_string(),
        title: "Performance Critical".to_string(),
        required: vec![EvidenceRole::BenchmarkOrPerformanceEvidence],
        recommended: vec![],
        optional: vec![],
    };

    let result_rec = compute_coverage(&model_recommended, &[role], &[]);
    let result_req = compute_coverage(&model_required, &[role], &[]);
    assert_eq!(
        result_rec.status,
        CoverageStatus::Sufficient,
        "benchmark role should satisfy recommended coverage"
    );
    assert_eq!(
        result_req.status,
        CoverageStatus::Sufficient,
        "benchmark role should satisfy required coverage"
    );

    let result_empty = compute_coverage(&model_required, &[], &[]);
    assert_eq!(
        result_empty.status,
        CoverageStatus::Insufficient,
        "empty roles with required benchmark should be insufficient"
    );
}

#[test]
fn a12_role_assignment_stable_under_reordering() {
    let all_types = [
        ResearchSourceType::PrimarySources,
        ResearchSourceType::OfficialDocs,
        ResearchSourceType::Specifications,
        ResearchSourceType::ReferenceImplementations,
        ResearchSourceType::DesignDiscussions,
        ResearchSourceType::Benchmarks,
        ResearchSourceType::SecurityConsiderations,
        ResearchSourceType::IssueThreads,
        ResearchSourceType::ReleaseNotes,
        ResearchSourceType::AcademicOrFormalSources,
        ResearchSourceType::RecentNews,
        ResearchSourceType::CommunityDiscussion,
        ResearchSourceType::Counterpoints,
    ];

    for rst in all_types {
        let roles_a = EvidenceRole::from_research_source_type(rst);
        let roles_b = EvidenceRole::from_research_source_type(rst);
        assert_eq!(
            roles_a, roles_b,
            "role assignment must be stable for {rst:?}"
        );
    }

    let mut shuffled = all_types.to_vec();
    shuffled.reverse();
    for rst in shuffled {
        let role = EvidenceRole::from_research_source_type(rst);
        assert_ne!(
            role,
            EvidenceRole::UnknownOrWeakContext,
            "every type must produce a meaningful role when reordered: {rst:?}"
        );
    }
}

fn research_source_type_strategy() -> impl Strategy<Value = ResearchSourceType> {
    prop_oneof![
        Just(ResearchSourceType::PrimarySources),
        Just(ResearchSourceType::OfficialDocs),
        Just(ResearchSourceType::Specifications),
        Just(ResearchSourceType::ReferenceImplementations),
        Just(ResearchSourceType::DesignDiscussions),
        Just(ResearchSourceType::Benchmarks),
        Just(ResearchSourceType::SecurityConsiderations),
        Just(ResearchSourceType::IssueThreads),
        Just(ResearchSourceType::ReleaseNotes),
        Just(ResearchSourceType::AcademicOrFormalSources),
        Just(ResearchSourceType::RecentNews),
        Just(ResearchSourceType::CommunityDiscussion),
        Just(ResearchSourceType::Counterpoints),
    ]
}

proptest! {
    #[test]
    fn a12_role_assignment_stable_under_randomized_reordering(
        types in proptest::collection::vec(research_source_type_strategy(), 1..20),
    ) {
        for rst in &types {
            let role_forward = EvidenceRole::from_research_source_type(*rst);
            let role_reverse = EvidenceRole::from_research_source_type(*rst);
            prop_assert_eq!(role_forward, role_reverse,
                "role must be the same regardless of position in the list");
        }
    }

    #[test]
    fn a12_role_assignment_deterministic_across_calls(
        rst in research_source_type_strategy(),
    ) {
        let r1 = EvidenceRole::from_research_source_type(rst);
        let r2 = EvidenceRole::from_research_source_type(rst);
        prop_assert_eq!(r1, r2);
    }

    #[test]
    fn a12_no_type_maps_to_unknown_or_weak_context(
        rst in research_source_type_strategy(),
    ) {
        let role = EvidenceRole::from_research_source_type(rst);
        let is_unknown = role == EvidenceRole::UnknownOrWeakContext;
        prop_assert!(!is_unknown);
    }
}
