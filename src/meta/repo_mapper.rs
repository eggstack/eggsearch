//! Repository map planning and classification.
//!
//! Provides deterministic classification of repository structure
//! entries and suggested-fetch generation for the `repo_map` MCP tool.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::core::code_evidence::{infer_source_role, SourceRole};
use crate::core::code_metadata::CodeHost;
use crate::core::local::{is_binary_extension, language_from_extension, LocalConfig, SKIP_DIRS};
use crate::core::repo_fetch::RepoFetchRequest;
use crate::core::repo_map::{
    classify_important_directory, classify_important_file, ImportantDirKind, ImportantFileKind,
    RepoEntrypoint, RepoImportantDirectory, RepoImportantFile, RepoLanguageDistribution,
    RepoMapEntry, RepoMapEntryKind, RepoMapMode, RepoMapRequest, RepoMapResponse,
    RepoMapSuggestedFetch, RepoModuleSummary, RepoPackageSummary, RepoPathSummary,
    RepoTestRelationship, RepoTopSymbol,
};
use crate::core::result::SearchWarning;
use crate::core::sanitize::TrustMarkers;
use crate::meta::local_symbols::{self, TestHintConfidence};

const MAX_SUGGESTED_FETCHES: usize = 8;
const STRUCTURE_SCAN_FILE_CAP: usize = 2_000;
const STRUCTURE_SCAN_DEPTH: usize = 4;
const STRUCTURE_MANIFEST_READ_CAP: usize = 65_536;
const STRUCTURE_SYMBOLS_PER_FILE: usize = 32;
const STRUCTURE_TEST_FILE_CAP: usize = 100;
const STRUCTURE_SOURCE_FILE_CAP: usize = 100;
const STRUCTURE_RELATIONSHIP_CAP: usize = 100;

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
                structured_repo_fetch: structured.clone(),
                batch_item: structured
                    .as_ref()
                    .map(crate::core::fetch_locator::structured_repo_fetch_to_batch_item),
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
                structured_repo_fetch: structured.clone(),
                batch_item: structured
                    .as_ref()
                    .map(crate::core::fetch_locator::structured_repo_fetch_to_batch_item),
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
            structured_repo_fetch: structured.clone(),
            batch_item: structured
                .as_ref()
                .map(crate::core::fetch_locator::structured_repo_fetch_to_batch_item),
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
            structured_repo_fetch: structured.clone(),
            batch_item: structured
                .as_ref()
                .map(crate::core::fetch_locator::structured_repo_fetch_to_batch_item),
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
                structured_repo_fetch: structured.clone(),
                batch_item: structured
                    .as_ref()
                    .map(crate::core::fetch_locator::structured_repo_fetch_to_batch_item),
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
                structured_repo_fetch: structured.clone(),
                batch_item: structured
                    .as_ref()
                    .map(crate::core::fetch_locator::structured_repo_fetch_to_batch_item),
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
            structured_repo_fetch: structured.clone(),
            batch_item: structured
                .as_ref()
                .map(crate::core::fetch_locator::structured_repo_fetch_to_batch_item),
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
            structured_repo_fetch: structured.clone(),
            batch_item: structured
                .as_ref()
                .map(crate::core::fetch_locator::structured_repo_fetch_to_batch_item),
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

/// Bounded structural enrichment for a local checkout.
///
/// Scans at most `STRUCTURE_SCAN_FILE_CAP` files to depth
/// `STRUCTURE_SCAN_DEPTH`, parses at most `config.max_structured_files`
/// files with the deterministic structured parser, and caps total retained
/// structural entries at `config.repo_map_structure_cap`. Budget breaches
/// set `structure_truncated` rather than failing the request. Never executes
/// workspace code.
pub fn populate_structure_from_local_checkout(
    response: &mut RepoMapResponse,
    root: &Path,
    config: &LocalConfig,
) {
    let structure = build_local_structure(root, config);
    response.packages = structure.packages;
    response.language_distribution = structure.language_distribution;
    response.modules = structure.modules;
    response.entrypoints = structure.entrypoints;
    response.top_symbols = structure.top_symbols;
    response.test_relationships = structure.test_relationships;
    response.build_configs = structure.build_configs;
    response.structure_truncated = structure.truncated;
}

/// Structural enrichment result for a local checkout scan.
pub struct LocalStructure {
    /// Workspace package boundaries.
    pub packages: Vec<RepoPackageSummary>,
    /// Language distribution.
    pub language_distribution: Vec<RepoLanguageDistribution>,
    /// Major modules.
    pub modules: Vec<RepoModuleSummary>,
    /// Entrypoint candidates.
    pub entrypoints: Vec<RepoEntrypoint>,
    /// Top structured symbols.
    pub top_symbols: Vec<RepoTopSymbol>,
    /// Source-to-test hints.
    pub test_relationships: Vec<RepoTestRelationship>,
    /// Build/CI configs.
    pub build_configs: Vec<RepoImportantFile>,
    /// Whether any explicit cap truncated the scan.
    pub truncated: bool,
}

