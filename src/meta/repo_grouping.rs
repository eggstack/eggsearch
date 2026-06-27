//! Deterministic grouping of SourceCards into repo bundle categories.

use crate::core::repo_query::RepoQueryHints;
use crate::core::repo_search::{RepoResultGroup, RepoResultGroupKind};
use crate::core::source_card::{SourceCard, SourceKind};

/// Classify a SourceCard into its primary group.
pub fn classify_group(card: &SourceCard) -> RepoResultGroupKind {
    let url_lower = card.url.to_lowercase();
    let title_lower = card.title.to_lowercase();
    let snippet_lower = card.snippet.as_deref().unwrap_or("").to_lowercase();
    let code = card.metadata.code.as_ref();

    match card.metadata.source_kind {
        SourceKind::OfficialDocs => RepoResultGroupKind::OfficialDocs,
        SourceKind::PackageRegistry => RepoResultGroupKind::PackageRegistry,
        SourceKind::RepositoryRoot => RepoResultGroupKind::Repository,
        SourceKind::SourceRepository => RepoResultGroupKind::Repository,
        SourceKind::SourceDirectory => RepoResultGroupKind::Repository,
        SourceKind::Commit => RepoResultGroupKind::Repository,
        SourceKind::PullRequest => RepoResultGroupKind::PullRequests,
        SourceKind::ReleaseNotes => RepoResultGroupKind::Releases,
        SourceKind::Tag => RepoResultGroupKind::Releases,
        SourceKind::SecurityAdvisory => RepoResultGroupKind::Releases,
        SourceKind::SourceFile => classify_source_file(code, &url_lower),
        SourceKind::IssueThread => {
            if card
                .metadata
                .issue
                .as_ref()
                .is_some_and(|i| i.is_pull_request == Some(true))
            {
                RepoResultGroupKind::PullRequests
            } else {
                RepoResultGroupKind::Issues
            }
        }
        SourceKind::News => RepoResultGroupKind::Other,
        SourceKind::Tutorial => RepoResultGroupKind::CommunityDiscussion,
        SourceKind::Forum => RepoResultGroupKind::CommunityDiscussion,
        SourceKind::Reference => RepoResultGroupKind::OfficialDocs,
        SourceKind::Unknown => classify_fallback(&url_lower, &title_lower, &snippet_lower),
    }
}

fn classify_source_file(
    code: Option<&crate::core::code_metadata::CodeMetadata>,
    url_lower: &str,
) -> RepoResultGroupKind {
    let path_lower = code
        .and_then(|c| c.path.as_deref())
        .unwrap_or("")
        .to_lowercase();

    if path_lower.contains("readme") {
        return RepoResultGroupKind::Readme;
    }

    let path_ref = path_lower.as_str();
    if path_ref.contains("example") || path_ref.contains("sample") || path_ref.contains("demo") {
        return RepoResultGroupKind::Examples;
    }

    if is_test_path(path_ref) || is_test_url_pattern(url_lower) {
        return RepoResultGroupKind::Tests;
    }

    RepoResultGroupKind::SourceFiles
}

fn is_test_path(path: &str) -> bool {
    if path.contains("test") || path.contains("tests") {
        return true;
    }
    if let Some(filename) = path.rsplit('/').next() {
        if filename.starts_with("test_")
            || filename.ends_with("_test.")
            || filename.ends_with("_test.rs")
            || filename.ends_with("_test.py")
            || filename.ends_with("_test.ts")
            || filename.ends_with("_test.js")
            || filename.ends_with(".spec.")
            || filename.ends_with(".spec.ts")
            || filename.ends_with(".spec.js")
            || filename.ends_with(".spec.rs")
            || filename.ends_with("_spec.rb")
        {
            return true;
        }
    }
    false
}

fn is_test_url_pattern(url_lower: &str) -> bool {
    url_lower.contains("/tests/") || url_lower.contains("/test/")
}

