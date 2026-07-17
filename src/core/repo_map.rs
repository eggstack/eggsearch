//! Repository map types for structured repo discovery.
//!
//! `repo_map` provides a structured view of a repository's root-level
//! layout and important files/directories without fetching or cloning.
//! The response is designed to help agents understand a repository's
//! structure quickly and decide which files to inspect next.

use serde::{Deserialize, Serialize};

use crate::core::code_metadata::CodeHost;
use crate::core::repo_fetch::RepoFetchRequest;
use crate::core::result::SearchWarning;
use crate::core::sanitize::TrustMarkers;
use crate::core::warning::AgentWarning;
use crate::meta::response::ProviderFailure;

/// Classification of an important file at a repository root.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ImportantFileKind {
    /// README or equivalent documentation file.
    #[default]
    Readme,
    /// Package or project manifest (Cargo.toml, package.json, pyproject.toml, etc.).
    Manifest,
    /// Dockerfile or container configuration.
    Dockerfile,
    /// Changelog or release notes file.
    Changelog,
    /// Security-related file (SECURITY.md, security policy, etc.).
    Security,
    /// License file.
    License,
    /// Contributing guide.
    Contributing,
    /// CI/CD configuration file (GitHub Actions workflow, GitLab CI, etc.).
    CiConfig,
    /// Lockfile (Cargo.lock, package-lock.json, yarn.lock, etc.).
    Lockfile,
    /// Editor or IDE configuration (.editorconfig, .vscode, etc.).
    EditorConfig,
    /// Gitignore or git-related config.
    GitIgnore,
    /// Makefile or build script.
    BuildScript,
    /// Documentation file outside the main docs directory.
    Documentation,
    /// Ignore this entry.
    Ignored,
    /// Unrecognized file.
    Unknown,
}

/// Classification of an important directory in a repository.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ImportantDirKind {
    /// Source code root directory (src, lib, crates, packages, etc.).
    #[default]
    SourceRoot,
    /// Examples or samples directory.
    Examples,
    /// Test or benchmark directory.
    Tests,
    /// Documentation directory (docs, doc, website, book).
    Docs,
    /// CI/CD workflow directory (.github/workflows, .gitlab-ci, etc.).
    CiConfig,
    /// Security-related directory or file path.
    Security,
    /// Configuration directory (.config, .vscode, etc.).
    Config,
    /// Generated or vendored directory (target, node_modules, vendor, etc.).
    Generated,
    /// Unrecognized directory.
    Unknown,
}

/// Discriminator for root-level repository entries.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RepoMapEntryKind {
    /// A file.
    #[default]
    File,
    /// A directory.
    Directory,
    /// A symbolic link.
    Symlink,
    /// A git submodule.
    Submodule,
    /// Unrecognized entry type.
    Unknown,
}

/// Operating mode for the repo map response.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RepoMapMode {
    /// Native mode: the server fetched the repository root tree directly
    /// from the code-host API.
    #[default]
    Native,
    /// Fallback mode: no native provider was available; the map is
    /// synthesized from a web search or local workspace scan.
    FallbackSearch,
}

/// A single root-level repository entry (file, directory, symlink, etc.).
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoMapEntry {
    /// Relative path from the repository root.
    pub path: String,
    /// The kind of entry (file, directory, symlink, submodule).
    pub kind: RepoMapEntryKind,
    /// File size in bytes, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Inferred programming language, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// An important file in the repository with classification metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoImportantFile {
    /// Relative path from the repository root.
    pub path: String,
    /// Classification of this important file.
    pub kind: ImportantFileKind,
    /// Human-readable reasons why this file is classified as important.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    /// File size in bytes, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// An important directory in the repository with classification metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoImportantDirectory {
    /// Relative path from the repository root.
    pub path: String,
    /// Classification of this important directory.
    pub kind: ImportantDirKind,
    /// Human-readable reasons why this directory is classified as important.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    /// Estimated number of entries (files + subdirectories), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_entry_count: Option<usize>,
}

/// A short summary of a repository path (e.g. source root, docs directory).
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoPathSummary {
    /// Relative path from the repository root.
    pub path: String,
    /// Human-readable label (e.g. "Source root", "Documentation").
    pub label: String,
    /// Estimated number of entries, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_count: Option<usize>,
}

/// A suggested fetch URL for exploring the repository further.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoMapSuggestedFetch {
    /// The URL to fetch.
    pub url: String,
    /// Human-readable reason for the suggestion.
    pub reason: String,
    /// Suggested fetch priority (1 = highest). Higher values mean lower priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<usize>,
    /// Structured locator for `repo_fetch`, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_repo_fetch: Option<RepoFetchRequest>,
}

