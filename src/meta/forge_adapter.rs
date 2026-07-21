//! Native remote repository tree adapter for code hosts.
//!
//! Provides bounded, provider-neutral tree retrieval for GitHub, GitLab,
//! Gitea, Forgejo, and Codeberg without cloning repositories. All tree
//! operations enforce entry, depth, byte, pagination, concurrency, and
//! timeout limits.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::core::code_metadata::{language_from_extension, CodeHost};
use crate::core::repo_fetch::{
    codeberg_browser_url, codeberg_raw_url, gitea_browser_url, gitea_raw_url, github_browser_url,
    github_permalink_url, github_raw_permalink_url, github_raw_url, gitlab_browser_url,
    gitlab_raw_url,
};
use crate::core::repo_map::{
    classify_important_directory, classify_important_file, ImportantDirKind, ImportantFileKind,
    RepoImportantDirectory, RepoImportantFile, RepoMapEntry, RepoMapEntryKind, RepoMapMode,
    RepoMapRequest, RepoMapResponse, RepoMapTelemetry, RepoPathSummary,
};
use crate::core::result::SearchWarning;
use crate::core::sanitize::TrustMarkers;
use crate::core::warning::{AgentWarning, WarningCode};
use crate::meta::repo_mapper::build_repo_map_suggested_fetches;

const DEFAULT_MAX_ENTRIES: usize = 1000;
const DEFAULT_MAX_DEPTH: usize = 10;
const DEFAULT_MAX_PAGES: usize = 10;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_IMPORTANT_FILE_PROBES: usize = 50;
const MAX_IMPORTANT_DIR_PROBES: usize = 50;
const MAX_CONCURRENT_FORGE_REQUESTS: usize = 4;
const GITHUB_API_BASE: &str = "https://api.github.com";
const GITLAB_API_BASE: &str = "https://gitlab.com/api/v4";
const CODEBERG_API_BASE: &str = "https://codeberg.org/api/v1";

static FORGE_SEMAPHORE: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(MAX_CONCURRENT_FORGE_REQUESTS));

async fn read_bounded_response(
    resp: reqwest::Response,
    per_response_cap: usize,
    total_bytes: &mut usize,
) -> Result<String, String> {
    if let Some(content_length) = resp.content_length() {
        if content_length as usize > per_response_cap {
            return Err("response_too_large".into());
        }
    }
    let mut body = String::new();
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("failed to read response chunk: {e}"))?;
        *total_bytes += chunk.len();
        if *total_bytes > per_response_cap {
            return Err("response_too_large".into());
        }
        body.push_str(
            std::str::from_utf8(&chunk)
                .map_err(|e| format!("response body is not valid UTF-8: {e}"))?,
        );
    }
    Ok(body)
}

/// Configuration for connecting to a forge API.
#[derive(Debug, Clone)]
pub struct ForgeTreeConfig {
    /// Optional API key for authenticated requests.
    pub api_key: Option<String>,
    /// Optional base URL override for the API endpoint.
    pub base_url: Option<String>,
}

/// Response from a forge tree API call, containing raw entries and metadata.
#[derive(Debug)]
pub struct ForgeTreeResponse {
    /// Raw tree entries from the API.
    pub entries: Vec<ForgeRawEntry>,
    /// The repository's default branch, if resolved.
    pub default_branch: Option<String>,
    /// The resolved ref/commit SHA used for the tree.
    pub resolved_ref: Option<String>,
    /// Whether the provider reported a truncated response.
    pub truncated_by_provider: bool,
    /// Warnings accumulated during the fetch.
    pub warnings: Vec<SearchWarning>,
    /// The provider ID that served this response.
    pub provider_id: String,
}

/// A raw tree entry from a forge API response.
#[derive(Debug, Clone)]
pub struct ForgeRawEntry {
    /// Relative path from the repository root.
    pub path: String,
    /// The kind of entry.
    pub kind: EntryKind,
    /// File size in bytes, if known.
    pub size: Option<u64>,
    /// Object SHA, if available.
    pub sha: Option<String>,
}

/// Entry kind in a forge tree response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A file.
    File,
    /// A directory.
    Directory,
    /// A symbolic link.
    Symlink,
    /// A git submodule.
    Submodule,
}

fn build_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .user_agent("eggsearch/1.0")
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

fn timeout_duration(req: &RepoMapRequest) -> Duration {
    let ms = req.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_SECS * 1000);
    Duration::from_millis(ms.clamp(1000, DEFAULT_TIMEOUT_SECS * 1000))
}

fn max_entries(req: &RepoMapRequest) -> usize {
    req.max_entries.unwrap_or(DEFAULT_MAX_ENTRIES).max(1)
}

fn max_depth(req: &RepoMapRequest) -> usize {
    req.max_depth.unwrap_or(DEFAULT_MAX_DEPTH).max(1)
}

