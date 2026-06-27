//! Suggested fetch generation for repo bundle search.

use crate::core::fetch::ExtractMode;
use crate::core::repo_query::RepoQueryHints;
use crate::core::repo_search::{RepoResultGroup, RepoResultGroupKind, RepoSuggestedFetch};
use crate::core::source_card::SourceKind;

/// Generate suggested fetches from grouped results and resolved hints.
pub fn generate_suggested_fetches(
    groups: &[RepoResultGroup],
    _hints: &RepoQueryHints,
) -> Vec<RepoSuggestedFetch> {
    let mut suggestions = Vec::new();
    let mut priority: u8 = 1;

    if let Some(group) = find_group(groups, RepoResultGroupKind::OfficialDocs) {
        if let Some(card) = group.results.first() {
            suggestions.push(RepoSuggestedFetch {
                url: card.url.clone(),
                reason: "official_docs".to_string(),
                group: RepoResultGroupKind::OfficialDocs,
                expected_kind: SourceKind::OfficialDocs,
                recommended_extract_mode: Some(ExtractMode::Markdown),
                priority,
            });
            priority = priority.saturating_add(1);
        }
    }

    if let Some(group) = find_group(groups, RepoResultGroupKind::PackageRegistry) {
        if let Some(card) = group.results.first() {
            suggestions.push(RepoSuggestedFetch {
                url: card.url.clone(),
                reason: "package_registry".to_string(),
                group: RepoResultGroupKind::PackageRegistry,
                expected_kind: SourceKind::PackageRegistry,
                recommended_extract_mode: Some(ExtractMode::Markdown),
                priority,
            });
            priority = priority.saturating_add(1);
        }
    }

    if let Some(group) = find_group(groups, RepoResultGroupKind::Readme) {
        if let Some(card) = group.results.first() {
            suggestions.push(RepoSuggestedFetch {
                url: card.url.clone(),
                reason: "readme".to_string(),
                group: RepoResultGroupKind::Readme,
                expected_kind: SourceKind::SourceFile,
                recommended_extract_mode: Some(ExtractMode::Markdown),
                priority,
            });
            priority = priority.saturating_add(1);
        }
    }

    if let Some(group) = find_group(groups, RepoResultGroupKind::SourceFiles) {
        if let Some(card) = group.results.first() {
            suggestions.push(RepoSuggestedFetch {
                url: card.url.clone(),
                reason: "source_file_symbol_match".to_string(),
                group: RepoResultGroupKind::SourceFiles,
                expected_kind: SourceKind::SourceFile,
                recommended_extract_mode: None,
                priority,
            });
            priority = priority.saturating_add(1);
        }
    }

    if let Some(group) = find_group(groups, RepoResultGroupKind::Examples) {
        if let Some(card) = group.results.first() {
            suggestions.push(RepoSuggestedFetch {
                url: card.url.clone(),
                reason: "example_file".to_string(),
                group: RepoResultGroupKind::Examples,
                expected_kind: SourceKind::SourceFile,
                recommended_extract_mode: None,
                priority,
            });
            priority = priority.saturating_add(1);
        }
    }

    if let Some(group) = find_group(groups, RepoResultGroupKind::Releases) {
        if let Some(card) = group.results.first() {
            suggestions.push(RepoSuggestedFetch {
                url: card.url.clone(),
                reason: "recent_release".to_string(),
                group: RepoResultGroupKind::Releases,
                expected_kind: SourceKind::ReleaseNotes,
                recommended_extract_mode: Some(ExtractMode::Markdown),
                priority,
            });
            priority = priority.saturating_add(1);
        }
    }

    if let Some(group) = find_group(groups, RepoResultGroupKind::MigrationNotes) {
        if let Some(card) = group.results.first() {
            suggestions.push(RepoSuggestedFetch {
                url: card.url.clone(),
                reason: "migration_note".to_string(),
                group: RepoResultGroupKind::MigrationNotes,
                expected_kind: SourceKind::Tutorial,
                recommended_extract_mode: Some(ExtractMode::Markdown),
                priority,
            });
            priority = priority.saturating_add(1);
        }
    }

    if let Some(group) = find_group(groups, RepoResultGroupKind::Issues) {
        if let Some(card) = group.results.first() {
            suggestions.push(RepoSuggestedFetch {
                url: card.url.clone(),
                reason: "issue_thread".to_string(),
                group: RepoResultGroupKind::Issues,
                expected_kind: SourceKind::IssueThread,
                recommended_extract_mode: None,
                priority,
            });
        }
    }

    suggestions.truncate(8);
    suggestions
}

fn find_group(groups: &[RepoResultGroup], kind: RepoResultGroupKind) -> Option<&RepoResultGroup> {
    groups.iter().find(|g| g.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::repo_search::RepoResultGroup;
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

    fn make_group(kind: RepoResultGroupKind, cards: Vec<SourceCard>) -> RepoResultGroup {
        RepoResultGroup {
            kind,
            label: format!("{kind:?}"),
            results: cards,
            truncated: false,
        }
    }

    #[test]
    fn generates_fetch_for_official_docs() {
        let groups = vec![make_group(
            RepoResultGroupKind::OfficialDocs,
            vec![make_card("Docs", "https://docs.example.com")],
        )];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);
        assert_eq!(fetches.len(), 1);
        assert_eq!(fetches[0].reason, "official_docs");
        assert_eq!(fetches[0].priority, 1);
    }

    #[test]
    fn generates_fetches_in_priority_order() {
        let groups = vec![
            make_group(
                RepoResultGroupKind::OfficialDocs,
                vec![make_card("Docs", "https://docs.example.com")],
            ),
            make_group(
                RepoResultGroupKind::SourceFiles,
                vec![make_card(
                    "Source",
                    "https://github.com/foo/bar/blob/main/src/lib.rs",
                )],
            ),
            make_group(
                RepoResultGroupKind::Issues,
                vec![make_card("Issue #1", "https://github.com/foo/bar/issues/1")],
            ),
        ];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);
        assert!(fetches.len() >= 3);
        assert_eq!(fetches[0].reason, "official_docs");
        assert_eq!(fetches[1].reason, "source_file_symbol_match");
        assert_eq!(fetches[2].reason, "issue_thread");
    }

    #[test]
    fn caps_at_8_suggestions() {
        let mut groups = Vec::new();
        for kind in [
            RepoResultGroupKind::OfficialDocs,
            RepoResultGroupKind::PackageRegistry,
            RepoResultGroupKind::Readme,
            RepoResultGroupKind::SourceFiles,
            RepoResultGroupKind::Examples,
            RepoResultGroupKind::Releases,
            RepoResultGroupKind::MigrationNotes,
            RepoResultGroupKind::Issues,
            RepoResultGroupKind::PullRequests,
        ] {
            groups.push(make_group(
                kind,
                vec![make_card("x", "https://example.com")],
            ));
        }
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);
        assert!(fetches.len() <= 8);
    }

    #[test]
    fn empty_groups_produce_no_suggestions() {
        let groups = vec![];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);
        assert!(fetches.is_empty());
    }
}
