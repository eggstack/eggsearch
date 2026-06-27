//! Research search planner: generates bounded subqueries from a research request.

use crate::core::query::SearchIntent;
use crate::core::research::{
    ResearchDomain, ResearchSearchRequest, ResearchSourceType, ResearchSubquery,
};

/// Complete plan for a research search.
#[derive(Clone, Debug)]
pub struct ResearchSearchPlan {
    /// Resolved research domain.
    pub domain: ResearchDomain,
    /// The generated subqueries (max 8).
    pub subqueries: Vec<ResearchSubquery>,
}

/// Maximum number of subqueries in a research plan.
const MAX_SUBQUERIES: usize = 8;

/// Priority order for subquery selection when capping at MAX_SUBQUERIES.
/// Lower index = higher priority.
const PRIORITY_ORDER: &[ResearchSourceType] = &[
    ResearchSourceType::PrimarySources,
    ResearchSourceType::OfficialDocs,
    ResearchSourceType::Specifications,
    ResearchSourceType::ReferenceImplementations,
    ResearchSourceType::SecurityConsiderations,
    ResearchSourceType::DesignDiscussions,
    ResearchSourceType::Counterpoints,
];

/// Build a research search plan from a request.
pub fn build_research_search_plan(req: &ResearchSearchRequest) -> ResearchSearchPlan {
    let domain = req.research_domain.unwrap_or_default();
    let freshness = req.freshness;

    let mut source_types: Vec<ResearchSourceType> = req.desired_source_types.clone();

    if source_types.is_empty() {
        source_types = default_source_types();
    }

    if req.include_primary_sources == Some(true)
        && !source_types.contains(&ResearchSourceType::PrimarySources)
    {
        source_types.push(ResearchSourceType::PrimarySources);
    }

    if req.include_counterpoints == Some(true)
        && !source_types.contains(&ResearchSourceType::Counterpoints)
    {
        source_types.push(ResearchSourceType::Counterpoints);
    }

    if req.include_recent_discussion == Some(true)
        && !source_types.contains(&ResearchSourceType::RecentNews)
    {
        source_types.push(ResearchSourceType::RecentNews);
    }

    if req.include_security_considerations == Some(true)
        && !source_types.contains(&ResearchSourceType::SecurityConsiderations)
    {
        source_types.push(ResearchSourceType::SecurityConsiderations);
    }

    source_types.dedup();

    if source_types.len() > MAX_SUBQUERIES {
        source_types = prioritize_source_types(&source_types);
    }

    source_types.truncate(MAX_SUBQUERIES);

    let subqueries: Vec<ResearchSubquery> = source_types
        .iter()
        .enumerate()
        .map(|(i, source_type)| {
            let query = build_query_for_source_type(*source_type, &req.query);
            let intent = intent_for_source_type(*source_type);
            ResearchSubquery {
                id: format!("rq_{i}"),
                source_type: *source_type,
                query,
                intent,
                freshness,
            }
        })
        .collect();

    ResearchSearchPlan { domain, subqueries }
}

/// Default source types when none are specified.
fn default_source_types() -> Vec<ResearchSourceType> {
    vec![
        ResearchSourceType::PrimarySources,
        ResearchSourceType::OfficialDocs,
        ResearchSourceType::ReferenceImplementations,
        ResearchSourceType::DesignDiscussions,
        ResearchSourceType::SecurityConsiderations,
        ResearchSourceType::Counterpoints,
    ]
}

/// Prioritize source types to fit within MAX_SUBQUERIES.
fn prioritize_source_types(source_types: &[ResearchSourceType]) -> Vec<ResearchSourceType> {
    let mut result: Vec<ResearchSourceType> = Vec::new();

    for &priority_type in PRIORITY_ORDER {
        if result.len() >= MAX_SUBQUERIES {
            break;
        }
        if source_types.contains(&priority_type) {
            result.push(priority_type);
        }
    }

    for &st in source_types {
        if result.len() >= MAX_SUBQUERIES {
            break;
        }
        if !result.contains(&st) {
            result.push(st);
        }
    }

    result
}

