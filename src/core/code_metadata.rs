//! Deterministic code-host URL parsing and metadata extraction.
//!
//! This module provides helpers for classifying code-host URLs
//! (GitHub, GitLab, Codeberg, Gitea, Forgejo) into structured
//! metadata without fetching or cloning. All functions are pure
//! and deterministic.

use serde::{Deserialize, Serialize};

use crate::core::source_card::SourceKind;

/// Identifies the code-hosting platform behind a URL.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CodeHost {
    /// Unrecognized or non-code-host URL.
    #[default]
    Unknown,
    /// GitHub (github.com).
    Github,
    /// GitLab (gitlab.com).
    Gitlab,
    /// Codeberg (codeberg.org).
    Codeberg,
    /// Gitea instance.
    Gitea,
    /// Forgejo instance.
    Forgejo,
}

impl CodeHost {
    /// Parse a user-facing code host name or alias.
    pub fn parse_alias(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "github" | "gh" | "github.com" => Some(Self::Github),
            "gitlab" | "gl" | "gitlab.com" => Some(Self::Gitlab),
            "codeberg" | "cb" | "codeberg.org" => Some(Self::Codeberg),
            "gitea" => Some(Self::Gitea),
            "forgejo" => Some(Self::Forgejo),
            _ => None,
        }
    }

    /// User-facing host aliases accepted by MCP tools and query hints.
    pub fn accepted_aliases() -> &'static str {
        "github (gh), gitlab (gl), codeberg (cb), gitea, forgejo"
    }
}

/// Structured code/repo metadata extracted from a code-host URL.
///
/// All fields are optional because not every URL shape produces every
/// piece of metadata. The struct is `Default` (all `None`) so callers
/// can skip it for non-code results.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeMetadata {
    /// The code-hosting platform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CodeHost>,
    /// Repository owner (or namespace for GitLab nested groups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Repository name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// File or directory path within the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Branch, tag, or commit ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    /// Inferred programming language from file extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Optional symbol hint (e.g. from fragment anchors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_hint: Option<String>,
    /// Start line number (from `#L10` anchors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u32>,
    /// End line number (from `#L10-L25` anchors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
}

