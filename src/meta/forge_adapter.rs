//! Native remote repository tree adapter for code hosts.
//!
//! Provides bounded, provider-neutral tree retrieval for GitHub, GitLab,
//! Gitea, Forgejo, and Codeberg without cloning repositories. All tree
//! operations enforce entry, depth, byte, pagination, concurrency, and
//! timeout limits.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::redirect::Policy;
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

/// Policy controlling which forge endpoint addresses and schemes are permitted.
#[derive(Debug, Clone)]
pub struct ForgeEndpointPolicy {
    /// Whether to allow loopback addresses (localhost, 127.0.0.1, ::1).
    pub allow_loopback: bool,
    /// Whether to allow private network addresses (RFC 1918, ULA, etc.).
    pub allow_private_network: bool,
    /// Whether to require HTTPS for all endpoints.
    pub require_https: bool,
}

impl Default for ForgeEndpointPolicy {
    fn default() -> Self {
        Self {
            allow_loopback: false,
            allow_private_network: false,
            require_https: true,
        }
    }
}

/// Identifies the type of forge HTTP request for budget tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeRequestKind {
    /// Commit SHA resolution request.
    CommitResolution,
    /// Tree page retrieval request.
    TreePage,
    /// Contents API fallback request.
    ContentsFallback,
    /// Repository metadata request (default branch, project info).
    RepositoryMetadata,
}

/// Error type for forge bounded-read operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForgeReadError {
    /// A single response exceeded the per-response byte limit.
    PerResponseLimitExceeded,
    /// The aggregate operation budget was exhausted.
    AggregateBudgetExhausted,
    /// The declared Content-Length exceeded the effective byte cap.
    ContentLengthTooLarge,
    /// Reading a response stream chunk failed.
    StreamReadFailure,
}

impl std::fmt::Display for ForgeReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PerResponseLimitExceeded => write!(f, "response_too_large"),
            Self::AggregateBudgetExhausted => write!(f, "aggregate_budget_exhausted"),
            Self::ContentLengthTooLarge => write!(f, "response_too_large"),
            Self::StreamReadFailure => write!(f, "stream_read_failure"),
        }
    }
}

impl ForgeReadError {
    /// Return a stable static string representation of the error variant.
    pub fn as_static_str(&self) -> &'static str {
        match self {
            Self::PerResponseLimitExceeded => "response_too_large",
            Self::AggregateBudgetExhausted => "aggregate_budget_exhausted",
            Self::ContentLengthTooLarge => "response_too_large",
            Self::StreamReadFailure => "stream_read_failure",
        }
    }
}

pub(crate) struct ForgeReadBudgetTelemetry {
    pub aggregate_limit: usize,
    pub aggregate_observed: usize,
    pub remaining: usize,
    pub request_count: usize,
    pub exhausted_by: Option<ForgeRequestKind>,
    pub per_response_cap_hits: usize,
}

pub(crate) struct ForgeReadBudget {
    pub per_response_limit: usize,
    pub aggregate_limit: usize,
    pub aggregate_observed: usize,
    pub exhausted: bool,
    pub exhausted_by: Option<ForgeRequestKind>,
    pub request_count: usize,
    pub per_response_cap_hits: usize,
}

impl ForgeReadBudget {
    pub fn new(aggregate_limit: usize) -> Self {
        Self {
            per_response_limit: DEFAULT_MAX_RESPONSE_BYTES,
            aggregate_limit,
            aggregate_observed: 0,
            exhausted: false,
            exhausted_by: None,
            request_count: 0,
            per_response_cap_hits: 0,
        }
    }

    pub fn remaining(&self) -> usize {
        self.aggregate_limit.saturating_sub(self.aggregate_observed)
    }

    pub fn consume(&mut self, bytes: usize, kind: ForgeRequestKind) {
        self.aggregate_observed = self.aggregate_observed.saturating_add(bytes);
        self.request_count += 1;
        if self.aggregate_observed >= self.aggregate_limit && !self.exhausted {
            self.exhausted = true;
            self.exhausted_by = Some(kind);
        }
    }

    pub fn exceeded(&self) -> bool {
        self.exhausted
    }

    pub fn telemetry(&self) -> ForgeReadBudgetTelemetry {
        ForgeReadBudgetTelemetry {
            aggregate_limit: self.aggregate_limit,
            aggregate_observed: self.aggregate_observed,
            remaining: self.remaining(),
            request_count: self.request_count,
            exhausted_by: self.exhausted_by,
            per_response_cap_hits: self.per_response_cap_hits,
        }
    }
}

pub(crate) async fn read_with_budget(
    resp: reqwest::Response,
    budget: &mut ForgeReadBudget,
    kind: ForgeRequestKind,
) -> Result<Vec<u8>, ForgeReadError> {
    let effective_cap = budget.per_response_limit.min(budget.remaining());
    if effective_cap == 0 {
        return Err(ForgeReadError::AggregateBudgetExhausted);
    }
    if let Some(content_length) = resp.content_length() {
        if content_length > effective_cap as u64 {
            if content_length > budget.per_response_limit as u64 {
                budget.per_response_cap_hits += 1;
                return Err(ForgeReadError::ContentLengthTooLarge);
            }
            return Err(ForgeReadError::AggregateBudgetExhausted);
        }
    }
    let mut body = Vec::with_capacity(effective_cap.min(64 * 1024));
    let mut observed = 0usize;
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ForgeReadError::StreamReadFailure)?;
        observed += chunk.len();
        if observed > effective_cap {
            if observed > budget.per_response_limit {
                budget.per_response_cap_hits += 1;
                return Err(ForgeReadError::PerResponseLimitExceeded);
            }
            return Err(ForgeReadError::AggregateBudgetExhausted);
        }
        body.extend_from_slice(&chunk);
    }
    budget.consume(observed, kind);
    Ok(body)
}

const ERROR_BODY_CAP: usize = 8 * 1024;

/// Read a preview of an error response body, capped at 8KB.
///
/// Strips control characters (except newline, carriage return, tab)
/// and truncates at the byte cap.
pub async fn read_error_body_preview(resp: reqwest::Response) -> String {
    let mut body = Vec::with_capacity(ERROR_BODY_CAP.min(8192));
    let mut stream = resp.bytes_stream();
    let mut observed = 0usize;
    let mut stream_read_failed = false;
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => {
                observed += chunk.len();
                if observed > ERROR_BODY_CAP {
                    break;
                }
                body.extend_from_slice(&chunk);
            }
            Err(error) => {
                tracing::warn!(error = ?error, "failed to read forge error response body");
                stream_read_failed = true;
                break;
            }
        }
    }
    if stream_read_failed {
        const STREAM_FAILURE_MARKER: &[u8] = b"[error reading response body]";
        let keep = ERROR_BODY_CAP.saturating_sub(STREAM_FAILURE_MARKER.len());
        body.truncate(keep);
        body.extend_from_slice(STREAM_FAILURE_MARKER);
    }
    String::from_utf8_lossy(&body)
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect()
}