/// Request type for the `repo_map` tool.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoMapRequest {
    /// Free-text query. May be empty when a repository locator
    /// (owner+repo) is provided.
    pub query: String,
    /// The code host (GitHub, GitLab, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<CodeHost>,
    /// Repository owner (or namespace for GitLab nested groups).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Branch, tag, or commit ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    /// Full commit SHA, when known. Preferred over `ref_name` for
    /// URL stability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// Maximum number of root entries to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_entries: Option<usize>,
    /// Maximum directory depth to explore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<usize>,
    /// Whether to include file entries in the response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_files: Option<bool>,
    /// Whether to include directory entries in the response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_directories: Option<bool>,
    /// Whether to include CI/CD-related entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_ci: Option<bool>,
    /// Whether to include security-related entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_security: Option<bool>,
    /// Timeout in milliseconds for the overall request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Explicit list of providers to query. When empty, the server
    /// selects providers based on the host and configuration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
}

impl RepoMapRequest {
    /// Validate the request fields.
    pub fn validate(&self) -> Result<(), String> {
        if self.owner.trim().is_empty() {
            return Err("owner must not be empty".to_string());
        }
        if self.repo.trim().is_empty() {
            return Err("repo must not be empty".to_string());
        }
        if let Some(0) = self.max_entries {
            return Err("max_entries must be > 0".to_string());
        }
        if let Some(0) = self.max_depth {
            return Err("max_depth must be > 0".to_string());
        }
        if let Some(0) = self.timeout_ms {
            return Err("timeout_ms must be > 0".to_string());
        }
        Ok(())
    }
}

/// Response type for the `repo_map` tool.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoMapResponse {
    /// The free-text query that was used.
    pub query: String,
    /// The code host for this repository.
    pub host: CodeHost,
    /// Repository owner (or namespace for GitLab nested groups).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Branch, tag, or commit ref used for the response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    /// Full commit SHA, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// The repository's default branch, if determined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    /// The mode used to produce this response.
    pub mode: RepoMapMode,
    /// Root-level entries in the repository.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_entries: Vec<RepoMapEntry>,
    /// Important files classified by the server.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub important_files: Vec<RepoImportantFile>,
    /// Important directories classified by the server.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub important_directories: Vec<RepoImportantDirectory>,
    /// Discovered source root directories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_roots: Vec<RepoPathSummary>,
    /// Discovered documentation directories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<RepoPathSummary>,
    /// Discovered example directories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<RepoPathSummary>,
    /// Discovered test directories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<RepoPathSummary>,
    /// Discovered CI/CD directories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ci: Vec<RepoPathSummary>,
    /// Discovered security-related directory, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<RepoPathSummary>,
    /// Detected package manifest files (Cargo.toml, package.json, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manifests: Vec<RepoImportantFile>,
    /// Suggested fetch URLs for further exploration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_fetches: Vec<RepoMapSuggestedFetch>,
    /// Providers that were queried.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers_queried: Vec<String>,
    /// Providers that failed during the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers_failed: Vec<ProviderFailure>,
    /// Advisory warnings emitted during the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<SearchWarning>,
    /// Structured warnings with stable codes, severity, and context.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_warnings: Vec<AgentWarning>,
    /// Trust and sanitization metadata for the response.
    #[serde(default)]
    pub trust_markers: TrustMarkers,
    /// Local checkout metadata, present when a matching local
    /// repository was found under the configured workspace roots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_checkout: Option<RepoMapLocalCheckout>,
    /// Telemetry for the repo map response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<RepoMapTelemetry>,
}

/// Telemetry for a repo map response.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoMapTelemetry {
    /// Providers that were queried.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers_queried: Vec<String>,
    /// Whether the request timed out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deadline_exceeded: bool,
    /// Human-readable mode description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_reason: Option<String>,
}

/// Local checkout metadata for a repo map response.
///
/// Present when a matching local Git checkout was found under the
/// configured workspace roots.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoMapLocalCheckout {
    /// Root directory name of the local checkout.
    pub root_name: String,
    /// Canonical path to the local checkout root.
    pub root_path: String,
    /// Remote host (e.g. `github`, `gitlab`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_host: Option<String>,
    /// Remote owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_owner: Option<String>,
    /// Remote repo name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_repo: Option<String>,
    /// Current branch of the local checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Current commit SHA.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Working tree dirty state: `clean`, `dirty`, `unknown`.
    pub dirty_state: String,
    /// Detected package manifests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manifests: Vec<RepoMapLocalManifest>,
}

