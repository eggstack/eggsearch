//! Suggested fetch generation for repo bundle search.
//!
//! Uses the deterministic ranking pipeline from `fetch_ranking` to
//! score, rank, and diversify fetch candidates extracted from grouped
//! search results.

use crate::core::fetch::ExtractMode;
use crate::core::repo_fetch::RepoFetchRequest;
use crate::core::repo_query::RepoQueryHints;
use crate::core::repo_search::{RepoResultGroup, RepoResultGroupKind, RepoSuggestedFetch};
use crate::core::source_card::SourceKind;

use super::fetch_ranking::{
    extract_domain, is_pinned_permalink, is_raw_url, rank_and_select, DiversityConfig,
    FetchCandidate, FetchRankMode, RankContext,
};

/// Resolve the fetch URL from a card using the code_evidence URL priority.
fn resolve_fetch_url(card: &crate::core::source_card::SourceCard) -> &str {
    card.metadata
        .code_evidence
        .as_ref()
        .and_then(|ce| {
            ce.raw_permalink_url
                .as_deref()
                .or(ce.raw_url.as_deref())
                .or(ce.permalink_url.as_deref())
                .or(ce.browser_url.as_deref())
        })
        .unwrap_or(&card.url)
}

/// Build a structured `RepoFetchRequest` when code evidence has all
/// required locator fields.
fn build_structured_fetch(card: &crate::core::source_card::SourceCard) -> Option<RepoFetchRequest> {
    let ce = card.metadata.code_evidence.as_ref()?;
    let host = ce.host?;
    let owner = ce.owner.as_deref()?;
    let repo = ce.repo.as_deref()?;
    let ref_name = ce.ref_name.as_deref()?;
    let path = ce.path.as_deref()?;
    Some(RepoFetchRequest {
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
        symbol: ce.matched_symbol.clone(),
        symbol_kind: ce.symbol_kind,
        match_text: None,
        expand_to_block: Some(matches!(
            ce.source_role,
            Some(crate::core::code_evidence::SourceRole::Implementation)
                | Some(crate::core::code_evidence::SourceRole::Test)
                | Some(crate::core::code_evidence::SourceRole::Example)
        )),
        max_block_lines: None,
        prefer_local: None,
    })
}

/// Determine expected content kind from source kind.
fn expected_kind_for(source_kind: SourceKind) -> SourceKind {
    source_kind
}

/// Determine recommended extract mode from source kind and group.
fn recommended_extract_mode(
    source_kind: SourceKind,
    group: &RepoResultGroupKind,
) -> Option<ExtractMode> {
    match group {
        RepoResultGroupKind::OfficialDocs
        | RepoResultGroupKind::PackageRegistry
        | RepoResultGroupKind::Readme
        | RepoResultGroupKind::Releases
        | RepoResultGroupKind::MigrationNotes
        | RepoResultGroupKind::Changelog => Some(ExtractMode::Markdown),
        _ => match source_kind {
            SourceKind::OfficialDocs | SourceKind::PackageRegistry => Some(ExtractMode::Markdown),
            _ => None,
        },
    }
}

/// Generate suggested fetches from grouped results and resolved hints.
///
/// Delegates to [`generate_suggested_fetches_with_mode`] with
/// [`FetchRankMode::Normal`].
pub fn generate_suggested_fetches(
    groups: &[RepoResultGroup],
    hints: &RepoQueryHints,
) -> Vec<RepoSuggestedFetch> {
    generate_suggested_fetches_with_mode(groups, hints, FetchRankMode::Normal)
}