/// Configuration for connecting to a forge API.
#[derive(Debug, Clone, Default)]
pub struct ForgeTreeConfig {
    /// Optional API key for authenticated requests.
    pub api_key: Option<String>,
    /// Optional base URL override for the API endpoint.
    pub base_url: Option<String>,
    /// Endpoint policy controlling allowed addresses and schemes.
    pub endpoint_policy: ForgeEndpointPolicy,
    /// Optional override for the aggregate byte budget limit.
    /// When `None`, uses `DEFAULT_MAX_RESPONSE_BYTES`.
    pub forge_budget_limit: Option<usize>,
}

/// Resolved repository identity after fetching a tree.
///
/// Separates the caller-supplied ref from provider-resolved commit and
/// tree SHAs. This prevents treating tree or blob object SHAs as commit
/// SHAs in permalink construction.
#[derive(Debug, Clone, Default)]
pub struct ResolvedRepositoryIdentity {
    /// The caller-supplied branch, tag, commit, or symbolic ref.
    pub requested_ref: Option<String>,
    /// The resolved ref name (branch or tag) used by the provider, when known.
    pub resolved_ref_name: Option<String>,
    /// The actual commit SHA resolved by the provider.
    /// For GitHub, this comes from a separate commit/ref resolution endpoint.
    /// For GitLab, this comes from the repository commit endpoint.
    /// For Gitea/Forgejo/Codeberg, this comes from the ref resolution endpoint.
    /// Must never contain a tree SHA, blob SHA, or branch name.
    pub resolved_commit_sha: Option<String>,
    /// The root tree SHA associated with the resolved commit, when available.
    pub tree_sha: Option<String>,
    /// The repository's default branch, if determined.
    pub default_branch: Option<String>,
}

/// Response from a forge tree API call, containing raw entries and metadata.
#[derive(Debug)]
pub struct ForgeTreeResponse {
    /// Raw tree entries from the API.
    pub entries: Vec<ForgeRawEntry>,
    /// Resolved repository identity with separated commit/tree/object SHAs.
    pub identity: ResolvedRepositoryIdentity,
    /// Whether the provider reported a truncated response.
    pub truncated_by_provider: bool,
    /// Warnings accumulated during the fetch.
    pub warnings: Vec<SearchWarning>,
    /// The provider ID that served this response.
    pub provider_id: String,
    /// Endpoint origin used for forge requests.
    pub endpoint_origin: Option<String>,
    /// Total response bytes observed across all forge pages.
    pub response_bytes_observed: usize,
    /// Whether any response hit the per-response byte cap.
    pub response_cap_applied: bool,
    /// DNS policy classification of the endpoint.
    pub dns_policy_class: Option<String>,
    /// Whether the aggregate byte budget was reached.
    pub aggregate_byte_cap_reached: bool,
    /// The configured aggregate byte limit for this operation.
    pub aggregate_limit: usize,
    /// Remaining aggregate budget after the operation.
    pub aggregate_remaining: usize,
    /// Number of HTTP requests made during this operation.
    pub request_count: usize,
    /// The request kind that exhausted the budget, if any.
    pub exhausted_by: Option<ForgeRequestKind>,
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
    /// The blob, tree, or submodule object SHA for this specific entry.
    /// This is NOT the commit SHA; it identifies the individual object
    /// within the tree. Use `ResolvedRepositoryIdentity.resolved_commit_sha`
    /// for permalink construction.
    pub object_sha: Option<String>,
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
        .redirect(Policy::none())
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
        validate_base_url_async(base, config.api_key.as_deref(), &config.endpoint_policy).await?;
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
            let base = config.base_url.as_deref().ok_or_else(|| {
                let host_label = match host {
                    CodeHost::Gitea => "gitea",
                    CodeHost::Forgejo => "forgejo",
                    _ => unreachable!(),
                };
                format!(
                    "{host_label} host requires an explicit base_url; \
                     set [forge].{host_label}.base_url in config"
                )
            })?;
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
    let max_e = max_entries(req);
    let aggregate_limit = config
        .forge_budget_limit
        .unwrap_or(DEFAULT_MAX_RESPONSE_BYTES);
    let mut budget = ForgeReadBudget::new(aggregate_limit);

    let mut identity = ResolvedRepositoryIdentity {
        requested_ref: Some(ref_name.to_string()),
        ..Default::default()
    };

    let (commit_sha, tree_sha) =
        resolve_github_commit(client, owner, repo, ref_name, config, timeout, &mut budget).await;

    identity.resolved_commit_sha = commit_sha.clone();
    identity.tree_sha = tree_sha.clone();

    let default_branch =
        resolve_github_default_branch(client, owner, repo, config, timeout, &mut budget).await;
    identity.default_branch = default_branch.clone();

    let tree_ref = tree_sha.as_deref().unwrap_or(ref_name);