/// Fetch the repository tree from a supported code host.
///
/// Routes to the appropriate host-specific adapter (GitHub, GitLab,
/// Gitea, Forgejo, Codeberg) and returns raw tree entries with metadata.
pub async fn fetch_tree(
    host: CodeHost,
    owner: &str,
    repo: &str,
    req: &RepoMapRequest,
    config: &ForgeTreeConfig,
) -> Result<ForgeTreeResponse, String> {
    let _permit = FORGE_SEMAPHORE
        .acquire()
        .await
        .map_err(|_| "concurrency limit exceeded".to_string())?;
    if let Some(ref base) = config.base_url {
        validate_base_url(base, config.api_key.as_deref())?;
    }
    let client = build_client()?;
    let timeout = timeout_duration(req);

    match host {
        CodeHost::Github => fetch_github_tree(&client, owner, repo, req, config, timeout).await,
        CodeHost::Gitlab => fetch_gitlab_tree(&client, owner, repo, req, config, timeout).await,
        CodeHost::Codeberg => {
            let base = config.base_url.as_deref().unwrap_or(CODEBERG_API_BASE);
            fetch_forge_tree(ForgeTreeParams {
                client: &client,
                owner,
                repo,
                req,
                config,
                timeout,
                api_base: base,
                provider_id: "codeberg_tree",
            })
            .await
        }
        CodeHost::Gitea | CodeHost::Forgejo => {
            let base = config
                .base_url
                .as_deref()
                .unwrap_or("https://gitea.example.com/api/v1");
            let provider_id = match host {
                CodeHost::Gitea => "gitea_tree",
                CodeHost::Forgejo => "forgejo_tree",
                _ => unreachable!(),
            };
            fetch_forge_tree(ForgeTreeParams {
                client: &client,
                owner,
                repo,
                req,
                config,
                timeout,
                api_base: base,
                provider_id,
            })
            .await
        }
        CodeHost::Unknown => Err("unsupported host: cannot fetch tree for unknown host".into()),
    }
}

async fn fetch_github_tree(
    client: &Client,
    owner: &str,
    repo: &str,
    req: &RepoMapRequest,
    config: &ForgeTreeConfig,
    timeout: Duration,
) -> Result<ForgeTreeResponse, String> {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let ref_name = req.ref_name.as_deref().unwrap_or("HEAD");
    let max_d = max_depth(req);

    let mut builder = client
        .get(format!("{base}/repos/{owner}/{repo}/git/trees/{ref_name}"))
        .query(&[("recursive", if max_d > 1 { "1" } else { "0" })])
        .timeout(timeout);

    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", format!("Bearer {key}"));
    }

    let resp = builder
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {e}"))?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err("repository_not_found".into());
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err("authentication_required".into());
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        let msg = resp.text().await.unwrap_or_default();
        if msg.contains("rate limit") || msg.contains("Rate limit") {
            return Err("rate_limited".into());
        }
        return Err("permission_denied".into());
    }
    if !status.is_success() {
        let msg = resp.text().await.unwrap_or_default();
        return Err(format!("provider_unavailable: {status} - {msg}"));
    }

    let mut total_bytes = 0usize;
    let body = read_bounded_response(resp, DEFAULT_MAX_RESPONSE_BYTES, &mut total_bytes).await?;

    let tree: GitHubTreeResponse =
        serde_json::from_str(&body).map_err(|e| format!("malformed response: {e}"))?;

    let truncated_by_provider = tree.truncated.unwrap_or(false);
    let resolved_ref = tree.sha.clone();
    let default_branch = resolve_github_default_branch(client, owner, repo, config, timeout).await;

    let mut entries: Vec<ForgeRawEntry> = tree
        .tree
        .into_iter()
        .map(|item| {
            let kind = match item.type_field.as_str() {
                "blob" => {
                    if item.mode.as_deref() == Some("120000") {
                        EntryKind::Symlink
                    } else {
                        EntryKind::File
                    }
                }
                "tree" => EntryKind::Directory,
                "commit" => EntryKind::Submodule,
                _ => EntryKind::File,
            };
            ForgeRawEntry {
                path: item.path,
                kind,
                size: item.size,
                sha: item.sha,
            }
        })
        .collect();

    if truncated_by_provider {
        if let Ok(fallback) = fetch_github_contents_root(client, owner, repo, config, timeout).await
        {
            let existing_paths: std::collections::HashSet<String> =
                entries.iter().map(|e| e.path.clone()).collect();
            for entry in fallback {
                if !existing_paths.contains(&entry.path) {
                    entries.push(entry);
                }
            }
        }
    }

    let mut warnings = Vec::new();
    if truncated_by_provider {
        warnings.push(SearchWarning::new(
            "github_tree",
            "response_truncated_by_provider: GitHub tree response was truncated; \
             results may be incomplete",
        ));
    }

    Ok(ForgeTreeResponse {
        entries,
        default_branch,
        resolved_ref,
        truncated_by_provider,
        warnings,
        provider_id: "github_tree".to_string(),
    })
}

async fn resolve_github_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
) -> Option<String> {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let mut builder = client
        .get(format!("{base}/repos/{owner}/{repo}"))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", format!("Bearer {key}"));
    }
    let resp = builder.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let repo_info: GitHubRepoInfo = resp.json().await.ok()?;
    Some(repo_info.default_branch)
}

