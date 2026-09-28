//! Native remote repository tree adapter for code hosts.
//!
//! Provides bounded, provider-neutral tree retrieval for GitHub, GitLab,
//! Gitea, Forgejo, and Codeberg without cloning repositories. All tree
//! operations enforce entry, depth, byte, pagination, concurrency, and
//! timeout limits.

use std::sync::LazyLock;
use std::time::Duration;

use eggfetch_core::Client;
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

mod budget;

pub(crate) use budget::read_with_budget;
#[cfg(test)]
pub(crate) use budget::ForgeReadError;
pub use budget::{read_error_body_preview, ForgeReadBudget, ForgeReadBudgetTelemetry};

mod policy;

#[cfg(test)]
use policy::is_loopback_addr;
use policy::{classify_host_from_url, encode_url_component, extract_host, validate_base_url_async};
pub use policy::{
    classify_ipv4_forge, classify_ipv6_forge, validate_base_url, ForgeAddressClass,
    ForgeEndpointPolicy, ForgeRequestKind,
};

mod urls;
use urls::build_entry_urls;
pub use urls::derive_gitea_instance_root;

mod gitea;
mod github;
mod gitlab;

use gitea::{fetch_forge_tree, ForgeTreeParams};
use github::fetch_github_tree;
use gitlab::fetch_gitlab_tree;

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

fn forge_timeout(duration: Duration) -> eggfetch_core::Timeout {
    eggfetch_core::Timeout {
        pool: Some(duration),
        connect: Some(duration),
        write: Some(duration),
        read: Some(duration),
        total: Some(duration),
    }
}

fn build_client() -> Result<Client, String> {
    Ok(Client::builder()
        .timeout(forge_timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)))
        .user_agent("eggsearch/1.0")
        .follow_redirects(false)
        .build())
}

fn timeout_duration(req: &RepoMapRequest) -> Duration {
    let ms = req.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_SECS * 1000);
    let clamped = ms.clamp(1000, DEFAULT_TIMEOUT_SECS * 1000);
    if clamped != ms {
        tracing::warn!(
            requested_ms = ms,
            clamped_ms = clamped,
            "forge timeout clamped to [1000, {}]ms",
            DEFAULT_TIMEOUT_SECS * 1000
        );
    }
    Duration::from_millis(clamped)
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
                    ForgeRequestKind::ErrorBody => "error_body",
                }
                .to_string()
            }),
        }),
        freshness_confidence: None,
        ..Default::default()
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

        let response = eggfetch_core::Client::new()
            .get(server.url("/large").as_str())
            .unwrap()
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