    let mut builder = client
        .get(format!(
            "{base}/repos/{}/{}/git/trees/{}",
            encode_url_component(owner),
            encode_url_component(repo),
            encode_url_component(tree_ref)
        ))
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
        let msg = read_error_body_preview(resp).await;
        if msg.contains("rate limit") || msg.contains("Rate limit") {
            return Err("rate_limited".into());
        }
        return Err("permission_denied".into());
    }
    if !status.is_success() {
        let msg = read_error_body_preview(resp).await;
        return Err(format!("provider_unavailable: {status} - {msg}"));
    }

    let body = read_with_budget(resp, &mut budget, ForgeRequestKind::TreePage)
        .await
        .map_err(|e| e.as_static_str().to_string())?;

    let body_str = std::str::from_utf8(&body)
        .map(|s| s.to_owned())
        .map_err(|_| "invalid_utf8".to_string())?;

    let tree: GitHubTreeResponse =
        serde_json::from_str(&body_str).map_err(|e| format!("malformed response: {e}"))?;

    let truncated_by_provider = tree.truncated.unwrap_or(false);

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
                object_sha: item.sha,
            }
        })
        .collect();

    if truncated_by_provider && !budget.exceeded() {
        if let Ok(fallback) =
            fetch_github_contents_root(client, owner, repo, config, timeout, tree_ref, &mut budget)
                .await
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

    let mut truncated_by_eggsearch = false;
    if entries.len() > max_e {
        entries.truncate(max_e);
        truncated_by_eggsearch = true;
    }

    let mut warnings = Vec::new();
    if truncated_by_eggsearch {
        warnings.push(SearchWarning::new(
            "github_tree",
            "response_truncated_by_eggsearch: entry limit reached",
        ));
    }
    if truncated_by_provider {
        warnings.push(SearchWarning::new(
            "github_tree",
            "response_truncated_by_provider: GitHub tree response was truncated; \
             results may be incomplete",
        ));
    }
    if commit_sha.is_none() {
        identity.resolved_ref_name = Some(ref_name.to_string());
        warnings.push(SearchWarning::new(
            "github_tree",
            "commit_resolution_unavailable: could not resolve ref to commit SHA; \
             URLs will use mutable ref instead of immutable commit",
        ));
    } else {
        identity.resolved_ref_name = Some(ref_name.to_string());
    }
    if budget.exceeded() {
        warnings.push(SearchWarning::new(
            "github_tree",
            "aggregate_budget_exhausted: aggregate byte budget reached",
        ));
    }

    let telemetry = budget.telemetry();

    Ok(ForgeTreeResponse {
        entries,
        identity,
        truncated_by_provider,
        warnings,
        provider_id: "github_tree".to_string(),
        endpoint_origin: extract_host(base),
        response_bytes_observed: telemetry.aggregate_observed,
        response_cap_applied: telemetry.per_response_cap_hits > 0,
        dns_policy_class: classify_host_from_url(base).await,
        aggregate_byte_cap_reached: budget.exceeded(),
        aggregate_limit: telemetry.aggregate_limit,
        aggregate_remaining: telemetry.remaining,
        request_count: telemetry.request_count,
        exhausted_by: telemetry.exhausted_by,
    })
}

async fn resolve_github_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    budget: &mut ForgeReadBudget,
) -> Option<String> {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let mut builder = client
        .get(format!(
            "{base}/repos/{}/{}",
            encode_url_component(owner),
            encode_url_component(repo)
        ))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", format!("Bearer {key}"));
    }
    let resp = builder.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = read_with_budget(resp, budget, ForgeRequestKind::RepositoryMetadata)
        .await
        .ok()?;
    let body_str = std::str::from_utf8(&body).ok()?;
    let repo_info: GitHubRepoInfo = serde_json::from_str(body_str).ok()?;
    Some(repo_info.default_branch)
}

/// Resolve a GitHub ref to a commit SHA and tree SHA.
///
/// Uses `GET /repos/{owner}/{repo}/commits/{ref}` to obtain the commit
/// SHA and the root tree SHA for the given ref. Returns
/// `(commit_sha, tree_sha)` where either may be `None` if resolution
/// fails.
async fn resolve_github_commit(
    client: &Client,
    owner: &str,
    repo: &str,
    ref_name: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    budget: &mut ForgeReadBudget,
) -> (Option<String>, Option<String>) {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let mut builder = client
        .get(format!(
            "{base}/repos/{}/{}/commits/{}",
            encode_url_component(owner),
            encode_url_component(repo),
            encode_url_component(ref_name)
        ))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", format!("Bearer {key}"));
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(_) => return (None, None),
    };
    if !resp.status().is_success() {
        return (None, None);
    }
    let body = match read_with_budget(resp, budget, ForgeRequestKind::CommitResolution).await {
        Ok(b) => b,
        Err(_) => return (None, None),
    };
    let body_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return (None, None),
    };
    let commit: GitHubCommitInfo = match serde_json::from_str(body_str) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let commit_sha = Some(commit.sha);
    let tree_sha = Some(commit.commit_info.tree.sha);
    (commit_sha, tree_sha)
}

async fn fetch_github_contents_root(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    tree_ref: &str,
    budget: &mut ForgeReadBudget,
) -> Result<Vec<ForgeRawEntry>, String> {
    let base = config.base_url.as_deref().unwrap_or(GITHUB_API_BASE);
    let mut builder = client
        .get(format!(
            "{base}/repos/{}/{}/contents/",
            encode_url_component(owner),
            encode_url_component(repo)
        ))
        .query(&[("ref", tree_ref)])
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
    let body = read_with_budget(resp, budget, ForgeRequestKind::ContentsFallback)
        .await
        .map_err(|e| e.as_static_str().to_string())?;
    let body_str = std::str::from_utf8(&body).map_err(|_| "invalid_utf8".to_string())?;
    let items: Vec<GitHubContentsEntry> =
        serde_json::from_str(body_str).map_err(|e| format!("malformed Contents response: {e}"))?;
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
                object_sha: item.sha,
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
struct GitHubCommitInfo {
    sha: String,
    #[serde(rename = "commit")]
    commit_info: GitHubCommitObject,
}

#[derive(Deserialize)]
struct GitHubCommitObject {
    tree: GitHubTreeRef,
}

#[derive(Deserialize)]
struct GitHubTreeRef {
    sha: String,
}

#[derive(Deserialize)]
struct GitHubTreeResponse {
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

    let mut identity = ResolvedRepositoryIdentity {
        requested_ref: Some(ref_name.to_string()),
        resolved_ref_name: Some(ref_name.to_string()),
        ..Default::default()
    };

    let mut budget = ForgeReadBudget::new(
        config
            .forge_budget_limit
            .unwrap_or(DEFAULT_MAX_RESPONSE_BYTES),
    );

    let (commit_sha, tree_sha) =
        resolve_gitlab_commit(client, owner, repo, ref_name, config, timeout, &mut budget).await;
    identity.resolved_commit_sha = commit_sha;
    identity.tree_sha = tree_sha;

    let default_branch =
        resolve_gitlab_default_branch(client, owner, repo, config, timeout, &mut budget).await;
    identity.default_branch = default_branch;

    let mut all_entries: Vec<ForgeRawEntry> = Vec::new();
    let mut page = 1u32;
    let mut truncated_by_provider = false;
    let mut warnings = Vec::new();
    let max_pages = DEFAULT_MAX_PAGES;

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
        if budget.exceeded() {
            warnings.push(SearchWarning::new(
                "gitlab_tree",
                "aggregate_budget_exhausted: aggregate byte budget reached",
            ));
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
            if all_entries.is_empty() {
                return Err("rate_limited".into());
            }
            warnings.push(SearchWarning::new(
                "gitlab_tree",
                "rate_limited_partial: rate limited mid-pagination; returning partial results",
            ));
            truncated_by_provider = true;
            break;
        }
        if !status.is_success() {
            let msg = read_error_body_preview(resp).await;
            return Err(format!("provider_unavailable: {status} - {msg}"));
        }

        let body = read_with_budget(resp, &mut budget, ForgeRequestKind::TreePage)
            .await
            .map_err(|e| e.as_static_str().to_string())?;

        let body_str = std::str::from_utf8(&body).map_err(|_| "invalid_utf8".to_string())?;

        let items: Vec<GitLabTreeEntry> =
            serde_json::from_str(body_str).map_err(|e| format!("malformed response: {e}"))?;

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
                object_sha: item.id,
            });
        }

        if page_len < per_page {
            break;
        }
        page += 1;
    }

    if all_entries.len() >= max_e {
        warnings.push(SearchWarning::new(
            "gitlab_tree",
            "response_truncated_by_eggsearch: entry limit reached",
        ));
        all_entries.truncate(max_e);
    }

    let telemetry = budget.telemetry();

    Ok(ForgeTreeResponse {
        entries: all_entries,
        identity,
        truncated_by_provider,
        warnings,
        provider_id: "gitlab_tree".to_string(),
        endpoint_origin: extract_host(config.base_url.as_deref().unwrap_or(GITLAB_API_BASE)),
        response_bytes_observed: telemetry.aggregate_observed,
        response_cap_applied: telemetry.per_response_cap_hits > 0,
        dns_policy_class: classify_host_from_url(
            config.base_url.as_deref().unwrap_or(GITLAB_API_BASE),
        )
        .await,
        aggregate_byte_cap_reached: budget.exceeded(),
        aggregate_limit: telemetry.aggregate_limit,
        aggregate_remaining: telemetry.remaining,
        request_count: telemetry.request_count,
        exhausted_by: telemetry.exhausted_by,
    })
}

