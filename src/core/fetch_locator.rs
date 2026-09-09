//! Shared locator resolution for web, repo, and local fetches.
//!
//! Consolidates host/ref/path/line/symbol resolution shared by
//! `batch_fetch`, `repo_fetch`, and suggested-fetch conversions so
//! locator semantics stay uniform without exposing one generic public
//! MCP fetch tool.

use std::path::{Component, Path};

use crate::core::code_metadata::CodeHost;
use crate::core::repo_fetch::RepoLocator;

/// Internal unified fetch locator distinguishing web, repo, and local
/// targets without collapsing public tool semantics.
#[derive(Clone, Debug)]
pub enum FetchLocator {
    /// Explicit HTTP(S) URL.
    WebUrl(String),
    /// Structured remote repository locator.
    Repo(RepoLocator),
    /// Local workspace file locator.
    Local(LocalLocator),
}

/// Local workspace file locator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalLocator {
    /// Workspace root directory name.
    pub root: String,
    /// File path relative to the workspace root.
    pub path: String,
}

/// Parsed repo host for batch items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchRepoHost {
    /// Remote code host.
    Remote(CodeHost),
    /// Local workspace host.
    Workspace,
}

/// Validate an explicit web URL for batch fetching.
pub fn validate_web_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("url must not be empty".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(format!(
            "url scheme must be http or https, got: {}",
            trimmed.chars().take(20).collect::<String>()
        ));
    }
    Ok(trimmed.to_string())
}

/// Validate a repository-relative file path.
pub fn validate_repo_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("path must not be empty".to_string());
    }
    let path_obj = Path::new(path);
    if path_obj
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("path must not contain '..'".to_string());
    }
    if path_obj.is_absolute()
        || path_obj
            .components()
            .any(|c| matches!(c, Component::RootDir | Component::Prefix(_)))
    {
        return Err(
            "path must be relative and must not be absolute (do not start with '/')".to_string(),
        );
    }
    Ok(())
}

/// Parse a batch repo host string.
pub fn parse_batch_repo_host(host: Option<&str>) -> Result<BatchRepoHost, String> {
    match host {
        None => Ok(BatchRepoHost::Remote(CodeHost::Github)),
        Some(h) => {
            let normalized = h.trim().to_ascii_lowercase();
            if normalized == "workspace" {
                return Ok(BatchRepoHost::Workspace);
            }
            match CodeHost::parse_alias(h) {
                Some(c) => Ok(BatchRepoHost::Remote(c)),
                None => Err(format!(
                    "unknown host '{normalized}'; accepted: {}, workspace",
                    CodeHost::accepted_aliases()
                )),
            }
        }
    }
}

/// Build an internal locator from batch repo item fields.
pub fn batch_repo_item_to_locator(
    host: Option<&str>,
    owner: &str,
    repo: &str,
    ref_name: Option<&str>,
    commit_sha: Option<&str>,
    path: &str,
) -> Result<FetchLocator, String> {
    if owner.trim().is_empty() {
        return Err("owner must not be empty".to_string());
    }
    if repo.trim().is_empty() {
        return Err("repo must not be empty".to_string());
    }
    validate_repo_path(path)?;
    match parse_batch_repo_host(host)? {
        BatchRepoHost::Workspace => Ok(FetchLocator::Local(LocalLocator {
            root: owner.to_string(),
            path: path.to_string(),
        })),
        BatchRepoHost::Remote(code_host) => Ok(FetchLocator::Repo(RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(code_host),
            owner: Some(owner.to_string()),
            repo: Some(repo.to_string()),
            ref_name: Some(ref_name.unwrap_or("main").to_string()),
            commit_sha: commit_sha.map(String::from),
            path: path.to_string(),
            workspace_root: None,
        })),
    }
}

/// Convert a code host to its canonical string form.
pub fn code_host_to_string(host: CodeHost) -> String {
    match host {
        CodeHost::Github => "github".to_string(),
        CodeHost::Gitlab => "gitlab".to_string(),
        CodeHost::Codeberg => "codeberg".to_string(),
        CodeHost::Gitea => "gitea".to_string(),
        CodeHost::Forgejo => "forgejo".to_string(),
        CodeHost::Unknown => "unknown".to_string(),
    }
}

/// Convert a structured repo fetch request to a batch repo item.
pub fn structured_repo_fetch_to_batch_item(
    req: &crate::core::repo_fetch::RepoFetchRequest,
) -> crate::core::batch_fetch::BatchFetchItem {
    use crate::core::batch_fetch::BatchFetchItem;
    let host = req.host.map(code_host_to_string);
    BatchFetchItem::Repo {
        host,
        owner: req.owner.clone(),
        repo: req.repo.clone(),
        ref_name: req.ref_name.clone(),
        commit_sha: req.commit_sha.clone(),
        path: req.path.clone(),
        line_start: req.line_start,
        line_end: req.line_end,
        context_before: req.context_before,
        context_after: req.context_after,
        max_chars: req.max_chars,
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
    }
}

/// Convert a URL to a batch web item.
pub fn url_to_batch_web_item(
    url: &str,
    extract_mode: Option<crate::core::fetch::ExtractMode>,
) -> crate::core::batch_fetch::BatchFetchItem {
    crate::core::batch_fetch::BatchFetchItem::Web {
        url: url.to_string(),
        extract_mode,
        include_links: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_url_validation() {
        assert!(validate_web_url("").is_err());
        assert!(validate_web_url("ftp://example.com").is_err());
        assert!(validate_web_url("https://example.com").is_ok());
        assert!(validate_web_url("HTTP://example.com").is_ok());
    }

    #[test]
    fn repo_path_validation() {
        assert!(validate_repo_path("").is_err());
        assert!(validate_repo_path("../evil").is_err());
        assert!(validate_repo_path("/absolute").is_err());
        assert!(validate_repo_path("src/lib.rs").is_ok());
    }

    #[test]
    fn batch_host_parsing() {
        assert!(matches!(
            parse_batch_repo_host(None).unwrap(),
            BatchRepoHost::Remote(_)
        ));
        assert!(matches!(
            parse_batch_repo_host(Some("workspace")).unwrap(),
            BatchRepoHost::Workspace
        ));
        assert!(parse_batch_repo_host(Some("nope")).is_err());
    }

    #[test]
    fn repo_item_to_locator() {
        let loc = batch_repo_item_to_locator(
            Some("github"),
            "tokio-rs",
            "axum",
            None,
            None,
            "src/lib.rs",
        )
        .unwrap();
        assert!(matches!(loc, FetchLocator::Repo(_)));
        let loc = batch_repo_item_to_locator(
            Some("workspace"),
            "myroot",
            "ignored",
            None,
            None,
            "src/main.rs",
        )
        .unwrap();
        assert!(matches!(loc, FetchLocator::Local(_)));
    }
}
