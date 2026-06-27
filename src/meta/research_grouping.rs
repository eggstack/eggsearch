//! Deterministic grouping of SourceCards into research evidence categories.

use crate::core::research::{
    EvidenceQuality, ResearchResultGroup, ResearchResultGroupKind, ResearchSourceType,
};
use crate::core::source_card::{SourceCard, SourceKind};

/// Classify a SourceCard into its evidence quality tier.
///
/// Uses `SourceKind`, domain priors, URL heuristics, and provider ID
/// to produce a deterministic quality classification.
pub fn classify_evidence_quality(card: &SourceCard) -> EvidenceQuality {
    let url_lower = card.url.to_lowercase();
    let title_lower = card.title.to_lowercase();
    let provider = card.providers.first().map(|s| s.as_str());

    match card.metadata.source_kind {
        SourceKind::OfficialDocs => EvidenceQuality::OfficialPrimary,
        SourceKind::PackageRegistry => EvidenceQuality::PackageRegistry,
        SourceKind::SecurityAdvisory => EvidenceQuality::SecurityAdvisory,
        SourceKind::News => EvidenceQuality::NewsOrPress,
        SourceKind::Tutorial => EvidenceQuality::BlogOrTutorial,
        SourceKind::Forum => EvidenceQuality::CommunityDiscussion,
        SourceKind::Reference => EvidenceQuality::StandardsOrSpecification,
        SourceKind::ReleaseNotes => classify_release_quality(&url_lower),
        SourceKind::SourceFile | SourceKind::SourceRepository | SourceKind::RepositoryRoot => {
            classify_maintainer_quality(&url_lower, provider)
        }
        SourceKind::IssueThread | SourceKind::PullRequest => {
            classify_maintainer_quality(&url_lower, provider)
        }
        SourceKind::Tag | SourceKind::Commit => classify_maintainer_quality(&url_lower, provider),
        SourceKind::SourceDirectory => classify_maintainer_quality(&url_lower, provider),
        SourceKind::Unknown => classify_unknown_quality(&url_lower, &title_lower, provider),
    }
}

fn classify_release_quality(url_lower: &str) -> EvidenceQuality {
    if is_github_or_gitlab(url_lower) {
        EvidenceQuality::VendorPrimary
    } else {
        EvidenceQuality::Unknown
    }
}

fn classify_maintainer_quality(url_lower: &str, provider: Option<&str>) -> EvidenceQuality {
    if is_github_or_gitlab(url_lower) {
        return EvidenceQuality::MaintainerPrimary;
    }
    if provider == Some("github_code")
        || provider == Some("github_issues")
        || provider == Some("github_releases")
    {
        return EvidenceQuality::MaintainerPrimary;
    }
    EvidenceQuality::Unknown
}

fn classify_unknown_quality(
    url_lower: &str,
    title_lower: &str,
    provider: Option<&str>,
) -> EvidenceQuality {
    // Academic sources
    if url_lower.contains("arxiv.org")
        || url_lower.contains("acm.org")
        || url_lower.contains("ieee.org")
    {
        return EvidenceQuality::AcademicOrFormal;
    }

    // Standards/spec bodies
    if url_lower.contains("ietf.org")
        || url_lower.contains("w3.org")
        || url_lower.contains("whatwg.org")
        || url_lower.contains("rfc-editor.org")
        || url_lower.contains(".rfc")
        || url_lower.contains("/rfc")
    {
        return EvidenceQuality::StandardsOrSpecification;
    }

    // Official docs domains
    if url_lower.contains("docs.rs")
        || url_lower.contains("doc.rust-lang.org")
        || url_lower.contains("developer.mozilla.org")
        || url_lower.contains("go.dev")
        || url_lower.contains("pkg.go.dev")
        || url_lower.contains("doc.python.org")
        || url_lower.contains("docs.python.org")
    {
        return EvidenceQuality::OfficialPrimary;
    }

    // Package registries
    if url_lower.contains("crates.io")
        || url_lower.contains("npmjs.com")
        || url_lower.contains("pypi.org")
    {
        return EvidenceQuality::PackageRegistry;
    }

    // Security advisory domains
    if provider == Some("osv")
        || url_lower.contains("osv.dev")
        || url_lower.contains("nvd.nist.gov")
        || url_lower.contains("github.com/advisories")
    {
        return EvidenceQuality::SecurityAdvisory;
    }

    // Benchmark/performance keywords
    if url_lower.contains("benchmark")
        || title_lower.contains("benchmark")
        || title_lower.contains("performance comparison")
    {
        return EvidenceQuality::BenchmarkOrMeasurement;
    }

    // Community Q&A / forums
    if url_lower.contains("stackoverflow.com")
        || url_lower.contains("reddit.com")
        || url_lower.contains("forum.")
    {
        return EvidenceQuality::CommunityDiscussion;
    }

    EvidenceQuality::Unknown
}