async fn resolve_gitlab_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    budget: &mut ForgeReadBudget,
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
    let body = read_with_budget(resp, budget, ForgeRequestKind::RepositoryMetadata)
        .await
        .ok()?;
    let body_str = std::str::from_utf8(&body).ok()?;
    let info: GitLabProjectInfo = serde_json::from_str(body_str).ok()?;
    Some(info.default_branch)
}

#[derive(Deserialize)]
struct GitLabProjectInfo {
    default_branch: String,
}

/// Resolve a GitLab ref to a commit SHA and tree SHA.
///
/// Uses `GET /projects/:id/repository/commits/:sha` to obtain the commit
/// SHA and root tree SHA for the given ref. Returns
/// `(commit_sha, tree_sha)` where either may be `None` if resolution
/// fails.
async fn resolve_gitlab_commit(
    client: &Client,
    owner: &str,
    repo: &str,
    ref_name: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    budget: &mut ForgeReadBudget,
) -> (Option<String>, Option<String>) {
    let base = config.base_url.as_deref().unwrap_or(GITLAB_API_BASE);
    let project_path_raw = format!("{owner}/{repo}");
    let project_path = urlencoding::encode(&project_path_raw);
    let encoded_ref = encode_url_component(ref_name);
    let mut builder = client
        .get(format!(
            "{base}/projects/{project_path}/repository/commits/{encoded_ref}"
        ))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("PRIVATE-TOKEN", key.as_str());
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(_) => return (None, None),
    };
    if !resp.status().is_success() {
        return (None, None);
    }
    let body = match read_with_budget(resp, budget, ForgeRequestKind::CommitResolution).await {
        Ok(b) => b,
        Err(_) => return (None, None),
    };
    let body_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return (None, None),
    };
    let commit: GitLabCommitInfo = match serde_json::from_str(body_str) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let commit_sha = Some(commit.id);
    let tree_sha = commit.tree_id;
    (commit_sha, tree_sha)
}

#[derive(Deserialize)]
struct GitLabCommitInfo {
    id: String,
    tree_id: Option<String>,
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

    let mut identity = ResolvedRepositoryIdentity {
        requested_ref: Some(ref_name.to_string()),
        resolved_ref_name: Some(ref_name.to_string()),
        ..Default::default()
    };

    let mut budget = ForgeReadBudget::new(
        config
            .forge_budget_limit
            .unwrap_or(DEFAULT_MAX_RESPONSE_BYTES),
    );

    let (commit_sha, tree_sha) = resolve_forge_commit(
        client,
        owner,
        repo,
        ref_name,
        config,
        timeout,
        api_base,
        &mut budget,
    )
    .await;
    identity.resolved_commit_sha = commit_sha;
    identity.tree_sha = tree_sha;

    let tree_ref = identity
        .tree_sha
        .as_deref()
        .or(identity.resolved_commit_sha.as_deref())
        .unwrap_or(ref_name);

    let default_branch =
        resolve_forge_default_branch(client, owner, repo, config, timeout, api_base, &mut budget)
            .await;
    identity.default_branch = default_branch;

    let mut all_entries: Vec<ForgeRawEntry> = Vec::new();
    let mut page = 1u32;
    let mut truncated_by_provider = false;
    let mut warnings = Vec::new();
    let max_pages = DEFAULT_MAX_PAGES;

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
        if budget.exceeded() {
            warnings.push(SearchWarning::new(
                provider_id,
                "aggregate_budget_exhausted: aggregate byte budget reached",
            ));
            break;
        }

        let mut builder = client
            .get(format!(
                "{api_base}/repos/{}/{}/git/trees/{}",
                encode_url_component(owner),
                encode_url_component(repo),
                encode_url_component(tree_ref)
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
            if all_entries.is_empty() {
                return Err("rate_limited".into());
            }
            warnings.push(SearchWarning::new(
                provider_id,
                "rate_limited_partial: rate limited mid-pagination; returning partial results",
            ));
            truncated_by_provider = true;
            break;
        }
        if !status.is_success() {
            let msg = read_error_body_preview(resp).await;
            return Err(format!("provider_unavailable: {status} - {msg}"));
        }

        let body = read_with_budget(resp, &mut budget, ForgeRequestKind::TreePage)
            .await
            .map_err(|e| e.as_static_str().to_string())?;

        let body_str = std::str::from_utf8(&body).map_err(|_| "invalid_utf8".to_string())?;

        let tree: ForgeTreeApiResponse =
            serde_json::from_str(body_str).map_err(|e| format!("malformed response: {e}"))?;

        truncated_by_provider |= tree.truncated.unwrap_or(false);

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
                object_sha: item.sha,
            });
        }

        if page_len < per_page {
            break;
        }
        page += 1;
    }

    if all_entries.len() >= max_e {
        warnings.push(SearchWarning::new(
            provider_id,
            "response_truncated_by_eggsearch: entry limit reached",
        ));
        all_entries.truncate(max_e);
    }

    if truncated_by_provider {
        warnings.push(SearchWarning::new(
            provider_id,
            "response_truncated_by_provider: forge tree response was truncated",
        ));
    }

    if identity.resolved_commit_sha.is_none() {
        warnings.push(SearchWarning::new(
            provider_id,
            "commit_resolution_unavailable: could not resolve ref to commit SHA; \
             URLs will use mutable ref instead of immutable commit",
        ));
    }
    if budget.exceeded() {
        warnings.push(SearchWarning::new(
            provider_id,
            "aggregate_budget_exhausted: aggregate byte budget reached",
        ));
    }

    let telemetry = budget.telemetry();

    Ok(ForgeTreeResponse {
        entries: all_entries,
        identity,
        truncated_by_provider,
        warnings,
        provider_id: provider_id.to_string(),
        endpoint_origin: extract_host(api_base),
        response_bytes_observed: telemetry.aggregate_observed,
        response_cap_applied: telemetry.per_response_cap_hits > 0,
        dns_policy_class: classify_host_from_url(api_base).await,
        aggregate_byte_cap_reached: budget.exceeded(),
        aggregate_limit: telemetry.aggregate_limit,
        aggregate_remaining: telemetry.remaining,
        request_count: telemetry.request_count,
        exhausted_by: telemetry.exhausted_by,
    })
}

