//! Suggested fetch generation for research search.

use crate::core::fetch::ExtractMode;
use crate::core::research::{
    EvidenceQuality, ResearchResultGroup, ResearchResultGroupKind, ResearchSuggestedFetch,
};
use crate::core::source_card::SourceKind;
use crate::meta::fetch_ranking::{
    extract_domain, rank_and_select, DiversityConfig, FetchCandidate, FetchRankMode, RankContext,
};
use crate::meta::research_grouping::classify_evidence_quality;

/// Map a group string label back to its `ResearchResultGroupKind`.
fn group_from_str(s: &str) -> ResearchResultGroupKind {
    match s {
        "primary_sources" => ResearchResultGroupKind::PrimarySources,
        "official_docs" => ResearchResultGroupKind::OfficialDocs,
        "specifications" => ResearchResultGroupKind::Specifications,
        "reference_implementations" => ResearchResultGroupKind::ReferenceImplementations,
        "design_discussions" => ResearchResultGroupKind::DesignDiscussions,
        "benchmarks" => ResearchResultGroupKind::Benchmarks,
        "security_considerations" => ResearchResultGroupKind::SecurityConsiderations,
        "issue_threads" => ResearchResultGroupKind::IssueThreads,
        "release_notes" => ResearchResultGroupKind::ReleaseNotes,
        "academic_or_formal_sources" => ResearchResultGroupKind::AcademicOrFormalSources,
        "recent_news" => ResearchResultGroupKind::RecentNews,
        "community_discussion" => ResearchResultGroupKind::CommunityDiscussion,
        "counterpoints" => ResearchResultGroupKind::Counterpoints,
        _ => ResearchResultGroupKind::Unknown,
    }
}

/// Generate suggested fetches from grouped research results.
///
/// Uses the deterministic ranking pipeline with research-mode scoring
/// and diversity caps (max 2 per domain, max 8 total).
pub fn generate_research_suggested_fetches(
    groups: &[ResearchResultGroup],
) -> Vec<ResearchSuggestedFetch> {
    let mut candidates: Vec<FetchCandidate> = Vec::new();

    for group in groups {
        let Some(card) = group.results.first() else {
            continue;
        };

        let expected_kind = expected_kind_for_group(group.kind);
        let recommended_extract_mode = recommended_extract_mode_for_group(group.kind);
        let domain = extract_domain(&card.url);

        let (source_role, evidence_confidence) = card
            .metadata
            .code_evidence
            .as_ref()
            .map(|ce| (ce.source_role, ce.evidence_confidence))
            .unwrap_or((None, None));

        candidates.push(FetchCandidate {
            url: card.url.clone(),
            structured_repo_fetch: false,
            group: serde_json::to_string(&group.kind)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
            expected_kind,
            recommended_extract_mode,
            original_order: candidates.len(),
            source_kind: expected_kind,
            source_role,
            evidence_confidence,
            is_pinned_permalink: false,
            is_raw_url: false,
            is_browser_url: card.url.starts_with("http"),
            domain,
            score: 0,
            reasons: Vec::new(),
            information_gain: 0.0,
            stable: false,
        });
    }

    let ctx = RankContext {
        mode: FetchRankMode::Research,
        ..Default::default()
    };
    let config = DiversityConfig {
        max_per_domain: 2,
        max_per_group: 0,
        total_cap: 8,
    };

    let ranked = rank_and_select(candidates, &ctx, &config);

    ranked
        .into_iter()
        .enumerate()
        .map(|(i, candidate)| {
            let group_kind = group_from_str(&candidate.group);
            let card = find_group(groups, group_kind).and_then(|g| g.results.first());
            let evidence_quality = card
                .map(classify_evidence_quality)
                .unwrap_or(EvidenceQuality::Unknown);

            let reason = candidate
                .reasons
                .first()
                .map(|r| r.as_str().to_string())
                .unwrap_or_else(|| "suggested".to_string());

            ResearchSuggestedFetch {
                url: candidate.url,
                group: group_kind,
                expected_kind: candidate.expected_kind,
                evidence_quality,
                reason,
                recommended_extract_mode: candidate.recommended_extract_mode,
                priority: (i + 1) as u8,
                score: Some(candidate.score),
                rank_reasons: candidate
                    .reasons
                    .iter()
                    .map(|r| r.as_str().to_string())
                    .collect(),
                information_gain: Some(candidate.information_gain),
                stable_id: None,
            }
        })
        .collect()
}

