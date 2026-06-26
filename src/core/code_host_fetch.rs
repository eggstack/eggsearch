//! Deterministic code-host URL rewriting for `web_fetch`.
//!
//! Converts recognized code-host source-file browser URLs to raw
//! content URLs for fetching. Returns `None` for non-code-host URLs,
//! directory URLs, issue/PR/release/tag/commit URLs, or any URL
//! that does not represent a single source file.

use crate::core::code_metadata::{classify_and_extract, CodeMetadata};
use crate::core::fetch::{FetchTransform, FetchTransformKind};
use crate::core::source_card::SourceKind;

/// The result of resolving a code-host URL for fetching.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeHostFetchTarget {
    /// The original user-provided URL.
    pub original_url: String,
    /// The raw content URL to fetch (if a rewrite is well-understood).
    pub raw_url: Option<String>,
    /// The source kind classification.
    pub source_kind: SourceKind,
    /// Structured code metadata extracted from the URL.
    pub code: Option<CodeMetadata>,
}

impl CodeHostFetchTarget {
    /// Build a `FetchTransform` describing this URL transformation.
    pub fn to_fetch_transform(&self, raw_url: &str) -> Option<FetchTransform> {
        let code = self.code.as_ref()?;
        let host = code.host?;
        let kind = match host {
            crate::core::code_metadata::CodeHost::Github => FetchTransformKind::GithubRawFile,
            crate::core::code_metadata::CodeHost::Gitlab => FetchTransformKind::GitlabRawFile,
            crate::core::code_metadata::CodeHost::Codeberg => FetchTransformKind::CodebergRawFile,
            _ => return None,
        };
        Some(FetchTransform {
            kind,
            original_url: self.original_url.clone(),
            transformed_url: raw_url.to_string(),
        })
    }
}