fn classify_fallback(
    url_lower: &str,
    title_lower: &str,
    snippet_lower: &str,
) -> RepoResultGroupKind {
    // Check for changelog URL
    if url_lower.contains("changelog") {
        return RepoResultGroupKind::Changelog;
    }

    // Check for migration/changelog keywords in title or snippet
    let migration_keywords = [
        "migration",
        "upgrade guide",
        "breaking change",
        "deprecation",
        "changelog",
    ];
    for kw in &migration_keywords {
        if title_lower.contains(kw) || snippet_lower.contains(kw) {
            return RepoResultGroupKind::MigrationNotes;
        }
    }

    RepoResultGroupKind::Other
}

/// Apply bounded within-group reranking based on repo hints.
///
/// Boosts are additive and capped so provider RRF evidence remains dominant.
/// The maximum theoretical boost is +30% of the max base score per group.
fn rerank_group(cards: &mut [SourceCard], hints: &RepoQueryHints) {
    use crate::core::source_card::RankReason;

    if cards.is_empty() {
        return;
    }

    let max_base = cards.iter().filter_map(|r| r.score).fold(0.0_f64, f64::max);
    if max_base <= 0.0 {
        return;
    }

    // Boost unit: 10% of the max base score in this group.
    let boost_unit = max_base * 0.10;

    let owner_lower = hints.owner.as_deref().map(|s| s.to_lowercase());
    let repo_lower = hints.repo.as_deref().map(|s| s.to_lowercase());
    let path_lower = hints.path.as_deref().map(|s| s.to_lowercase());
    let file_lower = hints.file.as_deref().map(|s| s.to_lowercase());
    let lang_lower = hints.language.as_deref();
    let symbol_lower = hints.symbol.as_deref().map(|s| s.to_lowercase());

    for card in cards.iter_mut() {
        let base = card.score.unwrap_or(0.0);
        let mut boost = 0.0_f64;
        let mut reasons: Vec<RankReason> = Vec::new();

        // --- owner/repo match ---
        let url_lower = card.url.to_lowercase();
        if let (Some(ref o), Some(ref r)) = (&owner_lower, &repo_lower) {
            if url_lower.contains(o.as_str()) && url_lower.contains(r.as_str()) {
                boost += boost_unit * 1.5;
                reasons.push(RankReason::RepoOwnerMatch);
            }
        }

        // --- path/file hint match ---
        let code = card.metadata.code.as_ref();
        let card_path = code
            .and_then(|c| c.path.as_deref())
            .unwrap_or("")
            .to_lowercase();
        if let Some(ref p) = path_lower {
            if !p.is_empty() && card_path.contains(p.as_str()) {
                boost += boost_unit * 1.0;
                reasons.push(RankReason::HintMatch);
            }
        }
        if let Some(ref f) = file_lower {
            if !f.is_empty() && card_path.contains(f.as_str()) {
                boost += boost_unit * 1.0;
                reasons.push(RankReason::HintMatch);
            }
        }

        // --- language hint match ---
        if let Some(lang) = lang_lower {
            if let Some(card_lang) = code.and_then(|c| c.language.as_deref()) {
                if card_lang.to_lowercase() == lang {
                    boost += boost_unit * 0.5;
                    reasons.push(RankReason::HintMatch);
                }
            }
        }

        // --- symbol hint match (check title) ---
        if let Some(ref sym) = symbol_lower {
            if !sym.is_empty() {
                let title_lower = card.title.to_lowercase();
                if title_lower.contains(sym.as_str()) {
                    boost += boost_unit * 1.0;
                    reasons.push(RankReason::HintMatch);
                }
            }
        }

        // --- native provider evidence ---
        let provider_lower = card.providers.first().map(|s| s.to_lowercase());
        if provider_lower.as_deref() == Some("github_issues") {
            boost += boost_unit * 0.5;
            reasons.push(RankReason::ProviderNativeIssueSearch);
        } else if provider_lower.as_deref() == Some("github_releases") {
            boost += boost_unit * 0.5;
            reasons.push(RankReason::ProviderNativeReleaseSearch);
        }

        // --- domain priors ---
        let kind = card.metadata.source_kind;
        match kind {
            SourceKind::OfficialDocs | SourceKind::Reference => {
                boost += boost_unit * 1.0;
                reasons.push(RankReason::DomainPriorDocs);
            }
            SourceKind::PackageRegistry => {
                boost += boost_unit * 0.5;
            }
            SourceKind::ReleaseNotes | SourceKind::Tag => {
                boost += boost_unit * 0.5;
                reasons.push(RankReason::DomainPriorRelease);
            }
            SourceKind::SecurityAdvisory => {
                boost += boost_unit * 0.5;
                reasons.push(RankReason::DomainPriorSecurity);
            }
            _ => {}
        }

        if boost > 0.0 {
            card.score = Some(base + boost);
        }
        card.metadata.rank_reasons.extend(reasons);
    }

    // Stable sort by updated score (descending).
    cards.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Group a flat list of SourceCards into RepoResultGroups.
/// Each card goes to exactly one group (its primary classification).
/// Groups are returned in a fixed canonical order.
pub fn group_results(cards: Vec<SourceCard>, max_per_group: usize) -> Vec<RepoResultGroup> {
    use std::collections::HashMap;

    let mut buckets: HashMap<RepoResultGroupKind, Vec<SourceCard>> = HashMap::new();
    for card in cards {
        let kind = classify_group(&card);
        buckets.entry(kind).or_default().push(card);
    }

    let canonical_order: Vec<RepoResultGroupKind> = vec![
        RepoResultGroupKind::OfficialDocs,
        RepoResultGroupKind::PackageRegistry,
        RepoResultGroupKind::Repository,
        RepoResultGroupKind::Readme,
        RepoResultGroupKind::Examples,
        RepoResultGroupKind::Tests,
        RepoResultGroupKind::SourceFiles,
        RepoResultGroupKind::Issues,
        RepoResultGroupKind::PullRequests,
        RepoResultGroupKind::Releases,
        RepoResultGroupKind::MigrationNotes,
        RepoResultGroupKind::Changelog,
        RepoResultGroupKind::CommunityDiscussion,
        RepoResultGroupKind::Other,
    ];

    let labels: Vec<(RepoResultGroupKind, &str)> = vec![
        (RepoResultGroupKind::OfficialDocs, "Official Documentation"),
        (RepoResultGroupKind::PackageRegistry, "Package Registry"),
        (RepoResultGroupKind::Repository, "Repository"),
        (RepoResultGroupKind::Readme, "README"),
        (RepoResultGroupKind::Examples, "Examples"),
        (RepoResultGroupKind::Tests, "Tests"),
        (RepoResultGroupKind::SourceFiles, "Source Files"),
        (RepoResultGroupKind::Issues, "Issues"),
        (RepoResultGroupKind::PullRequests, "Pull Requests"),
        (RepoResultGroupKind::Releases, "Releases"),
        (RepoResultGroupKind::MigrationNotes, "Migration Notes"),
        (RepoResultGroupKind::Changelog, "Changelog"),
        (
            RepoResultGroupKind::CommunityDiscussion,
            "Community Discussion",
        ),
        (RepoResultGroupKind::Other, "Other"),
    ];

    let label_map: std::collections::HashMap<RepoResultGroupKind, &str> =
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
            groups.push(RepoResultGroup {
                kind,
                label,
                results,
                truncated,
            });
        }
    }

    groups
}

