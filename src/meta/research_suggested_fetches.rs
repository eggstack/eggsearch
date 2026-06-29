//! Suggested fetch generation for research search.

use crate::core::fetch::ExtractMode;
use crate::core::research::{ResearchResultGroup, ResearchResultGroupKind, ResearchSuggestedFetch};
use crate::core::source_card::SourceKind;
use crate::meta::research_grouping::classify_evidence_quality;

/// Canonical priority order for research result groups.
/// Lower number = higher priority.
const RESEARCH_PRIORITY: &[(ResearchResultGroupKind, &str)] = &[
    (ResearchResultGroupKind::PrimarySources, "primary_source"),
    (ResearchResultGroupKind::OfficialDocs, "official_docs"),
    (ResearchResultGroupKind::Specifications, "specification"),
    (
        ResearchResultGroupKind::ReferenceImplementations,
        "reference_implementation",
    ),
    (
        ResearchResultGroupKind::DesignDiscussions,
        "active_design_discussion",
    ),
    (ResearchResultGroupKind::Benchmarks, "benchmark"),
    (
        ResearchResultGroupKind::SecurityConsiderations,
        "security_consideration",
    ),
    (ResearchResultGroupKind::Counterpoints, "counterpoint"),
    (ResearchResultGroupKind::IssueThreads, "diversity_source"),
    (ResearchResultGroupKind::ReleaseNotes, "diversity_source"),
    (
        ResearchResultGroupKind::AcademicOrFormalSources,
        "diversity_source",
    ),
    (ResearchResultGroupKind::RecentNews, "diversity_source"),
    (
        ResearchResultGroupKind::CommunityDiscussion,
        "diversity_source",
    ),
];

/// Max suggested fetches from the same domain.
const DOMAIN_SOFT_CAP: usize = 2;

/// Max total suggested fetches.
const TOTAL_CAP: usize = 8;

/// Domains that are clearly primary/official and exempt from the domain cap.
fn is_primary_domain(domain: &str) -> bool {
    let d = domain.to_lowercase();
    d.ends_with(".rs")
        || d == "docs.rs"
        || d == "crates.io"
        || d == "github.com"
        || d == "gitlab.com"
        || d == "codeberg.org"
        || d == "rust-lang.org"
        || d == "doc.rust-lang.org"
}

/// Generate suggested fetches from grouped research results.
pub fn generate_research_suggested_fetches(
    groups: &[ResearchResultGroup],
) -> Vec<ResearchSuggestedFetch> {
    let mut suggestions = Vec::new();
    let mut domain_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut priority: u8 = 1;

    for &(kind, reason) in RESEARCH_PRIORITY {
        let Some(group) = find_group(groups, kind) else {
            continue;
        };
        let Some(card) = group.results.first() else {
            continue;
        };

        let domain = extract_domain(&card.url).unwrap_or_default();
        let count = domain_counts.get(&domain).copied().unwrap_or(0);
        let primary = is_primary_domain(&domain);

        if !primary && count >= DOMAIN_SOFT_CAP {
            priority = priority.saturating_add(1);
            continue;
        }

        let expected_kind = expected_kind_for_group(kind);
        let evidence_quality = classify_evidence_quality(card);
        let recommended_extract_mode = recommended_extract_mode_for_group(kind);

        suggestions.push(ResearchSuggestedFetch {
            url: card.url.clone(),
            group: kind,
            expected_kind,
            evidence_quality,
            reason: reason.to_string(),
            recommended_extract_mode,
            priority,
        });

        if !domain.is_empty() {
            *domain_counts.entry(domain).or_insert(0) += 1;
        }
        priority = priority.saturating_add(1);

        if suggestions.len() >= TOTAL_CAP {
            break;
        }
    }

    suggestions
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

fn extract_domain(url: &str) -> Option<String> {
    url.split("://")
        .nth(1)?
        .split('/')
        .next()?
        .to_string()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn priority_order_matches_canonical_order() {
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
        assert_eq!(fetches[0].group, ResearchResultGroupKind::PrimarySources);
        assert_eq!(fetches[1].group, ResearchResultGroupKind::Benchmarks);
        assert_eq!(
            fetches[2].group,
            ResearchResultGroupKind::CommunityDiscussion
        );
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
            .filter(|f| extract_domain(&f.url).as_deref() == Some("same.example.com"))
            .count();
        assert!(
            same_domain_count <= DOMAIN_SOFT_CAP,
            "expected at most {DOMAIN_SOFT_CAP} from same domain, got {same_domain_count}"
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
            fetches.len() <= TOTAL_CAP,
            "expected at most {TOTAL_CAP}, got {}",
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
            extract_domain("https://docs.example.com/page"),
            Some("docs.example.com".to_string())
        );
        assert_eq!(
            extract_domain("http://example.com"),
            Some("example.com".to_string())
        );
        assert_eq!(extract_domain("not-a-url"), None);
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
    fn github_domain_is_primary_exempt_from_cap() {
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
        // All three should be included because github.com is primary
        assert_eq!(fetches.len(), 3);
    }
}
