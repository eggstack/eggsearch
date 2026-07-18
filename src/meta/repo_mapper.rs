//! Repository map planning and classification.
//!
//! Provides deterministic classification of repository structure
//! entries and suggested-fetch generation for the `repo_map` MCP tool.

use std::path::Path;

use crate::core::code_metadata::CodeHost;
use crate::core::repo_fetch::RepoFetchRequest;
use crate::core::repo_map::{
    classify_important_directory, classify_important_file, ImportantDirKind, ImportantFileKind,
    RepoImportantDirectory, RepoImportantFile, RepoMapEntry, RepoMapEntryKind, RepoMapMode,
    RepoMapRequest, RepoMapResponse, RepoMapSuggestedFetch, RepoPathSummary,
};
use crate::core::result::SearchWarning;
use crate::core::sanitize::TrustMarkers;

const MAX_SUGGESTED_FETCHES: usize = 8;

/// Build a raw-content URL for the given host, owner, repo, ref, and path.
pub fn build_raw_url(
    host: CodeHost,
    owner: &str,
    repo: &str,
    ref_name: &str,
    path: &str,
) -> String {
    match host {
        CodeHost::Github => {
            format!("https://raw.githubusercontent.com/{owner}/{repo}/{ref_name}/{path}")
        }
        CodeHost::Gitlab => {
            format!("https://gitlab.com/{owner}/{repo}/-/raw/{ref_name}/{path}")
        }
        CodeHost::Codeberg => {
            format!("https://codeberg.org/{owner}/{repo}/raw/branch/{ref_name}/{path}")
        }
        // Gitea/Forgejo require a configured base URL; return empty for unmapped hosts.
        _ => String::new(),
    }
}

