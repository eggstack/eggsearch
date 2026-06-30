//! Local repository inventory: Git worktree discovery, remote URL
//! normalization, identity matching, and manifest detection.
//!
//! This module provides lightweight, bounded inventory of Git
//! repositories under configured local roots. It normalizes remote
//! URLs to structured identities, detects worktree state (branch,
//! commit, dirty), and identifies package manifests — all without
//! cloning, indexing, or running build commands.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::code_metadata::CodeHost;
use crate::core::local::LocalConfig;

/// A normalized remote URL identity for a Git repository.
///
/// Produced by parsing remote `origin` URLs from the repository's
/// Git configuration. Supports HTTPS, SSH scp-style, and SSH URL forms.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NormalizedRepoId {
    /// The code-hosting platform.
    pub host: CodeHost,
    /// The host domain (e.g. `github.com`, `gitlab.com`, `git.example.com`).
    pub host_domain: Option<String>,
    /// Repository owner (or namespace for GitLab nested groups).
    pub owner: String,
    /// Repository name (without `.git` suffix).
    pub repo: String,
}

impl std::fmt::Display for NormalizedRepoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{}/{}",
            self.host_domain.as_deref().unwrap_or("unknown"),
            self.owner,
            self.repo
        )
    }
}

/// Dirty state of a Git working tree.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalDirtyState {
    /// Working tree has no uncommitted changes.
    #[default]
    Clean,
    /// Working tree has uncommitted changes (staged or unstaged).
    Dirty,
    /// Dirty state could not be determined.
    Unknown,
    /// Path is not a Git repository.
    NotGit,
}

impl std::fmt::Display for LocalDirtyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clean => write!(f, "clean"),
            Self::Dirty => write!(f, "dirty"),
            Self::Unknown => write!(f, "unknown"),
            Self::NotGit => write!(f, "not_git"),
        }
    }
}

/// Summary of a detected package manifest in a local repository.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalManifestSummary {
    /// The manifest file path relative to the repository root.
    pub path: String,
    /// Detected package ecosystem.
    pub ecosystem: LocalManifestEcosystem,
    /// Package name extracted from the manifest, if available.
    pub package_name: Option<String>,
}

/// Ecosystem detected from a manifest file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalManifestEcosystem {
    /// Rust workspace or crate (`Cargo.toml`).
    CratesIo,
    /// Node.js project (`package.json`).
    Npm,
    /// Python project (`pyproject.toml`, `setup.py`, `setup.cfg`).
    PyPI,
    /// Go module (`go.mod`).
    Go,
    /// Java/Kotlin with Maven (`pom.xml`).
    Maven,
    /// Java/Kotlin with Gradle (`build.gradle`, `build.gradle.kts`, `settings.gradle`).
    Gradle,
    /// .NET solution or project (`.sln`, `.csproj`).
    Dotnet,
    /// Unrecognized manifest.
    Other,
}