/// A detected package manifest in a local checkout.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoMapLocalManifest {
    /// Manifest file path relative to the repo root.
    pub path: String,
    /// Detected ecosystem (e.g. `crates_io`, `npm`, `pypi`).
    pub ecosystem: String,
    /// Package name, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
}

/// Deterministically classify a file path as an important file.
///
/// The `path` should be the filename (e.g. `"README.md"`) for root
/// files or the full relative path (e.g. `".github/workflows/ci.yml"`).
pub fn classify_important_file(path: &str) -> (ImportantFileKind, Vec<String>) {
    let basename = path.rsplit('/').next().unwrap_or(path);
    let lower_basename = basename.to_lowercase();
    let lower_path = path.to_lowercase();

    let mut reasons = Vec::new();

    // README variants
    if lower_basename == "readme" || lower_basename.starts_with("readme.") {
        reasons.push("readme_file".to_string());
        return (ImportantFileKind::Readme, reasons);
    }

    // Security policy
    if lower_basename == "security.md" || lower_basename == "security.txt" {
        reasons.push("security_policy".to_string());
        return (ImportantFileKind::Security, reasons);
    }

    // Contributing guide
    if lower_basename == "contributing.md" || lower_basename == "contributing" {
        reasons.push("contributing_guide".to_string());
        return (ImportantFileKind::Contributing, reasons);
    }

    // Changelog
    if lower_basename == "changelog" || lower_basename.starts_with("changelog.") {
        reasons.push("changelog_file".to_string());
        return (ImportantFileKind::Changelog, reasons);
    }
    if lower_basename == "changes" || lower_basename.starts_with("changes.") {
        reasons.push("changelog_file".to_string());
        return (ImportantFileKind::Changelog, reasons);
    }
    if lower_basename == "history" || lower_basename.starts_with("history.") {
        reasons.push("changelog_file".to_string());
        return (ImportantFileKind::Changelog, reasons);
    }

    // License
    if lower_basename == "license"
        || lower_basename.starts_with("license.")
        || lower_basename.starts_with("license-")
    {
        reasons.push("license_file".to_string());
        return (ImportantFileKind::License, reasons);
    }
    if lower_basename == "licence"
        || lower_basename.starts_with("licence.")
        || lower_basename.starts_with("licence-")
    {
        reasons.push("license_file".to_string());
        return (ImportantFileKind::License, reasons);
    }
    if lower_basename == "copying" || lower_basename.starts_with("copying.") {
        reasons.push("license_file".to_string());
        return (ImportantFileKind::License, reasons);
    }

    // CI/CD configs
    if lower_path.starts_with(".github/workflows/") {
        reasons.push("github_actions_workflow".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }
    if lower_path == ".gitlab-ci.yml" || lower_path.starts_with(".gitlab-ci/") {
        reasons.push("gitlab_ci_config".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }
    if lower_path.starts_with(".forgejo/workflows/") || lower_path.starts_with(".gitea/workflows/")
    {
        reasons.push("forgejo_gitea_ci_config".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }
    if lower_path == "appveyor.yml" || lower_path == ".appveyor.yml" {
        reasons.push("appveyor_ci_config".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }
    if lower_path == "azure-pipelines.yml" || lower_path == ".azure-pipelines.yml" {
        reasons.push("azure_pipelines_config".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }
    if lower_path == ".circleci/config.yml" {
        reasons.push("circleci_config".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }
    if lower_path == ".travis.yml" {
        reasons.push("travis_ci_config".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }
    if lower_path == "cloudbuild.yaml" || lower_path == "cloudbuild.yml" {
        reasons.push("cloudbuild_config".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }
    if lower_path == "bitbucket-pipelines.yml" {
        reasons.push("bitbucket_pipelines_config".to_string());
        return (ImportantFileKind::CiConfig, reasons);
    }

    // Dockerfile
    if lower_basename == "dockerfile" || lower_basename.starts_with("dockerfile.") {
        reasons.push("dockerfile".to_string());
        return (ImportantFileKind::Dockerfile, reasons);
    }
    if lower_basename == "docker-compose.yml" || lower_basename == "docker-compose.yaml" {
        reasons.push("docker_compose".to_string());
        return (ImportantFileKind::Dockerfile, reasons);
    }
    if lower_basename == "compose.yml" || lower_basename == "compose.yaml" {
        reasons.push("docker_compose".to_string());
        return (ImportantFileKind::Dockerfile, reasons);
    }
    if lower_path == ".dockerignore" {
        reasons.push("docker_ignore".to_string());
        return (ImportantFileKind::Dockerfile, reasons);
    }

    // Lockfiles
    if lower_basename == "cargo.lock"
        || lower_basename == "package-lock.json"
        || lower_basename == "yarn.lock"
        || lower_basename == "pnpm-lock.yaml"
        || lower_basename == "poetry.lock"
        || lower_basename == "pnpm-lock.yml"
        || lower_basename == "bun.lockb"
    {
        reasons.push("lockfile".to_string());
        return (ImportantFileKind::Lockfile, reasons);
    }
    if lower_path == "composer.lock"
        || lower_path == "go.sum"
        || lower_path == "Gemfile.lock"
        || lower_path == "mix.lock"
        || lower_path == "shrinkwrap.json"
    {
        reasons.push("lockfile".to_string());
        return (ImportantFileKind::Lockfile, reasons);
    }

    // Manifests
    if lower_basename == "cargo.toml" {
        reasons.push("rust_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "package.json" {
        reasons.push("nodejs_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "pyproject.toml"
        || lower_basename == "setup.py"
        || lower_basename == "setup.cfg"
    {
        reasons.push("python_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "go.mod" {
        reasons.push("go_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "gemspec" || lower_basename.ends_with(".gemspec") {
        reasons.push("ruby_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "gemfile" {
        reasons.push("ruby_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "mix.exs" {
        reasons.push("elixir_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "composer.json" {
        reasons.push("php_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "build.gradle" || lower_basename == "build.gradle.kts" {
        reasons.push("jvm_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "pom.xml" {
        reasons.push("jvm_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "pubspec.yaml" {
        reasons.push("dart_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "elm.json" {
        reasons.push("elm_manifest".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "requirements.txt" || lower_basename == "requirements.in" {
        reasons.push("python_requirements".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }
    if lower_basename == "pipfile" {
        reasons.push("python_pipenv".to_string());
        return (ImportantFileKind::Manifest, reasons);
    }

    // Editor configs
    if lower_basename == ".editorconfig" {
        reasons.push("editor_config".to_string());
        return (ImportantFileKind::EditorConfig, reasons);
    }
    if lower_path.starts_with(".vscode/") {
        reasons.push("vscode_config".to_string());
        return (ImportantFileKind::EditorConfig, reasons);
    }
    if lower_path.starts_with(".idea/") {
        reasons.push("jetbrains_config".to_string());
        return (ImportantFileKind::EditorConfig, reasons);
    }

    // Gitignore and git config
    if lower_basename == ".gitignore" || lower_basename == ".gitignore_global" {
        reasons.push("gitignore".to_string());
        return (ImportantFileKind::GitIgnore, reasons);
    }
    if lower_basename == ".gitattributes" {
        reasons.push("git_attributes".to_string());
        return (ImportantFileKind::GitIgnore, reasons);
    }

    // Build scripts and Makefiles
    if lower_basename == "makefile" || lower_basename == "gnumakefile" {
        reasons.push("makefile".to_string());
        return (ImportantFileKind::BuildScript, reasons);
    }
    if lower_basename == "justfile" || lower_basename == "justfile.lock" {
        reasons.push("justfile".to_string());
        return (ImportantFileKind::BuildScript, reasons);
    }
    if lower_basename == "cmakelists.txt" {
        reasons.push("cmake_build".to_string());
        return (ImportantFileKind::BuildScript, reasons);
    }
    if lower_path == "build.rs" || lower_path == "build.zig" || lower_path == "build.zig.zon" {
        reasons.push("build_script".to_string());
        return (ImportantFileKind::BuildScript, reasons);
    }

    // Documentation
    if lower_basename == "docs" || lower_path.starts_with("docs/") {
        reasons.push("documentation_directory".to_string());
        return (ImportantFileKind::Documentation, reasons);
    }
    if lower_basename == "book.toml" {
        reasons.push("mdbook_config".to_string());
        return (ImportantFileKind::Documentation, reasons);
    }

    (ImportantFileKind::Unknown, reasons)
}

/// Deterministically classify a directory path as an important directory.
///
/// The `path` should be a relative directory path (e.g. `"src"`,
/// `".github/workflows"`, `"internal/pkg"`).
pub fn classify_important_directory(path: &str) -> (ImportantDirKind, Vec<String>) {
    let lower = path.to_lowercase();
    let mut reasons = Vec::new();

    // Source roots
    if lower == "src"
        || lower == "lib"
        || lower == "crates"
        || lower == "packages"
        || lower == "apps"
        || lower == "cmd"
        || lower == "internal"
        || lower == "pkg"
    {
        reasons.push("well_known_source_directory".to_string());
        return (ImportantDirKind::SourceRoot, reasons);
    }

    // Examples / samples
    if lower == "examples" || lower == "samples" || lower == "demo" || lower == "demos" {
        reasons.push("examples_directory".to_string());
        return (ImportantDirKind::Examples, reasons);
    }

    // Tests
    if lower == "tests"
        || lower == "test"
        || lower == "spec"
        || lower == "specs"
        || lower == "benches"
        || lower == "benchmarks"
        || lower == "__tests__"
    {
        reasons.push("test_directory".to_string());
        return (ImportantDirKind::Tests, reasons);
    }

    // Docs
    if lower == "docs"
        || lower == "doc"
        || lower == "website"
        || lower == "book"
        || lower == "content"
        || lower == "wiki"
    {
        reasons.push("documentation_directory".to_string());
        return (ImportantDirKind::Docs, reasons);
    }

    // CI/CD directories
    if lower == ".github/workflows" || lower == ".github/actions" {
        reasons.push("github_actions_directory".to_string());
        return (ImportantDirKind::CiConfig, reasons);
    }
    if lower == ".github" {
        reasons.push("github_config_directory".to_string());
        return (ImportantDirKind::CiConfig, reasons);
    }
    if lower == ".gitlab-ci" || lower == ".gitlab" {
        reasons.push("gitlab_ci_directory".to_string());
        return (ImportantDirKind::CiConfig, reasons);
    }
    if lower == ".forgejo" || lower == ".gitea" {
        reasons.push("forgejo_gitea_ci_directory".to_string());
        return (ImportantDirKind::CiConfig, reasons);
    }

    // Security-related directories
    if lower == "security" || lower == "vulnerabilities" || lower == "advisories" {
        reasons.push("security_directory".to_string());
        return (ImportantDirKind::Security, reasons);
    }

    // Config directories
    if lower == ".config" || lower == "config" || lower == "conf" || lower == "cfg" {
        reasons.push("config_directory".to_string());
        return (ImportantDirKind::Config, reasons);
    }

    // Generated / vendored directories
    if lower == "target"
        || lower == "node_modules"
        || lower == "vendor"
        || lower == "__pycache__"
        || lower == ".next"
        || lower == ".nuxt"
        || lower == "dist"
        || lower == "build"
        || lower == "out"
        || lower == ".gradle"
        || lower == ".m2"
    {
        reasons.push("generated_directory".to_string());
        return (ImportantDirKind::Generated, reasons);
    }

    (ImportantDirKind::Unknown, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_important_file_readme() {
        let (kind, reasons) = classify_important_file("README.md");
        assert_eq!(kind, ImportantFileKind::Readme);
        assert!(reasons.contains(&"readme_file".to_string()));
    }

    #[test]
    fn classify_important_file_readme_uppercase() {
        let (kind, reasons) = classify_important_file("README");
        assert_eq!(kind, ImportantFileKind::Readme);
        assert!(reasons.contains(&"readme_file".to_string()));
    }

    #[test]
    fn classify_important_file_cargo_toml() {
        let (kind, reasons) = classify_important_file("Cargo.toml");
        assert_eq!(kind, ImportantFileKind::Manifest);
        assert!(reasons.contains(&"rust_manifest".to_string()));
    }

    #[test]
    fn classify_important_file_package_json() {
        let (kind, reasons) = classify_important_file("package.json");
        assert_eq!(kind, ImportantFileKind::Manifest);
        assert!(reasons.contains(&"nodejs_manifest".to_string()));
    }

    #[test]
    fn classify_important_file_pyproject_toml() {
        let (kind, reasons) = classify_important_file("pyproject.toml");
        assert_eq!(kind, ImportantFileKind::Manifest);
        assert!(reasons.contains(&"python_manifest".to_string()));
    }

    #[test]
    fn classify_important_file_dockerfile() {
        let (kind, reasons) = classify_important_file("Dockerfile");
        assert_eq!(kind, ImportantFileKind::Dockerfile);
        assert!(reasons.contains(&"dockerfile".to_string()));
    }

    #[test]
    fn classify_important_file_docker_compose() {
        let (kind, reasons) = classify_important_file("docker-compose.yml");
        assert_eq!(kind, ImportantFileKind::Dockerfile);
        assert!(reasons.contains(&"docker_compose".to_string()));
    }

    #[test]
    fn classify_important_file_changelog() {
        let (kind, reasons) = classify_important_file("CHANGELOG.md");
        assert_eq!(kind, ImportantFileKind::Changelog);
        assert!(reasons.contains(&"changelog_file".to_string()));
    }

    #[test]
    fn classify_important_file_license() {
        let (kind, reasons) = classify_important_file("LICENSE");
        assert_eq!(kind, ImportantFileKind::License);
        assert!(reasons.contains(&"license_file".to_string()));
    }

    #[test]
    fn classify_important_file_license_mit() {
        let (kind, reasons) = classify_important_file("LICENSE-MIT");
        assert_eq!(kind, ImportantFileKind::License);
        assert!(reasons.contains(&"license_file".to_string()));
    }

    #[test]
    fn classify_important_file_contributing() {
        let (kind, reasons) = classify_important_file("CONTRIBUTING.md");
        assert_eq!(kind, ImportantFileKind::Contributing);
        assert!(reasons.contains(&"contributing_guide".to_string()));
    }

    #[test]
    fn classify_important_file_security() {
        let (kind, reasons) = classify_important_file("SECURITY.md");
        assert_eq!(kind, ImportantFileKind::Security);
        assert!(reasons.contains(&"security_policy".to_string()));
    }

    #[test]
    fn classify_important_file_github_workflow() {
        let (kind, reasons) = classify_important_file(".github/workflows/ci.yml");
        assert_eq!(kind, ImportantFileKind::CiConfig);
        assert!(reasons.contains(&"github_actions_workflow".to_string()));
    }

    #[test]
    fn classify_important_file_gitlab_ci() {
        let (kind, reasons) = classify_important_file(".gitlab-ci.yml");
        assert_eq!(kind, ImportantFileKind::CiConfig);
        assert!(reasons.contains(&"gitlab_ci_config".to_string()));
    }

    #[test]
    fn classify_important_file_travis() {
        let (kind, reasons) = classify_important_file(".travis.yml");
        assert_eq!(kind, ImportantFileKind::CiConfig);
        assert!(reasons.contains(&"travis_ci_config".to_string()));
    }

    #[test]
    fn classify_important_file_cargo_lock() {
        let (kind, reasons) = classify_important_file("Cargo.lock");
        assert_eq!(kind, ImportantFileKind::Lockfile);
        assert!(reasons.contains(&"lockfile".to_string()));
    }

    #[test]
    fn classify_important_file_package_lock() {
        let (kind, reasons) = classify_important_file("package-lock.json");
        assert_eq!(kind, ImportantFileKind::Lockfile);
        assert!(reasons.contains(&"lockfile".to_string()));
    }

    #[test]
    fn classify_important_file_editorconfig() {
        let (kind, reasons) = classify_important_file(".editorconfig");
        assert_eq!(kind, ImportantFileKind::EditorConfig);
        assert!(reasons.contains(&"editor_config".to_string()));
    }

    #[test]
    fn classify_important_file_gitignore() {
        let (kind, reasons) = classify_important_file(".gitignore");
        assert_eq!(kind, ImportantFileKind::GitIgnore);
        assert!(reasons.contains(&"gitignore".to_string()));
    }

    #[test]
    fn classify_important_file_makefile() {
        let (kind, reasons) = classify_important_file("Makefile");
        assert_eq!(kind, ImportantFileKind::BuildScript);
        assert!(reasons.contains(&"makefile".to_string()));
    }

    #[test]
    fn classify_important_file_build_rs() {
        let (kind, _) = classify_important_file("build.rs");
        assert_eq!(kind, ImportantFileKind::BuildScript);
    }

    #[test]
    fn classify_important_file_unknown() {
        let (kind, reasons) = classify_important_file("foo.xyz");
        assert_eq!(kind, ImportantFileKind::Unknown);
        assert!(reasons.is_empty());
    }

    #[test]
    fn classify_important_directory_src() {
        let (kind, reasons) = classify_important_directory("src");
        assert_eq!(kind, ImportantDirKind::SourceRoot);
        assert!(reasons.contains(&"well_known_source_directory".to_string()));
    }

    #[test]
    fn classify_important_directory_lib() {
        let (kind, reasons) = classify_important_directory("lib");
        assert_eq!(kind, ImportantDirKind::SourceRoot);
        assert!(reasons.contains(&"well_known_source_directory".to_string()));
    }

    #[test]
    fn classify_important_directory_crates() {
        let (kind, _) = classify_important_directory("crates");
        assert_eq!(kind, ImportantDirKind::SourceRoot);
    }

    #[test]
    fn classify_important_directory_examples() {
        let (kind, reasons) = classify_important_directory("examples");
        assert_eq!(kind, ImportantDirKind::Examples);
        assert!(reasons.contains(&"examples_directory".to_string()));
    }

    #[test]
    fn classify_important_directory_samples() {
        let (kind, _) = classify_important_directory("samples");
        assert_eq!(kind, ImportantDirKind::Examples);
    }

    #[test]
    fn classify_important_directory_tests() {
        let (kind, reasons) = classify_important_directory("tests");
        assert_eq!(kind, ImportantDirKind::Tests);
        assert!(reasons.contains(&"test_directory".to_string()));
    }

    #[test]
    fn classify_important_directory_benches() {
        let (kind, _) = classify_important_directory("benches");
        assert_eq!(kind, ImportantDirKind::Tests);
    }

    #[test]
    fn classify_important_directory_docs() {
        let (kind, reasons) = classify_important_directory("docs");
        assert_eq!(kind, ImportantDirKind::Docs);
        assert!(reasons.contains(&"documentation_directory".to_string()));
    }

    #[test]
    fn classify_important_directory_website() {
        let (kind, _) = classify_important_directory("website");
        assert_eq!(kind, ImportantDirKind::Docs);
    }

    #[test]
    fn classify_important_directory_github_workflows() {
        let (kind, reasons) = classify_important_directory(".github/workflows");
        assert_eq!(kind, ImportantDirKind::CiConfig);
        assert!(reasons.contains(&"github_actions_directory".to_string()));
    }

    #[test]
    fn classify_important_directory_github_root() {
        let (kind, reasons) = classify_important_directory(".github");
        assert_eq!(kind, ImportantDirKind::CiConfig);
        assert!(reasons.contains(&"github_config_directory".to_string()));
    }

    #[test]
    fn classify_important_directory_security() {
        let (kind, _) = classify_important_directory("security");
        assert_eq!(kind, ImportantDirKind::Security);
    }

    #[test]
    fn classify_important_directory_config() {
        let (kind, _) = classify_important_directory(".config");
        assert_eq!(kind, ImportantDirKind::Config);
    }

    #[test]
    fn classify_important_directory_generated() {
        let (kind, _) = classify_important_directory("node_modules");
        assert_eq!(kind, ImportantDirKind::Generated);
    }

    #[test]
    fn classify_important_directory_unknown() {
        let (kind, reasons) = classify_important_directory("some_random_dir");
        assert_eq!(kind, ImportantDirKind::Unknown);
        assert!(reasons.is_empty());
    }

    #[test]
    fn validate_empty_owner() {
        let req = RepoMapRequest {
            owner: " ".to_string(),
            repo: "repo".to_string(),
            ..Default::default()
        };
        let err = req.validate().unwrap_err();
        assert!(err.contains("owner"));
    }

    #[test]
    fn validate_empty_repo() {
        let req = RepoMapRequest {
            owner: "owner".to_string(),
            repo: "  ".to_string(),
            ..Default::default()
        };
        let err = req.validate().unwrap_err();
        assert!(err.contains("repo"));
    }

    #[test]
    fn validate_zero_max_entries() {
        let req = RepoMapRequest {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            max_entries: Some(0),
            ..Default::default()
        };
        let err = req.validate().unwrap_err();
        assert!(err.contains("max_entries"));
    }

    #[test]
    fn validate_zero_max_depth() {
        let req = RepoMapRequest {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            max_depth: Some(0),
            ..Default::default()
        };
        let err = req.validate().unwrap_err();
        assert!(err.contains("max_depth"));
    }

    #[test]
    fn validate_zero_timeout_ms() {
        let req = RepoMapRequest {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            timeout_ms: Some(0),
            ..Default::default()
        };
        let err = req.validate().unwrap_err();
        assert!(err.contains("timeout_ms"));
    }

    #[test]
    fn validate_valid_request() {
        let req = RepoMapRequest {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            max_entries: Some(50),
            max_depth: Some(3),
            ..Default::default()
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn validate_default_is_valid() {
        let req = RepoMapRequest::default();
        // owner and repo are empty strings, which are not trimmed-empty
        // actually "".trim() == "" is true, so default fails validation
        // This is expected — callers must set owner and repo.
        assert!(req.validate().is_err());
    }

    #[test]
    fn repo_map_response_serde_roundtrip() {
        let resp = RepoMapResponse {
            query: "test query".to_string(),
            host: CodeHost::Github,
            owner: "test-org".to_string(),
            repo: "test-repo".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: Some("abc123".to_string()),
            default_branch: Some("main".to_string()),
            mode: RepoMapMode::Native,
            root_entries: vec![RepoMapEntry {
                path: "src".to_string(),
                kind: RepoMapEntryKind::Directory,
                size: None,
                language: None,
            }],
            important_files: vec![RepoImportantFile {
                path: "Cargo.toml".to_string(),
                kind: ImportantFileKind::Manifest,
                reasons: vec!["rust_manifest".to_string()],
                size: Some(1024),
            }],
            important_directories: vec![RepoImportantDirectory {
                path: "src".to_string(),
                kind: ImportantDirKind::SourceRoot,
                reasons: vec!["well_known_source_directory".to_string()],
                estimated_entry_count: Some(10),
            }],
            source_roots: vec![RepoPathSummary {
                path: "src".to_string(),
                label: "Source root".to_string(),
                entry_count: Some(10),
            }],
            docs: vec![],
            examples: vec![],
            tests: vec![],
            ci: vec![],
            security: None,
            manifests: vec![],
            suggested_fetches: vec![RepoMapSuggestedFetch {
                url: "https://example.com".to_string(),
                reason: "test".to_string(),
                priority: Some(1),
                structured_repo_fetch: None,
            }],
            providers_queried: vec!["github".to_string()],
            providers_failed: vec![],
            warnings: vec![],
            structured_warnings: vec![],
            trust_markers: TrustMarkers::default(),
            local_checkout: None,
            telemetry: None,
        };

        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: RepoMapResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host, CodeHost::Github);
        assert_eq!(deserialized.owner, "test-org");
        assert_eq!(deserialized.repo, "test-repo");
        assert_eq!(deserialized.root_entries.len(), 1);
        assert_eq!(deserialized.important_files.len(), 1);
        assert_eq!(deserialized.important_directories.len(), 1);
        assert_eq!(deserialized.source_roots.len(), 1);
        assert_eq!(deserialized.suggested_fetches.len(), 1);
        assert_eq!(deserialized.providers_queried, vec!["github"]);
    }

    #[test]
    fn repo_map_request_serde_roundtrip() {
        let req = RepoMapRequest {
            query: "find docs".to_string(),
            host: Some(CodeHost::Gitlab),
            owner: "org".to_string(),
            repo: "project".to_string(),
            ref_name: Some("develop".to_string()),
            commit_sha: None,
            max_entries: Some(100),
            max_depth: Some(4),
            include_files: Some(true),
            include_directories: Some(true),
            include_ci: Some(false),
            include_security: Some(true),
            timeout_ms: Some(5000),
            providers: vec!["gitlab".to_string()],
        };

        let json = serde_json::to_string(&req).unwrap();
        let deserialized: RepoMapRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.query, "find docs");
        assert_eq!(deserialized.host, Some(CodeHost::Gitlab));
        assert_eq!(deserialized.owner, "org");
        assert_eq!(deserialized.repo, "project");
        assert_eq!(deserialized.ref_name, Some("develop".to_string()));
        assert_eq!(deserialized.max_entries, Some(100));
        assert_eq!(deserialized.max_depth, Some(4));
        assert_eq!(deserialized.include_files, Some(true));
        assert_eq!(deserialized.include_ci, Some(false));
        assert_eq!(deserialized.providers, vec!["gitlab"]);
    }

    #[test]
    fn classify_important_file_terraform() {
        let (kind, _) = classify_important_file("main.tf");
        assert_eq!(kind, ImportantFileKind::Unknown);
    }

    #[test]
    fn classify_important_file_go_mod() {
        let (kind, reasons) = classify_important_file("go.mod");
        assert_eq!(kind, ImportantFileKind::Manifest);
        assert!(reasons.contains(&"go_manifest".to_string()));
    }

    #[test]
    fn classify_important_file_compose_yml() {
        let (kind, reasons) = classify_important_file("compose.yml");
        assert_eq!(kind, ImportantFileKind::Dockerfile);
        assert!(reasons.contains(&"docker_compose".to_string()));
    }

    #[test]
    fn classify_important_directory_node_modules() {
        let (kind, _) = classify_important_directory("node_modules");
        assert_eq!(kind, ImportantDirKind::Generated);
    }

    #[test]
    fn classify_important_directory_pypi_cache() {
        let (kind, _) = classify_important_directory("__pycache__");
        assert_eq!(kind, ImportantDirKind::Generated);
    }

    #[test]
    fn classify_important_directory_vendor() {
        let (kind, _) = classify_important_directory("vendor");
        assert_eq!(kind, ImportantDirKind::Generated);
    }

    #[test]
    fn classify_important_file_readme_txt() {
        let (kind, _) = classify_important_file("readme.txt");
        assert_eq!(kind, ImportantFileKind::Readme);
    }

    #[test]
    fn classify_important_directory_test() {
        let (kind, _) = classify_important_directory("test");
        assert_eq!(kind, ImportantDirKind::Tests);
    }

    #[test]
    fn classify_important_directory_spec() {
        let (kind, _) = classify_important_directory("spec");
        assert_eq!(kind, ImportantDirKind::Tests);
    }
}
