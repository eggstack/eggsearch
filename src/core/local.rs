//! Types for the optional local workspace search backend.
//!
//! Local workspace search provides structured source-file discovery
//! within operator-configured filesystem roots. Results carry
//! `TrustLevel::LocalTrusted` — they reflect operator-configured
//! provenance, not instruction trust.

use crate::core::code_evidence::SymbolKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
        .unwrap_or("");
    BINARY_EXTENSIONS.contains(&ext)
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
    fn local_search_request_defaults() {
        let req = LocalSearchRequest::default();
        assert!(req.query.is_empty());
        assert!(req.path.is_none());
        assert!(req.language.is_none());
    }
}
