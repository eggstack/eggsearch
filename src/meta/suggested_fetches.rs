//! Suggested fetch generation for repo bundle search.

use crate::core::fetch::ExtractMode;
use crate::core::repo_query::RepoQueryHints;
use crate::core::repo_search::{RepoResultGroup, RepoResultGroupKind, RepoSuggestedFetch};
use crate::core::source_card::SourceKind;

const SUGGESTION_CAP: usize = 8;

#[derive(Clone, Copy)]
struct SuggestionRule {
    group: RepoResultGroupKind,
    reason: &'static str,
    expected_kind: SourceKind,
    extract_mode: Option<ExtractMode>,
}

const SUGGESTION_RULES: &[SuggestionRule] = &[
    SuggestionRule {
        group: RepoResultGroupKind::OfficialDocs,
        reason: "official_docs",
        expected_kind: SourceKind::OfficialDocs,
        extract_mode: Some(ExtractMode::Markdown),
    },
    SuggestionRule {
        group: RepoResultGroupKind::PackageRegistry,
        reason: "package_registry",
        expected_kind: SourceKind::PackageRegistry,
        extract_mode: Some(ExtractMode::Markdown),
    },
    SuggestionRule {
        group: RepoResultGroupKind::Readme,
        reason: "readme",
        expected_kind: SourceKind::SourceFile,
        extract_mode: Some(ExtractMode::Markdown),
    },
    SuggestionRule {
        group: RepoResultGroupKind::SourceFiles,
        reason: "source_file_symbol_match",
        expected_kind: SourceKind::SourceFile,
        extract_mode: None,
    },
    SuggestionRule {
        group: RepoResultGroupKind::Examples,
        reason: "example_file",
        expected_kind: SourceKind::SourceFile,
        extract_mode: None,
    },
    SuggestionRule {
        group: RepoResultGroupKind::Releases,
        reason: "recent_release",
        expected_kind: SourceKind::ReleaseNotes,
        extract_mode: Some(ExtractMode::Markdown),
    },
    SuggestionRule {
        group: RepoResultGroupKind::MigrationNotes,
        reason: "migration_note",
        expected_kind: SourceKind::Tutorial,
        extract_mode: Some(ExtractMode::Markdown),
    },
    SuggestionRule {
        group: RepoResultGroupKind::Issues,
        reason: "issue_thread",
        expected_kind: SourceKind::IssueThread,
        extract_mode: None,
    },
];

/// Generate suggested fetches from grouped results and resolved hints.
pub fn generate_suggested_fetches(
    groups: &[RepoResultGroup],
    _hints: &RepoQueryHints,
) -> Vec<RepoSuggestedFetch> {
    let mut suggestions = Vec::new();

    for rule in SUGGESTION_RULES {
        let Some(group) = find_group(groups, rule.group) else {
            continue;
        };
        let Some(card) = group.results.first() else {
            continue;
        };

        let fetch_url = card
            .metadata
            .code_evidence
            .as_ref()
            .and_then(|ce| ce.raw_url.as_deref().or(ce.permalink_url.as_deref()))
            .unwrap_or(&card.url);

        // Build structured repo_fetch request when code evidence
        // has all required locator fields.
        let structured = card
            .metadata
            .code_evidence
            .as_ref()
            .and_then(|ce| {
                let host = ce.host?;
                let owner = ce.owner.as_deref()?;
                let repo = ce.repo.as_deref()?;
                let ref_name = ce.ref_name.as_deref()?;
                let path = ce.path.as_deref()?;
                Some(crate::core::repo_fetch::RepoFetchRequest {
                    host: Some(host),
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                    ref_name: Some(ref_name.to_string()),
                    commit_sha: ce.commit_sha.clone(),
                    path: path.to_string(),
                    line_start: ce.match_line_start,
                    line_end: ce.match_line_end,
                    context_before: ce.context_line_start.map(|_| 3),
                    context_after: ce.context_line_end.map(|_| 3),
                    max_chars: None,
                    timeout_ms: None,
                })
            });

        let priority = suggestions.len().saturating_add(1) as u8;
        suggestions.push(RepoSuggestedFetch {
            url: fetch_url.to_string(),
            reason: rule.reason.to_string(),
            group: rule.group,
            expected_kind: rule.expected_kind,
            recommended_extract_mode: rule.extract_mode,
            priority,
            structured_repo_fetch: structured,
        });

        if suggestions.len() >= SUGGESTION_CAP {
            break;
        }
    }

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