fn find_group(
    groups: &[ResearchResultGroup],
    kind: ResearchResultGroupKind,
) -> Option<&ResearchResultGroup> {
    groups.iter().find(|g| g.kind == kind)
}

fn expected_kind_for_group(kind: ResearchResultGroupKind) -> SourceKind {
    match kind {
        ResearchResultGroupKind::PrimarySources
        | ResearchResultGroupKind::OfficialDocs
        | ResearchResultGroupKind::Specifications => SourceKind::OfficialDocs,
        ResearchResultGroupKind::ReferenceImplementations => SourceKind::SourceFile,
        ResearchResultGroupKind::DesignDiscussions | ResearchResultGroupKind::IssueThreads => {
            SourceKind::IssueThread
        }
        ResearchResultGroupKind::Benchmarks => SourceKind::Reference,
        ResearchResultGroupKind::SecurityConsiderations => SourceKind::SecurityAdvisory,
        ResearchResultGroupKind::ReleaseNotes => SourceKind::ReleaseNotes,
        ResearchResultGroupKind::AcademicOrFormalSources => SourceKind::Reference,
        ResearchResultGroupKind::RecentNews => SourceKind::News,
        ResearchResultGroupKind::CommunityDiscussion => SourceKind::Forum,
        ResearchResultGroupKind::Counterpoints => SourceKind::Unknown,
        ResearchResultGroupKind::Unknown => SourceKind::Unknown,
    }
}