fn is_github_or_gitlab(url_lower: &str) -> bool {
    url_lower.contains("github.com") || url_lower.contains("gitlab.com")
}

/// Classify a SourceCard into a research result group.
///
/// If `source_type_hint` is `Some`, it is used as the primary
/// classification (mapped to `ResearchResultGroupKind`). Otherwise,
/// classification is based on `SourceKind` and URL heuristics.
pub fn classify_research_group(
    card: &SourceCard,
    source_type_hint: Option<&ResearchSourceType>,
) -> ResearchResultGroupKind {
    if let Some(hint) = source_type_hint {
        return source_type_to_group_kind(*hint);
    }

    let url_lower = card.url.to_lowercase();
    let title_lower = card.title.to_lowercase();
    let snippet_lower = card.snippet.as_deref().unwrap_or("").to_lowercase();

    match card.metadata.source_kind {
        SourceKind::OfficialDocs => ResearchResultGroupKind::OfficialDocs,
        SourceKind::PackageRegistry => ResearchResultGroupKind::Unknown,
        SourceKind::SecurityAdvisory => ResearchResultGroupKind::SecurityConsiderations,
        SourceKind::ReleaseNotes => ResearchResultGroupKind::ReleaseNotes,
        SourceKind::News => ResearchResultGroupKind::RecentNews,
        SourceKind::Tutorial => ResearchResultGroupKind::CommunityDiscussion,
        SourceKind::Forum => ResearchResultGroupKind::CommunityDiscussion,
        SourceKind::Reference => classify_reference_group(&url_lower),
        SourceKind::IssueThread => classify_issue_group(card),
        SourceKind::PullRequest => ResearchResultGroupKind::DesignDiscussions,
        SourceKind::SourceFile | SourceKind::SourceRepository | SourceKind::RepositoryRoot => {
            ResearchResultGroupKind::ReferenceImplementations
        }
        SourceKind::Tag | SourceKind::Commit => ResearchResultGroupKind::ReferenceImplementations,
        SourceKind::SourceDirectory => ResearchResultGroupKind::ReferenceImplementations,
        SourceKind::Unknown => classify_unknown_group(&url_lower, &title_lower, &snippet_lower),
    }
}

fn source_type_to_group_kind(st: ResearchSourceType) -> ResearchResultGroupKind {
    match st {
        ResearchSourceType::PrimarySources => ResearchResultGroupKind::PrimarySources,
        ResearchSourceType::OfficialDocs => ResearchResultGroupKind::OfficialDocs,
        ResearchSourceType::Specifications => ResearchResultGroupKind::Specifications,
        ResearchSourceType::ReferenceImplementations => {
            ResearchResultGroupKind::ReferenceImplementations
        }
        ResearchSourceType::DesignDiscussions => ResearchResultGroupKind::DesignDiscussions,
        ResearchSourceType::Benchmarks => ResearchResultGroupKind::Benchmarks,
        ResearchSourceType::SecurityConsiderations => {
            ResearchResultGroupKind::SecurityConsiderations
        }
        ResearchSourceType::IssueThreads => ResearchResultGroupKind::IssueThreads,
        ResearchSourceType::ReleaseNotes => ResearchResultGroupKind::ReleaseNotes,
        ResearchSourceType::AcademicOrFormalSources => {
            ResearchResultGroupKind::AcademicOrFormalSources
        }
        ResearchSourceType::RecentNews => ResearchResultGroupKind::RecentNews,
        ResearchSourceType::CommunityDiscussion => ResearchResultGroupKind::CommunityDiscussion,
        ResearchSourceType::Counterpoints => ResearchResultGroupKind::Counterpoints,
    }
}

fn classify_reference_group(url_lower: &str) -> ResearchResultGroupKind {
    if url_lower.contains("ietf.org")
        || url_lower.contains("w3.org")
        || url_lower.contains("whatwg.org")
        || url_lower.contains(".rfc")
        || url_lower.contains("/rfc")
    {
        ResearchResultGroupKind::Specifications
    } else {
        ResearchResultGroupKind::OfficialDocs
    }
}