async fn fetch_github_contents_root(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
) -> Result<Vec<ForgeRawEntry>, String> {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let mut builder = client
        .get(format!("{base}/repos/{owner}/{repo}/contents/"))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", format!("Bearer {key}"));
    }
    let resp = builder
        .send()
        .await
        .map_err(|e| format!("GitHub Contents API request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("contents_api_failed: {}", resp.status()));
    }
    let mut total_bytes = 0usize;
    let body = read_bounded_response(resp, DEFAULT_MAX_RESPONSE_BYTES, &mut total_bytes).await?;
    let items: Vec<GitHubContentsEntry> =
        serde_json::from_str(&body).map_err(|e| format!("malformed Contents response: {e}"))?;
    let entries = items
        .into_iter()
        .map(|item| {
            let kind = match item.type_field.as_str() {
                "dir" => EntryKind::Directory,
                "file" => EntryKind::File,
                "symlink" => EntryKind::Symlink,
                "submodule" => EntryKind::Submodule,
                _ => EntryKind::File,
            };
            ForgeRawEntry {
                path: item.name,
                kind,
                size: item.size,
                sha: item.sha,
            }
        })
        .collect();
    Ok(entries)
}

#[derive(Deserialize)]
struct GitHubContentsEntry {
    name: String,
    #[serde(rename = "type")]
    type_field: String,
    size: Option<u64>,
    sha: Option<String>,
}

#[derive(Deserialize)]
struct GitHubRepoInfo {
    default_branch: String,
}

#[derive(Deserialize)]
struct GitHubTreeResponse {
    sha: Option<String>,
    truncated: Option<bool>,
    tree: Vec<GitHubTreeEntry>,
}

#[derive(Deserialize)]
struct GitHubTreeEntry {
    path: String,
    mode: Option<String>,
    #[serde(rename = "type")]
    type_field: String,
    size: Option<u64>,
    sha: Option<String>,
}

async fn fetch_gitlab_tree(
    client: &Client,
    owner: &str,
    repo: &str,
    req: &RepoMapRequest,
    config: &ForgeTreeConfig,
    timeout: Duration,
) -> Result<ForgeTreeResponse, String> {
    let base = config.base_url.as_deref().unwrap_or(GITLAB_API_BASE);
    let project_path_raw = format!("{owner}/{repo}");
    let project_path = urlencoding::encode(&project_path_raw);
    let ref_name = req.ref_name.as_deref().unwrap_or("HEAD");
    let max_d = max_depth(req);
    let max_e = max_entries(req);
    let per_page = 100.min(max_e);

    let mut all_entries: Vec<ForgeRawEntry> = Vec::new();
    let mut page = 1u32;
    let mut truncated_by_provider = false;
    let mut warnings = Vec::new();
    let max_pages = DEFAULT_MAX_PAGES;
    let mut total_bytes = 0usize;

    loop {
        if page > max_pages as u32 {
            warnings.push(SearchWarning::new(
                "gitlab_tree",
                "pagination_limit_reached: GitLab tree pagination hit page limit",
            ));
            break;
        }
        if all_entries.len() >= max_e {
            break;
        }

        let mut builder = client
            .get(format!("{base}/projects/{project_path}/repository/tree"))
            .query(&[
                ("ref", ref_name),
                ("recursive", if max_d > 1 { "true" } else { "false" }),
                ("per_page", &per_page.to_string()),
                ("page", &page.to_string()),
            ])
            .timeout(timeout);

        if let Some(ref key) = config.api_key {
            builder = builder.header("PRIVATE-TOKEN", key.as_str());
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| format!("GitLab API request failed: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            if all_entries.is_empty() {
                return Err("repository_not_found".into());
            }
            break;
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("authentication_required".into());
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err("permission_denied".into());
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err("rate_limited".into());
        }
        if !status.is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(format!("provider_unavailable: {status} - {msg}"));
        }

        let body =
            read_bounded_response(resp, DEFAULT_MAX_RESPONSE_BYTES, &mut total_bytes).await?;

        let items: Vec<GitLabTreeEntry> =
            serde_json::from_str(&body).map_err(|e| format!("malformed response: {e}"))?;

        let page_len = items.len();
        for item in items {
            let kind = match item.type_field.as_str() {
                "blob" => EntryKind::File,
                "tree" => EntryKind::Directory,
                "commit" => EntryKind::Submodule,
                _ => EntryKind::File,
            };
            all_entries.push(ForgeRawEntry {
                path: item.path,
                kind,
                size: item.size,
                sha: item.id,
            });
        }

        if page_len < per_page {
            break;
        }
        page += 1;
    }

    if all_entries.len() >= max_e {
        truncated_by_provider = true;
        warnings.push(SearchWarning::new(
            "gitlab_tree",
            "response_truncated_by_eggsearch: entry limit reached",
        ));
        all_entries.truncate(max_e);
    }

    let default_branch = resolve_gitlab_default_branch(client, owner, repo, config, timeout).await;

    Ok(ForgeTreeResponse {
        entries: all_entries,
        default_branch,
        resolved_ref: Some(ref_name.to_string()),
        truncated_by_provider,
        warnings,
        provider_id: "gitlab_tree".to_string(),
    })
}

async fn resolve_gitlab_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
) -> Option<String> {
    let base = config.base_url.as_deref().unwrap_or(GITLAB_API_BASE);
    let project_path_raw = format!("{owner}/{repo}");
    let project_path = urlencoding::encode(&project_path_raw);
    let mut builder = client
        .get(format!("{base}/projects/{project_path}"))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("PRIVATE-TOKEN", key.as_str());
    }
    let resp = builder.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let info: GitLabProjectInfo = resp.json().await.ok()?;
    Some(info.default_branch)
}