impl std::fmt::Display for LocalManifestEcosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CratesIo => write!(f, "crates_io"),
            Self::Npm => write!(f, "npm"),
            Self::PyPI => write!(f, "pypi"),
            Self::Go => write!(f, "go"),
            Self::Maven => write!(f, "maven"),
            Self::Gradle => write!(f, "gradle"),
            Self::Dotnet => write!(f, "dotnet"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Identity and state of a local Git repository checkout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalRepoIdentity {
    /// Root directory name of the checkout (e.g. `axum`).
    pub root_name: String,
    /// Canonical path to the repository root.
    pub root_path: PathBuf,
    /// Worktree path (may differ from root_path in worktree setups).
    pub worktree_path: PathBuf,
    /// All configured remote URLs, normalized.
    pub remotes: Vec<NormalizedRepoId>,
    /// Whether the checkout matches a recognized code host.
    pub matched_host: Option<CodeHost>,
    /// Matched owner (from remote URL).
    pub matched_owner: Option<String>,
    /// Matched repo name (from remote URL).
    pub matched_repo: Option<String>,
    /// Current branch name, if determinable.
    pub current_branch: Option<String>,
    /// Current commit SHA, if determinable.
    pub current_commit: Option<String>,
    /// Working tree dirty state.
    pub dirty_state: LocalDirtyState,
    /// Detected package manifests.
    pub manifests: Vec<LocalManifestSummary>,
}

impl LocalRepoIdentity {
    /// Whether this identity matches the given (host, owner, repo) tuple.
    pub fn matches(&self, host: Option<&CodeHost>, owner: &str, repo: &str) -> bool {
        let owner_eq = self
            .matched_owner
            .as_deref()
            .is_some_and(|o| o.eq_ignore_ascii_case(owner));
        let repo_eq = self
            .matched_repo
            .as_deref()
            .is_some_and(|r| r.eq_ignore_ascii_case(repo));
        if !owner_eq || !repo_eq {
            return false;
        }
        match host {
            Some(h) => self.matched_host.as_ref() == Some(h),
            None => true,
        }
    }
}

/// Parse a Git remote URL into a `NormalizedRepoId`.
///
/// Supports:
/// - `https://github.com/owner/repo.git`
/// - `https://github.com/owner/repo`
/// - `git@github.com:owner/repo.git`
/// - `ssh://git@github.com/owner/repo.git`
/// - `git://github.com/owner/repo.git`
///
/// Returns `None` if the URL cannot be parsed.
pub fn normalize_remote_url(url: &str) -> Option<NormalizedRepoId> {
    let url = url.trim();

    // Try HTTPS/SSH URL form first
    if let Some(repo_id) = parse_url_form(url) {
        return Some(repo_id);
    }

    // Try SCP-style: git@host:owner/repo.git
    if let Some(repo_id) = parse_scp_form(url) {
        return Some(repo_id);
    }

    None
}

fn parse_url_form(url: &str) -> Option<NormalizedRepoId> {
    let parsed = url::Url::parse(url).ok()?;
    let scheme = parsed.scheme();
    if !["https", "http", "ssh", "git"].contains(&scheme) {
        return None;
    }

    let host_domain = parsed.host_str()?.to_string();
    let host = classify_host(&host_domain);

    let path = parsed.path().trim_start_matches('/');
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);

    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 2 {
        return None;
    }

    // For GitLab nested groups, owner may be multiple segments
    let owner = segments[..segments.len() - 1].join("/");
    let repo = segments[segments.len() - 1].to_string();

    Some(NormalizedRepoId {
        host,
        host_domain: Some(host_domain),
        owner,
        repo,
    })
}

fn parse_scp_form(url: &str) -> Option<NormalizedRepoId> {
    // Pattern: [user@]host:owner/repo[.git]
    // Reject strings that look like HTTP(S) URLs
    if url.contains("://") {
        return None;
    }

    let rest = if let Some(after_at) = url.rsplit('@').next() {
        after_at
    } else {
        url
    };

    let (host_part, path_part) = rest.split_once(':')?;
    if host_part.is_empty() || path_part.is_empty() {
        return None;
    }

    let host_domain = host_part.to_string();
    let host = classify_host(&host_domain);

    let path_part = path_part.trim_start_matches('/');
    let path_part = path_part.strip_suffix(".git").unwrap_or(path_part);

    let segments: Vec<&str> = path_part.split('/').collect();
    if segments.len() < 2 {
        return None;
    }

    let owner = segments[..segments.len() - 1].join("/");
    let repo = segments[segments.len() - 1].to_string();

    Some(NormalizedRepoId {
        host,
        host_domain: Some(host_domain),
        owner,
        repo,
    })
}

/// Classify a host domain into a `CodeHost` variant.
pub fn classify_host(domain: &str) -> CodeHost {
    let domain = domain.trim_start_matches("www.").to_lowercase();
    match domain.as_str() {
        "github.com" => CodeHost::Github,
        "gitlab.com" => CodeHost::Gitlab,
        "codeberg.org" => CodeHost::Codeberg,
        _ => {
            // Heuristic: if it contains "gitlab" it's likely a GitLab instance,
            // if it contains "gitea" or "forgejo" it's likely one of those.
            if domain.contains("gitlab") {
                CodeHost::Gitlab
            } else if domain.contains("gitea") || domain.contains("forgejo") {
                CodeHost::Gitea
            } else {
                CodeHost::Unknown
            }
        }
    }
}

/// Detect whether a directory is a Git repository by checking for
/// `.git` directory or `.git` file (gitfile for worktrees/submodules).
pub fn detect_git_worktree(path: &Path) -> bool {
    let git_path = path.join(".git");
    if git_path.exists() {
        return true;
    }
    false
}

