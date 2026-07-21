//! Types for the optional local workspace search backend.
//!
//! Local workspace search provides structured source-file discovery
//! within operator-configured filesystem roots. Results carry
//! `TrustLevel::LocalTrusted` — they reflect operator-configured
//! provenance, not instruction trust.

use crate::core::code_evidence::SymbolKind;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// Configuration for the `[local]` section of the eggsearch config file.
///
/// Local search is disabled by default. When enabled, the operator
/// must configure at least one root directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalConfig {
    /// Whether local workspace search is enabled. Default: `false`.
    #[serde(default)]
    pub enabled: bool,
    /// Filesystem roots to index. Paths are canonicalized at startup.
    /// Empty when disabled.
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    /// Maximum file size in bytes to consider. Files larger than this
    /// are skipped. Default: 1 MB.
    #[serde(default = "default_max_file_bytes")]
    pub max_file_bytes: usize,
    /// Maximum number of files to index per search. Bounded scan
    /// prevents unbounded traversal. Default: 50000.
    #[serde(default = "default_max_indexed_files")]
    pub max_indexed_files: usize,
    /// Whether to include hidden files (dotfiles). Default: `false`.
    #[serde(default)]
    pub include_hidden: bool,
    /// Whether to respect .gitignore rules. Default: `true`.
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,
    /// Whether to follow symlinks. Default: `false`.
    #[serde(default)]
    pub follow_symlinks: bool,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            roots: Vec::new(),
            max_file_bytes: default_max_file_bytes(),
            max_indexed_files: default_max_indexed_files(),
            include_hidden: false,
            respect_gitignore: default_true(),
            follow_symlinks: false,
        }
    }
}

fn default_max_file_bytes() -> usize {
    1_048_576
}
fn default_max_indexed_files() -> usize {
    50_000
}
fn default_true() -> bool {
    true
}

/// A file entry discovered during local workspace walking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalFileEntry {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// Root-relative path (root_id + relative_path).
    pub relative_path: String,
    /// Root index this file belongs to.
    pub root_index: usize,
    /// File size in bytes.
    pub size: u64,
    /// Detected language from file extension.
    pub language: Option<String>,
}

/// Request for a local workspace search.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LocalSearchRequest {
    /// Free-text query to match against file paths and content.
    pub query: String,
    /// Optional path hint to narrow results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional language hint to filter results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Optional file hint to narrow results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Optional symbol hint for definition matching.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Maximum results to return. Bounded by config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    /// Per-search timeout in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// A scored match from local workspace search.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalMatch {
    /// The matched file entry.
    pub file: LocalFileEntry,
    /// Match score (higher is better).
    pub score: f64,
    /// Matched line start (1-indexed), if text match.
    pub line_start: Option<u32>,
    /// Matched line end (1-indexed), if text match.
    pub line_end: Option<u32>,
    /// Snippet of matched content.
    pub snippet: Option<String>,
    /// Matched symbol name, if symbol match.
    pub matched_symbol: Option<String>,
    /// Kind of the matched symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<SymbolKind>,
}

/// Inventory build telemetry for a local search.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InventoryTelemetry {
    /// Whether the inventory cache was used.
    pub used_inventory: bool,
    /// Total entries in the inventory.
    pub inventory_entries: usize,
    /// Number of candidates after filtering.
    pub candidates_filtered: usize,
    /// Number of files whose content was read.
    pub content_reads: usize,
    /// Time spent building the inventory in milliseconds.
    pub inventory_build_time_ms: u64,
    /// Whether the cached inventory was still fresh.
    pub inventory_fresh: bool,
    /// Whether this was a cold build (first search built inventory).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cold_build: bool,
    /// Whether the inventory was rebuilt due to staleness.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stale_rebuild: bool,
    /// Whether the search fell back to legacy bounded walk.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fallback_walk: bool,
    /// Whether the git backend was used for the inventory.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub uses_git_backend: bool,
    /// Number of untracked files included via git --others flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untracked_file_count: Option<usize>,
    /// Confidence that the inventory reflects current filesystem state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_confidence: Option<FreshnessConfidence>,
}

/// How confident the system is that the inventory is fresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessConfidence {
    /// Inventory built within the last 5 minutes and HEAD matches.
    High,
    /// Inventory built within the last 30 minutes.
    Medium,
    /// Inventory is stale or age is unknown.
    Low,
}

/// Result from a local workspace search.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LocalSearchResult {
    /// Matches found.
    pub matches: Vec<LocalMatch>,
    /// Total files scanned.
    pub files_scanned: usize,
    /// Whether the scan was truncated by file count limit.
    pub truncated: bool,
    /// Whether the scan timed out.
    pub timed_out: bool,
    /// Inventory telemetry, if inventory path was used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<InventoryTelemetry>,
}