async fn resolve_forge_default_branch(
    client: &Client,
    owner: &str,
    repo: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    api_base: &str,
    budget: &mut ForgeReadBudget,
) -> Option<String> {
    let mut builder = client
        .get(format!(
            "{api_base}/repos/{}/{}",
            encode_url_component(owner),
            encode_url_component(repo)
        ))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", format!("token {key}"));
    }
    let resp = builder.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = read_with_budget(resp, budget, ForgeRequestKind::RepositoryMetadata)
        .await
        .ok()?;
    let body_str = std::str::from_utf8(&body).ok()?;
    let info: ForgeRepoInfo = serde_json::from_str(body_str).ok()?;
    Some(info.default_branch)
}

/// Resolve a forge ref (Gitea/Forgejo/Codeberg) to a commit SHA.
///
/// Uses `GET /repos/{owner}/{repo}/commits/{ref}` to obtain the commit
/// SHA. The tree SHA is not directly available from this endpoint for
/// all providers, so it may be `None`. Returns `(commit_sha, tree_sha)`
/// where either may be `None` if resolution fails.
#[allow(clippy::too_many_arguments)]
async fn resolve_forge_commit(
    client: &Client,
    owner: &str,
    repo: &str,
    ref_name: &str,
    config: &ForgeTreeConfig,
    timeout: Duration,
    api_base: &str,
    budget: &mut ForgeReadBudget,
) -> (Option<String>, Option<String>) {
    let mut builder = client
        .get(format!(
            "{api_base}/repos/{}/{}/commits/{}",
            encode_url_component(owner),
            encode_url_component(repo),
            encode_url_component(ref_name)
        ))
        .timeout(timeout);
    if let Some(ref key) = config.api_key {
        builder = builder.header("Authorization", format!("token {key}"));
    }
    let resp = match builder.send().await {
        Ok(r) => r,
        Err(_) => return (None, None),
    };
    if !resp.status().is_success() {
        return (None, None);
    }
    let body = match read_with_budget(resp, budget, ForgeRequestKind::CommitResolution).await {
        Ok(b) => b,
        Err(_) => return (None, None),
    };
    let body_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return (None, None),
    };
    let commit: ForgeCommitInfo = match serde_json::from_str(body_str) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let commit_sha = Some(commit.sha);
    (commit_sha, None)
}