/// Read the remote URLs from a Git repository's config file.
///
/// Reads `.git/config` directly (no `git` command invocation).
/// Returns a list of `(remote_name, normalized_url)` pairs.
pub fn read_remotes_from_config(repo_root: &Path) -> Vec<(String, NormalizedRepoId)> {
    let config_path = repo_root.join(".git").join("config");
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut remotes = Vec::new();
    let mut current_remote: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Match [remote "origin"] sections
        if trimmed.starts_with("[remote") {
            if let Some(name_start) = trimmed.find('"') {
                if let Some(name_end) = trimmed[name_start + 1..].find('"') {
                    let name = &trimmed[name_start + 1..name_start + 1 + name_end];
                    current_remote = Some(name.to_string());
                }
            }
            continue;
        }

        // Match url = ... within a remote section
        if let Some(remote) = &current_remote {
            if let Some(url_value) = trimmed.strip_prefix("url") {
                let url_value = url_value.trim();
                if let Some(url_value) = url_value.strip_prefix('=') {
                    let url_value = url_value.trim();
                    if let Some(normalized) = normalize_remote_url(url_value) {
                        remotes.push((remote.clone(), normalized));
                    }
                }
            }
        }

        // Reset on new section
        if trimmed.starts_with('[') && !trimmed.starts_with("[remote") {
            current_remote = None;
        }
    }

    remotes
}

/// Read the HEAD branch from `.git/HEAD`.
///
/// Returns `Some("main")` if HEAD points to `refs/heads/main`, or
/// `None` if HEAD is detached or unreadable.
pub fn read_head_branch(repo_root: &Path) -> Option<String> {
    let head_path = repo_root.join(".git").join("HEAD");
    let content = std::fs::read_to_string(&head_path).ok()?;
    let content = content.trim();
    content
        .strip_prefix("ref: refs/heads/")
        .map(|s| s.to_string())
}

/// Read the current commit SHA from `.git/HEAD`.
///
/// For detached HEAD, reads the SHA directly. For symbolic refs,
/// reads the packed-refs or the ref file.
pub fn read_head_commit(repo_root: &Path) -> Option<String> {
    let head_path = repo_root.join(".git").join("HEAD");
    let content = std::fs::read_to_string(&head_path).ok()?;
    let content = content.trim().to_string();

    // Detached HEAD: the content is a SHA
    if !content.starts_with("ref: ") {
        // Validate it looks like a SHA (hex chars, 7-40 chars)
        let sha = content;
        if sha.len() >= 7 && sha.len() <= 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(sha);
        }
        return None;
    }

    // Symbolic ref: resolve through packed-refs
    let ref_path = content.strip_prefix("ref: ")?;
    let full_ref_path = repo_root.join(".git").join(ref_path);

    // Try loose ref first
    if let Ok(sha) = std::fs::read_to_string(&full_ref_path) {
        let sha = sha.trim().to_string();
        if !sha.is_empty() {
            return Some(sha);
        }
    }

    // Try packed-refs
    let packed_path = repo_root.join(".git").join("packed-refs");
    if let Ok(packed) = std::fs::read_to_string(&packed_path) {
        for line in packed.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((sha, ref_name)) = line.split_once(' ') {
                if ref_name == ref_path {
                    return Some(sha.to_string());
                }
            }
        }
    }

    None
}

/// Detect the dirty state of a Git working tree.
///
/// Uses `git status --porcelain` via direct process invocation.
/// This is acceptable for bounded inventory detection. Returns
/// `Unknown` if the command fails or is not available.
pub fn detect_dirty_state(repo_root: &Path) -> LocalDirtyState {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("status")
        .arg("--porcelain")
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.trim().is_empty() {
                    LocalDirtyState::Clean
                } else {
                    LocalDirtyState::Dirty
                }
            } else {
                LocalDirtyState::Unknown
            }
        }
        Err(_) => LocalDirtyState::Unknown,
    }
}