struct ScannedFile {
    relative: String,
    size: u64,
    language: Option<String>,
}

fn manifest_ecosystem(file_name: &str) -> Option<(&'static str, &'static str)> {
    let lower = file_name.to_lowercase();
    match lower.as_str() {
        "cargo.toml" => Some(("rust", "rust")),
        "package.json" => Some(("npm", "npm")),
        "pyproject.toml" | "setup.py" | "setup.cfg" | "requirements.txt" | "pipfile" => {
            Some(("python", "python"))
        }
        "go.mod" => Some(("go", "go")),
        "pom.xml" | "build.gradle" | "build.gradle.kts" => Some(("jvm", "jvm")),
        "gemfile" => Some(("ruby", "ruby")),
        "composer.json" => Some(("php", "php")),
        "mix.exs" => Some(("elixir", "elixir")),
        _ if lower.ends_with(".gemspec") => Some(("ruby", "ruby")),
        _ => None,
    }
}

fn parse_package_name(root: &Path, relative: &str, ecosystem: &str) -> Option<String> {
    let bytes = std::fs::read(root.join(relative)).ok()?;
    if bytes.len() > STRUCTURE_MANIFEST_READ_CAP {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes);
    match ecosystem {
        "rust" => {
            let mut in_package = false;
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    in_package = trimmed == "[package]";
                } else if in_package && trimmed.starts_with("name") {
                    return trimmed
                        .split('=')
                        .nth(1)
                        .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
                        .filter(|v| !v.is_empty());
                }
            }
            None
        }
        "npm" => serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string)),
        "python" if relative.to_lowercase().ends_with("pyproject.toml") => {
            let mut in_project = false;
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    in_project = trimmed == "[project]";
                } else if in_project && trimmed.starts_with("name") {
                    return trimmed
                        .split('=')
                        .nth(1)
                        .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
                        .filter(|v| !v.is_empty());
                }
            }
            None
        }
        "go" => text
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("module "))
            .and_then(|line| line.split_whitespace().nth(1).map(str::to_string)),
        _ => None,
    }
}

fn entrypoint_reason(relative: &str) -> Option<&'static str> {
    match relative {
        "src/main.rs" | "main.rs" => Some("rust_main"),
        "src/lib.rs" | "lib.rs" => Some("rust_lib"),
        "src/main.py" | "main.py" | "__main__.py" => Some("python_main"),
        "app.py" | "src/app.py" => Some("python_app"),
        "index.js" | "src/index.js" => Some("node_entry"),
        "index.ts" | "src/index.ts" | "src/main.ts" => Some("node_entry"),
        "cmd/main.go" | "main.go" => Some("go_main"),
        _ => None,
    }
}

fn is_structured_language(language: Option<&str>) -> bool {
    matches!(
        language,
        Some("rust" | "python" | "javascript" | "typescript" | "go")
    )
}

fn confidence_label(confidence: TestHintConfidence) -> String {
    match confidence {
        TestHintConfidence::Syntax => "syntax".to_string(),
        TestHintConfidence::Path => "path".to_string(),
        TestHintConfidence::NameReference => "name_reference".to_string(),
        TestHintConfidence::Package => "package".to_string(),
    }
}