/// Build a `RepoFetchRequest` when the host supports structured fetch.
fn build_structured_fetch(
    host: CodeHost,
    owner: &str,
    repo: &str,
    ref_name: &str,
    path: &str,
) -> Option<RepoFetchRequest> {
    match host {
        CodeHost::Github
        | CodeHost::Gitlab
        | CodeHost::Codeberg
        | CodeHost::Gitea
        | CodeHost::Forgejo => Some(RepoFetchRequest {
            host: Some(host),
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            ref_name: Some(ref_name.to_owned()),
            commit_sha: None,
            path: path.to_owned(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        }),
        _ => None,
    }
}

/// Generate prioritized fetch suggestions from a repo map response.
///
/// Priority order:
/// 1. README / primary docs (important_files where kind is Readme)
/// 2. Primary manifest(s) (important_files where kind is Manifest)
/// 3. Main source entrypoint(s) (source_roots)
/// 4. Examples or quickstart files (examples)
/// 5. Changelog / migration files (important_files where kind is Changelog)
/// 6. Security policy (security)
/// 7. Test entrypoints (tests)
pub fn build_repo_map_suggested_fetches(response: &RepoMapResponse) -> Vec<RepoMapSuggestedFetch> {
    let mut suggestions = Vec::new();
    let owner = &response.owner;
    let repo = &response.repo;
    let ref_name = response.ref_name.as_deref().unwrap_or("HEAD");
    let host = response.host;

    // 1. README / primary docs
    for file in &response.important_files {
        if suggestions.len() >= MAX_SUGGESTED_FETCHES {
            break;
        }
        if file.kind == ImportantFileKind::Readme {
            let url = build_raw_url(host, owner, repo, ref_name, &file.path);
            let structured = build_structured_fetch(host, owner, repo, ref_name, &file.path);
            suggestions.push(RepoMapSuggestedFetch {
                url,
                reason: format!("README documentation for {owner}/{repo}"),
                priority: Some(suggestions.len() + 1),
                structured_repo_fetch: structured,
            });
        }
    }

    // 2. Primary manifest(s)
    for file in &response.important_files {
        if suggestions.len() >= MAX_SUGGESTED_FETCHES {
            break;
        }
        if file.kind == ImportantFileKind::Manifest {
            let url = build_raw_url(host, owner, repo, ref_name, &file.path);
            let structured = build_structured_fetch(host, owner, repo, ref_name, &file.path);
            suggestions.push(RepoMapSuggestedFetch {
                url,
                reason: format!("Package manifest for {owner}/{repo}"),
                priority: Some(suggestions.len() + 1),
                structured_repo_fetch: structured,
            });
        }
    }

    // 3. Main source entrypoint(s)
    for dir in &response.source_roots {
        if suggestions.len() >= MAX_SUGGESTED_FETCHES {
            break;
        }
        let url = build_raw_url(host, owner, repo, ref_name, &dir.path);
        let structured = build_structured_fetch(host, owner, repo, ref_name, &dir.path);
        suggestions.push(RepoMapSuggestedFetch {
            url,
            reason: format!("Source root directory: {}", dir.path),
            priority: Some(suggestions.len() + 1),
            structured_repo_fetch: structured,
        });
    }

    // 4. Examples or quickstart files
    for dir in &response.examples {
        if suggestions.len() >= MAX_SUGGESTED_FETCHES {
            break;
        }
        let url = build_raw_url(host, owner, repo, ref_name, &dir.path);
        let structured = build_structured_fetch(host, owner, repo, ref_name, &dir.path);
        suggestions.push(RepoMapSuggestedFetch {
            url,
            reason: format!("Examples directory: {}", dir.path),
            priority: Some(suggestions.len() + 1),
            structured_repo_fetch: structured,
        });
    }

    // 5. Changelog / migration files
    for file in &response.important_files {
        if suggestions.len() >= MAX_SUGGESTED_FETCHES {
            break;
        }
        if file.kind == ImportantFileKind::Changelog {
            let url = build_raw_url(host, owner, repo, ref_name, &file.path);
            let structured = build_structured_fetch(host, owner, repo, ref_name, &file.path);
            suggestions.push(RepoMapSuggestedFetch {
                url,
                reason: format!("Changelog for {owner}/{repo}"),
                priority: Some(suggestions.len() + 1),
                structured_repo_fetch: structured,
            });
        }
    }

    // 6. Security policy
    if suggestions.len() < MAX_SUGGESTED_FETCHES {
        if let Some(ref security) = response.security {
            let url = build_raw_url(host, owner, repo, ref_name, &security.path);
            let structured = build_structured_fetch(host, owner, repo, ref_name, &security.path);
            suggestions.push(RepoMapSuggestedFetch {
                url,
                reason: format!("Security policy for {owner}/{repo}"),
                priority: Some(suggestions.len() + 1),
                structured_repo_fetch: structured,
            });
        }
    }

    // 7. Test entrypoints
    for dir in &response.tests {
        if suggestions.len() >= MAX_SUGGESTED_FETCHES {
            break;
        }
        let url = build_raw_url(host, owner, repo, ref_name, &dir.path);
        let structured = build_structured_fetch(host, owner, repo, ref_name, &dir.path);
        suggestions.push(RepoMapSuggestedFetch {
            url,
            reason: format!("Test directory: {}", dir.path),
            priority: Some(suggestions.len() + 1),
            structured_repo_fetch: structured,
        });
    }

    suggestions
}

/// Classify root entries into categories based on their structural kind.
///
/// This is a deterministic pass-through that groups entries by their
/// `RepoMapEntryKind`, returning categorized slices.
pub fn classify_root_entries(entries: &[RepoMapEntry]) -> ClassifiedEntries<'_> {
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut symlinks = Vec::new();
    let mut submodules = Vec::new();
    let mut other = Vec::new();

    for entry in entries {
        match entry.kind {
            RepoMapEntryKind::File => files.push(entry),
            RepoMapEntryKind::Directory => directories.push(entry),
            RepoMapEntryKind::Symlink => symlinks.push(entry),
            RepoMapEntryKind::Submodule => submodules.push(entry),
            RepoMapEntryKind::Unknown => other.push(entry),
        }
    }

    ClassifiedEntries {
        files,
        directories,
        symlinks,
        submodules,
        other,
    }
}