fn recommended_extract_mode_for_group(kind: ResearchResultGroupKind) -> Option<ExtractMode> {
    match kind {
        ResearchResultGroupKind::OfficialDocs
        | ResearchResultGroupKind::Specifications
        | ResearchResultGroupKind::DesignDiscussions
        | ResearchResultGroupKind::AcademicOrFormalSources => Some(ExtractMode::Markdown),
        ResearchResultGroupKind::ReferenceImplementations => None,
        _ => Some(ExtractMode::Markdown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::code_evidence::{CodeEvidence, EvidenceConfidence, SourceRole};
    use crate::core::research::EvidenceQuality;
    use crate::core::result::TrustLevel;
    use crate::core::source_card::SourceCard;

    fn make_card(title: &str, url: &str) -> SourceCard {
        SourceCard::new(
            title,
            url,
            vec!["test".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        )
    }

    fn make_group(kind: ResearchResultGroupKind, cards: Vec<SourceCard>) -> ResearchResultGroup {
        ResearchResultGroup {
            kind,
            label: format!("{kind:?}"),
            results: cards,
            truncated: false,
            quality_summary: None,
        }
    }

    #[test]
    fn covers_all_non_empty_groups() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://primary.example.com/p1")],
            ),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://docs.example.com/d1")],
            ),
            make_group(
                ResearchResultGroupKind::Benchmarks,
                vec![make_card("B1", "https://bench.example.com/b1")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        let kinds: Vec<ResearchResultGroupKind> = fetches.iter().map(|f| f.group).collect();
        assert!(kinds.contains(&ResearchResultGroupKind::PrimarySources));
        assert!(kinds.contains(&ResearchResultGroupKind::OfficialDocs));
        assert!(kinds.contains(&ResearchResultGroupKind::Benchmarks));
    }

    #[test]
    fn primary_sources_outrank_community_discussion() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::CommunityDiscussion,
                vec![make_card("C1", "https://forum.example.com/t/1")],
            ),
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://primary.example.com/p1")],
            ),
            make_group(
                ResearchResultGroupKind::Benchmarks,
                vec![make_card("B1", "https://bench.example.com/b1")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        assert!(fetches.len() >= 3);
        // Primary sources (OfficialDocs source_kind) score higher than
        // benchmarks (Reference) and community discussion (Forum) in
        // research mode, so they always rank first.
        assert_eq!(fetches[0].group, ResearchResultGroupKind::PrimarySources);
        // Benchmarks and CommunityDiscussion tie (both get GenericWebUrl +5);
        // input order breaks the tie via stable sort.
    }

    #[test]
    fn domain_cap_prevents_same_domain_dominance() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://same.example.com/a")],
            ),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://same.example.com/b")],
            ),
            make_group(
                ResearchResultGroupKind::Specifications,
                vec![make_card("S1", "https://same.example.com/c")],
            ),
            make_group(
                ResearchResultGroupKind::Benchmarks,
                vec![make_card("B1", "https://same.example.com/d")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        let same_domain_count = fetches
            .iter()
            .filter(|f| extract_domain(&f.url) == "same.example.com")
            .count();
        assert!(
            same_domain_count <= 2,
            "expected at most 2 from same domain, got {same_domain_count}"
        );
    }

    #[test]
    fn official_primary_sources_retained_even_with_domain_cap() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://same.example.com/a")],
            ),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://same.example.com/b")],
            ),
            make_group(
                ResearchResultGroupKind::Benchmarks,
                vec![make_card("B1", "https://same.example.com/c")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        let groups_in_fetch: Vec<ResearchResultGroupKind> =
            fetches.iter().map(|f| f.group).collect();
        assert!(groups_in_fetch.contains(&ResearchResultGroupKind::PrimarySources));
        assert!(groups_in_fetch.contains(&ResearchResultGroupKind::OfficialDocs));
    }

    #[test]
    fn empty_groups_produce_no_suggestions() {
        let groups: Vec<ResearchResultGroup> = vec![];
        let fetches = generate_research_suggested_fetches(&groups);
        assert!(fetches.is_empty());
    }

    #[test]
    fn caps_at_8_suggestions() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://p.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://d.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::Specifications,
                vec![make_card("S1", "https://s.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::ReferenceImplementations,
                vec![make_card("R1", "https://r.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::DesignDiscussions,
                vec![make_card("DD1", "https://dd.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::Benchmarks,
                vec![make_card("B1", "https://b.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::SecurityConsiderations,
                vec![make_card("SC1", "https://sc.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::Counterpoints,
                vec![make_card("CP1", "https://cp.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::IssueThreads,
                vec![make_card("I1", "https://i.example.com/1")],
            ),
            make_group(
                ResearchResultGroupKind::ReleaseNotes,
                vec![make_card("RN1", "https://rn.example.com/1")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        assert!(
            fetches.len() <= 8,
            "expected at most 8, got {}",
            fetches.len()
        );
    }

    #[test]
    fn empty_group_not_included() {
        let groups = vec![
            make_group(ResearchResultGroupKind::PrimarySources, vec![]),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://docs.example.com/d1")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        assert_eq!(fetches.len(), 1);
        assert_eq!(fetches[0].group, ResearchResultGroupKind::OfficialDocs);
    }

    #[test]
    fn extract_domain_parses_correctly() {
        assert_eq!(
            super::extract_domain("https://docs.example.com/page"),
            "docs.example.com".to_string()
        );
        assert_eq!(
            super::extract_domain("http://example.com"),
            "example.com".to_string()
        );
        assert_eq!(super::extract_domain("not-a-url"), "".to_string());
    }

    #[test]
    fn recommended_extract_mode_none_for_reference_impl() {
        let groups = vec![make_group(
            ResearchResultGroupKind::ReferenceImplementations,
            vec![make_card("R1", "https://code.example.com/main.rs")],
        )];
        let fetches = generate_research_suggested_fetches(&groups);
        assert_eq!(fetches.len(), 1);
        assert_eq!(fetches[0].recommended_extract_mode, None);
    }

    #[test]
    fn recommended_extract_mode_markdown_for_official_docs() {
        let groups = vec![make_group(
            ResearchResultGroupKind::OfficialDocs,
            vec![make_card("D1", "https://docs.example.com/guide")],
        )];
        let fetches = generate_research_suggested_fetches(&groups);
        assert_eq!(fetches.len(), 1);
        assert_eq!(
            fetches[0].recommended_extract_mode,
            Some(ExtractMode::Markdown)
        );
    }

    #[test]
    fn evidence_quality_is_set() {
        let groups = vec![make_group(
            ResearchResultGroupKind::SecurityConsiderations,
            vec![make_card("SEC", "https://osv.dev/vulnerability/GHSA-xxxx")],
        )];
        let fetches = generate_research_suggested_fetches(&groups);
        assert_eq!(fetches.len(), 1);
        assert_eq!(
            fetches[0].evidence_quality,
            EvidenceQuality::SecurityAdvisory
        );
    }

    #[test]
    fn github_domain_respects_diversity_cap() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://github.com/org/repo/paper")],
            ),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://github.com/org/repo/docs")],
            ),
            make_group(
                ResearchResultGroupKind::Specifications,
                vec![make_card("S1", "https://github.com/org/repo/spec")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        // The ranking module applies max_per_domain=2 uniformly;
        // github.com is no longer exempt from the diversity cap.
        let github_count = fetches
            .iter()
            .filter(|f| extract_domain(&f.url) == "github.com")
            .count();
        assert!(
            github_count <= 2,
            "expected at most 2 from github.com, got {github_count}"
        );
    }

    #[test]
    fn score_and_rank_reasons_are_populated() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://primary.example.com/p1")],
            ),
            make_group(
                ResearchResultGroupKind::CommunityDiscussion,
                vec![make_card("C1", "https://forum.example.com/t/1")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        assert!(!fetches.is_empty());
        for fetch in &fetches {
            assert!(
                fetch.score.is_some(),
                "score should be populated for {}",
                fetch.url
            );
            assert!(
                !fetch.rank_reasons.is_empty(),
                "rank_reasons should be populated for {}",
                fetch.url
            );
        }
    }

    #[test]
    fn domain_diversity_caps_via_ranking_module() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://same.example.com/a")],
            ),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://same.example.com/b")],
            ),
            make_group(
                ResearchResultGroupKind::Specifications,
                vec![make_card("S1", "https://same.example.com/c")],
            ),
            make_group(
                ResearchResultGroupKind::Benchmarks,
                vec![make_card("B1", "https://same.example.com/d")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        let same_domain_count = fetches
            .iter()
            .filter(|f| extract_domain(&f.url) == "same.example.com")
            .count();
        assert!(
            same_domain_count <= 2,
            "expected at most 2 from same domain via ranking module, got {same_domain_count}"
        );
    }

    #[test]
    fn priority_is_sequential_after_ranking() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://primary.example.com/p1")],
            ),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://docs.example.com/d1")],
            ),
            make_group(
                ResearchResultGroupKind::Benchmarks,
                vec![make_card("B1", "https://bench.example.com/b1")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        for (i, fetch) in fetches.iter().enumerate() {
            assert_eq!(
                fetch.priority,
                (i + 1) as u8,
                "priority should be sequential starting at 1"
            );
        }
    }

    #[test]
    fn information_gain_is_populated() {
        let groups = vec![
            make_group(
                ResearchResultGroupKind::PrimarySources,
                vec![make_card("P1", "https://primary.example.com/p1")],
            ),
            make_group(
                ResearchResultGroupKind::OfficialDocs,
                vec![make_card("D1", "https://docs.example.com/d1")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        assert!(!fetches.is_empty());
        for fetch in &fetches {
            assert!(
                fetch.information_gain.is_some(),
                "information_gain should be populated for {}",
                fetch.url
            );
        }
    }

    #[test]
    fn code_evidence_source_role_used_in_ranking() {
        let mut card = make_card("C1", "https://github.com/org/repo/blob/main/src/lib.rs");
        card.metadata.code_evidence = Some(CodeEvidence {
            source_role: Some(SourceRole::Implementation),
            evidence_confidence: Some(EvidenceConfidence::Exact),
            ..Default::default()
        });

        let groups = vec![
            make_group(
                ResearchResultGroupKind::ReferenceImplementations,
                vec![card],
            ),
            make_group(
                ResearchResultGroupKind::CommunityDiscussion,
                vec![make_card("D1", "https://forum.example.com/t/1")],
            ),
        ];
        let fetches = generate_research_suggested_fetches(&groups);
        assert!(fetches.len() >= 2);
        // ReferenceImplementation with exact code evidence should outrank community discussion
        assert_eq!(
            fetches[0].group,
            ResearchResultGroupKind::ReferenceImplementations
        );
        // The rank_reasons should include source_role_implementation or exact_confidence
        let has_evidence_reason = fetches[0]
            .rank_reasons
            .iter()
            .any(|r| r == "source_role_implementation" || r == "exact_confidence");
        assert!(
            has_evidence_reason,
            "expected code evidence rank reasons, got {:?}",
            fetches[0].rank_reasons
        );
    }
}