fn classify_issue_group(card: &SourceCard) -> ResearchResultGroupKind {
    if card
        .metadata
        .issue
        .as_ref()
        .is_some_and(|i| i.is_pull_request == Some(true))
    {
        return ResearchResultGroupKind::DesignDiscussions;
    }

    let title_lower = card.title.to_lowercase();
    let snippet_lower = card.snippet.as_deref().unwrap_or("").to_lowercase();
    let combined = format!("{title_lower} {snippet_lower}");

    if is_rfc_or_proposal(&combined) {
        ResearchResultGroupKind::DesignDiscussions
    } else {
        ResearchResultGroupKind::IssueThreads
    }
}

fn is_rfc_or_proposal(text: &str) -> bool {
    text.contains("rfc")
        || text.contains("proposal")
        || text.contains("design doc")
        || text.contains("architecture decision")
        || text.contains("adr")
}

fn classify_unknown_group(
    url_lower: &str,
    title_lower: &str,
    snippet_lower: &str,
) -> ResearchResultGroupKind {
    // Academic sources
    if url_lower.contains("arxiv.org")
        || url_lower.contains("acm.org")
        || url_lower.contains("ieee.org")
    {
        return ResearchResultGroupKind::AcademicOrFormalSources;
    }

    // Standards/spec bodies
    if url_lower.contains("ietf.org")
        || url_lower.contains("w3.org")
        || url_lower.contains("rfc-editor.org")
        || url_lower.contains(".rfc")
        || url_lower.contains("/rfc")
    {
        return ResearchResultGroupKind::Specifications;
    }

    // Benchmark/performance keywords
    let combined = format!("{title_lower} {snippet_lower}");
    if url_lower.contains("benchmark")
        || combined.contains("benchmark")
        || combined.contains("performance")
    {
        return ResearchResultGroupKind::Benchmarks;
    }

    // Counterpoint keywords
    if combined.contains("drawback")
        || combined.contains("limitation")
        || combined.contains("tradeoff")
        || combined.contains("criticism")
    {
        return ResearchResultGroupKind::Counterpoints;
    }

    // Community Q&A / forums
    if url_lower.contains("stackoverflow.com")
        || url_lower.contains("reddit.com")
        || url_lower.contains("forum.")
    {
        return ResearchResultGroupKind::CommunityDiscussion;
    }

    ResearchResultGroupKind::Unknown
}