/// Categorized slices of root entries.
pub struct ClassifiedEntries<'a> {
    /// File entries.
    pub files: Vec<&'a RepoMapEntry>,
    /// Directory entries.
    pub directories: Vec<&'a RepoMapEntry>,
    /// Symlink entries.
    pub symlinks: Vec<&'a RepoMapEntry>,
    /// Submodule entries.
    pub submodules: Vec<&'a RepoMapEntry>,
    /// Other or unrecognized entries.
    pub other: Vec<&'a RepoMapEntry>,
}

/// Populate a `RepoMapResponse` from a local checkout directory.
///
/// Walks the top level of `root_path`, classifies entries using
/// `classify_important_file` and `classify_important_directory`, and
/// fills in `root_entries`, `important_files`, `important_directories`,
/// `source_roots`, `docs`, `examples`, `tests`, `ci`, `security`, and
/// `suggested_fetches`. Skips dotfiles and well-known generated directories
/// to keep the response bounded. Honors the `include_*` flags from the
/// request when supplied.
pub fn populate_from_local_checkout(
    response: &mut RepoMapResponse,
    request: &RepoMapRequest,
    root_path: &Path,
) {
    let walk_dir = match std::fs::read_dir(root_path) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(root = %root_path.display(), error = %e, "failed to read local checkout for repo_map");
            return;
        }
    };

    let include_files = request.include_files.unwrap_or(true);
    let include_directories = request.include_directories.unwrap_or(true);
    let include_ci = request.include_ci.unwrap_or(true);
    let include_security = request.include_security.unwrap_or(true);

    let mut entries: Vec<RepoMapEntry> = Vec::new();
    let mut important_files: Vec<RepoImportantFile> = Vec::new();
    let mut important_directories: Vec<RepoImportantDirectory> = Vec::new();
    let mut source_roots: Vec<RepoPathSummary> = Vec::new();
    let mut docs: Vec<RepoPathSummary> = Vec::new();
    let mut examples: Vec<RepoPathSummary> = Vec::new();
    let mut tests: Vec<RepoPathSummary> = Vec::new();
    let mut ci: Vec<RepoPathSummary> = Vec::new();
    let mut security: Option<RepoPathSummary> = None;

    for entry in walk_dir.flatten() {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        if file_name_str.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let rel = file_name_str.to_string();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let ft = metadata.file_type();
        let kind = if ft.is_dir() {
            RepoMapEntryKind::Directory
        } else if ft.is_symlink() {
            RepoMapEntryKind::Symlink
        } else if ft.is_file() {
            RepoMapEntryKind::File
        } else {
            RepoMapEntryKind::Unknown
        };
        let size = if ft.is_file() {
            Some(metadata.len())
        } else {
            None
        };
        let include_in_root = (kind == RepoMapEntryKind::File && include_files)
            || (kind == RepoMapEntryKind::Directory && include_directories)
            || (kind == RepoMapEntryKind::Symlink && include_files)
            || (kind == RepoMapEntryKind::Submodule && include_directories);
        if include_in_root {
            entries.push(RepoMapEntry {
                path: rel.clone(),
                kind,
                size,
                language: None,
                url: None,
                raw_url: None,
            });
        }

        if kind == RepoMapEntryKind::File && include_files {
            let (file_kind, reasons) = classify_important_file(&rel);
            if file_kind != ImportantFileKind::Unknown && file_kind != ImportantFileKind::Ignored {
                important_files.push(RepoImportantFile {
                    path: rel.clone(),
                    kind: file_kind,
                    reasons,
                    size,
                });
            }
        } else if kind == RepoMapEntryKind::Directory && include_directories {
            let (dir_kind, reasons) = classify_important_directory(&rel);
            if dir_kind != ImportantDirKind::Unknown {
                important_directories.push(RepoImportantDirectory {
                    path: rel.clone(),
                    kind: dir_kind,
                    reasons,
                    estimated_entry_count: None,
                });
                let summary = RepoPathSummary {
                    path: rel.clone(),
                    label: format!("{dir_kind:?}"),
                    entry_count: None,
                };
                let suppressed_ci = matches!(dir_kind, ImportantDirKind::CiConfig) && !include_ci;
                let suppressed_security =
                    matches!(dir_kind, ImportantDirKind::Security) && !include_security;
                if suppressed_ci || suppressed_security {
                    // skip emitting the categorized summary
                } else {
                    match dir_kind {
                        ImportantDirKind::SourceRoot => source_roots.push(summary),
                        ImportantDirKind::Docs => docs.push(summary),
                        ImportantDirKind::Examples => examples.push(summary),
                        ImportantDirKind::Tests => tests.push(summary),
                        ImportantDirKind::CiConfig => ci.push(summary),
                        ImportantDirKind::Security => security = Some(summary),
                        _ => {}
                    }
                }
            }
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    important_files.sort_by(|a, b| a.path.cmp(&b.path));
    important_directories.sort_by(|a, b| a.path.cmp(&b.path));

    let host = response.host;
    let owner = response.owner.clone();
    let repo = response.repo.clone();
    let ref_name = response
        .ref_name
        .clone()
        .unwrap_or_else(|| "HEAD".to_string());

    let mut suggested_fetches: Vec<RepoMapSuggestedFetch> = Vec::new();
    for file in &important_files {
        if suggested_fetches.len() >= MAX_SUGGESTED_FETCHES {
            break;
        }
        let url = build_raw_url(host, &owner, &repo, &ref_name, &file.path);
        let structured = build_structured_fetch(host, &owner, &repo, &ref_name, &file.path);
        let reason = match file.kind {
            ImportantFileKind::Readme => format!("README documentation for {owner}/{repo}"),
            ImportantFileKind::Manifest => format!("Package manifest for {owner}/{repo}"),
            ImportantFileKind::Changelog => format!("Changelog for {owner}/{repo}"),
            ImportantFileKind::Security => format!("Security policy for {owner}/{repo}"),
            _ => format!("Important file in {owner}/{repo}"),
        };
        suggested_fetches.push(RepoMapSuggestedFetch {
            url,
            reason,
            priority: Some(suggested_fetches.len() + 1),
            structured_repo_fetch: structured,
        });
    }

    response.root_entries = entries;
    response.important_files = important_files;
    response.important_directories = important_directories;
    response.manifests = response
        .important_files
        .iter()
        .filter(|f| f.kind == ImportantFileKind::Manifest)
        .cloned()
        .collect();
    response.source_roots = source_roots;
    response.docs = docs;
    response.examples = examples;
    response.tests = tests;
    response.ci = ci;
    response.security = security;
    response.suggested_fetches = suggested_fetches;
    response
        .providers_queried
        .push("local_workspace".to_string());
}

/// Create a `RepoMapResponse` with fallback search mode when no native
/// tree provider is available.
pub fn build_fallback_response(request: &RepoMapRequest) -> RepoMapResponse {
    RepoMapResponse {
        query: request.query.clone(),
        host: request.host.unwrap_or(CodeHost::Unknown),
        owner: request.owner.clone(),
        repo: request.repo.clone(),
        ref_name: request.ref_name.clone(),
        commit_sha: None,
        default_branch: None,
        mode: RepoMapMode::FallbackSearch,
        root_entries: Vec::new(),
        important_files: Vec::new(),
        important_directories: Vec::new(),
        manifests: Vec::new(),
        source_roots: Vec::new(),
        docs: Vec::new(),
        examples: Vec::new(),
        tests: Vec::new(),
        ci: Vec::new(),
        security: None,
        suggested_fetches: Vec::new(),
        providers_queried: Vec::new(),
        providers_failed: Vec::new(),
        warnings: vec![SearchWarning::new(
            "_system",
            "no_native_tree_provider: no native tree/list API provider is available; \
             remote discovery is not performed, provide a local checkout or \
             configure a native tree provider for repo_map results",
        )],
        structured_warnings: Vec::new(),
        trust_markers: TrustMarkers::default(),
        local_checkout: None,
        telemetry: None,
    }
}

/// A search subquery for fallback discovery.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct RepoMapSearchSubquery {
    /// Human-readable label for this subquery.
    pub label: String,
    /// The search query string.
    pub query: String,
    /// Intended result group classification.
    pub intended_group: String,
}