/// Supported language detection from file extensions.
pub fn language_from_extension(path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(path).extension()?.to_str()?;
    match ext {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "js" => Some("javascript"),
        "ts" => Some("typescript"),
        "jsx" => Some("javascript"),
        "tsx" => Some("typescript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        "c" => Some("c"),
        "cpp" | "cc" | "cxx" => Some("cpp"),
        "h" | "hpp" => Some("c"),
        "cs" => Some("csharp"),
        "swift" => Some("swift"),
        "kt" | "kts" => Some("kotlin"),
        "scala" => Some("scala"),
        "sh" | "bash" | "zsh" => Some("shell"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "json" => Some("json"),
        "md" | "markdown" => Some("markdown"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        "sql" => Some("sql"),
        "proto" => Some("protobuf"),
        "dart" => Some("dart"),
        "ex" | "exs" => Some("elixir"),
        "erl" | "hrl" => Some("erlang"),
        "hs" => Some("haskell"),
        "lua" => Some("lua"),
        "zig" => Some("zig"),
        "nim" => Some("nim"),
        "v" => Some("v"),
        "ml" | "mli" => Some("ocaml"),
        "clj" | "cljs" => Some("clojure"),
        "r" => Some("r"),
        "jl" => Some("julia"),
        "php" => Some("php"),
        _ => None,
    }
}

/// Directories to skip during local workspace walking.
pub const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".venv",
    "venv",
    "dist",
    "build",
    ".mypy_cache",
    ".pytest_cache",
    "__pycache__",
    ".next",
    ".turbo",
    "coverage",
];

/// Binary file extensions to skip.
pub const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "o", "a", "lib", "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg",
    "webp", "mp3", "mp4", "wav", "avi", "mov", "mkv", "flac", "zip", "tar", "gz", "bz2", "xz",
    "7z", "rar", "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "bin", "dat", "db", "sqlite",
    "woff", "woff2", "ttf", "otf", "wasm",
];

/// Whether a file extension indicates a binary file.
pub fn is_binary_extension(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    BINARY_EXTENSIONS.contains(&ext.as_str())
}

/// Check if a path component should be skipped based on hidden and SKIP_DIRS rules.
pub fn should_skip_component(name: &str, include_hidden: bool) -> bool {
    if !include_hidden && name.starts_with('.') {
        return true;
    }
    SKIP_DIRS.contains(&name)
}

/// Check if a file path is eligible for indexing based on binary extension,
/// file size, and symlink policy.
pub fn is_eligible_for_indexing(path: &Path, config: &LocalConfig) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if is_binary_extension(name) {
            return false;
        }
    }

    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > config.max_file_bytes as u64 {
            return false;
        }
    } else {
        return false;
    }

    if !config.follow_symlinks {
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            if meta.file_type().is_symlink() {
                return false;
            }
        }
    }

    true
}

/// Check if a relative path from git ls-files output is eligible for indexing.
/// Applies hidden component checks, SKIP_DIRS, binary extension, and size limits.
pub fn is_git_path_eligible(relative_path: &str, root_path: &Path, config: &LocalConfig) -> bool {
    let path = Path::new(relative_path);

    for component in path.components() {
        if let Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                if should_skip_component(name_str, config.include_hidden) {
                    return false;
                }
            }
        }
    }

    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if is_binary_extension(name) {
            return false;
        }
    }

    let absolute_path = root_path.join(relative_path);
    if let Ok(metadata) = std::fs::metadata(&absolute_path) {
        if metadata.len() > config.max_file_bytes as u64 {
            return false;
        }
    } else {
        return false;
    }

    true
}

/// Errors that can occur when validating a local fetch path.
#[derive(Clone, Debug, thiserror::Error)]
pub enum LocalFetchPathError {
    /// The path is empty.
    #[error("path must not be empty")]
    Empty,
    /// The path contains `..` traversal segments.
    #[error("path contains '..' (path traversal)")]
    PathTraversal,
    /// The path is absolute (starts with `/`).
    #[error("path must be relative, not absolute")]
    AbsolutePath,
    /// The resolved path escapes the allowed workspace root.
    #[error("path escapes workspace root")]
    EscapesRoot,
    /// The file is a known binary extension and cannot be fetched as text.
    #[error("binary file extension: {0}")]
    BinaryFile(String),
    /// The symlink target escapes the allowed workspace root.
    #[error("symlink escapes workspace root")]
    SymlinkEscapesRoot,
    /// The file is a symlink but follow_symlinks is disabled.
    #[error("symlink not followed (follow_symlinks = false)")]
    SymlinkNotAllowed,
    /// The path cannot be canonicalized.
    #[error("failed to canonicalize path: {0}")]
    CanonicalizeFailed(String),
    /// The resolved path does not exist or is not a file.
    #[error("file not found")]
    NotFound,
    /// The file exceeds the configured `max_file_bytes` limit.
    #[error("file size {0} exceeds max_file_bytes ({1})")]
    FileTooLarge(u64, usize),
    /// A path component is hidden (starts with `.`) and `include_hidden` is false.
    #[error("hidden path component '{0}' not allowed (include_hidden = false)")]
    HiddenPath(String),
    /// A path component is in the configured skip-dirs list and is not
    /// allowed under the workspace fetch policy.
    #[error("path traverses skipped directory '{0}'")]
    SkippedDirectory(String),
}