#[derive(Deserialize)]
struct ForgeCommitInfo {
    sha: String,
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
/// For GitHub, immutable permalinks use `commit_sha` (the resolved commit
/// SHA), not the entry's `object_sha` (blob/tree SHA). For Gitea/Forgejo,
/// `gitea_base_url` should be the instance root URL
/// (e.g. `https://gitea.example.com`), not the API base.
///
/// Directory entries do not receive raw-file URLs.
#[allow(clippy::too_many_arguments)]
fn build_entry_urls(
    host: CodeHost,
    owner: &str,
    repo: &str,
    ref_name: &str,
    commit_sha: Option<&str>,
    object_sha: Option<&str>,
    path: &str,
    kind: EntryKind,
    gitea_base_url: Option<&str>,
) -> (Option<String>, Option<String>) {
    if kind == EntryKind::Directory {
        let browser = match host {
            CodeHost::Github => {
                let r = commit_sha.unwrap_or(ref_name);
                github_browser_url(owner, repo, r, path)
            }
            CodeHost::Gitlab => {
                let r = commit_sha.unwrap_or(ref_name);
                gitlab_browser_url(owner, repo, r, path)
            }
            CodeHost::Codeberg => {
                let r = commit_sha.unwrap_or(ref_name);
                codeberg_browser_url(owner, repo, r, path)
            }
            CodeHost::Gitea | CodeHost::Forgejo => {
                let r = commit_sha.unwrap_or(ref_name);
                if let Some(base) = gitea_base_url {
                    gitea_browser_url(base, owner, repo, r, path)
                } else {
                    String::new()
                }
            }
            CodeHost::Unknown => String::new(),
        };
        let browser_opt = if browser.is_empty() {
            None
        } else {
            Some(browser)
        };
        return (browser_opt, None);
    }

    let (browser, raw) = match host {
        CodeHost::Github => {
            if let Some(sha) = commit_sha {
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
        CodeHost::Gitlab => {
            if let Some(sha) = commit_sha {
                (
                    gitlab_browser_url(owner, repo, sha, path),
                    gitlab_raw_url(owner, repo, sha, path),
                )
            } else {
                (
                    gitlab_browser_url(owner, repo, ref_name, path),
                    gitlab_raw_url(owner, repo, ref_name, path),
                )
            }
        }
        CodeHost::Codeberg => {
            if let Some(sha) = commit_sha {
                (
                    codeberg_browser_url(owner, repo, sha, path),
                    codeberg_raw_url(owner, repo, sha, path),
                )
            } else {
                (
                    codeberg_browser_url(owner, repo, ref_name, path),
                    codeberg_raw_url(owner, repo, ref_name, path),
                )
            }
        }
        CodeHost::Gitea | CodeHost::Forgejo => {
            let ref_or_commit = commit_sha.unwrap_or(ref_name);
            if let Some(base) = gitea_base_url {
                (
                    gitea_browser_url(base, owner, repo, ref_or_commit, path),
                    gitea_raw_url(base, owner, repo, ref_or_commit, path),
                )
            } else {
                (String::new(), String::new())
            }
        }
        CodeHost::Unknown => (String::new(), String::new()),
    };
    let _ = object_sha;
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
pub fn validate_base_url(
    url: &str,
    api_key: Option<&str>,
    policy: &ForgeEndpointPolicy,
) -> Result<(), String> {
    let host_to_resolve = validate_base_url_common(url, api_key, policy)?;
    if let Some(host) = host_to_resolve {
        let addrs = std::net::ToSocketAddrs::to_socket_addrs(&host)
            .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?;
        validate_resolved_addresses(addrs, policy)?;
    }
    Ok(())
}

async fn validate_base_url_async(
    url: &str,
    api_key: Option<&str>,
    policy: &ForgeEndpointPolicy,
) -> Result<(), String> {
    let host_to_resolve = validate_base_url_common(url, api_key, policy)?;
    if let Some(host) = host_to_resolve {
        let addrs = tokio::net::lookup_host(&host)
            .await
            .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?;
        validate_resolved_addresses(addrs, policy)?;
    }
    Ok(())
}

fn validate_base_url_common(
    url: &str,
    api_key: Option<&str>,
    policy: &ForgeEndpointPolicy,
) -> Result<Option<String>, String> {
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
    if let Some(host) = parsed.host_str() {
        let is_loopback = is_loopback_addr(host);

        if parsed.scheme() == "http" {
            if !is_loopback {
                if api_key.is_some() {
                    return Err("credential-bearing endpoint must use HTTPS".into());
                }
                if policy.require_https {
                    return Err("base URL must use HTTPS per policy".into());
                }
            }
        } else {
            if is_loopback && !policy.allow_loopback {
                return Err(format!(
                    "HTTPS base URL must not point to localhost: {host}"
                ));
            }
            if !is_loopback {
                if let Some(ip) = parse_literal_ip(host) {
                    classify_and_reject_address(ip, policy)?;
                } else {
                    return Ok(Some(format!("{host}:443")));
                }
            }
        }
    }
    Ok(None)
}

fn validate_resolved_addresses(
    addrs: impl IntoIterator<Item = std::net::SocketAddr>,
    policy: &ForgeEndpointPolicy,
) -> Result<(), String> {
    for addr in addrs {
        match addr {
            std::net::SocketAddr::V4(v4) => {
                classify_and_reject_address(IpAddr::V4(*v4.ip()), policy)?;
            }
            std::net::SocketAddr::V6(v6) => {
                classify_and_reject_address(IpAddr::V6(*v6.ip()), policy)?;
            }
        }
    }
    Ok(())
}

use std::net::IpAddr;

fn parse_literal_ip(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Some(IpAddr::V4(ip));
    }
    let inner = if host.starts_with('[') && host.ends_with(']') {
        &host[1..host.len() - 1]
    } else {
        host
    };
    if let Ok(ip) = inner.parse::<Ipv6Addr>() {
        return Some(IpAddr::V6(ip));
    }
    None
}

fn classify_and_reject_address(ip: IpAddr, policy: &ForgeEndpointPolicy) -> Result<(), String> {
    let class = match ip {
        IpAddr::V4(v4) => classify_ipv4_forge(v4),
        IpAddr::V6(v6) => classify_ipv6_forge(v6),
    };
    match class {
        ForgeAddressClass::Loopback if !policy.allow_loopback => Err(format!(
            "resolved address {ip} is loopback, rejected by policy"
        )),
        ForgeAddressClass::Private | ForgeAddressClass::LinkLocal
            if !policy.allow_private_network =>
        {
            Err(format!(
                "resolved address {ip} is private/link-local, rejected by policy"
            ))
        }
        ForgeAddressClass::Documentation | ForgeAddressClass::Reserved => Err(format!(
            "resolved address {ip} is reserved/documentation, rejected"
        )),
        _ => Ok(()),
    }
}

/// Classification of an IP address for forge endpoint safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeAddressClass {
    /// Loopback address (127.0.0.0/8, ::1).
    Loopback,
    /// Private network address (RFC 1918, ULA).
    Private,
    /// Link-local address (169.254.0.0/16, fe80::/10).
    LinkLocal,
    /// Documentation address (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24, 2001:db8::/32).
    Documentation,
    /// Reserved address (multicast, unspecified, etc.).
    Reserved,
    /// Public routable address.
    Public,
}

impl ForgeAddressClass {
    /// Stable string representation for telemetry.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Loopback => "loopback",
            Self::Private => "private",
            Self::LinkLocal => "link_local",
            Self::Documentation => "documentation",
            Self::Reserved => "reserved",
            Self::Public => "public",
        }
    }
}