/// Build bounded structural summaries for a local checkout directory.
pub fn build_local_structure(root: &Path, config: &LocalConfig) -> LocalStructure {
    let mut files: Vec<ScannedFile> = Vec::new();
    let mut truncated = false;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if files.len() >= STRUCTURE_SCAN_FILE_CAP.min(config.max_indexed_files) {
            truncated = true;
            break;
        }
        let read_dir = match std::fs::read_dir(&dir) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let mut entries: Vec<_> = read_dir.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if files.len() >= STRUCTURE_SCAN_FILE_CAP.min(config.max_indexed_files) {
                truncated = true;
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !config.include_hidden && name.starts_with('.') {
                continue;
            }
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            let path = entry.path();
            if !config.follow_symlinks {
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.file_type().is_symlink() {
                        continue;
                    }
                }
            }
            if path.is_dir() {
                if depth < STRUCTURE_SCAN_DEPTH {
                    stack.push((path, depth + 1));
                } else {
                    truncated = true;
                }
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let relative = match path.strip_prefix(root) {
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => continue,
            };
            if is_binary_extension(&name) {
                continue;
            }
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            if size > config.max_file_bytes as u64 {
                continue;
            }
            let language = language_from_extension(&relative).map(str::to_string);
            files.push(ScannedFile {
                relative,
                size,
                language,
            });
        }
    }
    files.sort_by(|a, b| a.relative.cmp(&b.relative));

    let mut packages = Vec::new();
    let mut lang_counts: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    let mut module_files: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut build_configs = Vec::new();

    for file in &files {
        if let Some(lang) = file.language.as_deref() {
            let entry = lang_counts.entry(lang.to_string()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += file.size;
        }
        let top_dir = file.relative.split('/').next().unwrap_or("").to_string();
        if !top_dir.is_empty() && !top_dir.contains('.') {
            module_files.entry(top_dir).or_default().push(0);
        }
        let file_name = file.relative.rsplit('/').next().unwrap_or(&file.relative);
        if manifest_ecosystem(file_name).is_some() {
            let (ecosystem, _) = manifest_ecosystem(file_name).unwrap_or(("other", "other"));
            let name = parse_package_name(root, &file.relative, ecosystem);
            packages.push(RepoPackageSummary {
                path: file.relative.clone(),
                ecosystem: ecosystem.to_string(),
                name,
            });
        }
        let (kind, reasons) = classify_important_file(&file.relative);
        if matches!(
            kind,
            ImportantFileKind::CiConfig
                | ImportantFileKind::BuildScript
                | ImportantFileKind::Dockerfile
        ) {
            build_configs.push(RepoImportantFile {
                path: file.relative.clone(),
                kind,
                reasons,
                size: Some(file.size),
            });
        }
    }
    packages.sort_by(|a, b| a.path.cmp(&b.path));
    packages.truncate(32);
    build_configs.sort_by(|a, b| a.path.cmp(&b.path));
    build_configs.truncate(32);

    let mut language_distribution: Vec<RepoLanguageDistribution> = lang_counts
        .into_iter()
        .map(|(language, (count, bytes))| RepoLanguageDistribution {
            language,
            files: count,
            bytes: Some(bytes),
        })
        .collect();
    language_distribution.sort_by(|a, b| {
        b.files
            .cmp(&a.files)
            .then_with(|| a.language.cmp(&b.language))
    });
    language_distribution.truncate(16);

    let file_set: std::collections::HashSet<&str> =
        files.iter().map(|f| f.relative.as_str()).collect();
    let mut entrypoints: Vec<RepoEntrypoint> = Vec::new();
    for file in &files {
        if let Some(reason) = entrypoint_reason(&file.relative) {
            entrypoints.push(RepoEntrypoint {
                path: file.relative.clone(),
                reason: reason.to_string(),
            });
        }
    }
    if file_set.contains("Cargo.toml") && !entrypoints.iter().any(|e| e.reason.starts_with("rust"))
    {
        if let Some(f) = files.iter().find(|f| f.relative.ends_with("main.rs")) {
            entrypoints.push(RepoEntrypoint {
                path: f.relative.clone(),
                reason: "rust_main".to_string(),
            });
        }
    }
    entrypoints.sort_by(|a, b| a.path.cmp(&b.path));
    entrypoints.truncate(16);

    let parse_cap = config.max_structured_files.min(files.len());
    let per_file_cap = STRUCTURE_SYMBOLS_PER_FILE.min(config.max_symbols_per_file);
    let total_cap = config.repo_map_structure_cap.max(1);
    let mut top_symbols: Vec<RepoTopSymbol> = Vec::new();
    let mut module_symbol_counts: HashMap<String, usize> = HashMap::new();
    let mut parsed = 0usize;
    for file in &files {
        if top_symbols.len() >= total_cap {
            truncated = true;
            break;
        }
        if parsed >= parse_cap {
            truncated = parsed
                < files
                    .iter()
                    .filter(|f| is_structured_language(f.language.as_deref()))
                    .count();
            break;
        }
        if !is_structured_language(file.language.as_deref()) {
            continue;
        }
        parsed += 1;
        let bytes = match std::fs::read(root.join(&file.relative)) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if bytes.len() > config.max_parse_bytes {
            truncated = true;
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let (symbols, provenance, file_truncated) = local_symbols::symbols_in_file(
            &file.relative,
            file.language.as_deref(),
            &text,
            config.max_parse_bytes,
            per_file_cap,
        );
        if file_truncated {
            truncated = true;
        }
        if provenance != local_symbols::SymbolProvenance::Structured {
            continue;
        }
        let top_dir = file.relative.split('/').next().unwrap_or("").to_string();
        for symbol in symbols {
            if top_symbols.len() >= total_cap {
                truncated = true;
                break;
            }
            if symbol.relationship.as_deref() == Some("import") {
                continue;
            }
            if symbol.kind == crate::core::code_evidence::SymbolKind::Module
                && symbol.relationship.as_deref() == Some("import")
            {
                continue;
            }
            *module_symbol_counts.entry(top_dir.clone()).or_insert(0) += 1;
            top_symbols.push(RepoTopSymbol {
                path: file.relative.clone(),
                language: file.language.clone(),
                name: symbol.name,
                kind: symbol.kind,
                line: symbol.line_start,
                container: symbol.container,
            });
        }
    }
    top_symbols.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.line.cmp(&b.line)));
    if top_symbols.len() > total_cap {
        top_symbols.truncate(total_cap);
        truncated = true;
    }

    let mut modules: Vec<RepoModuleSummary> = Vec::new();
    for (dir, indices) in &module_files {
        let dir_prefix = format!("{dir}/");
        let mut lang_tally: HashMap<String, usize> = HashMap::new();
        let mut count = 0usize;
        for file in files
            .iter()
            .filter(|f| f.relative.starts_with(&dir_prefix) || f.relative == *dir)
        {
            if file.language.is_some() {
                count += 1;
                if let Some(lang) = file.language.as_deref() {
                    *lang_tally.entry(lang.to_string()).or_insert(0) += 1;
                }
            }
        }
        let _ = indices;
        if count == 0 {
            continue;
        }
        let dominant = lang_tally
            .into_iter()
            .max_by_key(|(_, c)| *c)
            .map(|(l, _)| l);
        modules.push(RepoModuleSummary {
            path: dir.clone(),
            language: dominant,
            file_count: Some(count),
            symbol_count: module_symbol_counts.get(dir).copied(),
        });
    }
    modules.sort_by(|a, b| {
        b.file_count
            .unwrap_or(0)
            .cmp(&a.file_count.unwrap_or(0))
            .then_with(|| a.path.cmp(&b.path))
    });
    modules.truncate(32);

    let test_files: Vec<String> = files
        .iter()
        .filter(|f| infer_source_role(&f.relative) == SourceRole::Test)
        .take(STRUCTURE_TEST_FILE_CAP)
        .map(|f| f.relative.clone())
        .collect();
    let source_files: Vec<&ScannedFile> = files
        .iter()
        .filter(|f| infer_source_role(&f.relative) == SourceRole::Implementation)
        .take(STRUCTURE_SOURCE_FILE_CAP)
        .collect();
    let mut test_texts: HashMap<String, String> = HashMap::new();
    for test_path in &test_files {
        if let Ok(bytes) = std::fs::read(root.join(test_path)) {
            if bytes.len() <= STRUCTURE_MANIFEST_READ_CAP {
                test_texts.insert(
                    test_path.clone(),
                    String::from_utf8_lossy(&bytes).to_string(),
                );
            }
        }
    }
    let symbol_names: HashMap<String, Vec<String>> =
        top_symbols.iter().fold(HashMap::new(), |mut acc, sym| {
            acc.entry(sym.path.clone())
                .or_default()
                .push(sym.name.clone());
            acc
        });
    let mut test_relationships: Vec<RepoTestRelationship> = Vec::new();
    for source in source_files {
        if test_relationships.len() >= STRUCTURE_RELATIONSHIP_CAP {
            truncated = true;
            break;
        }
        let names = symbol_names
            .get(&source.relative)
            .cloned()
            .unwrap_or_default();
        let hints =
            local_symbols::related_test_hints(&source.relative, &names, &test_files, &test_texts);
        for hint in hints.into_iter().take(4) {
            if test_relationships.len() >= STRUCTURE_RELATIONSHIP_CAP {
                truncated = true;
                break;
            }
            test_relationships.push(RepoTestRelationship {
                source_path: hint.source_path,
                test_path: hint.test_path,
                confidence: confidence_label(hint.confidence),
                reasons: hint.reasons,
            });
        }
    }
    test_relationships.sort_by(|a, b| {
        a.source_path
            .cmp(&b.source_path)
            .then_with(|| a.test_path.cmp(&b.test_path))
    });

    if packages.len() > total_cap {
        truncated = true;
        packages.truncate(total_cap);
    }

    LocalStructure {
        packages,
        language_distribution,
        modules,
        entrypoints,
        top_symbols,
        test_relationships,
        build_configs,
        truncated,
    }
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
        tree_sha: None,
        resolved_ref_name: None,
        default_branch: None,
        provenance_pinned: false,
        mode: RepoMapMode::FallbackSearch,
        root_entries: Vec::new(),
        entries: Vec::new(),
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
        freshness_confidence: None,
        ..Default::default()
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