fn has_parent_dir_component(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}

fn is_absolute_or_prefixed(path: &Path) -> bool {
    path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::RootDir | Component::Prefix(_)))
}

/// Validate a local workspace fetch path against a known root and
/// configuration. Returns the canonicalized path on success.
///
/// This centralizes all path safety checks for workspace fetch:
/// traversal rejection, symlink policy enforcement, binary extension
/// rejection, and canonical root containment.
pub fn validate_local_fetch_path(
    root: &Path,
    requested_relative_path: &str,
    cfg: &LocalConfig,
) -> Result<PathBuf, LocalFetchPathError> {
    // 1. Empty check
    if requested_relative_path.trim().is_empty() {
        return Err(LocalFetchPathError::Empty);
    }

    let requested_path = Path::new(requested_relative_path);

    // 2. Absolute/prefix path rejection
    if is_absolute_or_prefixed(requested_path) {
        return Err(LocalFetchPathError::AbsolutePath);
    }

    // 3. Path traversal rejection (component-aware: only reject ParentDir)
    if has_parent_dir_component(requested_path) {
        return Err(LocalFetchPathError::PathTraversal);
    }

    // 3a. Mirror local-search policy: reject hidden components when
    // `include_hidden` is false, and always reject SKIP_DIRS components,
    // so direct workspace fetch cannot retrieve files that `repo_search`
    // intentionally hides (.git/config, target/..., node_modules/...).
    for component in requested_path.components() {
        if let Component::Normal(os) = component {
            if let Some(name) = os.to_str() {
                if should_skip_component(name, cfg.include_hidden) {
                    if SKIP_DIRS.contains(&name) {
                        return Err(LocalFetchPathError::SkippedDirectory(name.to_string()));
                    }
                    return Err(LocalFetchPathError::HiddenPath(name.to_string()));
                }
            }
        }
    }

    // 4. Binary extension rejection
    if is_binary_extension(requested_relative_path) {
        return Err(LocalFetchPathError::BinaryFile(
            requested_relative_path.to_string(),
        ));
    }

    // 5. Build the candidate path
    let candidate = root.join(requested_relative_path);

    // 6. Verify it exists and is a regular file (before canonicalize, which fails on missing paths)
    if !candidate.exists() {
        return Err(LocalFetchPathError::NotFound);
    }

    // 7. Check symlink semantics: when follow_symlinks is disabled,
    // reject any component (intermediate or final) that is a symlink.
    if !cfg.follow_symlinks {
        let mut accumulated = root.to_path_buf();
        for component in requested_path.components() {
            accumulated = accumulated.join(component);
            if let Ok(meta) = std::fs::symlink_metadata(&accumulated) {
                if meta.file_type().is_symlink() {
                    return Err(LocalFetchPathError::SymlinkNotAllowed);
                }
            }
        }
    }

    // 8. Canonicalize (follows symlinks) and validate containment
    let canonical = candidate
        .canonicalize()
        .map_err(|e| LocalFetchPathError::CanonicalizeFailed(e.to_string()))?;

    // Canonicalize root too (macOS /var → /private/var symlinks)
    let root_canonical = root
        .canonicalize()
        .map_err(|e| LocalFetchPathError::CanonicalizeFailed(e.to_string()))?;

    if !canonical.starts_with(&root_canonical) {
        return Err(LocalFetchPathError::EscapesRoot);
    }

    // 9. Verify it's a regular file (redundant after exists(), but safe)
    if !canonical.is_file() {
        return Err(LocalFetchPathError::NotFound);
    }

    // 10. Enforce the configured max_file_bytes bound so callers cannot
    // bypass the operator-configured local file-size limit via direct fetch.
    if let Ok(meta) = std::fs::metadata(&canonical) {
        if meta.len() > cfg.max_file_bytes as u64 {
            return Err(LocalFetchPathError::FileTooLarge(
                meta.len(),
                cfg.max_file_bytes,
            ));
        }
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_config_default_is_disabled() {
        let cfg = LocalConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.roots.is_empty());
        assert_eq!(cfg.max_file_bytes, 1_048_576);
        assert_eq!(cfg.max_indexed_files, 50_000);
        assert!(!cfg.include_hidden);
        assert!(cfg.respect_gitignore);
        assert!(!cfg.follow_symlinks);
    }

    #[test]
    fn local_config_serde_roundtrip() {
        let cfg = LocalConfig {
            enabled: true,
            roots: vec![PathBuf::from("/workspace")],
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: LocalConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.roots.len(), 1);
    }

    #[test]
    fn language_from_extension_common() {
        assert_eq!(language_from_extension("main.rs"), Some("rust"));
        assert_eq!(language_from_extension("app.py"), Some("python"));
        assert_eq!(language_from_extension("index.ts"), Some("typescript"));
        assert_eq!(language_from_extension("main.go"), Some("go"));
        assert_eq!(language_from_extension("README.md"), Some("markdown"));
        assert_eq!(language_from_extension("config.toml"), Some("toml"));
        assert_eq!(language_from_extension("data.json"), Some("json"));
    }

    #[test]
    fn language_from_extension_unknown() {
        assert_eq!(language_from_extension("file.xyz"), None);
        assert_eq!(language_from_extension("Makefile"), None);
    }

    #[test]
    fn is_binary_extension_detects_common() {
        assert!(is_binary_extension("image.png"));
        assert!(is_binary_extension("archive.zip"));
        assert!(is_binary_extension("document.pdf"));
        assert!(!is_binary_extension("source.rs"));
        assert!(!is_binary_extension("readme.md"));
    }

    #[test]
    fn skip_dirs_contains_common_dirs() {
        assert!(SKIP_DIRS.contains(&".git"));
        assert!(SKIP_DIRS.contains(&"target"));
        assert!(SKIP_DIRS.contains(&"node_modules"));
        assert!(SKIP_DIRS.contains(&"__pycache__"));
    }

    #[test]
    fn skip_dirs_contains_venv_dirs() {
        assert!(SKIP_DIRS.contains(&".venv"));
        assert!(SKIP_DIRS.contains(&"venv"));
    }

    #[test]
    fn skip_dirs_contains_build_dirs() {
        assert!(SKIP_DIRS.contains(&"dist"));
        assert!(SKIP_DIRS.contains(&"build"));
    }

    #[test]
    fn skip_dirs_contains_cache_dirs() {
        assert!(SKIP_DIRS.contains(&".mypy_cache"));
        assert!(SKIP_DIRS.contains(&".pytest_cache"));
        assert!(SKIP_DIRS.contains(&".next"));
        assert!(SKIP_DIRS.contains(&".turbo"));
        assert!(SKIP_DIRS.contains(&"coverage"));
    }

    #[test]
    fn binary_extensions_executables_and_libraries() {
        assert!(is_binary_extension("prog.exe"));
        assert!(is_binary_extension("lib.dll"));
        assert!(is_binary_extension("lib.so"));
        assert!(is_binary_extension("lib.dylib"));
        assert!(is_binary_extension("lib.o"));
        assert!(is_binary_extension("lib.a"));
        assert!(is_binary_extension("lib.lib"));
    }

    #[test]
    fn binary_extensions_images() {
        assert!(is_binary_extension("photo.png"));
        assert!(is_binary_extension("photo.jpg"));
        assert!(is_binary_extension("photo.jpeg"));
        assert!(is_binary_extension("photo.gif"));
        assert!(is_binary_extension("photo.bmp"));
        assert!(is_binary_extension("icon.ico"));
        assert!(is_binary_extension("icon.svg"));
        assert!(is_binary_extension("image.webp"));
    }

    #[test]
    fn binary_extensions_audio_video() {
        assert!(is_binary_extension("audio.mp3"));
        assert!(is_binary_extension("audio.wav"));
        assert!(is_binary_extension("audio.flac"));
        assert!(is_binary_extension("video.mp4"));
        assert!(is_binary_extension("video.avi"));
        assert!(is_binary_extension("video.mov"));
        assert!(is_binary_extension("video.mkv"));
    }

    #[test]
    fn binary_extensions_archives() {
        assert!(is_binary_extension("archive.zip"));
        assert!(is_binary_extension("archive.tar"));
        assert!(is_binary_extension("archive.gz"));
        assert!(is_binary_extension("archive.bz2"));
        assert!(is_binary_extension("archive.xz"));
        assert!(is_binary_extension("archive.7z"));
        assert!(is_binary_extension("archive.rar"));
    }

    #[test]
    fn binary_extensions_documents_and_fonts() {
        assert!(is_binary_extension("doc.pdf"));
        assert!(is_binary_extension("doc.doc"));
        assert!(is_binary_extension("doc.docx"));
        assert!(is_binary_extension("sheet.xls"));
        assert!(is_binary_extension("sheet.xlsx"));
        assert!(is_binary_extension("slide.ppt"));
        assert!(is_binary_extension("slide.pptx"));
        assert!(is_binary_extension("data.bin"));
        assert!(is_binary_extension("data.dat"));
        assert!(is_binary_extension("data.db"));
        assert!(is_binary_extension("data.sqlite"));
        assert!(is_binary_extension("font.woff"));
        assert!(is_binary_extension("font.woff2"));
        assert!(is_binary_extension("font.ttf"));
        assert!(is_binary_extension("font.otf"));
        assert!(is_binary_extension("module.wasm"));
    }

    #[test]
    fn text_extensions_not_binary() {
        assert!(!is_binary_extension("main.rs"));
        assert!(!is_binary_extension("app.py"));
        assert!(!is_binary_extension("index.ts"));
        assert!(!is_binary_extension("main.go"));
        assert!(!is_binary_extension("style.css"));
        assert!(!is_binary_extension("data.json"));
        assert!(!is_binary_extension("config.toml"));
        assert!(!is_binary_extension("doc.md"));
        assert!(!is_binary_extension("page.html"));
        assert!(!is_binary_extension("query.sql"));
        assert!(!is_binary_extension("Makefile"));
        assert!(!is_binary_extension("Dockerfile"));
    }

    #[test]
    fn binary_extensions_return_none_from_language() {
        assert_eq!(language_from_extension("image.png"), None);
        assert_eq!(language_from_extension("archive.zip"), None);
        assert_eq!(language_from_extension("document.pdf"), None);
        assert_eq!(language_from_extension("font.ttf"), None);
        assert_eq!(language_from_extension("module.wasm"), None);
        assert_eq!(language_from_extension("data.db"), None);
    }

    #[test]
    fn language_from_extension_extended() {
        assert_eq!(language_from_extension("app.jsx"), Some("javascript"));
        assert_eq!(language_from_extension("app.tsx"), Some("typescript"));
        assert_eq!(language_from_extension("main.java"), Some("java"));
        assert_eq!(language_from_extension("app.rb"), Some("ruby"));
        assert_eq!(language_from_extension("main.c"), Some("c"));
        assert_eq!(language_from_extension("main.cpp"), Some("cpp"));
        assert_eq!(language_from_extension("main.cc"), Some("cpp"));
        assert_eq!(language_from_extension("main.h"), Some("c"));
        assert_eq!(language_from_extension("main.hpp"), Some("c"));
        assert_eq!(language_from_extension("app.cs"), Some("csharp"));
        assert_eq!(language_from_extension("app.swift"), Some("swift"));
        assert_eq!(language_from_extension("app.kt"), Some("kotlin"));
        assert_eq!(language_from_extension("app.kts"), Some("kotlin"));
        assert_eq!(language_from_extension("app.scala"), Some("scala"));
        assert_eq!(language_from_extension("script.sh"), Some("shell"));
        assert_eq!(language_from_extension("script.bash"), Some("shell"));
        assert_eq!(language_from_extension("app.yaml"), Some("yaml"));
        assert_eq!(language_from_extension("app.yml"), Some("yaml"));
        assert_eq!(language_from_extension("page.htm"), Some("html"));
        assert_eq!(language_from_extension("app.proto"), Some("protobuf"));
        assert_eq!(language_from_extension("app.dart"), Some("dart"));
        assert_eq!(language_from_extension("app.ex"), Some("elixir"));
        assert_eq!(language_from_extension("app.exs"), Some("elixir"));
        assert_eq!(language_from_extension("app.erl"), Some("erlang"));
        assert_eq!(language_from_extension("app.hrl"), Some("erlang"));
        assert_eq!(language_from_extension("app.hs"), Some("haskell"));
        assert_eq!(language_from_extension("app.lua"), Some("lua"));
        assert_eq!(language_from_extension("app.zig"), Some("zig"));
        assert_eq!(language_from_extension("app.nim"), Some("nim"));
        assert_eq!(language_from_extension("app.v"), Some("v"));
        assert_eq!(language_from_extension("app.ml"), Some("ocaml"));
        assert_eq!(language_from_extension("app.mli"), Some("ocaml"));
        assert_eq!(language_from_extension("app.clj"), Some("clojure"));
        assert_eq!(language_from_extension("app.cljs"), Some("clojure"));
        assert_eq!(language_from_extension("app.r"), Some("r"));
        assert_eq!(language_from_extension("app.jl"), Some("julia"));
        assert_eq!(language_from_extension("app.php"), Some("php"));
    }

    #[test]
    fn language_from_extension_no_extension() {
        assert_eq!(language_from_extension("Makefile"), None);
        assert_eq!(language_from_extension("Dockerfile"), None);
        assert_eq!(language_from_extension(".gitignore"), None);
        assert_eq!(language_from_extension("justfile"), None);
    }

    #[test]
    fn local_config_max_file_bytes_default() {
        let cfg = LocalConfig::default();
        assert_eq!(cfg.max_file_bytes, 1_048_576);
    }

    #[test]
    fn local_config_max_indexed_files_default() {
        let cfg = LocalConfig::default();
        assert_eq!(cfg.max_indexed_files, 50_000);
    }

    #[test]
    fn local_config_include_hidden_default() {
        let cfg = LocalConfig::default();
        assert!(!cfg.include_hidden);
    }

    #[test]
    fn local_config_respect_gitignore_default() {
        let cfg = LocalConfig::default();
        assert!(cfg.respect_gitignore);
    }

    #[test]
    fn local_config_follow_symlinks_default() {
        let cfg = LocalConfig::default();
        assert!(!cfg.follow_symlinks);
    }

    #[test]
    fn local_config_roots_empty_by_default() {
        let cfg = LocalConfig::default();
        assert!(cfg.roots.is_empty());
    }

    #[test]
    fn local_search_request_defaults() {
        let req = LocalSearchRequest::default();
        assert!(req.query.is_empty());
        assert!(req.path.is_none());
        assert!(req.language.is_none());
    }

    #[test]
    fn local_match_defaults() {
        let req = LocalSearchResult::default();
        assert!(req.matches.is_empty());
        assert_eq!(req.files_scanned, 0);
        assert!(!req.truncated);
        assert!(!req.timed_out);
    }

    #[test]
    fn is_binary_extension_case_insensitive() {
        assert!(is_binary_extension("image.PNG"));
        assert!(is_binary_extension("archive.ZIP"));
        assert!(is_binary_extension("image.PnG"));
        assert!(is_binary_extension("Doc.PDF"));
        assert!(is_binary_extension("image.png"));
        assert!(is_binary_extension("archive.zip"));
    }

    #[test]
    fn is_binary_extension_no_extension() {
        assert!(!is_binary_extension("Makefile"));
        assert!(!is_binary_extension(".gitignore"));
        assert!(!is_binary_extension("README"));
    }

    #[test]
    fn is_binary_extension_double_extension() {
        assert!(is_binary_extension("archive.tar.gz"));
        assert!(is_binary_extension("backup.tar.bz2"));
    }

    #[test]
    fn validate_local_fetch_path_empty_rejected() {
        let root = std::env::temp_dir();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(&root, "", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::Empty));
    }

    #[test]
    fn validate_local_fetch_path_absolute_rejected() {
        let root = std::env::temp_dir();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(&root, "/etc/passwd", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::AbsolutePath));
    }

    #[test]
    fn validate_local_fetch_path_traversal_rejected() {
        let root = std::env::temp_dir();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(&root, "../secret.txt", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::PathTraversal));
    }

    #[test]
    fn validate_local_fetch_path_embedded_traversal_rejected() {
        let root = std::env::temp_dir();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(&root, "a/../../secret.txt", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::PathTraversal));
    }

    /// Validates that filenames containing `..` as a substring are accepted
    /// when `..` is not a directory traversal component.
    #[test]
    fn validate_local_fetch_path_double_dot_in_filename_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes..draft.md");
        std::fs::write(&file, "draft content").unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), "notes..draft.md", &cfg);
        assert!(
            result.is_ok(),
            "expected Ok for 'notes..draft.md', got {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_binary_rejected() {
        let root = std::env::temp_dir();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(&root, "image.png", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::BinaryFile(_)));
    }

    #[test]
    fn validate_local_fetch_path_uppercase_binary_rejected() {
        let root = std::env::temp_dir();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(&root, "report.PDF", &cfg).unwrap_err();
        assert!(
            matches!(err, LocalFetchPathError::BinaryFile(_)),
            "uppercase binary extension should be rejected, got: {err:?}"
        );
        let err = validate_local_fetch_path(&root, "image.PNG", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::BinaryFile(_)));
    }

    #[test]
    fn validate_local_fetch_path_symlink_not_allowed_when_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Create a real file
        std::fs::write(root.join("target.txt"), "hello").unwrap();
        // Create a symlink to it
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("target.txt"), root.join("link.txt")).unwrap();
            let cfg = LocalConfig {
                follow_symlinks: false,
                ..LocalConfig::default()
            };
            let err = validate_local_fetch_path(root, "link.txt", &cfg).unwrap_err();
            assert!(matches!(err, LocalFetchPathError::SymlinkNotAllowed));
        }
    }

    #[test]
    fn validate_local_fetch_path_not_found_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(dir.path(), "nonexistent.rs", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::NotFound));
    }

    #[test]
    fn validate_local_fetch_path_normal_file_accepted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), "main.rs", &cfg);
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert!(canonical.is_file());
    }

    #[test]
    fn validate_local_fetch_path_nested_double_dot_filename_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("src");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("generated..schema.rs"), "schema").unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), "src/generated..schema.rs", &cfg);
        assert!(
            result.is_ok(),
            "expected Ok for 'src/generated..schema.rs', got {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_single_dot_segment_allowed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.rs"), "content").unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), "./file.rs", &cfg);
        assert!(
            result.is_ok(),
            "expected Ok for './file.rs', got {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_symlink_escape_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("escaped.txt"), "pwned").unwrap();
        let cfg = LocalConfig {
            follow_symlinks: true,
            ..LocalConfig::default()
        };
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                outside.path().join("escaped.txt"),
                dir.path().join("link.txt"),
            )
            .unwrap();
            let err = validate_local_fetch_path(dir.path(), "link.txt", &cfg).unwrap_err();
            assert!(
                matches!(err, LocalFetchPathError::EscapesRoot),
                "expected EscapesRoot for symlink escaping root, got {err:?}"
            );
        }
    }

    #[test]
    fn validate_local_fetch_path_oversized_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.txt");
        let content = vec![b'a'; 2048];
        std::fs::write(&file, &content).unwrap();
        let cfg = LocalConfig {
            max_file_bytes: 1024,
            ..LocalConfig::default()
        };
        let err = validate_local_fetch_path(dir.path(), "big.txt", &cfg).unwrap_err();
        match err {
            LocalFetchPathError::FileTooLarge(size, max) => {
                assert_eq!(size, 2048);
                assert_eq!(max, 1024);
            }
            other => panic!("expected FileTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn validate_local_fetch_path_at_limit_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("at_limit.txt");
        let content = vec![b'a'; 1024];
        std::fs::write(&file, &content).unwrap();
        let cfg = LocalConfig {
            max_file_bytes: 1024,
            ..LocalConfig::default()
        };
        let result = validate_local_fetch_path(dir.path(), "at_limit.txt", &cfg);
        assert!(
            result.is_ok(),
            "file at exactly max_file_bytes should be accepted, got {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_hidden_file_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/config"), "[core]\nfoo=bar").unwrap();
        std::fs::write(dir.path().join(".env"), "SECRET=value").unwrap();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(dir.path(), ".git/config", &cfg).unwrap_err();
        assert!(
            matches!(
                err,
                LocalFetchPathError::SkippedDirectory(_) | LocalFetchPathError::HiddenPath(_)
            ),
            "expected SkippedDirectory or HiddenPath for '.git/config', got {err:?}"
        );
        let err = validate_local_fetch_path(dir.path(), ".env", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::HiddenPath(_)));
    }

    #[test]
    fn validate_local_fetch_path_hidden_allowed_when_include_hidden() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "SECRET=value").unwrap();
        let cfg = LocalConfig {
            include_hidden: true,
            ..LocalConfig::default()
        };
        let result = validate_local_fetch_path(dir.path(), ".env", &cfg);
        assert!(
            result.is_ok(),
            "expected Ok for '.env' with include_hidden=true, got {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_skip_dirs_always_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/config"), "[core]\nfoo=bar").unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/generated.rs"), "// generated").unwrap();
        std::fs::create_dir(dir.path().join("node_modules")).unwrap();
        std::fs::write(dir.path().join("node_modules/pkg.js"), "// pkg").unwrap();
        let cfg = LocalConfig {
            include_hidden: true,
            ..LocalConfig::default()
        };
        let err = validate_local_fetch_path(dir.path(), ".git/config", &cfg).unwrap_err();
        assert!(
            matches!(err, LocalFetchPathError::SkippedDirectory(_)),
            "expected SkippedDirectory for '.git/config' with include_hidden=true, got {err:?}"
        );
        let err = validate_local_fetch_path(dir.path(), "target/generated.rs", &cfg).unwrap_err();
        assert!(
            matches!(err, LocalFetchPathError::SkippedDirectory(_)),
            "expected SkippedDirectory for 'target/generated.rs' with include_hidden=true, got {err:?}"
        );
        let err = validate_local_fetch_path(dir.path(), "node_modules/pkg.js", &cfg).unwrap_err();
        assert!(
            matches!(err, LocalFetchPathError::SkippedDirectory(_)),
            "expected SkippedDirectory for 'node_modules/pkg.js' with include_hidden=true, got {err:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_skipped_directory_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/generated.rs"), "// generated").unwrap();
        std::fs::create_dir(dir.path().join("node_modules")).unwrap();
        std::fs::write(dir.path().join("node_modules/pkg.js"), "// pkg").unwrap();
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(dir.path(), "target/generated.rs", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::SkippedDirectory(_)));
        let err = validate_local_fetch_path(dir.path(), "node_modules/pkg.js", &cfg).unwrap_err();
        assert!(matches!(err, LocalFetchPathError::SkippedDirectory(_)));
    }

    #[test]
    fn validate_local_fetch_path_skipped_dir_always_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/generated.rs"), "// x").unwrap();
        let cfg = LocalConfig {
            include_hidden: true,
            ..LocalConfig::default()
        };
        let err = validate_local_fetch_path(dir.path(), "target/generated.rs", &cfg).unwrap_err();
        assert!(
            matches!(err, LocalFetchPathError::SkippedDirectory(_)),
            "expected SkippedDirectory for 'target/generated.rs' with include_hidden=true, got {err:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_double_slashes_normalized() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src").join("main.rs"), "fn main() {}").unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), "src//main.rs", &cfg);
        assert!(
            result.is_ok(),
            "expected Ok for 'src//main.rs' (double slashes normalized), got {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_double_slash_traversal_rejected() {
        let root = Path::new("/tmp/test_root");
        let cfg = LocalConfig::default();
        let err = validate_local_fetch_path(root, "src//../../secret.txt", &cfg).unwrap_err();
        assert!(
            matches!(err, LocalFetchPathError::PathTraversal),
            "expected PathTraversal for 'src//../../secret.txt', got: {err:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_trailing_slash_not_found() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.rs"), "content").unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), "file.rs/", &cfg);
        assert!(
            matches!(result, Err(LocalFetchPathError::NotFound)),
            "expected NotFound for trailing slash, got: {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_with_spaces_accepted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("my folder")).unwrap();
        std::fs::write(dir.path().join("my folder").join("file.rs"), "content").unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), "my folder/file.rs", &cfg);
        assert!(
            result.is_ok(),
            "expected Ok for path with spaces, got {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_very_long_name_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let long_name = "a".repeat(200);
        std::fs::write(dir.path().join(format!("{long_name}.rs")), "content").unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), &format!("{long_name}.rs"), &cfg);
        assert!(
            result.is_ok(),
            "expected Ok for very long filename (within OS limits), got {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_very_long_component_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let long_name = "a".repeat(300);
        let result = validate_local_fetch_path(
            dir.path(),
            &format!("{long_name}.rs"),
            &LocalConfig::default(),
        );
        assert!(
            matches!(result, Err(LocalFetchPathError::NotFound)),
            "expected NotFound for path exceeding OS limits, got: {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_symlink_followed_when_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "secret content").unwrap();
        #[cfg(unix)]
        {
            let link = dir.path().join("link.txt");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            let cfg = LocalConfig {
                follow_symlinks: true,
                ..Default::default()
            };
            let result = validate_local_fetch_path(dir.path(), "link.txt", &cfg);
            assert!(
                result.is_ok(),
                "expected Ok for symlink with follow_symlinks=true, got {result:?}"
            );
        }
    }

    #[test]
    fn validate_local_fetch_path_directory_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        let cfg = LocalConfig::default();
        let result = validate_local_fetch_path(dir.path(), "subdir", &cfg);
        assert!(
            matches!(result, Err(LocalFetchPathError::NotFound)),
            "expected NotFound for directory path, got: {result:?}"
        );
    }

    #[test]
    fn validate_local_fetch_path_symlink_outside_root_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("escaped.txt"), "pwned").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                outside.path().join("escaped.txt"),
                dir.path().join("link.txt"),
            )
            .unwrap();
            let cfg_follow = LocalConfig {
                follow_symlinks: true,
                ..LocalConfig::default()
            };
            let err = validate_local_fetch_path(dir.path(), "link.txt", &cfg_follow).unwrap_err();
            assert!(
                matches!(err, LocalFetchPathError::EscapesRoot),
                "expected EscapesRoot for symlink escaping root with follow_symlinks=true, got {err:?}"
            );
            let cfg_no_follow = LocalConfig {
                follow_symlinks: false,
                ..LocalConfig::default()
            };
            let err =
                validate_local_fetch_path(dir.path(), "link.txt", &cfg_no_follow).unwrap_err();
            assert!(
                matches!(err, LocalFetchPathError::SymlinkNotAllowed),
                "expected SymlinkNotAllowed for symlink escaping root with follow_symlinks=false, got {err:?}"
            );
        }
    }

    #[test]
    fn validate_local_fetch_path_intermediate_symlink_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real_dir = dir.path().join("real");
        std::fs::create_dir(&real_dir).unwrap();
        std::fs::write(real_dir.join("file.rs"), "fn main() {}").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&real_dir, dir.path().join("linkdir")).unwrap();
            let cfg = LocalConfig {
                follow_symlinks: false,
                ..LocalConfig::default()
            };
            let err = validate_local_fetch_path(dir.path(), "linkdir/file.rs", &cfg).unwrap_err();
            assert!(
                matches!(err, LocalFetchPathError::SymlinkNotAllowed),
                "expected SymlinkNotAllowed for intermediate symlink, got {err:?}"
            );
            let cfg_follow = LocalConfig {
                follow_symlinks: true,
                ..LocalConfig::default()
            };
            let result = validate_local_fetch_path(dir.path(), "linkdir/file.rs", &cfg_follow);
            assert!(
                result.is_ok(),
                "expected Ok for intermediate symlink with follow_symlinks=true, got {result:?}"
            );
        }
    }
}