/// Attempt to resolve a code-host source-file URL into a raw fetch target.
///
/// Returns `None` for non-code-host URLs, non-file URLs (directories,
/// repos, issues, PRs, releases, tags, commits), or when the raw URL
/// shape is not well-understood.
///
/// Safety: the caller must validate the original URL and any produced
/// `raw_url` through the same SSRF/localhost/private-network policy
/// before fetching.
pub fn resolve_code_host_fetch_target(url: &str) -> Option<CodeHostFetchTarget> {
    let (source_kind, code, _domain) = classify_and_extract(url);

    // Only rewrite source-file URLs.
    if source_kind != SourceKind::SourceFile {
        return None;
    }

    let code = code?;
    let host = code.host?;
    let owner = code.owner.as_deref()?;
    let repo = code.repo.as_deref()?;
    let ref_name = code.ref_name.as_deref()?;
    let file_path = code.path.as_deref()?;

    let raw_url = match host {
        crate::core::code_metadata::CodeHost::Github => Some(format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/{ref_name}/{file_path}"
        )),
        crate::core::code_metadata::CodeHost::Gitlab => {
            let namespace = if let Some(owner) = code.owner.as_deref() {
                if owner.is_empty() {
                    repo.to_string()
                } else {
                    format!("{}/{}", owner, repo)
                }
            } else {
                repo.to_string()
            };
            Some(format!(
                "https://gitlab.com/{namespace}/-/raw/{ref_name}/{file_path}"
            ))
        }
        crate::core::code_metadata::CodeHost::Codeberg => {
            // Codeberg raw URLs: /raw/branch/<ref>/<path> or /raw/tag/<ref>/<path>.
            // We use "branch" as the ref type for simplicity; this works for
            // branch names but may not be correct for tags. For Phase 5 we
            // default to "branch" and document the limitation.
            Some(format!(
                "https://codeberg.org/{owner}/{repo}/raw/branch/{ref_name}/{file_path}"
            ))
        }
        _ => None,
    };

    Some(CodeHostFetchTarget {
        original_url: url.to_string(),
        raw_url,
        source_kind,
        code: Some(code),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- GitHub URL resolution ---

    #[test]
    fn github_blob_resolves_to_raw() {
        let target =
            resolve_code_host_fetch_target("https://github.com/tokio-rs/axum/blob/main/src/lib.rs")
                .unwrap();
        assert_eq!(
            target.raw_url.as_deref(),
            Some("https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs")
        );
        assert_eq!(target.source_kind, SourceKind::SourceFile);
        let code = target.code.unwrap();
        assert_eq!(code.owner.as_deref(), Some("tokio-rs"));
        assert_eq!(code.repo.as_deref(), Some("axum"));
        assert_eq!(code.ref_name.as_deref(), Some("main"));
        assert_eq!(code.path.as_deref(), Some("src/lib.rs"));
    }

    #[test]
    fn github_blob_with_line_anchor_resolves() {
        let target = resolve_code_host_fetch_target(
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs#L10-L25",
        )
        .unwrap();
        assert!(target.raw_url.is_some());
        let code = target.code.unwrap();
        assert_eq!(code.line_start, Some(10));
        assert_eq!(code.line_end, Some(25));
    }

    #[test]
    fn github_blob_with_tag_ref_resolves() {
        let target = resolve_code_host_fetch_target(
            "https://github.com/tokio-rs/axum/blob/v0.7.0/src/lib.rs",
        )
        .unwrap();
        assert_eq!(
            target.raw_url.as_deref(),
            Some("https://raw.githubusercontent.com/tokio-rs/axum/v0.7.0/src/lib.rs")
        );
    }

    #[test]
    fn github_blob_with_sha_ref_resolves() {
        let target = resolve_code_host_fetch_target(
            "https://github.com/tokio-rs/axum/blob/abc123def/src/lib.rs",
        )
        .unwrap();
        assert_eq!(
            target.raw_url.as_deref(),
            Some("https://raw.githubusercontent.com/tokio-rs/axum/abc123def/src/lib.rs")
        );
    }

    // --- GitHub non-file URLs return None ---

    #[test]
    fn github_repo_root_returns_none() {
        assert!(resolve_code_host_fetch_target("https://github.com/tokio-rs/axum").is_none());
    }

    #[test]
    fn github_tree_returns_none() {
        assert!(
            resolve_code_host_fetch_target("https://github.com/tokio-rs/axum/tree/main/src")
                .is_none()
        );
    }

    #[test]
    fn github_issues_returns_none() {
        assert!(
            resolve_code_host_fetch_target("https://github.com/tokio-rs/axum/issues/123").is_none()
        );
    }

    #[test]
    fn github_pull_returns_none() {
        assert!(
            resolve_code_host_fetch_target("https://github.com/tokio-rs/axum/pull/789").is_none()
        );
    }

    #[test]
    fn github_releases_returns_none() {
        assert!(
            resolve_code_host_fetch_target("https://github.com/tokio-rs/axum/releases").is_none()
        );
    }

    #[test]
    fn github_release_tag_returns_none() {
        assert!(resolve_code_host_fetch_target(
            "https://github.com/tokio-rs/axum/releases/tag/v0.7.0"
        )
        .is_none());
    }

    #[test]
    fn github_tags_returns_none() {
        assert!(resolve_code_host_fetch_target("https://github.com/tokio-rs/axum/tags").is_none());
    }

    #[test]
    fn github_commit_returns_none() {
        assert!(resolve_code_host_fetch_target(
            "https://github.com/tokio-rs/axum/commit/abc123def"
        )
        .is_none());
    }

    // --- GitLab URL resolution ---

    #[test]
    fn gitlab_blob_resolves_to_raw() {
        let target = resolve_code_host_fetch_target(
            "https://gitlab.com/group/project/-/blob/main/src/lib.rs",
        )
        .unwrap();
        assert_eq!(
            target.raw_url.as_deref(),
            Some("https://gitlab.com/group/project/-/raw/main/src/lib.rs")
        );
        assert_eq!(target.source_kind, SourceKind::SourceFile);
    }

    #[test]
    fn gitlab_nested_namespace_blob_resolves() {
        let target = resolve_code_host_fetch_target(
            "https://gitlab.com/group/subgroup/project/-/blob/main/src/lib.rs",
        )
        .unwrap();
        assert_eq!(
            target.raw_url.as_deref(),
            Some("https://gitlab.com/group/subgroup/project/-/raw/main/src/lib.rs")
        );
    }

    #[test]
    fn gitlab_blob_with_line_anchor_resolves() {
        let target = resolve_code_host_fetch_target(
            "https://gitlab.com/group/project/-/blob/main/src/lib.rs#L10-L25",
        )
        .unwrap();
        assert!(target.raw_url.is_some());
        let code = target.code.unwrap();
        assert_eq!(code.line_start, Some(10));
        assert_eq!(code.line_end, Some(25));
    }

    // --- GitLab non-file URLs return None ---

    #[test]
    fn gitlab_tree_returns_none() {
        assert!(
            resolve_code_host_fetch_target("https://gitlab.com/group/project/-/tree/main/src")
                .is_none()
        );
    }

    #[test]
    fn gitlab_issues_returns_none() {
        assert!(
            resolve_code_host_fetch_target("https://gitlab.com/group/project/-/issues/123")
                .is_none()
        );
    }

    // --- Codeberg URL resolution ---

    #[test]
    fn codeberg_src_branch_resolves_to_raw() {
        let target = resolve_code_host_fetch_target(
            "https://codeberg.org/owner/repo/src/branch/main/src/lib.rs",
        )
        .unwrap();
        assert_eq!(
            target.raw_url.as_deref(),
            Some("https://codeberg.org/owner/repo/raw/branch/main/src/lib.rs")
        );
        assert_eq!(target.source_kind, SourceKind::SourceFile);
    }

    #[test]
    fn codeberg_src_tag_resolves_to_raw() {
        let target = resolve_code_host_fetch_target(
            "https://codeberg.org/owner/repo/src/tag/v1.2.3/src/lib.rs",
        )
        .unwrap();
        // Codeberg uses "branch" in the raw URL shape for both branches and tags
        assert_eq!(
            target.raw_url.as_deref(),
            Some("https://codeberg.org/owner/repo/raw/branch/v1.2.3/src/lib.rs")
        );
    }

    #[test]
    fn codeberg_src_with_line_anchor_resolves() {
        let target = resolve_code_host_fetch_target(
            "https://codeberg.org/owner/repo/src/branch/main/src/lib.rs#L10-L25",
        )
        .unwrap();
        assert!(target.raw_url.is_some());
        let code = target.code.unwrap();
        assert_eq!(code.line_start, Some(10));
        assert_eq!(code.line_end, Some(25));
    }

    // --- Codeberg non-file URLs return None ---

    #[test]
    fn codeberg_repo_root_returns_none() {
        assert!(resolve_code_host_fetch_target("https://codeberg.org/owner/repo").is_none());
    }

    #[test]
    fn codeberg_directory_returns_none() {
        assert!(resolve_code_host_fetch_target(
            "https://codeberg.org/owner/repo/src/branch/main/src"
        )
        .is_none());
    }

    // --- Non-code-host URLs return None ---

    #[test]
    fn docs_rs_returns_none() {
        assert!(resolve_code_host_fetch_target("https://docs.rs/tower-http").is_none());
    }

    #[test]
    fn unknown_url_returns_none() {
        assert!(resolve_code_host_fetch_target("https://example.com/page").is_none());
    }

    #[test]
    fn invalid_url_returns_none() {
        assert!(resolve_code_host_fetch_target("not a url").is_none());
    }
}