/// Generate suggested fetches with a specific ranking mode.
///
/// Builds `FetchCandidate` from every card in every group, ranks them
/// via the deterministic scoring pipeline with diversity caps, and
/// converts the top candidates to `RepoSuggestedFetch`.
pub fn generate_suggested_fetches_with_mode(
    groups: &[RepoResultGroup],
    hints: &RepoQueryHints,
    mode: FetchRankMode,
) -> Vec<RepoSuggestedFetch> {
    let ctx = RankContext::from_hints(hints, mode);

    // Build a map from lowercase group label to group kind for later lookup.
    let label_to_kind: std::collections::HashMap<String, RepoResultGroupKind> = groups
        .iter()
        .map(|g| {
            let label = format!("{:?}", g.kind).to_lowercase();
            (label, g.kind)
        })
        .collect();

    // Carry structured fetch requests alongside candidates, keyed by URL.
    let mut structured_fetch_map: std::collections::HashMap<String, Option<RepoFetchRequest>> =
        std::collections::HashMap::new();

    // Build candidates from every card in every group.
    let mut candidates = Vec::new();
    for group in groups {
        let group_label = format!("{:?}", group.kind).to_lowercase();
        for (idx, card) in group.results.iter().enumerate() {
            let fetch_url = resolve_fetch_url(card).to_string();
            let source_kind = card.metadata.source_kind;
            let code_evidence = card.metadata.code_evidence.as_ref();
            let source_role = code_evidence.and_then(|ce| ce.source_role);
            let evidence_confidence = code_evidence.and_then(|ce| ce.evidence_confidence);
            let structured = build_structured_fetch(card);

            structured_fetch_map
                .entry(fetch_url.clone())
                .or_insert_with(|| structured);

            candidates.push(FetchCandidate {
                url: fetch_url.clone(),
                structured_repo_fetch: structured_fetch_map[&fetch_url].is_some(),
                group: group_label.clone(),
                expected_kind: expected_kind_for(source_kind),
                recommended_extract_mode: recommended_extract_mode(source_kind, &group.kind),
                original_order: idx,
                source_kind,
                source_role,
                evidence_confidence,
                is_pinned_permalink: is_pinned_permalink(&fetch_url),
                is_raw_url: is_raw_url(&fetch_url),
                is_browser_url: fetch_url.starts_with("http"),
                domain: extract_domain(&fetch_url),
                score: 0,
                reasons: Vec::new(),
                information_gain: 0.0,
                stable: false,
            });
        }
    }

    // Rank with diversity caps.
    let config = DiversityConfig::default();
    let ranked = rank_and_select(candidates, &ctx, &config);

    // Convert ranked candidates to RepoSuggestedFetch.
    ranked
        .into_iter()
        .enumerate()
        .map(|(pos, candidate)| {
            let group_kind = label_to_kind
                .get(&candidate.group)
                .copied()
                .unwrap_or(RepoResultGroupKind::Other);

            let structured_repo_fetch = structured_fetch_map.remove(&candidate.url).flatten();

            let reason = candidate
                .reasons
                .first()
                .map(|r| r.as_str())
                .unwrap_or("suggested")
                .to_string();

            let rank_reasons: Vec<String> = candidate
                .reasons
                .iter()
                .map(|r| r.as_str().to_string())
                .collect();

            let preferred_tool = if candidate.structured_repo_fetch {
                Some("repo_fetch".to_string())
            } else {
                Some("web_fetch".to_string())
            };

            RepoSuggestedFetch {
                url: candidate.url,
                reason,
                group: group_kind,
                expected_kind: candidate.expected_kind,
                recommended_extract_mode: candidate.recommended_extract_mode,
                priority: (pos + 1) as u8,
                structured_repo_fetch,
                score: Some(candidate.score),
                rank_reasons,
                information_gain: Some(candidate.information_gain),
                stable: Some(candidate.stable),
                stable_id: None,
                preferred_tool,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::code_evidence::{CodeEvidence, EvidenceConfidence};
    use crate::core::code_metadata::CodeHost;
    use crate::core::repo_search::RepoResultGroup;
    use crate::core::result::TrustLevel;
    use crate::core::source_card::{SourceCard, SourceKind, SourceMetadata};

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
            quality_summary: None,
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
        assert_eq!(fetches[0].group, RepoResultGroupKind::OfficialDocs);
        assert_eq!(fetches[0].priority, 1);
        assert!(fetches[0].score.is_some());
        assert!(!fetches[0].rank_reasons.is_empty());
    }

    #[test]
    fn generates_fetches_scored_and_ranked() {
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
        // Official docs outranks source files and issues in normal mode.
        assert_eq!(fetches[0].group, RepoResultGroupKind::OfficialDocs);
        assert_eq!(fetches[1].group, RepoResultGroupKind::SourceFiles);
        assert_eq!(fetches[2].group, RepoResultGroupKind::Issues);
        // Score and rank_reasons are populated.
        assert!(fetches[0].score.is_some());
        assert!(!fetches[0].rank_reasons.is_empty());
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

    #[test]
    fn code_evidence_browser_url_wins_over_card_url() {
        let mut card = make_card("Source", "https://example.com/raw/card-url");
        card.metadata = SourceMetadata {
            source_kind: SourceKind::SourceFile,
            code_evidence: Some(CodeEvidence {
                host: Some(CodeHost::Github),
                owner: Some("owner".to_string()),
                repo: Some("repo".to_string()),
                ref_name: Some("main".to_string()),
                path: Some("src/lib.rs".to_string()),
                browser_url: Some("https://github.com/owner/repo/blob/main/src/lib.rs".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let groups = vec![make_group(RepoResultGroupKind::SourceFiles, vec![card])];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);

        assert!(!fetches.is_empty());
        assert_eq!(
            fetches[0].url, "https://github.com/owner/repo/blob/main/src/lib.rs",
            "suggested fetch should use code_evidence.browser_url before card.url"
        );
    }

    #[test]
    fn card_url_remains_final_fallback() {
        let mut card = make_card("Source", "https://example.com/raw/card-url");
        card.metadata = SourceMetadata {
            source_kind: SourceKind::SourceFile,
            code_evidence: Some(CodeEvidence {
                host: Some(CodeHost::Github),
                owner: Some("owner".to_string()),
                repo: Some("repo".to_string()),
                ref_name: Some("main".to_string()),
                path: Some("src/lib.rs".to_string()),
                // No URLs populated; code evidence is present but sparse.
                ..Default::default()
            }),
            ..Default::default()
        };

        let groups = vec![make_group(RepoResultGroupKind::SourceFiles, vec![card])];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);

        assert!(!fetches.is_empty());
        assert_eq!(
            fetches[0].url, "https://example.com/raw/card-url",
            "card.url should be the final fallback when no code-evidence URL is set"
        );
    }

    #[test]
    fn code_evidence_url_priority_order() {
        // All URLs populated: raw_permalink_url must win.
        let mut card = make_card("Source", "https://example.com/raw/card-url");
        card.metadata = SourceMetadata {
            source_kind: SourceKind::SourceFile,
            code_evidence: Some(CodeEvidence {
                host: Some(CodeHost::Github),
                owner: Some("owner".to_string()),
                repo: Some("repo".to_string()),
                ref_name: Some("main".to_string()),
                path: Some("src/lib.rs".to_string()),
                browser_url: Some("https://github.com/owner/repo/blob/main/src/lib.rs".to_string()),
                raw_url: Some(
                    "https://raw.githubusercontent.com/owner/repo/main/src/lib.rs".to_string(),
                ),
                permalink_url: Some(
                    "https://github.com/owner/repo/blob/abc/src/lib.rs".to_string(),
                ),
                raw_permalink_url: Some(
                    "https://raw.githubusercontent.com/owner/repo/abc/src/lib.rs".to_string(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        };

        let groups = vec![make_group(RepoResultGroupKind::SourceFiles, vec![card])];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);

        assert_eq!(
            fetches[0].url, "https://raw.githubusercontent.com/owner/repo/abc/src/lib.rs",
            "raw_permalink_url must win over raw_url, permalink_url, and browser_url"
        );
    }

    #[test]
    fn source_with_symbol_hint_outranks_docs() {
        let mut source_card = make_card(
            "lib.rs",
            "https://github.com/owner/repo/blob/abc123def45678901/src/lib.rs",
        );
        source_card.metadata = SourceMetadata {
            source_kind: SourceKind::SourceFile,
            code_evidence: Some(CodeEvidence {
                host: Some(CodeHost::Github),
                owner: Some("owner".to_string()),
                repo: Some("repo".to_string()),
                ref_name: Some("main".to_string()),
                path: Some("src/lib.rs".to_string()),
                raw_permalink_url: Some(
                    "https://raw.githubusercontent.com/owner/repo/abc123def45678901/src/lib.rs"
                        .to_string(),
                ),
                evidence_confidence: Some(EvidenceConfidence::Exact),
                source_role: Some(crate::core::code_evidence::SourceRole::Implementation),
                ..Default::default()
            }),
            ..Default::default()
        };

        let docs_card = make_card("Docs", "https://docs.example.com/api");

        let groups = vec![
            make_group(RepoResultGroupKind::SourceFiles, vec![source_card]),
            make_group(RepoResultGroupKind::OfficialDocs, vec![docs_card]),
        ];
        let hints = crate::core::repo_query::RepoQueryHints {
            symbol: Some("Router::layer".to_string()),
            ..Default::default()
        };
        let fetches = generate_suggested_fetches(&groups, &hints);

        assert!(fetches.len() >= 2, "should have at least 2 fetches");
        assert!(
            fetches[0].group == RepoResultGroupKind::SourceFiles,
            "source with symbol hint and pinned raw permalink should outrank docs, got {:?} (score {:?})",
            fetches[0].group,
            fetches[0].score,
        );
    }

    #[test]
    fn exact_error_mode_boosts_issues() {
        let issue_card = make_card("Issue #123", "https://github.com/owner/repo/issues/123");
        let docs_card = make_card("Docs", "https://docs.example.com/api");

        let groups = vec![
            make_group(RepoResultGroupKind::Issues, vec![issue_card]),
            make_group(RepoResultGroupKind::OfficialDocs, vec![docs_card]),
        ];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches =
            generate_suggested_fetches_with_mode(&groups, &hints, FetchRankMode::ExactError);

        assert!(fetches.len() >= 2);
        assert_eq!(
            fetches[0].group,
            RepoResultGroupKind::Issues,
            "issues should outrank docs in exact-error mode, got {:?} (score {:?})",
            fetches[0].group,
            fetches[0].score,
        );
    }

    #[test]
    fn diversity_caps_prevent_domain_dominance() {
        let cards: Vec<SourceCard> = vec![
            make_card("a", "https://same.example.com/a"),
            make_card("b", "https://same.example.com/b"),
            make_card("c", "https://same.example.com/c"),
            make_card("d", "https://other.example.com/d"),
        ];

        let groups = vec![
            make_group(RepoResultGroupKind::OfficialDocs, vec![cards[0].clone()]),
            make_group(RepoResultGroupKind::SourceFiles, vec![cards[1].clone()]),
            make_group(RepoResultGroupKind::Releases, vec![cards[2].clone()]),
            make_group(RepoResultGroupKind::Issues, vec![cards[3].clone()]),
        ];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);

        let same_domain_count = fetches
            .iter()
            .filter(|f| f.url.contains("same.example.com"))
            .count();
        assert!(
            same_domain_count <= 2,
            "expected at most 2 from same domain, got {same_domain_count}"
        );
    }

    #[test]
    fn diversity_caps_prevent_group_dominance() {
        let cards: Vec<SourceCard> = vec![
            make_card("a", "https://a.example.com/1"),
            make_card("b", "https://b.example.com/2"),
            make_card("c", "https://c.example.com/3"),
            make_card("d", "https://d.example.com/4"),
        ];

        let groups = vec![make_group(RepoResultGroupKind::SourceFiles, cards)];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);

        let source_count = fetches
            .iter()
            .filter(|f| f.group == RepoResultGroupKind::SourceFiles)
            .count();
        assert!(
            source_count <= 2,
            "expected at most 2 from same group, got {source_count}"
        );
    }

    #[test]
    fn score_and_rank_reasons_are_populated() {
        let groups = vec![
            make_group(
                RepoResultGroupKind::OfficialDocs,
                vec![make_card("Docs", "https://docs.example.com/api")],
            ),
            make_group(
                RepoResultGroupKind::SourceFiles,
                vec![make_card(
                    "Source",
                    "https://github.com/foo/bar/blob/main/src/lib.rs",
                )],
            ),
        ];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);

        for fetch in &fetches {
            assert!(
                fetch.score.is_some(),
                "score should be populated for all fetches"
            );
            assert!(
                !fetch.rank_reasons.is_empty(),
                "rank_reasons should be non-empty for all fetches"
            );
            assert!(
                fetch.information_gain.is_some(),
                "information_gain should be populated for all fetches"
            );
            assert!(
                fetch.stable.is_some(),
                "stable should be populated for all fetches"
            );
            assert!(
                fetch.preferred_tool.is_some(),
                "preferred_tool should be populated for all fetches"
            );
        }
    }

    #[test]
    fn preferred_tool_matches_structured_fetch_availability() {
        let mut card = make_card(
            "lib.rs",
            "https://github.com/owner/repo/blob/main/src/lib.rs",
        );
        card.metadata = SourceMetadata {
            source_kind: SourceKind::SourceFile,
            code_evidence: Some(CodeEvidence {
                host: Some(CodeHost::Github),
                owner: Some("owner".to_string()),
                repo: Some("repo".to_string()),
                ref_name: Some("main".to_string()),
                path: Some("src/lib.rs".to_string()),
                raw_url: Some(
                    "https://raw.githubusercontent.com/owner/repo/main/src/lib.rs".to_string(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        };

        let groups = vec![make_group(RepoResultGroupKind::SourceFiles, vec![card])];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);

        assert!(!fetches.is_empty());
        assert_eq!(
            fetches[0].preferred_tool.as_deref(),
            Some("repo_fetch"),
            "candidate with structured_repo_fetch should prefer repo_fetch"
        );
    }

    #[test]
    fn web_fetch_preferred_for_non_structured() {
        let groups = vec![make_group(
            RepoResultGroupKind::OfficialDocs,
            vec![make_card("Docs", "https://docs.example.com")],
        )];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches = generate_suggested_fetches(&groups, &hints);

        assert!(!fetches.is_empty());
        assert_eq!(
            fetches[0].preferred_tool.as_deref(),
            Some("web_fetch"),
            "non-structured candidate should prefer web_fetch"
        );
    }

    #[test]
    fn default_mode_delegates_to_normal() {
        let groups = vec![make_group(
            RepoResultGroupKind::OfficialDocs,
            vec![make_card("Docs", "https://docs.example.com")],
        )];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let a = generate_suggested_fetches(&groups, &hints);
        let b = generate_suggested_fetches_with_mode(&groups, &hints, FetchRankMode::Normal);
        assert_eq!(a.len(), b.len());
        for (a_f, b_f) in a.iter().zip(b.iter()) {
            assert_eq!(a_f.url, b_f.url);
            assert_eq!(a_f.score, b_f.score);
        }
    }

    #[test]
    fn package_migration_mode_boosts_releases() {
        let release_card = make_card(
            "Release v2.0",
            "https://github.com/owner/repo/releases/tag/v2.0",
        );
        let docs_card = make_card("Docs", "https://docs.example.com/api");

        let groups = vec![
            make_group(RepoResultGroupKind::Releases, vec![release_card]),
            make_group(RepoResultGroupKind::OfficialDocs, vec![docs_card]),
        ];
        let hints = crate::core::repo_query::RepoQueryHints::default();
        let fetches =
            generate_suggested_fetches_with_mode(&groups, &hints, FetchRankMode::PackageMigration);

        assert!(fetches.len() >= 2);
        assert_eq!(
            fetches[0].group,
            RepoResultGroupKind::Releases,
            "releases should outrank docs in package/migration mode, got {:?} (score {:?})",
            fetches[0].group,
            fetches[0].score,
        );
    }
}