/// Detect package manifests in a repository root.
///
/// Scans the root directory for known manifest files and returns
/// summaries. Does not parse manifest contents — only identifies
/// the file and infers the ecosystem.
pub fn detect_manifests(repo_root: &Path) -> Vec<LocalManifestSummary> {
    let mut manifests = Vec::new();

    let manifest_checks: &[(&str, LocalManifestEcosystem)] = &[
        ("Cargo.toml", LocalManifestEcosystem::CratesIo),
        ("package.json", LocalManifestEcosystem::Npm),
        ("pyproject.toml", LocalManifestEcosystem::PyPI),
        ("setup.py", LocalManifestEcosystem::PyPI),
        ("setup.cfg", LocalManifestEcosystem::PyPI),
        ("go.mod", LocalManifestEcosystem::Go),
        ("pom.xml", LocalManifestEcosystem::Maven),
        ("build.gradle", LocalManifestEcosystem::Gradle),
        ("build.gradle.kts", LocalManifestEcosystem::Gradle),
        ("settings.gradle", LocalManifestEcosystem::Gradle),
        ("settings.gradle.kts", LocalManifestEcosystem::Gradle),
    ];

    for &(filename, ecosystem) in manifest_checks {
        if repo_root.join(filename).exists() {
            manifests.push(LocalManifestSummary {
                path: filename.to_string(),
                ecosystem,
                package_name: None,
            });
        }
    }

    // Check for .sln and .csproj in root
    if let Ok(entries) = std::fs::read_dir(repo_root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".sln") || name.ends_with(".csproj") {
                manifests.push(LocalManifestSummary {
                    path: name,
                    ecosystem: LocalManifestEcosystem::Dotnet,
                    package_name: None,
                });
            }
        }
    }

    manifests
}

/// Discover all Git repositories under the configured roots.
///
/// Walks each root directory up to a bounded depth, detecting Git
/// worktrees. Returns identities for all discovered repositories.
pub fn discover_local_repos(config: &LocalConfig, max_depth: usize) -> Vec<LocalRepoIdentity> {
    if !config.enabled || config.roots.is_empty() {
        return Vec::new();
    }

    let mut repos = Vec::new();

    for root in &config.roots {
        let canonical = match root.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !canonical.is_dir() {
            continue;
        }
        discover_in_dir(&canonical, 0, max_depth, config, &mut repos);
    }

    repos
}

fn discover_in_dir(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    config: &LocalConfig,
    repos: &mut Vec<LocalRepoIdentity>,
) {
    if depth > max_depth {
        return;
    }

    if detect_git_worktree(dir) {
        if let Some(identity) = build_repo_identity(dir) {
            repos.push(identity);
        }
        // Don't recurse into the repo's subdirectories for nested repos
        return;
    }

    // Recurse into subdirectories
    let read_dir = match std::fs::read_dir(dir) {
        Ok(d) => d,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden directories and common non-repo dirs
        if !config.include_hidden && name_str.starts_with('.') {
            continue;
        }
        if crate::core::local::SKIP_DIRS.contains(&name_str.as_ref()) {
            continue;
        }

        let path = entry.path();
        if path.is_dir() {
            discover_in_dir(&path, depth + 1, max_depth, config, repos);
        }
    }
}

/// Build a `LocalRepoIdentity` from a Git repository root.
fn build_repo_identity(repo_root: &Path) -> Option<LocalRepoIdentity> {
    let root_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let remotes_data = read_remotes_from_config(repo_root);

    // Find the best remote to match: prefer "origin", then first
    let (matched_host, matched_owner, matched_repo) = remotes_data
        .iter()
        .find(|(name, _)| name == "origin")
        .or_else(|| remotes_data.first())
        .map(|(_, id)| (Some(id.host), Some(id.owner.clone()), Some(id.repo.clone())))
        .unwrap_or((None, None, None));

    let remotes: Vec<NormalizedRepoId> = remotes_data
        .into_iter()
        .map(|(_, id)| id)
        .collect();

    let current_branch = read_head_branch(repo_root);
    let current_commit = read_head_commit(repo_root);
    let dirty_state = detect_dirty_state(repo_root);
    let manifests = detect_manifests(repo_root);

    Some(LocalRepoIdentity {
        root_name,
        root_path: repo_root.to_path_buf(),
        worktree_path: repo_root.to_path_buf(),
        remotes,
        matched_host,
        matched_owner,
        matched_repo,
        current_branch,
        current_commit,
        dirty_state,
        manifests,
    })
}