/// Infer a programming language from a file extension.
///
/// Returns `None` for unknown or ambiguous extensions.
pub fn language_from_extension(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?;
    match ext {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "json" => Some("json"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" => Some("kotlin"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "hpp" => Some("cpp"),
        "md" => Some("markdown"),
        _ => None,
    }
}

/// Heuristic check for whether the trailing component of a path
/// looks like a file name (contains a dot, does not start or end
/// with a dot). Used by Codeberg `/src/branch/...` classification
/// to distinguish source files from directories when the extension
/// is not in the known language list.
fn looks_like_file_path(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.') && !last.ends_with('.') && !last.starts_with('.'))
}

fn parse_line_fragment(fragment: &str) -> (Option<u32>, Option<u32>) {
    // Fragment looks like "L10" or "L10-L25"
    let inner = match fragment.strip_prefix('L') {
        Some(s) => s,
        None => return (None, None),
    };
    if let Some((start_s, end_s)) = inner.split_once('-') {
        let start = match start_s.parse::<u32>() {
            Ok(n) => n,
            Err(_) => return (None, None),
        };
        let end_str = end_s.strip_prefix('L').unwrap_or(end_s);
        let end = match end_str.parse::<u32>() {
            Ok(n) => n,
            Err(_) => return (None, None),
        };
        (Some(start), Some(end))
    } else {
        let start = match inner.parse::<u32>() {
            Ok(n) => n,
            Err(_) => return (None, None),
        };
        (Some(start), None)
    }
}

/// Parse a GitHub URL into `SourceKind`, `CodeMetadata`, and domain.
///
/// Returns `(SourceKind, Option<CodeMetadata>, Option<String>)`.
pub fn parse_github_url(url: &str) -> (SourceKind, Option<CodeMetadata>, Option<String>) {
    use url::Url;

    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return (SourceKind::Unknown, None, None),
    };
    let host = parsed.host_str().unwrap_or("");
    if host != "github.com" {
        return (SourceKind::Unknown, None, None);
    }
    let domain = Some(host.to_string());
    let path = parsed.path();
    let fragment = parsed.fragment().unwrap_or("");

    let segments: Vec<&str> = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .collect();

    if segments.len() < 2 {
        return (SourceKind::SourceRepository, None, domain);
    }

    let owner = segments[0].to_string();
    let repo = segments[1].to_string();

    if segments.len() == 2 {
        return (
            SourceKind::RepositoryRoot,
            Some(CodeMetadata {
                host: Some(CodeHost::Github),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        );
    }

    let rest = &segments[2..];
    match rest[0] {
        "tree" => {
            let ref_name = rest.get(1).map(|s| s.to_string());
            let file_path = if rest.len() > 2 {
                Some(rest[2..].join("/"))
            } else {
                None
            };
            let language = file_path.as_deref().and_then(language_from_extension);
            (
                SourceKind::SourceDirectory,
                Some(CodeMetadata {
                    host: Some(CodeHost::Github),
                    owner: Some(owner),
                    repo: Some(repo),
                    ref_name,
                    path: file_path,
                    language: language.map(|s| s.to_string()),
                    ..Default::default()
                }),
                domain,
            )
        }
        "blob" => {
            let ref_name = rest.get(1).map(|s| s.to_string());
            let file_path = if rest.len() > 2 {
                Some(rest[2..].join("/"))
            } else {
                None
            };
            let language = file_path.as_deref().and_then(language_from_extension);
            let (line_start, line_end) = if !fragment.is_empty() {
                parse_line_fragment(fragment)
            } else {
                (None, None)
            };
            (
                SourceKind::SourceFile,
                Some(CodeMetadata {
                    host: Some(CodeHost::Github),
                    owner: Some(owner),
                    repo: Some(repo),
                    ref_name,
                    path: file_path,
                    language: language.map(|s| s.to_string()),
                    line_start,
                    line_end,
                    ..Default::default()
                }),
                domain,
            )
        }
        "issues" | "discussions" => (
            SourceKind::IssueThread,
            Some(CodeMetadata {
                host: Some(CodeHost::Github),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        ),
        "pull" => (
            SourceKind::PullRequest,
            Some(CodeMetadata {
                host: Some(CodeHost::Github),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        ),
        "releases" => {
            if rest.get(1) == Some(&"tag") {
                (
                    SourceKind::ReleaseNotes,
                    Some(CodeMetadata {
                        host: Some(CodeHost::Github),
                        owner: Some(owner),
                        repo: Some(repo),
                        ref_name: rest.get(2).map(|s| s.to_string()),
                        ..Default::default()
                    }),
                    domain,
                )
            } else {
                (
                    SourceKind::ReleaseNotes,
                    Some(CodeMetadata {
                        host: Some(CodeHost::Github),
                        owner: Some(owner),
                        repo: Some(repo),
                        ..Default::default()
                    }),
                    domain,
                )
            }
        }
        "tags" => (
            SourceKind::Tag,
            Some(CodeMetadata {
                host: Some(CodeHost::Github),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        ),
        "commit" | "commits" => (
            SourceKind::Commit,
            Some(CodeMetadata {
                host: Some(CodeHost::Github),
                owner: Some(owner),
                repo: Some(repo),
                ref_name: rest.get(1).map(|s| s.to_string()),
                ..Default::default()
            }),
            domain,
        ),
        _ => (
            SourceKind::SourceRepository,
            Some(CodeMetadata {
                host: Some(CodeHost::Github),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        ),
    }
}

/// Parse a GitLab URL into `SourceKind`, `CodeMetadata`, and domain.
///
/// GitLab uses nested groups: `gitlab.com/group/subgroup/project`.
/// The `/-/` separator marks the start of the action path.
pub fn parse_gitlab_url(url: &str) -> (SourceKind, Option<CodeMetadata>, Option<String>) {
    use url::Url;

    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return (SourceKind::Unknown, None, None),
    };
    let host = parsed.host_str().unwrap_or("");
    if host != "gitlab.com" {
        return (SourceKind::Unknown, None, None);
    }
    let domain = Some(host.to_string());
    let path = parsed.path();
    let fragment = parsed.fragment().unwrap_or("");

    let segments: Vec<&str> = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .collect();

    if segments.is_empty() {
        return (SourceKind::Unknown, None, domain);
    }

    // Find the `/-/` separator to split namespace from action.
    let sep_idx = segments.iter().position(|&s| s == "-");

    if let Some(idx) = sep_idx {
        // Everything before `/-/` is the namespace.
        let namespace = &segments[..idx];
        let action = &segments[idx + 1..];

        if namespace.is_empty() {
            return (SourceKind::Unknown, None, domain);
        }

        let owner = namespace[..namespace.len() - 1].join("/");
        let repo = namespace[namespace.len() - 1].to_string();

        let meta = CodeMetadata {
            host: Some(CodeHost::Gitlab),
            owner: if owner.is_empty() { None } else { Some(owner) },
            repo: Some(repo),
            ..Default::default()
        };

        match action.first() {
            Some(&"tree") => {
                let ref_name = action.get(1).map(|s| s.to_string());
                let file_path = if action.len() > 2 {
                    Some(action[2..].join("/"))
                } else {
                    None
                };
                let language = file_path.as_deref().and_then(language_from_extension);
                (
                    SourceKind::SourceDirectory,
                    Some(CodeMetadata {
                        ref_name,
                        path: file_path,
                        language: language.map(|s| s.to_string()),
                        ..meta
                    }),
                    domain,
                )
            }
            Some(&"blob") => {
                let ref_name = action.get(1).map(|s| s.to_string());
                let file_path = if action.len() > 2 {
                    Some(action[2..].join("/"))
                } else {
                    None
                };
                let language = file_path.as_deref().and_then(language_from_extension);
                let (line_start, line_end) = if !fragment.is_empty() {
                    parse_line_fragment(fragment)
                } else {
                    (None, None)
                };
                (
                    SourceKind::SourceFile,
                    Some(CodeMetadata {
                        ref_name,
                        path: file_path,
                        language: language.map(|s| s.to_string()),
                        line_start,
                        line_end,
                        ..meta
                    }),
                    domain,
                )
            }
            Some(&"issues") => (SourceKind::IssueThread, Some(meta), domain),
            Some(&"merge_requests") => (SourceKind::PullRequest, Some(meta), domain),
            Some(&"releases") => (
                SourceKind::ReleaseNotes,
                Some(CodeMetadata {
                    ref_name: action.get(1).map(|s| s.to_string()),
                    ..meta
                }),
                domain,
            ),
            Some(&"tags") => (
                SourceKind::Tag,
                Some(CodeMetadata {
                    ref_name: action.get(1).map(|s| s.to_string()),
                    ..meta
                }),
                domain,
            ),
            Some(&"commit") | Some(&"commits") => (
                SourceKind::Commit,
                Some(CodeMetadata {
                    ref_name: action.get(1).map(|s| s.to_string()),
                    ..meta
                }),
                domain,
            ),
            _ => (SourceKind::SourceRepository, Some(meta), domain),
        }
    } else if segments.len() >= 2 {
        // No `/-/` separator — treat last segment as repo.
        let owner = segments[..segments.len() - 1].join("/");
        let repo = segments[segments.len() - 1].to_string();
        (
            SourceKind::RepositoryRoot,
            Some(CodeMetadata {
                host: Some(CodeHost::Gitlab),
                owner: if owner.is_empty() { None } else { Some(owner) },
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        )
    } else {
        (SourceKind::Unknown, None, domain)
    }
}

/// Parse a Codeberg URL into `SourceKind`, `CodeMetadata`, and domain.
///
/// Codeberg uses `/src/branch/...` and `/src/tag/...` for source paths.
pub fn parse_codeberg_url(url: &str) -> (SourceKind, Option<CodeMetadata>, Option<String>) {
    use url::Url;

    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return (SourceKind::Unknown, None, None),
    };
    let host = parsed.host_str().unwrap_or("");
    if host != "codeberg.org" {
        return (SourceKind::Unknown, None, None);
    }
    let domain = Some(host.to_string());
    let path = parsed.path();
    let fragment = parsed.fragment().unwrap_or("");

    let segments: Vec<&str> = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .collect();

    if segments.len() < 2 {
        return (SourceKind::Unknown, None, domain);
    }

    let owner = segments[0].to_string();
    let repo = segments[1].to_string();

    if segments.len() == 2 {
        return (
            SourceKind::RepositoryRoot,
            Some(CodeMetadata {
                host: Some(CodeHost::Codeberg),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        );
    }

    let rest = &segments[2..];
    match rest[0] {
        "src" => {
            // /src/branch/main/src/lib.rs or /src/tag/v1.2.3/...
            let ref_name = rest.get(2).map(|s| s.to_string());
            let file_path = if rest.len() > 3 {
                Some(rest[3..].join("/"))
            } else {
                None
            };
            let language = file_path.as_deref().and_then(language_from_extension);
            let (line_start, line_end) = if !fragment.is_empty() {
                parse_line_fragment(fragment)
            } else {
                (None, None)
            };

            let kind = match file_path.as_deref() {
                None => SourceKind::SourceDirectory,
                Some(path) if language_from_extension(path).is_some() => SourceKind::SourceFile,
                Some(path) if looks_like_file_path(path) => SourceKind::SourceFile,
                Some(_) => SourceKind::SourceDirectory,
            };

            (
                kind,
                Some(CodeMetadata {
                    host: Some(CodeHost::Codeberg),
                    owner: Some(owner),
                    repo: Some(repo),
                    ref_name,
                    path: file_path,
                    language: language.map(|s| s.to_string()),
                    line_start,
                    line_end,
                    ..Default::default()
                }),
                domain,
            )
        }
        "issues" => (
            SourceKind::IssueThread,
            Some(CodeMetadata {
                host: Some(CodeHost::Codeberg),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        ),
        "pulls" => (
            SourceKind::PullRequest,
            Some(CodeMetadata {
                host: Some(CodeHost::Codeberg),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        ),
        "releases" => {
            let tag = if rest.get(1) == Some(&"tag") {
                rest.get(2).map(|s| s.to_string())
            } else {
                None
            };
            (
                SourceKind::ReleaseNotes,
                Some(CodeMetadata {
                    host: Some(CodeHost::Codeberg),
                    owner: Some(owner),
                    repo: Some(repo),
                    ref_name: tag,
                    ..Default::default()
                }),
                domain,
            )
        }
        "commit" => (
            SourceKind::Commit,
            Some(CodeMetadata {
                host: Some(CodeHost::Codeberg),
                owner: Some(owner),
                repo: Some(repo),
                ref_name: rest.get(1).map(|s| s.to_string()),
                ..Default::default()
            }),
            domain,
        ),
        _ => (
            SourceKind::SourceRepository,
            Some(CodeMetadata {
                host: Some(CodeHost::Codeberg),
                owner: Some(owner),
                repo: Some(repo),
                ..Default::default()
            }),
            domain,
        ),
    }
}

/// Deterministic URL classification and metadata extraction for
/// code-host URLs. Falls back to domain heuristics for non-code URLs.
///
/// Returns `(SourceKind, Option<CodeMetadata>, Option<String>)` where
/// the third element is the domain.
pub fn classify_and_extract(url: &str) -> (SourceKind, Option<CodeMetadata>, Option<String>) {
    use url::Url;

    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return (SourceKind::Unknown, None, None),
    };
    let host = parsed.host_str().unwrap_or("");

    match host {
        "github.com" => parse_github_url(url),
        "gitlab.com" => parse_gitlab_url(url),
        "codeberg.org" => parse_codeberg_url(url),
        _ => {
            // Fall back to the existing domain-only heuristics in
            // source_card::classify_source_kind. We don't produce
            // CodeMetadata for non-code-host URLs.
            let kind = crate::core::source_card::classify_source_kind(url);
            (kind, None, Some(host.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_code_host_aliases() {
        assert_eq!(CodeHost::parse_alias("github"), Some(CodeHost::Github));
        assert_eq!(CodeHost::parse_alias("gh"), Some(CodeHost::Github));
        assert_eq!(CodeHost::parse_alias("github.com"), Some(CodeHost::Github));
        assert_eq!(CodeHost::parse_alias("gitlab"), Some(CodeHost::Gitlab));
        assert_eq!(CodeHost::parse_alias("gl"), Some(CodeHost::Gitlab));
        assert_eq!(CodeHost::parse_alias("gitlab.com"), Some(CodeHost::Gitlab));
        assert_eq!(CodeHost::parse_alias("codeberg"), Some(CodeHost::Codeberg));
        assert_eq!(CodeHost::parse_alias("cb"), Some(CodeHost::Codeberg));
        assert_eq!(
            CodeHost::parse_alias("codeberg.org"),
            Some(CodeHost::Codeberg)
        );
        assert_eq!(CodeHost::parse_alias("gitea"), Some(CodeHost::Gitea));
        assert_eq!(CodeHost::parse_alias("forgejo"), Some(CodeHost::Forgejo));
    }

    #[test]
    fn parse_code_host_alias_trims_and_normalizes() {
        assert_eq!(CodeHost::parse_alias(" GitHub "), Some(CodeHost::Github));
        assert_eq!(CodeHost::parse_alias("FORGEJO"), Some(CodeHost::Forgejo));
        assert_eq!(CodeHost::parse_alias("bitbucket"), None);
    }

    // --- GitHub URL tests ---

    #[test]
    fn github_repo_root() {
        let (kind, code, domain) = classify_and_extract("https://github.com/tokio-rs/axum");
        assert_eq!(kind, SourceKind::RepositoryRoot);
        assert_eq!(domain.as_deref(), Some("github.com"));
        let code = code.unwrap();
        assert_eq!(code.host, Some(CodeHost::Github));
        assert_eq!(code.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(code.repo.as_deref(), Some("axum"));
    }

    #[test]
    fn github_blob_file() {
        let (kind, code, _) =
            classify_and_extract("https://github.com/tokio-rs/axum/blob/main/src/lib.rs");
        assert_eq!(kind, SourceKind::SourceFile);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("main"));
        assert_eq!(code.path.as_deref(), Some("src/lib.rs"));
        assert_eq!(code.language.as_deref(), Some("rust"));
    }

    #[test]
    fn github_blob_with_line_start() {
        let (kind, code, _) =
            classify_and_extract("https://github.com/tokio-rs/axum/blob/main/src/lib.rs#L10");
        assert_eq!(kind, SourceKind::SourceFile);
        let code = code.unwrap();
        assert_eq!(code.line_start, Some(10));
        assert!(code.line_end.is_none());
    }

    #[test]
    fn github_blob_with_line_range() {
        let (kind, code, _) =
            classify_and_extract("https://github.com/tokio-rs/axum/blob/main/src/lib.rs#L10-L25");
        assert_eq!(kind, SourceKind::SourceFile);
        let code = code.unwrap();
        assert_eq!(code.line_start, Some(10));
        assert_eq!(code.line_end, Some(25));
    }

    #[test]
    fn github_tree_directory() {
        let (kind, code, _) =
            classify_and_extract("https://github.com/tokio-rs/axum/tree/main/src/foo");
        assert_eq!(kind, SourceKind::SourceDirectory);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("main"));
        assert_eq!(code.path.as_deref(), Some("src/foo"));
    }

    #[test]
    fn github_tree_bare() {
        let (kind, _, _) = classify_and_extract("https://github.com/tokio-rs/axum/tree/main");
        assert_eq!(kind, SourceKind::SourceDirectory);
    }

    #[test]
    fn github_issues() {
        let (kind, code, _) = classify_and_extract("https://github.com/tokio-rs/axum/issues/123");
        assert_eq!(kind, SourceKind::IssueThread);
        let code = code.unwrap();
        assert_eq!(code.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(code.repo.as_deref(), Some("axum"));
    }

    #[test]
    fn github_discussions() {
        let (kind, _, _) = classify_and_extract("https://github.com/tokio-rs/axum/discussions/456");
        assert_eq!(kind, SourceKind::IssueThread);
    }

    #[test]
    fn github_pull_request() {
        let (kind, code, _) = classify_and_extract("https://github.com/tokio-rs/axum/pull/789");
        assert_eq!(kind, SourceKind::PullRequest);
        let code = code.unwrap();
        assert_eq!(code.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(code.repo.as_deref(), Some("axum"));
    }

    #[test]
    fn github_releases() {
        let (kind, _, _) = classify_and_extract("https://github.com/tokio-rs/axum/releases");
        assert_eq!(kind, SourceKind::ReleaseNotes);
    }

    #[test]
    fn github_release_tag() {
        let (kind, code, _) =
            classify_and_extract("https://github.com/tokio-rs/axum/releases/tag/v0.7.0");
        assert_eq!(kind, SourceKind::ReleaseNotes);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("v0.7.0"));
    }

    #[test]
    fn github_tags() {
        let (kind, _, _) = classify_and_extract("https://github.com/tokio-rs/axum/tags");
        assert_eq!(kind, SourceKind::Tag);
    }

    #[test]
    fn github_commit() {
        let (kind, code, _) =
            classify_and_extract("https://github.com/tokio-rs/axum/commit/abc123def");
        assert_eq!(kind, SourceKind::Commit);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("abc123def"));
    }

    #[test]
    fn github_commits_main() {
        let (kind, code, _) = classify_and_extract("https://github.com/tokio-rs/axum/commits/main");
        assert_eq!(kind, SourceKind::Commit);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("main"));
    }

    // --- GitLab URL tests ---

    #[test]
    fn gitlab_repo_root() {
        let (kind, code, _) = classify_and_extract("https://gitlab.com/group/project");
        assert_eq!(kind, SourceKind::RepositoryRoot);
        let code = code.unwrap();
        assert_eq!(code.host, Some(CodeHost::Gitlab));
        assert_eq!(code.owner.as_deref(), Some("group"));
        assert_eq!(code.repo.as_deref(), Some("project"));
    }

    #[test]
    fn gitlab_nested_group() {
        let (kind, code, _) = classify_and_extract("https://gitlab.com/group/subgroup/project");
        assert_eq!(kind, SourceKind::RepositoryRoot);
        let code = code.unwrap();
        assert_eq!(code.owner.as_deref(), Some("group/subgroup"));
        assert_eq!(code.repo.as_deref(), Some("project"));
    }

    #[test]
    fn gitlab_blob() {
        let (kind, code, _) =
            classify_and_extract("https://gitlab.com/group/project/-/blob/main/src/lib.rs");
        assert_eq!(kind, SourceKind::SourceFile);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("main"));
        assert_eq!(code.path.as_deref(), Some("src/lib.rs"));
    }

    #[test]
    fn gitlab_tree() {
        let (kind, code, _) =
            classify_and_extract("https://gitlab.com/group/project/-/tree/main/src");
        assert_eq!(kind, SourceKind::SourceDirectory);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("main"));
        assert_eq!(code.path.as_deref(), Some("src"));
    }

    #[test]
    fn gitlab_merge_request() {
        let (kind, _, _) =
            classify_and_extract("https://gitlab.com/group/project/-/merge_requests/456");
        assert_eq!(kind, SourceKind::PullRequest);
    }

    #[test]
    fn gitlab_issues() {
        let (kind, _, _) = classify_and_extract("https://gitlab.com/group/project/-/issues/123");
        assert_eq!(kind, SourceKind::IssueThread);
    }

    #[test]
    fn gitlab_release() {
        let (kind, code, _) =
            classify_and_extract("https://gitlab.com/group/project/-/releases/v1.2.3");
        assert_eq!(kind, SourceKind::ReleaseNotes);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("v1.2.3"));
    }

    #[test]
    fn gitlab_tags() {
        let (kind, code, _) =
            classify_and_extract("https://gitlab.com/group/project/-/tags/v1.2.3");
        assert_eq!(kind, SourceKind::Tag);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("v1.2.3"));
    }

    #[test]
    fn gitlab_commit() {
        let (kind, code, _) =
            classify_and_extract("https://gitlab.com/group/project/-/commit/abc123");
        assert_eq!(kind, SourceKind::Commit);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("abc123"));
    }

    // --- Codeberg URL tests ---

    #[test]
    fn codeberg_repo_root() {
        let (kind, code, _) = classify_and_extract("https://codeberg.org/owner/repo");
        assert_eq!(kind, SourceKind::RepositoryRoot);
        let code = code.unwrap();
        assert_eq!(code.host, Some(CodeHost::Codeberg));
        assert_eq!(code.owner.as_deref(), Some("owner"));
        assert_eq!(code.repo.as_deref(), Some("repo"));
    }

    #[test]
    fn codeberg_src_branch() {
        let (kind, code, _) =
            classify_and_extract("https://codeberg.org/owner/repo/src/branch/main/src/lib.rs");
        assert_eq!(kind, SourceKind::SourceFile);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("main"));
        assert_eq!(code.path.as_deref(), Some("src/lib.rs"));
        assert_eq!(code.language.as_deref(), Some("rust"));
    }

    #[test]
    fn codeberg_src_tag() {
        let (kind, code, _) =
            classify_and_extract("https://codeberg.org/owner/repo/src/tag/v1.2.3/src/lib.rs");
        assert_eq!(kind, SourceKind::SourceFile);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("v1.2.3"));
        assert_eq!(code.path.as_deref(), Some("src/lib.rs"));
    }

    #[test]
    fn codeberg_issues() {
        let (kind, _, _) = classify_and_extract("https://codeberg.org/owner/repo/issues/123");
        assert_eq!(kind, SourceKind::IssueThread);
    }

    #[test]
    fn codeberg_pulls() {
        let (kind, _, _) = classify_and_extract("https://codeberg.org/owner/repo/pulls/456");
        assert_eq!(kind, SourceKind::PullRequest);
    }

    #[test]
    fn codeberg_release() {
        let (kind, code, _) =
            classify_and_extract("https://codeberg.org/owner/repo/releases/tag/v1.2.3");
        assert_eq!(kind, SourceKind::ReleaseNotes);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("v1.2.3"));
    }

    #[test]
    fn codeberg_commit() {
        let (kind, code, _) = classify_and_extract("https://codeberg.org/owner/repo/commit/abc123");
        assert_eq!(kind, SourceKind::Commit);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("abc123"));
    }

    #[test]
    fn codeberg_branch_source_file() {
        let (kind, code, _) =
            classify_and_extract("https://codeberg.org/owner/repo/src/branch/main/src/lib.rs");
        assert_eq!(kind, SourceKind::SourceFile);
        let code = code.unwrap();
        assert_eq!(code.ref_name.as_deref(), Some("main"));
        assert_eq!(code.path.as_deref(), Some("src/lib.rs"));
        assert_eq!(code.language.as_deref(), Some("rust"));
    }

    #[test]
    fn codeberg_branch_directory() {
        let (kind, code, _) =
            classify_and_extract("https://codeberg.org/owner/repo/src/branch/main/src");
        assert_eq!(kind, SourceKind::SourceDirectory);
        assert_eq!(code.unwrap().path.as_deref(), Some("src"));
    }

    #[test]
    fn codeberg_tag_source_file() {
        let (kind, code, _) =
            classify_and_extract("https://codeberg.org/owner/repo/src/tag/v1.0.0/src/lib.rs");
        assert_eq!(kind, SourceKind::SourceFile);
        assert_eq!(code.unwrap().ref_name.as_deref(), Some("v1.0.0"));
    }

    #[test]
    fn codeberg_branch_unknown_extension_file() {
        let (kind, _, _) =
            classify_and_extract("https://codeberg.org/owner/repo/src/branch/main/README.md");
        assert_eq!(kind, SourceKind::SourceFile);
    }

    #[test]
    fn codeberg_branch_dotfile() {
        let (kind, _, _) =
            classify_and_extract("https://codeberg.org/owner/repo/src/branch/main/.gitignore");
        // .gitignore starts with a dot — not a file by heuristic.
        assert_eq!(kind, SourceKind::SourceDirectory);
    }

    // --- Non-code URLs ---

    #[test]
    fn non_code_url_returns_no_metadata() {
        let (kind, code, _) = classify_and_extract("https://docs.rs/tower-http");
        assert_eq!(kind, SourceKind::OfficialDocs);
        assert!(code.is_none());
    }

    #[test]
    fn unknown_url_returns_unknown() {
        let (kind, code, _) = classify_and_extract("https://example.com/page");
        assert_eq!(kind, SourceKind::Unknown);
        assert!(code.is_none());
    }

    #[test]
    fn invalid_url_returns_unknown() {
        let (kind, code, _) = classify_and_extract("not a url");
        assert_eq!(kind, SourceKind::Unknown);
        assert!(code.is_none());
    }

    // --- Gitea/Forgejo URL classification ---
    //
    // classify_and_extract only produces CodeMetadata for github.com,
    // gitlab.com, and codeberg.org. Arbitrary Gitea/Forgejo URLs fall
    // through to domain-only heuristics (no CodeMetadata).

    #[test]
    fn gitea_self_hosted_url_no_code_metadata() {
        // classify_and_extract only produces SourceKind/CodeMetadata for
        // github.com, gitlab.com, and codeberg.org. Gitea self-hosted
        // URLs fall through to domain-only heuristics (kind = Unknown).
        let (kind, code, domain) =
            classify_and_extract("https://gitea.example.com/owner/repo/src/branch/main/src/lib.rs");
        assert_eq!(kind, SourceKind::Unknown);
        assert!(code.is_none());
        assert_eq!(domain.as_deref(), Some("gitea.example.com"));
    }

    #[test]
    fn forgejo_self_hosted_url_no_code_metadata() {
        let (kind, code, domain) = classify_and_extract(
            "https://forgejo.example.com/owner/repo/src/branch/main/src/lib.rs",
        );
        assert_eq!(kind, SourceKind::Unknown);
        assert!(code.is_none());
        assert_eq!(domain.as_deref(), Some("forgejo.example.com"));
    }

    // --- Language inference ---

    #[test]
    fn language_from_extension_common() {
        assert_eq!(language_from_extension("foo.rs"), Some("rust"));
        assert_eq!(language_from_extension("foo.py"), Some("python"));
        assert_eq!(language_from_extension("foo.ts"), Some("typescript"));
        assert_eq!(language_from_extension("foo.tsx"), Some("typescript"));
        assert_eq!(language_from_extension("foo.js"), Some("javascript"));
        assert_eq!(language_from_extension("foo.jsx"), Some("javascript"));
        assert_eq!(language_from_extension("foo.go"), Some("go"));
        assert_eq!(language_from_extension("foo.java"), Some("java"));
        assert_eq!(language_from_extension("foo.kt"), Some("kotlin"));
        assert_eq!(language_from_extension("foo.c"), Some("c"));
        assert_eq!(language_from_extension("foo.h"), Some("c"));
        assert_eq!(language_from_extension("foo.cpp"), Some("cpp"));
        assert_eq!(language_from_extension("foo.cc"), Some("cpp"));
        assert_eq!(language_from_extension("foo.hpp"), Some("cpp"));
        assert_eq!(language_from_extension("foo.md"), Some("markdown"));
        assert_eq!(language_from_extension("foo.toml"), Some("toml"));
        assert_eq!(language_from_extension("foo.yaml"), Some("yaml"));
        assert_eq!(language_from_extension("foo.yml"), Some("yaml"));
        assert_eq!(language_from_extension("foo.json"), Some("json"));
    }

    #[test]
    fn language_from_extension_unknown() {
        assert_eq!(language_from_extension("foo.txt"), None);
        assert_eq!(language_from_extension("foo"), None);
        assert_eq!(language_from_extension("foo.xyz"), None);
    }

    // --- Line fragment parsing ---

    #[test]
    fn parse_line_fragment_single() {
        let (s, e) = parse_line_fragment("L10");
        assert_eq!(s, Some(10));
        assert!(e.is_none());
    }

    #[test]
    fn parse_line_fragment_range() {
        let (s, e) = parse_line_fragment("L10-L25");
        assert_eq!(s, Some(10));
        assert_eq!(e, Some(25));
    }

    #[test]
    fn parse_line_fragment_invalid() {
        let (s, e) = parse_line_fragment("abc");
        assert!(s.is_none());
        assert!(e.is_none());
    }

    #[test]
    fn looks_like_file_path_cases() {
        assert!(looks_like_file_path("src/lib.rs"));
        assert!(looks_like_file_path("Cargo.toml"));
        assert!(looks_like_file_path("README.md"));
        assert!(!looks_like_file_path("src"));
        assert!(!looks_like_file_path(".gitignore"));
        assert!(!looks_like_file_path("file."));
    }
}