#[derive(Deserialize)]
struct GitLabProjectInfo {
    default_branch: String,
}

#[derive(Deserialize)]
struct GitLabTreeEntry {
    id: Option<String>,
    path: String,
    #[serde(rename = "type")]
    type_field: String,
    size: Option<u64>,
}

struct ForgeTreeParams<'a> {
    client: &'a Client,
    owner: &'a str,
    repo: &'a str,
    req: &'a RepoMapRequest,
    config: &'a ForgeTreeConfig,
    timeout: Duration,
    api_base: &'a str,
    provider_id: &'a str,
}

async fn fetch_forge_tree(params: ForgeTreeParams<'_>) -> Result<ForgeTreeResponse, String> {
    let ForgeTreeParams {
        client,
        owner,
        repo,
        req,
        config,
        timeout,
        api_base,
        provider_id,
    } = params;
    let ref_name = req.ref_name.as_deref().unwrap_or("HEAD");
    let max_d = max_depth(req);
    let max_e = max_entries(req);
    let per_page = 100.min(max_e);

    let mut all_entries: Vec<ForgeRawEntry> = Vec::new();
    let mut page = 1u32;
    let mut truncated_by_provider = false;
    let mut warnings = Vec::new();
    let max_pages = DEFAULT_MAX_PAGES;
    let mut total_bytes = 0usize;

    loop {
        if page > max_pages as u32 {
            warnings.push(SearchWarning::new(
                provider_id,
                "pagination_limit_reached: forge tree pagination hit page limit",
            ));
            break;
        }
        if all_entries.len() >= max_e {
            break;
        }

        let mut builder = client
            .get(format!(
                "{api_base}/repos/{owner}/{repo}/git/trees/{ref_name}"
            ))
            .query(&[
                ("recursive", if max_d > 1 { "1" } else { "0" }),
                ("per_page", &per_page.to_string()),
                ("page", &page.to_string()),
            ])
            .timeout(timeout);

        if let Some(ref key) = config.api_key {
            builder = builder.header("Authorization", format!("token {key}"));
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| format!("forge API request failed: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            if all_entries.is_empty() {
                return Err("repository_not_found".into());
            }
            break;
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("authentication_required".into());
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err("permission_denied".into());
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err("rate_limited".into());
        }
        if !status.is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(format!("provider_unavailable: {status} - {msg}"));
        }

        let body =
            read_bounded_response(resp, DEFAULT_MAX_RESPONSE_BYTES, &mut total_bytes).await?;

        let tree: ForgeTreeApiResponse =
            serde_json::from_str(&body).map_err(|e| format!("malformed response: {e}"))?;

        truncated_by_provider = tree.truncated.unwrap_or(false);

        let page_len = tree.tree.len();
        for item in tree.tree {
            let kind = match item.type_field.as_str() {
                "blob" => {
                    if item.mode.as_deref() == Some("120000") {
                        EntryKind::Symlink
                    } else {
                        EntryKind::File
                    }
                }
                "tree" => EntryKind::Directory,
                "commit" => EntryKind::Submodule,
                _ => EntryKind::File,
            };
            all_entries.push(ForgeRawEntry {
                path: item.path,
                kind,
                size: item.size,
                sha: item.sha,
            });
        }

        if page_len < per_page {
            break;
        }
        page += 1;
    }

    if all_entries.len() >= max_e {
        truncated_by_provider = true;
        warnings.push(SearchWarning::new(
            provider_id,
            "response_truncated_by_eggsearch: entry limit reached",
        ));
        all_entries.truncate(max_e);
    }

    let default_branch =
        resolve_forge_default_branch(client, owner, repo, config, timeout, api_base).await;

    if truncated_by_provider {
        warnings.push(SearchWarning::new(
            provider_id,
            "response_truncated_by_provider: forge tree response was truncated",
        ));
    }

    Ok(ForgeTreeResponse {
        entries: all_entries,
        default_branch,
        resolved_ref: Some(ref_name.to_string()),
        truncated_by_provider,
        warnings,
        provider_id: provider_id.to_string(),
    })
}

async fn resolve_forge_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    api_base: &str,
) -> Option<String> {
    let mut builder = client
        .get(format!("{api_base}/repos/{owner}/{repo}"))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", format!("token {key}"));
    }
    let resp = builder.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let info: ForgeRepoInfo = resp.json().await.ok()?;
    Some(info.default_branch)
}

#[derive(Deserialize)]
struct ForgeRepoInfo {
    default_branch: String,
}

#[derive(Deserialize)]
struct ForgeTreeApiResponse {
    truncated: Option<bool>,
    tree: Vec<ForgeTreeApiEntry>,
}

#[derive(Deserialize)]
struct ForgeTreeApiEntry {
    path: String,
    mode: Option<String>,
    #[serde(rename = "type")]
    type_field: String,
    size: Option<u64>,
    sha: Option<String>,
}

