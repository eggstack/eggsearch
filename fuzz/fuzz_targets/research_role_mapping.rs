#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::evidence_role::EvidenceRole;
use eggsearch::core::research::ResearchSourceType;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let variant_count = 13;
    let variant_idx = (data[0] as usize) % variant_count;

    let rst = match variant_idx {
        0 => ResearchSourceType::PrimarySources,
        1 => ResearchSourceType::OfficialDocs,
        2 => ResearchSourceType::Specifications,
        3 => ResearchSourceType::ReferenceImplementations,
        4 => ResearchSourceType::DesignDiscussions,
        5 => ResearchSourceType::Benchmarks,
        6 => ResearchSourceType::SecurityConsiderations,
        7 => ResearchSourceType::IssueThreads,
        8 => ResearchSourceType::ReleaseNotes,
        9 => ResearchSourceType::AcademicOrFormalSources,
        10 => ResearchSourceType::RecentNews,
        11 => ResearchSourceType::CommunityDiscussion,
        12 => ResearchSourceType::Counterpoints,
        _ => return,
    };

    let role = EvidenceRole::from_research_source_type(rst);

    assert!(
        !format!("{:?}", role).is_empty(),
        "role must have a valid debug representation for {:?}", rst
    );

    let role2 = EvidenceRole::from_research_source_type(rst);
    assert_eq!(role, role2, "from_research_source_type must be deterministic for {:?}", rst);
});