/// Map a source type to the appropriate search intent.
fn intent_for_source_type(source_type: ResearchSourceType) -> SearchIntent {
    match source_type {
        ResearchSourceType::PrimarySources => SearchIntent::Docs,
        ResearchSourceType::OfficialDocs => SearchIntent::Docs,
        ResearchSourceType::Specifications => SearchIntent::Docs,
        ResearchSourceType::ReferenceImplementations => SearchIntent::Code,
        ResearchSourceType::DesignDiscussions => SearchIntent::Issues,
        ResearchSourceType::Benchmarks => SearchIntent::Web,
        ResearchSourceType::SecurityConsiderations => SearchIntent::Security,
        ResearchSourceType::IssueThreads => SearchIntent::Issues,
        ResearchSourceType::ReleaseNotes => SearchIntent::Releases,
        ResearchSourceType::AcademicOrFormalSources => SearchIntent::Web,
        ResearchSourceType::RecentNews => SearchIntent::News,
        ResearchSourceType::CommunityDiscussion => SearchIntent::Web,
        ResearchSourceType::Counterpoints => SearchIntent::Web,
    }
}

/// Build the query string for a specific source type.
fn build_query_for_source_type(source_type: ResearchSourceType, base_query: &str) -> String {
    let suffix = match source_type {
        ResearchSourceType::PrimarySources => "official docs source repository maintainer",
        ResearchSourceType::OfficialDocs => "official documentation API reference guide",
        ResearchSourceType::Specifications => "specification RFC standard protocol spec",
        ResearchSourceType::ReferenceImplementations => {
            "reference implementation github source code examples"
        }
        ResearchSourceType::DesignDiscussions => "design discussion proposal issue RFC discussion",
        ResearchSourceType::Benchmarks => "benchmark performance latency throughput comparison",
        ResearchSourceType::SecurityConsiderations => {
            "security considerations threat model vulnerability hardening"
        }
        ResearchSourceType::IssueThreads => {
            "issue discussion bug regression pull request github gitlab"
        }
        ResearchSourceType::ReleaseNotes => "release notes changelog migration breaking changes",
        ResearchSourceType::AcademicOrFormalSources => {
            "paper formal analysis arxiv conference proceedings"
        }
        ResearchSourceType::RecentNews => "recent update announcement news",
        ResearchSourceType::CommunityDiscussion => {
            "discussion forum reddit stack overflow users experience"
        }
        ResearchSourceType::Counterpoints => {
            "drawbacks limitations tradeoffs criticism alternatives"
        }
    };
    format!("{base_query} {suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::query::Freshness;

    fn base_request() -> ResearchSearchRequest {
        ResearchSearchRequest {
            query: "Raft consensus algorithm".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn primary_sources_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::PrimarySources],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        assert_eq!(plan.subqueries.len(), 1);
        let sq = &plan.subqueries[0];
        assert_eq!(sq.source_type, ResearchSourceType::PrimarySources);
        assert!(sq.query.contains("official docs"));
        assert!(sq.query.contains("source repository"));
        assert!(sq.query.contains("maintainer"));
    }

    #[test]
    fn official_docs_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::OfficialDocs],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("official documentation"));
        assert!(sq.query.contains("API reference"));
        assert!(sq.query.contains("guide"));
    }

    #[test]
    fn specifications_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::Specifications],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("specification"));
        assert!(sq.query.contains("RFC"));
        assert!(sq.query.contains("standard"));
    }

    #[test]
    fn reference_impl_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::ReferenceImplementations],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("reference implementation"));
        assert!(sq.query.contains("github"));
        assert!(sq.query.contains("source code"));
    }

    #[test]
    fn design_discussions_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::DesignDiscussions],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("design discussion"));
        assert!(sq.query.contains("proposal"));
        assert!(sq.query.contains("RFC discussion"));
    }

    #[test]
    fn benchmarks_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::Benchmarks],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("benchmark"));
        assert!(sq.query.contains("performance"));
        assert!(sq.query.contains("latency"));
    }

    #[test]
    fn security_generates_expected_query_and_intent() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::SecurityConsiderations],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("security considerations"));
        assert!(sq.query.contains("threat model"));
        assert!(sq.query.contains("hardening"));
        assert_eq!(sq.intent, SearchIntent::Security);
    }

    #[test]
    fn issue_threads_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::IssueThreads],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("issue discussion"));
        assert!(sq.query.contains("bug"));
        assert!(sq.query.contains("pull request"));
    }

    #[test]
    fn release_notes_generates_expected_query_and_intent() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::ReleaseNotes],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("release notes"));
        assert!(sq.query.contains("changelog"));
        assert!(sq.query.contains("migration"));
        assert!(sq.query.contains("breaking changes"));
        assert_eq!(sq.intent, SearchIntent::Releases);
    }

    #[test]
    fn academic_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::AcademicOrFormalSources],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("paper"));
        assert!(sq.query.contains("formal analysis"));
        assert!(sq.query.contains("arxiv"));
    }

    #[test]
    fn recent_news_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::RecentNews],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("recent update"));
        assert!(sq.query.contains("news"));
    }

    #[test]
    fn community_discussion_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::CommunityDiscussion],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("discussion"));
        assert!(sq.query.contains("reddit"));
        assert!(sq.query.contains("stack overflow"));
    }

    #[test]
    fn counterpoints_generates_expected_query() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::Counterpoints],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let sq = &plan.subqueries[0];
        assert!(sq.query.contains("drawbacks"));
        assert!(sq.query.contains("limitations"));
        assert!(sq.query.contains("tradeoffs"));
    }

    #[test]
    fn reference_impl_uses_code_intent() {
        let req = ResearchSearchRequest {
            query: "etcd".to_string(),
            desired_source_types: vec![ResearchSourceType::ReferenceImplementations],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        assert_eq!(plan.subqueries[0].intent, SearchIntent::Code);
    }

    #[test]
    fn counterpoints_generated_only_when_requested() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![
                ResearchSourceType::PrimarySources,
                ResearchSourceType::OfficialDocs,
            ],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let has_counterpoints = plan
            .subqueries
            .iter()
            .any(|sq| sq.source_type == ResearchSourceType::Counterpoints);
        assert!(!has_counterpoints);
    }

    #[test]
    fn counterpoints_included_when_desired() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![
                ResearchSourceType::PrimarySources,
                ResearchSourceType::Counterpoints,
            ],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let has_counterpoints = plan
            .subqueries
            .iter()
            .any(|sq| sq.source_type == ResearchSourceType::Counterpoints);
        assert!(has_counterpoints);
    }

    #[test]
    fn include_counterpoints_adds_it() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::PrimarySources],
            include_counterpoints: Some(true),
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let has_counterpoints = plan
            .subqueries
            .iter()
            .any(|sq| sq.source_type == ResearchSourceType::Counterpoints);
        assert!(has_counterpoints);
    }

    #[test]
    fn include_counterpoints_does_not_duplicate() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![
                ResearchSourceType::PrimarySources,
                ResearchSourceType::Counterpoints,
            ],
            include_counterpoints: Some(true),
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let count = plan
            .subqueries
            .iter()
            .filter(|sq| sq.source_type == ResearchSourceType::Counterpoints)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn include_security_considerations_adds_it() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::PrimarySources],
            include_security_considerations: Some(true),
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let has_security = plan
            .subqueries
            .iter()
            .any(|sq| sq.source_type == ResearchSourceType::SecurityConsiderations);
        assert!(has_security);
    }

    #[test]
    fn include_primary_sources_adds_it() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::OfficialDocs],
            include_primary_sources: Some(true),
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let has_primary = plan
            .subqueries
            .iter()
            .any(|sq| sq.source_type == ResearchSourceType::PrimarySources);
        assert!(has_primary);
    }

    #[test]
    fn include_recent_discussion_adds_it() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![ResearchSourceType::PrimarySources],
            include_recent_discussion: Some(true),
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let has_recent = plan
            .subqueries
            .iter()
            .any(|sq| sq.source_type == ResearchSourceType::RecentNews);
        assert!(has_recent);
    }

    #[test]
    fn query_expansion_bounded_at_8() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![
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
            ],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        assert!(plan.subqueries.len() <= MAX_SUBQUERIES);
    }

    #[test]
    fn default_source_types_when_none_specified() {
        let req = base_request();
        let plan = build_research_search_plan(&req);
        let source_types: Vec<ResearchSourceType> =
            plan.subqueries.iter().map(|sq| sq.source_type).collect();
        assert!(source_types.contains(&ResearchSourceType::PrimarySources));
        assert!(source_types.contains(&ResearchSourceType::OfficialDocs));
        assert!(source_types.contains(&ResearchSourceType::ReferenceImplementations));
        assert!(source_types.contains(&ResearchSourceType::DesignDiscussions));
        assert!(source_types.contains(&ResearchSourceType::SecurityConsiderations));
        assert!(source_types.contains(&ResearchSourceType::Counterpoints));
    }

    #[test]
    fn default_source_types_count_is_6() {
        let req = base_request();
        let plan = build_research_search_plan(&req);
        assert_eq!(plan.subqueries.len(), 6);
    }

    #[test]
    fn sequential_ids_assigned() {
        let req = base_request();
        let plan = build_research_search_plan(&req);
        for (i, sq) in plan.subqueries.iter().enumerate() {
            assert_eq!(sq.id, format!("rq_{i}"));
        }
    }

    #[test]
    fn freshness_applied_to_all_subqueries() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            freshness: Freshness::Week,
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        for sq in &plan.subqueries {
            assert_eq!(sq.freshness, Freshness::Week);
        }
    }

    #[test]
    fn domain_default_is_general() {
        let req = base_request();
        let plan = build_research_search_plan(&req);
        assert_eq!(plan.domain, ResearchDomain::General);
    }

    #[test]
    fn domain_from_request() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            research_domain: Some(ResearchDomain::DistributedSystems),
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        assert_eq!(plan.domain, ResearchDomain::DistributedSystems);
    }

    #[test]
    fn priority_order_preserved_when_capping() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![
                ResearchSourceType::CommunityDiscussion,
                ResearchSourceType::Benchmarks,
                ResearchSourceType::ReleaseNotes,
                ResearchSourceType::IssueThreads,
                ResearchSourceType::AcademicOrFormalSources,
                ResearchSourceType::PrimarySources,
                ResearchSourceType::OfficialDocs,
                ResearchSourceType::SecurityConsiderations,
                ResearchSourceType::Counterpoints,
            ],
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        assert_eq!(plan.subqueries.len(), MAX_SUBQUERIES);
        let first_type = plan.subqueries[0].source_type;
        assert!(
            PRIORITY_ORDER.contains(&first_type),
            "first subquery should be a priority type, got {first_type:?}"
        );
    }

    #[test]
    fn dedup_after_flag_expansion() {
        let req = ResearchSearchRequest {
            query: "Raft".to_string(),
            desired_source_types: vec![
                ResearchSourceType::PrimarySources,
                ResearchSourceType::SecurityConsiderations,
            ],
            include_primary_sources: Some(true),
            include_security_considerations: Some(true),
            ..Default::default()
        };
        let plan = build_research_search_plan(&req);
        let primary_count = plan
            .subqueries
            .iter()
            .filter(|sq| sq.source_type == ResearchSourceType::PrimarySources)
            .count();
        let security_count = plan
            .subqueries
            .iter()
            .filter(|sq| sq.source_type == ResearchSourceType::SecurityConsiderations)
            .count();
        assert_eq!(primary_count, 1);
        assert_eq!(security_count, 1);
    }
}