/// Compute browser and raw URLs for a tree entry based on the host.
///
/// For Gitea/Forgejo, `gitea_base_url` should be the instance root URL
/// (e.g. `https://gitea.example.com`), not the API base.
fn build_entry_urls(
    host: CodeHost,
    owner: &str,
    repo: &str,
    ref_name: &str,
    sha: Option<&str>,
    path: &str,
    gitea_base_url: Option<&str>,
) -> (Option<String>, Option<String>) {
    let (browser, raw) = match host {
        CodeHost::Github => {
            if let Some(sha) = sha {
                (
                    github_permalink_url(owner, repo, sha, path),
                    github_raw_permalink_url(owner, repo, sha, path),
                )
            } else {
                (
                    github_browser_url(owner, repo, ref_name, path),
                    github_raw_url(owner, repo, ref_name, path),
                )
            }
        }
        CodeHost::Gitlab => (
            gitlab_browser_url(owner, repo, ref_name, path),
            gitlab_raw_url(owner, repo, ref_name, path),
        ),
        CodeHost::Codeberg => (
            codeberg_browser_url(owner, repo, ref_name, path),
            codeberg_raw_url(owner, repo, ref_name, path),
        ),
        CodeHost::Gitea | CodeHost::Forgejo => {
            if let Some(base) = gitea_base_url {
                (
                    gitea_browser_url(base, owner, repo, ref_name, path),
                    gitea_raw_url(base, owner, repo, ref_name, path),
                )
            } else {
                (String::new(), String::new())
            }
        }
        CodeHost::Unknown => (String::new(), String::new()),
    };
    let browser_opt = if browser.is_empty() {
        None
    } else {
        Some(browser)
    };
    let raw_opt = if raw.is_empty() { None } else { Some(raw) };
    (browser_opt, raw_opt)
}