/// Group a flat list of SourceCards into ResearchResultGroups.
///
/// Each card goes to exactly one group (its primary classification).
/// Groups are returned in a fixed canonical order with empty groups
/// omitted. Each group is truncated to `max_per_group`.
pub fn group_research_results(
    cards: Vec<SourceCard>,
    max_per_group: usize,
) -> Vec<ResearchResultGroup> {
    use std::collections::HashMap;

    let mut buckets: HashMap<ResearchResultGroupKind, Vec<SourceCard>> = HashMap::new();
    for card in cards {
        let kind = classify_research_group(&card, None);
        buckets.entry(kind).or_default().push(card);
    }

    let canonical_order: Vec<ResearchResultGroupKind> = vec![
        ResearchResultGroupKind::PrimarySources,
        ResearchResultGroupKind::OfficialDocs,
        ResearchResultGroupKind::Specifications,
        ResearchResultGroupKind::ReferenceImplementations,
        ResearchResultGroupKind::DesignDiscussions,
        ResearchResultGroupKind::Benchmarks,
        ResearchResultGroupKind::SecurityConsiderations,
        ResearchResultGroupKind::IssueThreads,
        ResearchResultGroupKind::ReleaseNotes,
        ResearchResultGroupKind::AcademicOrFormalSources,
        ResearchResultGroupKind::RecentNews,
        ResearchResultGroupKind::CommunityDiscussion,
        ResearchResultGroupKind::Counterpoints,
        ResearchResultGroupKind::Unknown,
    ];

    let labels: Vec<(ResearchResultGroupKind, &str)> = vec![
        (ResearchResultGroupKind::PrimarySources, "Primary Sources"),
        (
            ResearchResultGroupKind::OfficialDocs,
            "Official Documentation",
        ),
        (ResearchResultGroupKind::Specifications, "Specifications"),
        (
            ResearchResultGroupKind::ReferenceImplementations,
            "Reference Implementations",
        ),
        (
            ResearchResultGroupKind::DesignDiscussions,
            "Design Discussions",
        ),
        (ResearchResultGroupKind::Benchmarks, "Benchmarks"),
        (
            ResearchResultGroupKind::SecurityConsiderations,
            "Security Considerations",
        ),
        (ResearchResultGroupKind::IssueThreads, "Issue Threads"),
        (ResearchResultGroupKind::ReleaseNotes, "Release Notes"),
        (
            ResearchResultGroupKind::AcademicOrFormalSources,
            "Academic & Formal Sources",
        ),
        (ResearchResultGroupKind::RecentNews, "Recent News"),
        (
            ResearchResultGroupKind::CommunityDiscussion,
            "Community Discussion",
        ),
        (ResearchResultGroupKind::Counterpoints, "Counterpoints"),
        (ResearchResultGroupKind::Unknown, "Other"),
    ];

    let label_map: std::collections::HashMap<ResearchResultGroupKind, &str> =
        labels.into_iter().collect();

    let mut groups = Vec::new();
    for kind in canonical_order {
        if let Some(mut results) = buckets.remove(&kind) {
            let full_count = results.len();
            results.truncate(max_per_group);
            let truncated = full_count > max_per_group;
            let label = label_map
                .get(&kind)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{kind:?}"));
            groups.push(ResearchResultGroup {
                kind,
                label,
                results,
                truncated,
            });
        }
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::result::TrustLevel;
    use crate::core::source_card::SourceMetadata;

    fn make_card(source_kind: SourceKind, url: &str) -> SourceCard {
        let mut card = SourceCard::new(
            "Test",
            url,
            vec!["test".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        card.metadata = SourceMetadata {
            source_kind,
            ..Default::default()
        };
        card
    }

    fn make_card_with_title(source_kind: SourceKind, url: &str, title: &str) -> SourceCard {
        let mut card = make_card(source_kind, url);
        card.title = title.to_string();
        card
    }

    fn make_card_with_snippet(source_kind: SourceKind, url: &str, snippet: &str) -> SourceCard {
        let mut card = make_card(source_kind, url);
        card.snippet = Some(snippet.to_string());
        card
    }

    fn make_card_with_provider(source_kind: SourceKind, url: &str, provider: &str) -> SourceCard {
        let mut card = make_card(source_kind, url);
        card.providers = vec![provider.to_string()];
        card
    }

    // ---- Evidence quality tests ----

    #[test]
    fn evidence_quality_official_docs() {
        let card = make_card(SourceKind::OfficialDocs, "https://docs.rs/axum");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::OfficialPrimary
        );
    }

    #[test]
    fn evidence_quality_package_registry() {
        let card = make_card(SourceKind::PackageRegistry, "https://crates.io/axum");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::PackageRegistry
        );
    }

    #[test]
    fn evidence_quality_security_advisory() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://osv.dev/vulnerability/GHSA-xxxx",
        );
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::SecurityAdvisory
        );
    }

    #[test]
    fn evidence_quality_news() {
        let card = make_card(SourceKind::News, "https://blog.example.com/post");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::NewsOrPress
        );
    }

    #[test]
    fn evidence_quality_tutorial() {
        let card = make_card(SourceKind::Tutorial, "https://dev.to/foo/tutorial");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::BlogOrTutorial
        );
    }

    #[test]
    fn evidence_quality_forum() {
        let card = make_card(SourceKind::Forum, "https://forum.example.com/t/topic");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::CommunityDiscussion
        );
    }

    #[test]
    fn evidence_quality_reference() {
        let card = make_card(SourceKind::Reference, "https://example.com/api-ref");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::StandardsOrSpecification
        );
    }

    #[test]
    fn evidence_quality_release_notes_github() {
        let card = make_card(
            SourceKind::ReleaseNotes,
            "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
        );
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::VendorPrimary
        );
    }

    #[test]
    fn evidence_quality_release_notes_unknown_host() {
        let card = make_card(SourceKind::ReleaseNotes, "https://example.com/releases");
        assert_eq!(classify_evidence_quality(&card), EvidenceQuality::Unknown);
    }

    #[test]
    fn evidence_quality_source_file_github() {
        let card = make_card(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
        );
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::MaintainerPrimary
        );
    }

    #[test]
    fn evidence_quality_issue_github() {
        let card = make_card(
            SourceKind::IssueThread,
            "https://github.com/tokio-rs/axum/issues/123",
        );
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::MaintainerPrimary
        );
    }

    #[test]
    fn evidence_quality_unknown_academic_url() {
        let card = make_card(SourceKind::Unknown, "https://arxiv.org/abs/2301.00001");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::AcademicOrFormal
        );
    }

    #[test]
    fn evidence_quality_unknown_rfc_url() {
        let card = make_card(SourceKind::Unknown, "https://example.com/rfc9110");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::StandardsOrSpecification
        );
    }

    #[test]
    fn evidence_quality_unknown_docs_url() {
        let card = make_card(SourceKind::Unknown, "https://docs.rs/axum");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::OfficialPrimary
        );
    }

    #[test]
    fn evidence_quality_unknown_crates_url() {
        let card = make_card(SourceKind::Unknown, "https://crates.io/axum");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::PackageRegistry
        );
    }

    #[test]
    fn evidence_quality_unknown_osv_provider() {
        let card = make_card_with_provider(SourceKind::Unknown, "https://example.com", "osv");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::SecurityAdvisory
        );
    }

    #[test]
    fn evidence_quality_unknown_benchmark_url() {
        let card = make_card(SourceKind::Unknown, "https://example.com/benchmark-results");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::BenchmarkOrMeasurement
        );
    }

    #[test]
    fn evidence_quality_unknown_benchmark_title() {
        let card = make_card_with_title(
            SourceKind::Unknown,
            "https://example.com/perf",
            "Benchmark: comparing frameworks",
        );
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::BenchmarkOrMeasurement
        );
    }

    #[test]
    fn evidence_quality_unknown_stackoverflow_url() {
        let card = make_card(SourceKind::Unknown, "https://stackoverflow.com/q/123");
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::CommunityDiscussion
        );
    }

    #[test]
    fn evidence_quality_unknown_reddit_url() {
        let card = make_card(
            SourceKind::Unknown,
            "https://reddit.com/r/rust/comments/abc",
        );
        assert_eq!(
            classify_evidence_quality(&card),
            EvidenceQuality::CommunityDiscussion
        );
    }

    // ---- Research group classification tests ----

    #[test]
    fn group_rfc_url_is_specifications() {
        let card = make_card(
            SourceKind::Unknown,
            "https://www.rfc-editor.org/rfc/rfc9110",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::Specifications
        );
    }

    #[test]
    fn group_w3c_url_is_specifications() {
        let card = make_card(SourceKind::Unknown, "https://www.w3.org/TR/html52/");
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::Specifications
        );
    }

    #[test]
    fn group_docs_url_is_official_docs() {
        let card = make_card(SourceKind::OfficialDocs, "https://docs.rs/axum");
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::OfficialDocs
        );
    }

    #[test]
    fn group_github_source_file_is_reference_implementations() {
        let card = make_card(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::ReferenceImplementations
        );
    }

    #[test]
    fn group_github_repository_root_is_reference_implementations() {
        let card = make_card(
            SourceKind::RepositoryRoot,
            "https://github.com/tokio-rs/axum",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::ReferenceImplementations
        );
    }

    #[test]
    fn group_issue_thread_is_issue_threads() {
        let card = make_card(
            SourceKind::IssueThread,
            "https://github.com/tokio-rs/axum/issues/123",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::IssueThreads
        );
    }

    #[test]
    fn group_rfc_issue_is_design_discussions() {
        let card = make_card_with_snippet(
            SourceKind::IssueThread,
            "https://github.com/tokio-rs/axum/issues/456",
            "This RFC proposes a new router design",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::DesignDiscussions
        );
    }

    #[test]
    fn group_pr_is_design_discussions() {
        let card = make_card(
            SourceKind::PullRequest,
            "https://github.com/tokio-rs/axum/pull/789",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::DesignDiscussions
        );
    }

    #[test]
    fn group_release_notes_is_release_notes() {
        let card = make_card(
            SourceKind::ReleaseNotes,
            "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::ReleaseNotes
        );
    }

    #[test]
    fn group_security_advisory_is_security_considerations() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://osv.dev/vulnerability/GHSA-xxxx",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::SecurityConsiderations
        );
    }

    #[test]
    fn group_benchmark_url_is_benchmarks() {
        let card = make_card(SourceKind::Unknown, "https://example.com/benchmark-results");
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::Benchmarks
        );
    }

    #[test]
    fn group_benchmark_title_is_benchmarks() {
        let card = make_card_with_title(
            SourceKind::Unknown,
            "https://example.com/perf",
            "Benchmark: comparing frameworks",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::Benchmarks
        );
    }

    #[test]
    fn group_stackoverflow_is_community() {
        let card = make_card(SourceKind::Unknown, "https://stackoverflow.com/q/123");
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::CommunityDiscussion
        );
    }

    #[test]
    fn group_forum_is_community() {
        let card = make_card(SourceKind::Forum, "https://forum.example.com/t/topic");
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::CommunityDiscussion
        );
    }

    #[test]
    fn group_tutorial_is_community() {
        let card = make_card(SourceKind::Tutorial, "https://dev.to/foo/tutorial");
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::CommunityDiscussion
        );
    }

    #[test]
    fn group_counterpoint_title() {
        let card = make_card_with_title(
            SourceKind::Unknown,
            "https://example.com/opinion",
            "The drawbacks of microservices",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::Counterpoints
        );
    }

    #[test]
    fn group_counterpoint_snippet() {
        let card = make_card_with_snippet(
            SourceKind::Unknown,
            "https://example.com/opinion",
            "Major tradeoffs in the design",
        );
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::Counterpoints
        );
    }

    #[test]
    fn group_with_source_type_hint() {
        let card = make_card(SourceKind::Unknown, "https://example.com/page");
        assert_eq!(
            classify_research_group(&card, Some(&ResearchSourceType::Benchmarks)),
            ResearchResultGroupKind::Benchmarks
        );
    }

    #[test]
    fn group_with_counterpoints_hint() {
        let card = make_card(SourceKind::Unknown, "https://example.com/page");
        assert_eq!(
            classify_research_group(&card, Some(&ResearchSourceType::Counterpoints)),
            ResearchResultGroupKind::Counterpoints
        );
    }

    #[test]
    fn group_academic_url_is_academic_sources() {
        let card = make_card(SourceKind::Unknown, "https://arxiv.org/abs/2301.00001");
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::AcademicOrFormalSources
        );
    }

    #[test]
    fn group_acm_url_is_academic_sources() {
        let card = make_card(SourceKind::Unknown, "https://dl.acm.org/doi/10.1145/12345");
        assert_eq!(
            classify_research_group(&card, None),
            ResearchResultGroupKind::AcademicOrFormalSources
        );
    }

    // ---- group_research_results tests ----

    #[test]
    fn empty_list_produces_empty_groups() {
        let groups = group_research_results(vec![], 10);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_results_canonical_order() {
        let cards = vec![
            make_card(SourceKind::ReleaseNotes, "https://example.com/releases"),
            make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
            make_card(
                SourceKind::SecurityAdvisory,
                "https://osv.dev/vulnerability/GHSA-xxxx",
            ),
            make_card(SourceKind::IssueThread, "https://example.com/issues/1"),
            make_card(SourceKind::Unknown, "https://arxiv.org/abs/2301.00001"),
            make_card(SourceKind::Unknown, "https://example.com/benchmark-results"),
            make_card(SourceKind::News, "https://blog.example.com/news"),
            make_card(SourceKind::Tutorial, "https://dev.to/foo/tutorial"),
        ];
        let groups = group_research_results(cards, 10);
        let kinds: Vec<ResearchResultGroupKind> = groups.iter().map(|g| g.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ResearchResultGroupKind::OfficialDocs,
                ResearchResultGroupKind::Benchmarks,
                ResearchResultGroupKind::SecurityConsiderations,
                ResearchResultGroupKind::IssueThreads,
                ResearchResultGroupKind::ReleaseNotes,
                ResearchResultGroupKind::AcademicOrFormalSources,
                ResearchResultGroupKind::RecentNews,
                ResearchResultGroupKind::CommunityDiscussion,
            ]
        );
    }

    #[test]
    fn group_results_truncation() {
        let cards: Vec<SourceCard> = (0..5)
            .map(|i| {
                let mut card = make_card(
                    SourceKind::OfficialDocs,
                    &format!("https://docs.example.com/{i}"),
                );
                card.title = format!("Doc {i}");
                card
            })
            .collect();
        let groups = group_research_results(cards, 3);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].results.len(), 3);
        assert!(groups[0].truncated);
    }

    #[test]
    fn group_results_no_truncation_when_under_limit() {
        let cards: Vec<SourceCard> = (0..2)
            .map(|i| {
                make_card(
                    SourceKind::OfficialDocs,
                    &format!("https://docs.example.com/{i}"),
                )
            })
            .collect();
        let groups = group_research_results(cards, 5);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].results.len(), 2);
        assert!(!groups[0].truncated);
    }

    #[test]
    fn group_results_empty_groups_excluded() {
        let cards = vec![
            make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
            make_card(SourceKind::OfficialDocs, "https://docs.rs/serde"),
        ];
        let groups = group_research_results(cards, 10);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, ResearchResultGroupKind::OfficialDocs);
    }
}