/// Match an incoming repo locator to a list of local repo identities.
///
/// Returns the matching identity, if any. Matching is case-insensitive
/// on owner and repo.
pub fn match_local_repo<'a>(
    identities: &'a [LocalRepoIdentity],
    host: Option<&CodeHost>,
    owner: &str,
    repo: &str,
) -> Option<&'a LocalRepoIdentity> {
    identities.iter().find(|id| id.matches(host, owner, repo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_https_github() {
        let id = normalize_remote_url("https://github.com/tokio-rs/axum.git").unwrap();
        assert_eq!(id.host, CodeHost::Github);
        assert_eq!(id.host_domain.as_deref(), Some("github.com"));
        assert_eq!(id.owner, "tokio-rs");
        assert_eq!(id.repo, "axum");
    }

    #[test]
    fn normalize_https_github_no_dot_git() {
        let id = normalize_remote_url("https://github.com/tokio-rs/axum").unwrap();
        assert_eq!(id.host, CodeHost::Github);
        assert_eq!(id.owner, "tokio-rs");
        assert_eq!(id.repo, "axum");
    }

    #[test]
    fn normalize_scp_github() {
        let id = normalize_remote_url("git@github.com:tokio-rs/axum.git").unwrap();
        assert_eq!(id.host, CodeHost::Github);
        assert_eq!(id.owner, "tokio-rs");
        assert_eq!(id.repo, "axum");
    }

    #[test]
    fn normalize_scp_github_no_dot_git() {
        let id = normalize_remote_url("git@github.com:tokio-rs/axum").unwrap();
        assert_eq!(id.host, CodeHost::Github);
        assert_eq!(id.owner, "tokio-rs");
        assert_eq!(id.repo, "axum");
    }

    #[test]
    fn normalize_ssh_url() {
        let id =
            normalize_remote_url("ssh://git@github.com/tokio-rs/axum.git").unwrap();
        assert_eq!(id.host, CodeHost::Github);
        assert_eq!(id.owner, "tokio-rs");
        assert_eq!(id.repo, "axum");
    }

    #[test]
    fn normalize_git_protocol() {
        let id =
            normalize_remote_url("git://github.com/tokio-rs/axum.git").unwrap();
        assert_eq!(id.host, CodeHost::Github);
        assert_eq!(id.owner, "tokio-rs");
        assert_eq!(id.repo, "axum");
    }

    #[test]
    fn normalize_gitlab() {
        let id = normalize_remote_url("https://gitlab.com/group/subgroup/project.git").unwrap();
        assert_eq!(id.host, CodeHost::Gitlab);
        assert_eq!(id.owner, "group/subgroup");
        assert_eq!(id.repo, "project");
    }

    #[test]
    fn normalize_gitlab_scp() {
        let id = normalize_remote_url("git@gitlab.com:group/subgroup/project.git").unwrap();
        assert_eq!(id.host, CodeHost::Gitlab);
        assert_eq!(id.owner, "group/subgroup");
        assert_eq!(id.repo, "project");
    }

    #[test]
    fn normalize_self_hosted_gitlab() {
        let id =
            normalize_remote_url("https://gitlab.example.com/team/project.git").unwrap();
        assert_eq!(id.host, CodeHost::Gitlab);
        assert_eq!(id.host_domain.as_deref(), Some("gitlab.example.com"));
        assert_eq!(id.owner, "team");
        assert_eq!(id.repo, "project");
    }

    #[test]
    fn normalize_codeberg() {
        let id = normalize_remote_url("https://codeberg.org/owner/repo.git").unwrap();
        assert_eq!(id.host, CodeHost::Codeberg);
        assert_eq!(id.owner, "owner");
        assert_eq!(id.repo, "repo");
    }

    #[test]
    fn normalize_unknown_host() {
        let id = normalize_remote_url("https://example.com/team/project.git").unwrap();
        assert_eq!(id.host, CodeHost::Unknown);
        assert_eq!(id.owner, "team");
        assert_eq!(id.repo, "project");
    }

    #[test]
    fn normalize_invalid_url() {
        assert!(normalize_remote_url("not a url").is_none());
    }

    #[test]
    fn normalize_too_few_segments() {
        assert!(normalize_remote_url("https://github.com/onlyowner").is_none());
    }

    #[test]
    fn classify_host_known() {
        assert_eq!(classify_host("github.com"), CodeHost::Github);
        assert_eq!(classify_host("gitlab.com"), CodeHost::Gitlab);
        assert_eq!(classify_host("codeberg.org"), CodeHost::Codeberg);
    }

    #[test]
    fn classify_host_gitlab_self_hosted() {
        assert_eq!(classify_host("gitlab.internal.co"), CodeHost::Gitlab);
    }

    #[test]
    fn classify_host_gitea_self_hosted() {
        assert_eq!(classify_host("gitea.example.com"), CodeHost::Gitea);
    }

    #[test]
    fn classify_host_forgejo() {
        assert_eq!(classify_host("forgejo.example.com"), CodeHost::Gitea);
    }

    #[test]
    fn classify_host_unknown() {
        assert_eq!(classify_host("example.com"), CodeHost::Unknown);
    }

    #[test]
    fn detect_git_worktree_with_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        assert!(detect_git_worktree(dir.path()));
    }

    #[test]
    fn detect_git_worktree_without_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!detect_git_worktree(dir.path()));
    }

    #[test]
    fn detect_git_worktree_with_gitfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".git"), "gitdir: ../.git/modules/foo\n").unwrap();
        assert!(detect_git_worktree(dir.path()));
    }

    #[test]
    fn read_head_branch_detached() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(
            dir.path().join(".git").join("HEAD"),
            "abc123def456789\n",
        )
        .unwrap();
        assert_eq!(read_head_branch(dir.path()), None);
    }

    #[test]
    fn read_head_branch_symbolic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(
            dir.path().join(".git").join("HEAD"),
            "ref: refs/heads/main\n",
        )
        .unwrap();
        assert_eq!(read_head_branch(dir.path()).as_deref(), Some("main"));
    }

    #[test]
    fn read_head_commit_detached() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(
            dir.path().join(".git").join("HEAD"),
            "abc123def456789012345678901234567890abcd\n",
        )
        .unwrap();
        let commit = read_head_commit(dir.path()).unwrap();
        assert_eq!(commit.len(), 40);
        assert!(commit.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn read_head_commit_symbolic_loose_ref() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(git_dir.join("refs").join("heads")).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(
            git_dir.join("refs").join("heads").join("main"),
            "abc123def456789012345678901234567890abcd\n",
        )
        .unwrap();
        let commit = read_head_commit(dir.path()).unwrap();
        assert_eq!(commit, "abc123def456789012345678901234567890abcd");
    }

    #[test]
    fn dirty_state_clean() {
        let dir = tempfile::tempdir().unwrap();
        // Initialize a git repo
        std::process::Command::new("git")
            .arg("init")
            .arg(dir.path())
            .output()
            .ok();
        assert_eq!(detect_dirty_state(dir.path()), LocalDirtyState::Clean);
    }

    #[test]
    fn dirty_state_dirty() {
        let dir = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .arg("init")
            .arg(dir.path())
            .output()
            .ok();
        // Create an untracked file
        std::fs::write(dir.path().join("new_file.txt"), "hello").unwrap();
        assert_eq!(detect_dirty_state(dir.path()), LocalDirtyState::Dirty);
    }

    #[test]
    fn dirty_state_non_git() {
        let dir = tempfile::tempdir().unwrap();
        // No git init — git status will fail
        let state = detect_dirty_state(dir.path());
        assert_eq!(state, LocalDirtyState::Unknown);
    }

    #[test]
    fn detect_manifests_empty() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = detect_manifests(dir.path());
        assert!(manifests.is_empty());
    }

    #[test]
    fn detect_manifests_cargo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        let manifests = detect_manifests(dir.path());
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].ecosystem, LocalManifestEcosystem::CratesIo);
    }

    #[test]
    fn detect_manifests_multiple() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let manifests = detect_manifests(dir.path());
        assert!(manifests.len() >= 2);
    }

    #[test]
    fn identity_matches_case_insensitive() {
        let id = LocalRepoIdentity {
            root_name: "axum".to_string(),
            root_path: PathBuf::from("/tmp/axum"),
            worktree_path: PathBuf::from("/tmp/axum"),
            remotes: vec![],
            matched_host: Some(CodeHost::Github),
            matched_owner: Some("tokio-rs".to_string()),
            matched_repo: Some("axum".to_string()),
            current_branch: Some("main".to_string()),
            current_commit: None,
            dirty_state: LocalDirtyState::Clean,
            manifests: vec![],
        };
        assert!(id.matches(Some(&CodeHost::Github), "tokio-rs", "axum"));
        assert!(id.matches(Some(&CodeHost::Github), "Tokio-Rs", "Axum"));
        assert!(id.matches(None, "tokio-rs", "axum"));
        assert!(!id.matches(Some(&CodeHost::Gitlab), "tokio-rs", "axum"));
        assert!(!id.matches(Some(&CodeHost::Github), "other", "axum"));
        assert!(!id.matches(Some(&CodeHost::Github), "tokio-rs", "other"));
    }

    #[test]
    fn match_local_repo_finds_match() {
        let identities = vec![
            LocalRepoIdentity {
                root_name: "axum".to_string(),
                root_path: PathBuf::from("/tmp/axum"),
                worktree_path: PathBuf::from("/tmp/axum"),
                remotes: vec![],
                matched_host: Some(CodeHost::Github),
                matched_owner: Some("tokio-rs".to_string()),
                matched_repo: Some("axum".to_string()),
                current_branch: None,
                current_commit: None,
                dirty_state: LocalDirtyState::Unknown,
                manifests: vec![],
            },
        ];
        let found = match_local_repo(&identities, Some(&CodeHost::Github), "tokio-rs", "axum");
        assert!(found.is_some());
        assert_eq!(found.unwrap().root_name, "axum");
    }

    #[test]
    fn match_local_repo_no_match() {
        let identities = vec![
            LocalRepoIdentity {
                root_name: "axum".to_string(),
                root_path: PathBuf::from("/tmp/axum"),
                worktree_path: PathBuf::from("/tmp/axum"),
                remotes: vec![],
                matched_host: Some(CodeHost::Github),
                matched_owner: Some("tokio-rs".to_string()),
                matched_repo: Some("axum".to_string()),
                current_branch: None,
                current_commit: None,
                dirty_state: LocalDirtyState::Unknown,
                manifests: vec![],
            },
        ];
        let found = match_local_repo(&identities, Some(&CodeHost::Github), "other", "repo");
        assert!(found.is_none());
    }

    #[test]
    fn discover_local_repos_disabled() {
        let config = LocalConfig::default();
        let repos = discover_local_repos(&config, 2);
        assert!(repos.is_empty());
    }

    #[test]
    fn discover_local_repos_finds_git_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo_dir = dir.path().join("myrepo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::process::Command::new("git")
            .arg("init")
            .arg(&repo_dir)
            .output()
            .ok();

        let config = LocalConfig {
            enabled: true,
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let repos = discover_local_repos(&config, 2);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].root_name, "myrepo");
    }

    #[test]
    fn discover_local_repos_with_remote() {
        let dir = tempfile::tempdir().unwrap();
        let repo_dir = dir.path().join("axum");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::process::Command::new("git")
            .arg("init")
            .arg(&repo_dir)
            .output()
            .ok();

        // Write a .git/config with remote origin
        let git_dir = repo_dir.join(".git");
        std::fs::write(
            git_dir.join("config"),
            "[remote \"origin\"]\n\turl = https://github.com/tokio-rs/axum.git\n",
        )
        .unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let repos = discover_local_repos(&config, 2);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].matched_owner.as_deref(), Some("tokio-rs"));
        assert_eq!(repos[0].matched_repo.as_deref(), Some("axum"));
        assert_eq!(repos[0].matched_host, Some(CodeHost::Github));
    }

    #[test]
    fn dirty_state_display() {
        assert_eq!(LocalDirtyState::Clean.to_string(), "clean");
        assert_eq!(LocalDirtyState::Dirty.to_string(), "dirty");
        assert_eq!(LocalDirtyState::Unknown.to_string(), "unknown");
        assert_eq!(LocalDirtyState::NotGit.to_string(), "not_git");
    }

    #[test]
    fn repo_id_display() {
        let id = NormalizedRepoId {
            host: CodeHost::Github,
            host_domain: Some("github.com".to_string()),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
        };
        assert_eq!(id.to_string(), "github.com/tokio-rs/axum");
    }
}