/// Validate a user-supplied base URL for safety.
///
/// Ensures the URL uses HTTPS, does not point to localhost, loopback,
/// or private IP ranges, does not contain embedded credentials, and
/// (when `api_key` is provided) rejects plain HTTP.
/// Returns `Ok(())` if valid, or an error message.
pub fn validate_base_url(url: &str, api_key: Option<&str>) -> Result<(), String> {
    let parsed = url
        .parse::<reqwest::Url>()
        .map_err(|e| format!("invalid base URL: {e}"))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(format!(
            "base URL must use http or https, got: {}",
            parsed.scheme()
        ));
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return Err("base URL must not contain embedded credentials".into());
    }
    if parsed.scheme() == "http" && api_key.is_some() {
        if let Some(host) = parsed.host_str() {
            if !is_loopback_host(host) {
                return Err("credential-bearing endpoint must use HTTPS".into());
            }
        }
    }
    if let Some(host) = parsed.host_str() {
        if parsed.scheme() == "https" {
            if host == "localhost"
                || host == "127.0.0.1"
                || host == "::1"
                || host == "0.0.0.0"
                || host.starts_with("192.168.")
                || host.starts_with("10.")
                || (host.starts_with("172.")
                    && host
                        .split('.')
                        .nth(1)
                        .and_then(|s| s.parse::<u8>().ok())
                        .is_some_and(|o| (16..=31).contains(&o)))
            {
                return Err(format!(
                    "HTTPS base URL must not point to localhost or private network: {host}"
                ));
            }
            if host.starts_with('[') {
                let inner = host.trim_start_matches('[').trim_end_matches(']');
                if let Ok(ip) = inner.parse::<Ipv6Addr>() {
                    let class = classify_ipv6_forge(ip);
                    if matches!(
                        class,
                        ForgeAddressClass::Loopback
                            | ForgeAddressClass::Private
                            | ForgeAddressClass::LinkLocal
                            | ForgeAddressClass::Documentation
                            | ForgeAddressClass::Reserved
                    ) {
                        return Err(format!(
                            "HTTPS base URL must not point to localhost or private network: {host}"
                        ));
                    }
                }
            }
        } else if !is_loopback_host(host) {
            return Err(format!(
                "HTTP base URL must point to localhost for development use: {host}"
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForgeAddressClass {
    Loopback,
    Private,
    LinkLocal,
    Documentation,
    Reserved,
    Public,
}

fn classify_ipv6_forge(v6: Ipv6Addr) -> ForgeAddressClass {
    if v6.is_loopback() {
        return ForgeAddressClass::Loopback;
    }
    if v6.is_unspecified() {
        return ForgeAddressClass::Reserved;
    }
    if v6.is_multicast() {
        return ForgeAddressClass::Reserved;
    }
    let seg0 = v6.segments()[0];
    if (seg0 & 0xfe00) == 0xfc00 {
        return ForgeAddressClass::Private;
    }
    if (seg0 & 0xffc0) == 0xfe80 {
        return ForgeAddressClass::LinkLocal;
    }
    if let Some(v4) = ipv4_mapped_from_v6_forge(v6) {
        return classify_ipv4_forge(v4);
    }
    let seg1 = v6.segments()[1];
    if seg0 == 0x2001 && seg1 == 0x0db8 {
        return ForgeAddressClass::Documentation;
    }
    if seg0 == 0x2001 && (seg1 == 0x0002 || seg1 == 0x0000) {
        return ForgeAddressClass::Reserved;
    }
    if seg0 == 0x2002 {
        return ForgeAddressClass::Reserved;
    }
    ForgeAddressClass::Public
}

fn classify_ipv4_forge(v4: Ipv4Addr) -> ForgeAddressClass {
    if v4.is_loopback() {
        return ForgeAddressClass::Loopback;
    }
    if v4.is_link_local() {
        return ForgeAddressClass::LinkLocal;
    }
    if v4.is_unspecified() {
        return ForgeAddressClass::Reserved;
    }
    let o = v4.octets();
    let octet0 = o[0];
    if octet0 == 0 || octet0 == 127 {
        return ForgeAddressClass::Loopback;
    }
    if octet0 == 10 {
        return ForgeAddressClass::Private;
    }
    if octet0 == 100 && (o[1] & 0b1100_0000) == 0b0100_0000 {
        return ForgeAddressClass::Reserved;
    }
    if octet0 == 169 && o[1] == 254 {
        return ForgeAddressClass::LinkLocal;
    }
    if octet0 == 172 && (o[1] & 0b1111_0000) == 16 {
        return ForgeAddressClass::Private;
    }
    if octet0 == 192 && o[1] == 168 {
        return ForgeAddressClass::Private;
    }
    if octet0 == 192 && o[1] == 0 && o[2] == 2 {
        return ForgeAddressClass::Documentation;
    }
    if octet0 == 198 && o[1] == 51 && o[2] == 100 {
        return ForgeAddressClass::Documentation;
    }
    if octet0 == 203 && o[1] == 0 && o[2] == 113 {
        return ForgeAddressClass::Documentation;
    }
    if (224..=239).contains(&octet0) {
        return ForgeAddressClass::Reserved;
    }
    if octet0 >= 240 {
        return ForgeAddressClass::Reserved;
    }
    ForgeAddressClass::Public
}

fn ipv4_mapped_from_v6_forge(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    match v6.to_ipv4_mapped() {
        Some(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

fn is_loopback_host(host: &str) -> bool {
    host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "0.0.0.0"
}

/// Derive the Gitea/Forgejo instance root URL from an API base URL.
///
/// E.g. `https://gitea.example.com/api/v1` → `https://gitea.example.com`.
pub fn derive_gitea_instance_root(api_base: &str) -> String {
    let base = api_base.trim_end_matches('/');
    if let Some(pos) = base.rfind("/api") {
        let root = &base[..pos];
        if root.starts_with("http") {
            return root.to_string();
        }
    }
    base.to_string()
}

/// Convert a `ForgeTreeResponse` into a provider-neutral `RepoMapResponse`.
///
/// Applies depth filtering, entry classification, and builds suggested fetches.
pub fn build_response(
    request: &RepoMapRequest,
    forge_response: ForgeTreeResponse,
    include_files: bool,
    include_directories: bool,
    include_ci: bool,
    include_security: bool,
    gitea_base_url: Option<&str>,
) -> RepoMapResponse {
    let host = request.host.unwrap_or(CodeHost::Unknown);
    let owner = request.owner.clone();
    let repo = request.repo.clone();
    let ref_name = request.ref_name.clone().or_else(|| {
        forge_response
            .resolved_ref
            .as_ref()
            .filter(|s| !s.chars().all(|c| c.is_ascii_hexdigit()))
            .cloned()
    });
    let commit_sha = forge_response.resolved_ref.clone();

    let max_d = max_depth(request);

    let mut entries: Vec<RepoMapEntry> = Vec::new();
    let mut root_entries: Vec<RepoMapEntry> = Vec::new();
    let mut important_files: Vec<RepoImportantFile> = Vec::new();
    let mut important_directories: Vec<RepoImportantDirectory> = Vec::new();
    let mut source_roots: Vec<RepoPathSummary> = Vec::new();
    let mut docs_dirs: Vec<RepoPathSummary> = Vec::new();
    let mut examples_dirs: Vec<RepoPathSummary> = Vec::new();
    let mut tests_dirs: Vec<RepoPathSummary> = Vec::new();
    let mut ci_dirs: Vec<RepoPathSummary> = Vec::new();
    let mut security_dir: Option<RepoPathSummary> = None;

    let _ref_str = ref_name.as_deref().unwrap_or("HEAD");

    for raw in &forge_response.entries {
        let depth = raw.path.matches('/').count() + 1;
        if depth > max_d {
            continue;
        }

        let kind = match raw.kind {
            EntryKind::File => RepoMapEntryKind::File,
            EntryKind::Directory => RepoMapEntryKind::Directory,
            EntryKind::Symlink => RepoMapEntryKind::Symlink,
            EntryKind::Submodule => RepoMapEntryKind::Submodule,
        };

        let include = (kind == RepoMapEntryKind::File && include_files)
            || (kind == RepoMapEntryKind::Directory && include_directories)
            || (kind == RepoMapEntryKind::Symlink && include_files)
            || (kind == RepoMapEntryKind::Submodule && include_directories);

        if include {
            let ref_str = ref_name.as_deref().unwrap_or("HEAD");
            let sha_str = raw.sha.as_deref();
            let (url, raw_url) = build_entry_urls(
                host,
                &owner,
                &repo,
                ref_str,
                sha_str,
                &raw.path,
                gitea_base_url,
            );
            let entry = RepoMapEntry {
                path: raw.path.clone(),
                kind,
                size: raw.size,
                language: language_from_extension(&raw.path).map(String::from),
                url,
                raw_url,
            };
            entries.push(entry.clone());
            if !raw.path.contains('/') {
                root_entries.push(entry);
            }
        }

        if kind == RepoMapEntryKind::File && include_files {
            let (file_kind, reasons) = classify_important_file(&raw.path);
            if file_kind != ImportantFileKind::Unknown && file_kind != ImportantFileKind::Ignored {
                important_files.push(RepoImportantFile {
                    path: raw.path.clone(),
                    kind: file_kind,
                    reasons,
                    size: raw.size,
                });
            }
        } else if kind == RepoMapEntryKind::Directory && include_directories {
            let (dir_kind, reasons) = classify_important_directory(&raw.path);
            if dir_kind != ImportantDirKind::Unknown {
                important_directories.push(RepoImportantDirectory {
                    path: raw.path.clone(),
                    kind: dir_kind,
                    reasons,
                    estimated_entry_count: None,
                });
                let summary = RepoPathSummary {
                    path: raw.path.clone(),
                    label: format!("{dir_kind:?}"),
                    entry_count: None,
                };
                let suppressed_ci = matches!(dir_kind, ImportantDirKind::CiConfig) && !include_ci;
                let suppressed_security =
                    matches!(dir_kind, ImportantDirKind::Security) && !include_security;
                if !suppressed_ci && !suppressed_security {
                    match dir_kind {
                        ImportantDirKind::SourceRoot => source_roots.push(summary),
                        ImportantDirKind::Docs => docs_dirs.push(summary),
                        ImportantDirKind::Examples => examples_dirs.push(summary),
                        ImportantDirKind::Tests => tests_dirs.push(summary),
                        ImportantDirKind::CiConfig => ci_dirs.push(summary),
                        ImportantDirKind::Security => security_dir = Some(summary),
                        _ => {}
                    }
                }
            }
        }
    }

    let mut structured_warnings: Vec<AgentWarning> = Vec::new();
    if forge_response.truncated_by_provider {
        structured_warnings.push(AgentWarning::new(
            WarningCode::ForgeTreeTruncated,
            "forge tree response was truncated by the provider",
        ));
    }

    let me = max_entries(request);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    important_files.sort_by(|a, b| a.path.cmp(&b.path));
    important_files.truncate(MAX_IMPORTANT_FILE_PROBES);
    important_directories.sort_by(|a, b| a.path.cmp(&b.path));
    important_directories.truncate(MAX_IMPORTANT_DIR_PROBES);

    if entries.len() > me {
        entries.truncate(me);
        structured_warnings.push(AgentWarning::new(
            WarningCode::ForgeTreeTruncated,
            "entry cap reached: response truncated to max_entries",
        ));
    }
    if root_entries.len() > me {
        root_entries.truncate(me);
    }

    let manifests: Vec<RepoImportantFile> = important_files
        .iter()
        .filter(|f| f.kind == ImportantFileKind::Manifest)
        .cloned()
        .collect();

    let warnings = forge_response.warnings;

    let mut response = RepoMapResponse {
        query: request.query.clone(),
        host,
        owner: owner.clone(),
        repo: repo.clone(),
        ref_name: ref_name.clone(),
        commit_sha,
        resolved_ref_name: ref_name.clone(),
        default_branch: forge_response.default_branch,
        mode: RepoMapMode::Native,
        root_entries,
        entries,
        important_files,
        important_directories,
        manifests,
        source_roots,
        docs: docs_dirs,
        examples: examples_dirs,
        tests: tests_dirs,
        ci: ci_dirs,
        security: security_dir,
        suggested_fetches: Vec::new(),
        providers_queried: vec![forge_response.provider_id.clone()],
        providers_failed: Vec::new(),
        warnings,
        structured_warnings,
        trust_markers: TrustMarkers::default(),
        local_checkout: None,
        telemetry: Some(RepoMapTelemetry {
            providers_queried: vec![forge_response.provider_id.clone()],
            deadline_exceeded: false,
            mode_reason: Some(format!("native tree from {}", forge_response.provider_id)),
        }),
        freshness_confidence: None,
    };

    response.suggested_fetches = build_repo_map_suggested_fetches(&response);

    response
}

/// Returns `true` if the host has a native tree API adapter.
pub fn is_supported_host(host: CodeHost) -> bool {
    matches!(
        host,
        CodeHost::Github
            | CodeHost::Gitlab
            | CodeHost::Codeberg
            | CodeHost::Gitea
            | CodeHost::Forgejo
    )
}

/// Return the provider ID for the native tree adapter, or `None` for unsupported hosts.
pub fn native_tree_provider_id(host: CodeHost) -> Option<&'static str> {
    match host {
        CodeHost::Github => Some("github_tree"),
        CodeHost::Gitlab => Some("gitlab_tree"),
        CodeHost::Codeberg => Some("codeberg_tree"),
        CodeHost::Gitea => Some("gitea_tree"),
        CodeHost::Forgejo => Some("forgejo_tree"),
        CodeHost::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_supported_host_github() {
        assert!(is_supported_host(CodeHost::Github));
    }

    #[test]
    fn is_supported_host_gitlab() {
        assert!(is_supported_host(CodeHost::Gitlab));
    }

    #[test]
    fn is_supported_host_codeberg() {
        assert!(is_supported_host(CodeHost::Codeberg));
    }

    #[test]
    fn is_supported_host_gitea() {
        assert!(is_supported_host(CodeHost::Gitea));
    }

    #[test]
    fn is_supported_host_forgejo() {
        assert!(is_supported_host(CodeHost::Forgejo));
    }

    #[test]
    fn is_supported_host_unknown() {
        assert!(!is_supported_host(CodeHost::Unknown));
    }

    #[test]
    fn native_tree_provider_id_github() {
        assert_eq!(
            native_tree_provider_id(CodeHost::Github),
            Some("github_tree")
        );
    }

    #[test]
    fn native_tree_provider_id_unknown() {
        assert_eq!(native_tree_provider_id(CodeHost::Unknown), None);
    }

    #[test]
    fn build_client_ok() {
        assert!(build_client().is_ok());
    }

    #[test]
    fn entry_kind_roundtrip() {
        let e1 = ForgeRawEntry {
            path: "src/main.rs".into(),
            kind: EntryKind::File,
            size: Some(1024),
            sha: Some("abc123".into()),
        };
        let e2 = ForgeRawEntry {
            path: "src".into(),
            kind: EntryKind::Directory,
            size: None,
            sha: Some("def456".into()),
        };
        assert_eq!(e1.kind, EntryKind::File);
        assert_eq!(e2.kind, EntryKind::Directory);
    }

    #[test]
    fn build_response_native_mode() {
        let req = RepoMapRequest {
            owner: "test".into(),
            repo: "repo".into(),
            host: Some(CodeHost::Github),
            ref_name: Some("main".into()),
            ..Default::default()
        };
        let forge = ForgeTreeResponse {
            entries: vec![ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(100),
                sha: Some("sha1".into()),
            }],
            default_branch: Some("main".into()),
            resolved_ref: Some("main".into()),
            truncated_by_provider: false,
            warnings: vec![],
            provider_id: "github_tree".into(),
        };
        let resp = build_response(&req, forge, true, true, true, true, None);
        assert!(matches!(resp.mode, RepoMapMode::Native));
        assert_eq!(resp.host, CodeHost::Github);
        assert_eq!(resp.root_entries.len(), 1);
        assert_eq!(resp.root_entries[0].path, "README.md");
        assert_eq!(resp.default_branch.as_deref(), Some("main"));
    }

    #[test]
    fn build_response_filters_by_depth() {
        let req = RepoMapRequest {
            owner: "test".into(),
            repo: "repo".into(),
            host: Some(CodeHost::Github),
            max_depth: Some(1),
            ..Default::default()
        };
        let forge = ForgeTreeResponse {
            entries: vec![
                ForgeRawEntry {
                    path: "src".into(),
                    kind: EntryKind::Directory,
                    size: None,
                    sha: None,
                },
                ForgeRawEntry {
                    path: "src/main.rs".into(),
                    kind: EntryKind::File,
                    size: Some(100),
                    sha: None,
                },
            ],
            default_branch: None,
            resolved_ref: Some("main".into()),
            truncated_by_provider: false,
            warnings: vec![],
            provider_id: "github_tree".into(),
        };
        let resp = build_response(&req, forge, true, true, true, true, None);
        assert_eq!(resp.root_entries.len(), 1);
        assert_eq!(resp.root_entries[0].path, "src");
    }

    #[test]
    fn build_response_respects_include_files() {
        let req = RepoMapRequest {
            owner: "test".into(),
            repo: "repo".into(),
            host: Some(CodeHost::Github),
            include_files: Some(false),
            ..Default::default()
        };
        let forge = ForgeTreeResponse {
            entries: vec![
                ForgeRawEntry {
                    path: "README.md".into(),
                    kind: EntryKind::File,
                    size: Some(100),
                    sha: None,
                },
                ForgeRawEntry {
                    path: "src".into(),
                    kind: EntryKind::Directory,
                    size: None,
                    sha: None,
                },
            ],
            default_branch: None,
            resolved_ref: Some("main".into()),
            truncated_by_provider: false,
            warnings: vec![],
            provider_id: "github_tree".into(),
        };
        let resp = build_response(&req, forge, false, true, true, true, None);
        let files: Vec<_> = resp
            .root_entries
            .iter()
            .filter(|e| e.kind == RepoMapEntryKind::File)
            .collect();
        assert!(files.is_empty());
    }

    #[test]
    fn build_response_truncated_warning() {
        let req = RepoMapRequest {
            owner: "test".into(),
            repo: "repo".into(),
            host: Some(CodeHost::Github),
            ..Default::default()
        };
        let forge = ForgeTreeResponse {
            entries: vec![],
            default_branch: None,
            resolved_ref: None,
            truncated_by_provider: true,
            warnings: vec![SearchWarning::new("github_tree", "truncated")],
            provider_id: "github_tree".into(),
        };
        let resp = build_response(&req, forge, true, true, true, true, None);
        assert!(!resp.warnings.is_empty());
    }
}
