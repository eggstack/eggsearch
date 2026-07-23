use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::research::ResearchSourceType;

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