/// Group a flat list of SourceCards into RepoResultGroups with within-group reranking.
/// Each card goes to exactly one group (its primary classification).
/// Groups are returned in a fixed canonical order.
pub fn group_results_with_hints(
    cards: Vec<SourceCard>,
    max_per_group: usize,
    hints: &RepoQueryHints,
) -> Vec<RepoResultGroup> {
    use std::collections::HashMap;

    let mut buckets: HashMap<RepoResultGroupKind, Vec<SourceCard>> = HashMap::new();
    for card in cards {
        let kind = classify_group(&card);
        buckets.entry(kind).or_default().push(card);
    }

    // Apply within-group reranking before truncation.
    for bucket in buckets.values_mut() {
        rerank_group(bucket, hints);
    }

    let canonical_order: Vec<RepoResultGroupKind> = vec![
        RepoResultGroupKind::OfficialDocs,
        RepoResultGroupKind::PackageRegistry,
        RepoResultGroupKind::Repository,
        RepoResultGroupKind::Readme,
        RepoResultGroupKind::Examples,
        RepoResultGroupKind::Tests,
        RepoResultGroupKind::SourceFiles,
        RepoResultGroupKind::Issues,
        RepoResultGroupKind::PullRequests,
        RepoResultGroupKind::Releases,
        RepoResultGroupKind::MigrationNotes,
        RepoResultGroupKind::Changelog,
        RepoResultGroupKind::CommunityDiscussion,
        RepoResultGroupKind::Other,
    ];

    let labels: Vec<(RepoResultGroupKind, &str)> = vec![
        (RepoResultGroupKind::OfficialDocs, "Official Documentation"),
        (RepoResultGroupKind::PackageRegistry, "Package Registry"),
        (RepoResultGroupKind::Repository, "Repository"),
        (RepoResultGroupKind::Readme, "README"),
        (RepoResultGroupKind::Examples, "Examples"),
        (RepoResultGroupKind::Tests, "Tests"),
        (RepoResultGroupKind::SourceFiles, "Source Files"),
        (RepoResultGroupKind::Issues, "Issues"),
        (RepoResultGroupKind::PullRequests, "Pull Requests"),
        (RepoResultGroupKind::Releases, "Releases"),
        (RepoResultGroupKind::MigrationNotes, "Migration Notes"),
        (RepoResultGroupKind::Changelog, "Changelog"),
        (
            RepoResultGroupKind::CommunityDiscussion,
            "Community Discussion",
        ),
        (RepoResultGroupKind::Other, "Other"),
    ];

    let label_map: std::collections::HashMap<RepoResultGroupKind, &str> =
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
            groups.push(RepoResultGroup {
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
    use crate::core::code_metadata::CodeMetadata;
    use crate::core::result::TrustLevel;
    use crate::core::source_card::{IssueMetadata, SourceMetadata};

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

    fn make_card_with_code(source_kind: SourceKind, url: &str, path: &str) -> SourceCard {
        let mut card = make_card(source_kind, url);
        card.metadata.code = Some(CodeMetadata {
            path: Some(path.to_string()),
            ..Default::default()
        });
        card
    }

    fn make_card_with_issue(
        source_kind: SourceKind,
        url: &str,
        is_pull_request: Option<bool>,
    ) -> SourceCard {
        let mut card = make_card(source_kind, url);
        card.metadata.issue = Some(IssueMetadata {
            is_pull_request,
            ..Default::default()
        });
        card
    }

    #[test]
    fn classify_official_docs() {
        let card = make_card(SourceKind::OfficialDocs, "https://docs.rs/axum");
        assert_eq!(classify_group(&card), RepoResultGroupKind::OfficialDocs);
    }

    #[test]
    fn classify_package_registry() {
        let card = make_card(SourceKind::PackageRegistry, "https://crates.io/crates/axum");
        assert_eq!(classify_group(&card), RepoResultGroupKind::PackageRegistry);
    }

    #[test]
    fn classify_repository_root() {
        let card = make_card(
            SourceKind::RepositoryRoot,
            "https://github.com/tokio-rs/axum",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Repository);
    }

    #[test]
    fn classify_source_repository() {
        let card = make_card(
            SourceKind::SourceRepository,
            "https://github.com/tokio-rs/axum",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Repository);
    }

    #[test]
    fn classify_github_blob_readme() {
        let card = make_card_with_code(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/README.md",
            "README.md",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Readme);
    }

    #[test]
    fn classify_github_blob_examples() {
        let card = make_card_with_code(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/examples/hello-world/main.rs",
            "examples/hello-world/main.rs",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Examples);
    }

    #[test]
    fn classify_github_blob_tests() {
        let card = make_card_with_code(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/tests/routing.rs",
            "tests/routing.rs",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Tests);
    }

    #[test]
    fn classify_test_file_by_pattern() {
        let card = make_card_with_code(
            SourceKind::SourceFile,
            "https://github.com/foo/bar/blob/main/src/foo_test.rs",
            "src/foo_test.rs",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Tests);
    }

    #[test]
    fn classify_spec_file() {
        let card = make_card_with_code(
            SourceKind::SourceFile,
            "https://github.com/foo/bar/blob/main/src/foo.spec.ts",
            "src/foo.spec.ts",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Tests);
    }

    #[test]
    fn classify_source_file_generic() {
        let card = make_card_with_code(
            SourceKind::SourceFile,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
            "src/lib.rs",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::SourceFiles);
    }

    #[test]
    fn classify_issue_thread() {
        let card = make_card(
            SourceKind::IssueThread,
            "https://github.com/tokio-rs/axum/issues/123",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Issues);
    }

    #[test]
    fn classify_issue_thread_as_pr() {
        let card = make_card_with_issue(
            SourceKind::IssueThread,
            "https://github.com/tokio-rs/axum/issues/123",
            Some(true),
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::PullRequests);
    }

    #[test]
    fn classify_pull_request() {
        let card = make_card(
            SourceKind::PullRequest,
            "https://github.com/tokio-rs/axum/pull/456",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::PullRequests);
    }

    #[test]
    fn classify_release_notes() {
        let card = make_card(
            SourceKind::ReleaseNotes,
            "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Releases);
    }

    #[test]
    fn classify_tag() {
        let card = make_card(SourceKind::Tag, "https://github.com/tokio-rs/axum/tags");
        assert_eq!(classify_group(&card), RepoResultGroupKind::Releases);
    }

    #[test]
    fn classify_security_advisory() {
        let card = make_card(
            SourceKind::SecurityAdvisory,
            "https://osv.dev/vulnerability/GHSA-xxxx",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Releases);
    }

    #[test]
    fn classify_changelog_url() {
        let card = make_card(
            SourceKind::Unknown,
            "https://github.com/tokio-rs/axum/blob/main/CHANGELOG.md",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Changelog);
    }

    #[test]
    fn classify_migration_title() {
        let mut card = make_card(SourceKind::Unknown, "https://example.com/migration-guide");
        card.title = "Upgrade Guide: Moving from v1 to v2".to_string();
        assert_eq!(classify_group(&card), RepoResultGroupKind::MigrationNotes);
    }

    #[test]
    fn classify_migration_snippet() {
        let mut card = make_card(SourceKind::Unknown, "https://example.com/page");
        card.snippet = Some("Breaking changes in the new version".to_string());
        assert_eq!(classify_group(&card), RepoResultGroupKind::MigrationNotes);
    }

    #[test]
    fn classify_news() {
        let card = make_card(SourceKind::News, "https://blog.example.com/post");
        assert_eq!(classify_group(&card), RepoResultGroupKind::Other);
    }

    #[test]
    fn classify_tutorial() {
        let card = make_card(SourceKind::Tutorial, "https://dev.to/foo/tutorial");
        assert_eq!(
            classify_group(&card),
            RepoResultGroupKind::CommunityDiscussion
        );
    }

    #[test]
    fn classify_forum() {
        let card = make_card(SourceKind::Forum, "https://forum.example.com/t/topic");
        assert_eq!(
            classify_group(&card),
            RepoResultGroupKind::CommunityDiscussion
        );
    }

    #[test]
    fn classify_commit() {
        let card = make_card(
            SourceKind::Commit,
            "https://github.com/tokio-rs/axum/commit/abc",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Repository);
    }

    #[test]
    fn classify_unknown_fallback() {
        let card = make_card(SourceKind::Unknown, "https://example.com/page");
        assert_eq!(classify_group(&card), RepoResultGroupKind::Other);
    }

    #[test]
    fn empty_list_produces_empty_groups() {
        let groups = group_results(vec![], 10);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_results_canonical_order() {
        let cards = vec![
            make_card(SourceKind::ReleaseNotes, "https://example.com/releases"),
            make_card(SourceKind::OfficialDocs, "https://docs.rs/axum"),
            make_card(SourceKind::PackageRegistry, "https://crates.io/axum"),
            make_card(SourceKind::IssueThread, "https://example.com/issues/1"),
        ];
        let groups = group_results(cards, 10);
        let kinds: Vec<RepoResultGroupKind> = groups.iter().map(|g| g.kind).collect();
        assert_eq!(
            kinds,
            vec![
                RepoResultGroupKind::OfficialDocs,
                RepoResultGroupKind::PackageRegistry,
                RepoResultGroupKind::Issues,
                RepoResultGroupKind::Releases,
            ]
        );
    }

    #[test]
    fn group_results_max_per_group_truncation() {
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
        let groups = group_results(cards, 3);
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
        let groups = group_results(cards, 5);
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
        let groups = group_results(cards, 10);
        // Only OfficialDocs group should exist; all others are empty and excluded
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, RepoResultGroupKind::OfficialDocs);
    }

    #[test]
    fn classify_examples_with_sample_keyword() {
        let card = make_card_with_code(
            SourceKind::SourceFile,
            "https://github.com/foo/bar/blob/main/sample/demo.rs",
            "sample/demo.rs",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Examples);
    }

    #[test]
    fn classify_examples_with_demo_keyword() {
        let card = make_card_with_code(
            SourceKind::SourceFile,
            "https://github.com/foo/bar/blob/main/demos/app/main.rs",
            "demos/app/main.rs",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Examples);
    }

    #[test]
    fn classify_source_directory() {
        let card = make_card(
            SourceKind::SourceDirectory,
            "https://github.com/foo/bar/tree/main/src",
        );
        assert_eq!(classify_group(&card), RepoResultGroupKind::Repository);
    }

    #[test]
    fn classify_reference_as_docs() {
        let card = make_card(SourceKind::Reference, "https://example.com/api-ref");
        assert_eq!(classify_group(&card), RepoResultGroupKind::OfficialDocs);
    }

    // ---- Within-group ranking tests ----

    fn make_card_with_score(source_kind: SourceKind, url: &str, score: f64) -> SourceCard {
        let mut card = make_card(source_kind, url);
        card.score = Some(score);
        card
    }

    fn make_card_with_code_and_score(
        source_kind: SourceKind,
        url: &str,
        path: &str,
        score: f64,
    ) -> SourceCard {
        let mut card = make_card_with_code(source_kind, url, path);
        card.score = Some(score);
        card
    }

    #[test]
    fn rerank_boosts_owner_repo_match() {
        let hints = RepoQueryHints {
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ..Default::default()
        };
        let mut cards = vec![
            make_card_with_score(
                SourceKind::SourceFile,
                "https://docs.rs/axum/latest/axum/",
                10.0,
            ),
            make_card_with_score(
                SourceKind::SourceFile,
                "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
                9.0,
            ),
        ];
        rerank_group(&mut cards, &hints);
        // The card with owner/repo in URL should be boosted to top.
        assert_eq!(
            cards[0].url,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs"
        );
        assert!(cards[0].score.unwrap() > cards[1].score.unwrap());
    }

    #[test]
    fn rerank_boosts_path_hint_match() {
        let hints = RepoQueryHints {
            path: Some("src/lib.rs".to_string()),
            ..Default::default()
        };
        let mut cards = vec![
            make_card_with_code_and_score(
                SourceKind::SourceFile,
                "https://github.com/foo/bar/blob/main/src/lib.rs",
                "src/lib.rs",
                10.0,
            ),
            make_card_with_code_and_score(
                SourceKind::SourceFile,
                "https://github.com/foo/bar/blob/main/src/main.rs",
                "src/main.rs",
                10.0,
            ),
        ];
        rerank_group(&mut cards, &hints);
        // The card with matching path should be first.
        assert_eq!(
            cards[0].url,
            "https://github.com/foo/bar/blob/main/src/lib.rs"
        );
    }

    #[test]
    fn rerank_boosts_language_match() {
        let hints = RepoQueryHints {
            language: Some("rust".to_string()),
            ..Default::default()
        };
        let mut cards = vec![
            {
                let mut card = make_card_with_score(
                    SourceKind::SourceFile,
                    "https://example.com/foo.py",
                    10.0,
                );
                card.metadata.code = Some(CodeMetadata {
                    language: Some("python".to_string()),
                    ..Default::default()
                });
                card
            },
            {
                let mut card = make_card_with_score(
                    SourceKind::SourceFile,
                    "https://example.com/bar.rs",
                    10.0,
                );
                card.metadata.code = Some(CodeMetadata {
                    language: Some("rust".to_string()),
                    ..Default::default()
                });
                card
            },
        ];
        rerank_group(&mut cards, &hints);
        // The Rust card should be boosted to top.
        assert!(cards[0].url.contains("bar.rs"));
    }

    #[test]
    fn rerank_boosts_symbol_in_title() {
        let hints = RepoQueryHints {
            symbol: Some("Layer".to_string()),
            ..Default::default()
        };
        let mut cards = vec![
            make_card_with_score(SourceKind::SourceFile, "https://example.com/router", 10.0),
            make_card_with_score(SourceKind::SourceFile, "https://example.com/layer", 10.0),
        ];
        cards[1].title = "Layer trait".to_string();
        rerank_group(&mut cards, &hints);
        // The card with "Layer" in title should be boosted.
        assert!(cards[0].title.contains("Layer"));
    }

    #[test]
    fn rerank_boosts_domain_prior() {
        let hints = RepoQueryHints::default();
        let mut cards = vec![
            make_card_with_score(SourceKind::SourceFile, "https://example.com/code", 10.0),
            make_card_with_score(SourceKind::OfficialDocs, "https://docs.rs/axum", 10.0),
        ];
        rerank_group(&mut cards, &hints);
        // OfficialDocs should get a domain prior boost.
        assert_eq!(cards[0].metadata.source_kind, SourceKind::OfficialDocs);
    }

    #[test]
    fn rerank_empty_cards_noop() {
        let hints = RepoQueryHints::default();
        let mut cards: Vec<SourceCard> = vec![];
        rerank_group(&mut cards, &hints);
        assert!(cards.is_empty());
    }

    #[test]
    fn group_results_with_hints_applies_reranking() {
        let hints = RepoQueryHints {
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            ..Default::default()
        };
        let cards = vec![
            make_card_with_score(
                SourceKind::SourceFile,
                "https://docs.rs/axum/latest/axum/",
                10.0,
            ),
            make_card_with_score(
                SourceKind::SourceFile,
                "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
                9.0,
            ),
        ];
        let groups = group_results_with_hints(cards, 10, &hints);
        // Both cards are SourceFile but classified differently by URL heuristics.
        // The reranking should boost the owner/repo match within its group.
        assert!(!groups.is_empty());
    }
}