/// Classify an IPv6 address for forge endpoint safety.
pub fn classify_ipv6_forge(v6: Ipv6Addr) -> ForgeAddressClass {
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

/// Classify an IPv4 address for forge endpoint safety.
pub fn classify_ipv4_forge(v4: Ipv4Addr) -> ForgeAddressClass {
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

fn extract_host(url: &str) -> Option<String> {
    url.parse::<reqwest::Url>()
        .ok()
        .and_then(|u| u.host_str().map(String::from))
}

async fn classify_host_from_url(url: &str) -> Option<String> {
    let parsed = url.parse::<reqwest::Url>().ok()?;
    let host = parsed.host_str()?;
    if let Some(ip) = parse_literal_ip(host) {
        let class = match ip {
            IpAddr::V4(v4) => classify_ipv4_forge(v4),
            IpAddr::V6(v6) => classify_ipv6_forge(v6),
        };
        return Some(class.as_str().to_string());
    }
    let addrs = tokio::net::lookup_host(format!("{host}:443")).await.ok()?;
    for addr in addrs {
        let class = match addr {
            std::net::SocketAddr::V4(v4) => classify_ipv4_forge(*v4.ip()),
            std::net::SocketAddr::V6(v6) => classify_ipv6_forge(*v6.ip()),
        };
        if class != ForgeAddressClass::Public {
            return Some(class.as_str().to_string());
        }
    }
    Some(ForgeAddressClass::Public.as_str().to_string())
}

fn encode_url_component(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

fn is_loopback_addr(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let Some(ip) = parse_literal_ip(host) else {
        return false;
    };
    match ip {
        IpAddr::V4(v4) => {
            matches!(classify_ipv4_forge(v4), ForgeAddressClass::Loopback) || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            matches!(classify_ipv6_forge(v6), ForgeAddressClass::Loopback) || v6.is_unspecified()
        }
    }
}

/// Derive the Gitea/Forgejo instance root URL from an API base URL.
///
/// E.g. `https://gitea.example.com/api/v1` → `https://gitea.example.com`.
pub fn derive_gitea_instance_root(api_base: &str) -> String {
    let base = api_base.trim_end_matches('/');
    if let Some(pos) = base.rfind("/api") {
        let root = &base[..pos];
        if super::engines::is_http_url(root) {
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
    let identity = &forge_response.identity;
    let ref_name = identity.requested_ref.clone().or_else(|| {
        identity
            .resolved_ref_name
            .as_ref()
            .filter(|s| !s.chars().all(|c| c.is_ascii_hexdigit()))
            .cloned()
    });
    let commit_sha = identity.resolved_commit_sha.clone();

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
            let (url, raw_url) = build_entry_urls(
                host,
                &owner,
                &repo,
                ref_str,
                commit_sha.as_deref(),
                raw.object_sha.as_deref(),
                &raw.path,
                raw.kind,
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

    let budget_aggregate_limit = forge_response.aggregate_limit;
    let budget_aggregate_remaining = forge_response.aggregate_remaining;
    let budget_request_count = forge_response.request_count;
    let budget_exhausted_by = forge_response.exhausted_by;

    let mut response = RepoMapResponse {
        query: request.query.clone(),
        host,
        owner: owner.clone(),
        repo: repo.clone(),
        ref_name: ref_name.clone(),
        commit_sha: commit_sha.clone(),
        tree_sha: identity.tree_sha.clone(),
        resolved_ref_name: identity.resolved_ref_name.clone(),
        default_branch: identity.default_branch.clone(),
        provenance_pinned: commit_sha.is_some(),
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
            endpoint_origin: forge_response.endpoint_origin,
            redirect_rejected: false,
            response_bytes_observed: if forge_response.response_bytes_observed > 0 {
                Some(forge_response.response_bytes_observed)
            } else {
                None
            },
            response_cap_applied: forge_response.response_cap_applied,
            dns_policy_class: forge_response.dns_policy_class,
            aggregate_byte_cap_reached: forge_response.aggregate_byte_cap_reached,
            aggregate_limit: Some(budget_aggregate_limit),
            aggregate_remaining: Some(budget_aggregate_remaining),
            request_count: Some(budget_request_count),
            exhausted_by: budget_exhausted_by.map(|k| {
                match k {
                    ForgeRequestKind::CommitResolution => "commit_resolution",
                    ForgeRequestKind::TreePage => "tree_page",
                    ForgeRequestKind::ContentsFallback => "contents_fallback",
                    ForgeRequestKind::RepositoryMetadata => "repository_metadata",
                }
                .to_string()
            }),
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
    fn loopback_detection_covers_literal_ranges_and_forms() {
        assert!(is_loopback_addr("localhost"));
        assert!(is_loopback_addr("LOCALHOST"));
        assert!(is_loopback_addr("127.0.0.2"));
        assert!(is_loopback_addr("127.255.255.255"));
        assert!(is_loopback_addr("::1"));
        assert!(is_loopback_addr("0:0:0:0:0:0:0:1"));
        assert!(is_loopback_addr("[::1]"));
        assert!(is_loopback_addr("0.0.0.0"));
        assert!(!is_loopback_addr("192.168.1.1"));
        assert!(!is_loopback_addr("example.com"));
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
            object_sha: Some("abc123".into()),
        };
        let e2 = ForgeRawEntry {
            path: "src".into(),
            kind: EntryKind::Directory,
            size: None,
            object_sha: Some("def456".into()),
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
                object_sha: Some("sha1".into()),
            }],
            identity: ResolvedRepositoryIdentity {
                requested_ref: Some("main".into()),
                resolved_ref_name: Some("main".into()),
                resolved_commit_sha: Some("commit_sha_abc".into()),
                tree_sha: Some("tree_sha_def".into()),
                default_branch: Some("main".into()),
            },
            truncated_by_provider: false,
            warnings: vec![],
            provider_id: "github_tree".into(),
            endpoint_origin: None,
            response_bytes_observed: 0,
            response_cap_applied: false,
            dns_policy_class: None,
            aggregate_byte_cap_reached: false,
            aggregate_limit: 10 * 1024 * 1024,
            aggregate_remaining: 10 * 1024 * 1024,
            request_count: 0,
            exhausted_by: None,
        };
        let resp = build_response(&req, forge, true, true, true, true, None);
        assert!(matches!(resp.mode, RepoMapMode::Native));
        assert_eq!(resp.host, CodeHost::Github);
        assert_eq!(resp.root_entries.len(), 1);
        assert_eq!(resp.root_entries[0].path, "README.md");
        assert_eq!(resp.default_branch.as_deref(), Some("main"));
        assert_eq!(resp.commit_sha.as_deref(), Some("commit_sha_abc"));
        assert_eq!(resp.tree_sha.as_deref(), Some("tree_sha_def"));
        assert!(resp.provenance_pinned);
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
                    object_sha: None,
                },
                ForgeRawEntry {
                    path: "src/main.rs".into(),
                    kind: EntryKind::File,
                    size: Some(100),
                    object_sha: None,
                },
            ],
            identity: ResolvedRepositoryIdentity {
                requested_ref: Some("main".into()),
                resolved_ref_name: Some("main".into()),
                ..Default::default()
            },
            truncated_by_provider: false,
            warnings: vec![],
            provider_id: "github_tree".into(),
            endpoint_origin: None,
            response_bytes_observed: 0,
            response_cap_applied: false,
            dns_policy_class: None,
            aggregate_byte_cap_reached: false,
            aggregate_limit: 10 * 1024 * 1024,
            aggregate_remaining: 10 * 1024 * 1024,
            request_count: 0,
            exhausted_by: None,
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
                    object_sha: None,
                },
                ForgeRawEntry {
                    path: "src".into(),
                    kind: EntryKind::Directory,
                    size: None,
                    object_sha: None,
                },
            ],
            identity: ResolvedRepositoryIdentity {
                requested_ref: Some("main".into()),
                resolved_ref_name: Some("main".into()),
                ..Default::default()
            },
            truncated_by_provider: false,
            warnings: vec![],
            provider_id: "github_tree".into(),
            endpoint_origin: None,
            response_bytes_observed: 0,
            response_cap_applied: false,
            dns_policy_class: None,
            aggregate_byte_cap_reached: false,
            aggregate_limit: 10 * 1024 * 1024,
            aggregate_remaining: 10 * 1024 * 1024,
            request_count: 0,
            exhausted_by: None,
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
            identity: ResolvedRepositoryIdentity::default(),
            truncated_by_provider: true,
            warnings: vec![SearchWarning::new("github_tree", "truncated")],
            provider_id: "github_tree".into(),
            endpoint_origin: None,
            response_bytes_observed: 0,
            response_cap_applied: false,
            dns_policy_class: None,
            aggregate_byte_cap_reached: false,
            aggregate_limit: 10 * 1024 * 1024,
            aggregate_remaining: 10 * 1024 * 1024,
            request_count: 0,
            exhausted_by: None,
        };
        let resp = build_response(&req, forge, true, true, true, true, None);
        assert!(!resp.warnings.is_empty());
    }

    #[test]
    fn build_entry_urls_uses_commit_sha_for_github() {
        let (browser, raw) = build_entry_urls(
            CodeHost::Github,
            "owner",
            "repo",
            "main",
            Some("commit_abc123"),
            Some("blob_def456"),
            "src/main.rs",
            EntryKind::File,
            None,
        );
        let browser = browser.unwrap();
        let raw = raw.unwrap();
        assert!(browser.contains("commit_abc123"));
        assert!(raw.contains("commit_abc123"));
        assert!(!browser.contains("blob_def456"));
        assert!(!raw.contains("blob_def456"));
    }

    #[test]
    fn build_entry_urls_falls_back_to_ref_when_no_commit() {
        let (browser, raw) = build_entry_urls(
            CodeHost::Github,
            "owner",
            "repo",
            "main",
            None,
            Some("blob_def456"),
            "src/main.rs",
            EntryKind::File,
            None,
        );
        let browser = browser.unwrap();
        let raw = raw.unwrap();
        assert!(browser.contains("main"));
        assert!(raw.contains("main"));
    }

    #[test]
    fn build_entry_urls_directory_omits_raw_url() {
        let (browser, raw) = build_entry_urls(
            CodeHost::Github,
            "owner",
            "repo",
            "main",
            Some("commit_abc123"),
            Some("tree_def456"),
            "src",
            EntryKind::Directory,
            None,
        );
        assert!(browser.is_some(), "Directory should have browser URL");
        assert!(raw.is_none(), "Directory should not have raw URL");
    }

    #[test]
    fn build_response_unpinned_when_no_commit() {
        let req = RepoMapRequest {
            owner: "test".into(),
            repo: "repo".into(),
            host: Some(CodeHost::Github),
            ref_name: Some("main".into()),
            ..Default::default()
        };
        let forge = ForgeTreeResponse {
            entries: vec![],
            identity: ResolvedRepositoryIdentity {
                requested_ref: Some("main".into()),
                resolved_ref_name: Some("main".into()),
                resolved_commit_sha: None,
                tree_sha: None,
                default_branch: Some("main".into()),
            },
            truncated_by_provider: false,
            warnings: vec![],
            provider_id: "github_tree".into(),
            endpoint_origin: None,
            response_bytes_observed: 0,
            response_cap_applied: false,
            dns_policy_class: None,
            aggregate_byte_cap_reached: false,
            aggregate_limit: 10 * 1024 * 1024,
            aggregate_remaining: 10 * 1024 * 1024,
            request_count: 0,
            exhausted_by: None,
        };
        let resp = build_response(&req, forge, true, true, true, true, None);
        assert!(!resp.provenance_pinned);
        assert!(resp.commit_sha.is_none());
    }
}