/// Generate search subqueries for fallback discovery when no native
/// tree provider is available.
pub fn generate_fallback_subqueries(owner: &str, repo: &str) -> Vec<RepoMapSearchSubquery> {
    vec![
        RepoMapSearchSubquery {
            label: "readme_documentation".into(),
            query: format!("README documentation {owner}/{repo}"),
            intended_group: "readme".into(),
        },
        RepoMapSearchSubquery {
            label: "manifest_package_config".into(),
            query: format!("manifest Cargo.toml package.json {owner}/{repo}"),
            intended_group: "manifest".into(),
        },
        RepoMapSearchSubquery {
            label: "examples_samples".into(),
            query: format!("examples samples {owner}/{repo}"),
            intended_group: "example".into(),
        },
        RepoMapSearchSubquery {
            label: "tests_spec".into(),
            query: format!("tests spec {owner}/{repo}"),
            intended_group: "test".into(),
        },
        RepoMapSearchSubquery {
            label: "ci_workflow".into(),
            query: format!("CI workflow .github {owner}/{repo}"),
            intended_group: "ci".into(),
        },
        RepoMapSearchSubquery {
            label: "security_policy".into(),
            query: format!("security SECURITY.md policy {owner}/{repo}"),
            intended_group: "security".into(),
        },
        RepoMapSearchSubquery {
            label: "changelog_releases".into(),
            query: format!("changelog CHANGELOG releases {owner}/{repo}"),
            intended_group: "changelog".into(),
        },
        RepoMapSearchSubquery {
            label: "source_code".into(),
            query: format!("source code src lib {owner}/{repo}"),
            intended_group: "source".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_raw_url_github() {
        let url = build_raw_url(CodeHost::Github, "octocat", "hello", "main", "README.md");
        assert_eq!(
            url,
            "https://raw.githubusercontent.com/octocat/hello/main/README.md"
        );
    }

    #[test]
    fn build_raw_url_gitlab() {
        let url = build_raw_url(
            CodeHost::Gitlab,
            "mygroup",
            "myproject",
            "v1.0",
            "src/main.rs",
        );
        assert_eq!(
            url,
            "https://gitlab.com/mygroup/myproject/-/raw/v1.0/src/main.rs"
        );
    }

    #[test]
    fn build_raw_url_unknown_returns_empty() {
        let url = build_raw_url(CodeHost::Unknown, "owner", "repo", "main", "file.txt");
        assert_eq!(url, "");
    }

    #[test]
    fn build_raw_url_codeberg() {
        let url = build_raw_url(CodeHost::Codeberg, "owner", "repo", "main", "src/lib.rs");
        assert_eq!(
            url,
            "https://codeberg.org/owner/repo/raw/branch/main/src/lib.rs"
        );
    }

    #[test]
    fn build_structured_fetch_github() {
        let fetch = build_structured_fetch(CodeHost::Github, "o", "r", "main", "Cargo.toml");
        assert!(fetch.is_some());
        let f = fetch.unwrap();
        assert_eq!(f.host, Some(CodeHost::Github));
        assert_eq!(f.owner, "o");
        assert_eq!(f.repo, "r");
        assert_eq!(f.ref_name.as_deref(), Some("main"));
        assert_eq!(f.path, "Cargo.toml");
    }

    #[test]
    fn build_structured_fetch_codeberg() {
        let fetch = build_structured_fetch(CodeHost::Codeberg, "o", "r", "main", "Cargo.toml");
        assert!(fetch.is_some());
        let f = fetch.unwrap();
        assert_eq!(f.host, Some(CodeHost::Codeberg));
        assert_eq!(f.owner, "o");
        assert_eq!(f.repo, "r");
        assert_eq!(f.ref_name.as_deref(), Some("main"));
        assert_eq!(f.path, "Cargo.toml");
    }

    #[test]
    fn build_structured_fetch_gitea() {
        let fetch = build_structured_fetch(CodeHost::Gitea, "o", "r", "main", "src/lib.rs");
        assert!(fetch.is_some());
        let f = fetch.unwrap();
        assert_eq!(f.host, Some(CodeHost::Gitea));
    }

    #[test]
    fn build_structured_fetch_forgejo() {
        let fetch = build_structured_fetch(CodeHost::Forgejo, "o", "r", "main", "src/lib.rs");
        assert!(fetch.is_some());
        let f = fetch.unwrap();
        assert_eq!(f.host, Some(CodeHost::Forgejo));
    }

    #[test]
    fn build_structured_fetch_unknown_returns_none() {
        let fetch = build_structured_fetch(CodeHost::Unknown, "o", "r", "main", "f.txt");
        assert!(fetch.is_none());
    }

    #[test]
    fn build_raw_url_gitea_returns_empty() {
        // Gitea/Forgejo require a configured base_url that is not available
        // from the CodeHost enum alone; build_raw_url returns empty string.
        let url = build_raw_url(CodeHost::Gitea, "owner", "repo", "main", "src/lib.rs");
        assert_eq!(url, "");
    }

    #[test]
    fn build_raw_url_forgejo_returns_empty() {
        let url = build_raw_url(CodeHost::Forgejo, "owner", "repo", "main", "src/lib.rs");
        assert_eq!(url, "");
    }

    #[test]
    fn build_fallback_response_fields() {
        let request = RepoMapRequest {
            query: "test query".into(),
            host: Some(CodeHost::Github),
            owner: "octocat".into(),
            repo: "hello".into(),
            ref_name: Some("main".into()),
            max_entries: None,
            timeout_ms: None,
            providers: Vec::new(),
            ..Default::default()
        };
        let response = build_fallback_response(&request);

        assert_eq!(response.query, "test query");
        assert!(matches!(response.mode, RepoMapMode::FallbackSearch));
        assert!(response.root_entries.is_empty());
        assert!(response.important_files.is_empty());
        assert!(response.important_directories.is_empty());
        assert!(response.source_roots.is_empty());
        assert!(response.examples.is_empty());
        assert!(response.tests.is_empty());
        assert!(response.security.is_none());
        assert!(response.suggested_fetches.is_empty());
        assert_eq!(response.warnings.len(), 1);
        assert!(response.warnings[0]
            .message
            .contains("no_native_tree_provider"));
        assert_eq!(response.warnings[0].provider_id, "_system");
    }

    #[test]
    fn generate_fallback_subqueries_count() {
        let subs = generate_fallback_subqueries("octocat", "hello");
        assert_eq!(subs.len(), 8);
    }

    #[test]
    fn generate_fallback_subqueries_content() {
        let subs = generate_fallback_subqueries("octocat", "hello");

        assert_eq!(subs[0].label, "readme_documentation");
        assert!(subs[0].query.contains("README"));
        assert!(subs[0].query.contains("octocat/hello"));
        assert_eq!(subs[0].intended_group, "readme");

        assert_eq!(subs[1].label, "manifest_package_config");
        assert!(subs[1].query.contains("Cargo.toml"));
        assert_eq!(subs[1].intended_group, "manifest");

        assert_eq!(subs[7].label, "source_code");
        assert!(subs[7].query.contains("source code"));
        assert_eq!(subs[7].intended_group, "source");
    }

    #[test]
    fn repo_map_search_subquery_serde_roundtrip() {
        let sub = RepoMapSearchSubquery {
            label: "test_label".into(),
            query: "test query string".into(),
            intended_group: "test_group".into(),
        };
        let json = serde_json::to_string(&sub).unwrap();
        let parsed: RepoMapSearchSubquery = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, sub);
    }
}