#[cfg(test)]
mod forge_budget_property_tests {
    use super::*;

    #[test]
    fn remaining_never_underflows() {
        let limits = [1, 100, 10_000, 1_000_000];
        let byte_sets: Vec<Vec<usize>> = vec![
            vec![],
            vec![0],
            vec![50, 30, 20],
            vec![100_000, 200_000, 300_000],
            vec![0, 0, 0, 0, 0],
        ];
        for limit in limits {
            for bytes in &byte_sets {
                let mut budget = ForgeReadBudget::new(limit);
                for b in bytes {
                    budget.consume(*b, ForgeRequestKind::TreePage);
                }
                assert!(
                    budget.remaining() <= limit,
                    "remaining {} must be <= limit {}",
                    budget.remaining(),
                    limit
                );
            }
        }
    }

    #[test]
    fn exhausted_set_exactly_once() {
        let mut budget = ForgeReadBudget::new(100);
        let mut exhaust_count = 0;
        for b in &[10, 20, 30, 40, 50, 60] {
            let was_exhausted = budget.exceeded();
            budget.consume(*b, ForgeRequestKind::TreePage);
            if !was_exhausted && budget.exceeded() {
                exhaust_count += 1;
            }
        }
        assert!(
            exhaust_count <= 1,
            "exhausted must be set at most once, was set {exhaust_count} times",
        );
    }

    #[test]
    fn request_count_matches_consume_count() {
        let mut budget = ForgeReadBudget::new(1_000_000);
        for b in &[10, 20, 30, 40, 50] {
            budget.consume(*b, ForgeRequestKind::TreePage);
        }
        assert_eq!(budget.request_count, 5);
    }

    #[test]
    fn aggregate_observed_saturating_add() {
        let mut budget = ForgeReadBudget::new(100);
        budget.consume(50, ForgeRequestKind::TreePage);
        budget.consume(60, ForgeRequestKind::TreePage);
        assert_eq!(budget.aggregate_observed, 110);
        assert!(budget.exceeded());
    }

    #[test]
    fn telemetry_reflects_actual_state() {
        let mut budget = ForgeReadBudget::new(1000);
        budget.consume(100, ForgeRequestKind::TreePage);
        budget.consume(200, ForgeRequestKind::TreePage);
        let tel = budget.telemetry();
        assert_eq!(tel.aggregate_limit, 1000);
        assert_eq!(tel.aggregate_observed, 300);
        assert_eq!(tel.remaining, 700);
        assert_eq!(tel.request_count, 2);
        assert!(!budget.exceeded());
    }

    #[tokio::test]
    async fn aggregate_limit_does_not_count_as_per_response_cap() {
        let server = httpmock::MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/large");
            then.status(200).body("x".repeat(2048));
        });

        let response = reqwest::Client::new()
            .get(server.url("/large"))
            .send()
            .await
            .unwrap();
        let mut budget = ForgeReadBudget::new(1024);
        let result = read_with_budget(response, &mut budget, ForgeRequestKind::TreePage).await;

        assert_eq!(result, Err(ForgeReadError::AggregateBudgetExhausted));
        assert_eq!(budget.per_response_cap_hits, 0);
    }

    #[test]
    fn zero_byte_consume_does_not_exhaust() {
        let mut budget = ForgeReadBudget::new(100);
        for _ in 0..50 {
            budget.consume(0, ForgeRequestKind::TreePage);
        }
        assert!(!budget.exceeded());
    }
}
