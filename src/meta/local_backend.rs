//! Local workspace search backend: bounded file walking, scoring, and
//! SourceCard conversion.
//!
//! The [`LocalWorkspaceBackend`] walks configured filesystem roots,
//! applies ignore rules and extension filters, scores path/text/language
//! matches, and converts results into [`SourceCard`] values with
//! `TrustLevel::LocalTrusted`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use regex::Regex;

use crate::core::code_evidence::{
    CodeEvidence, CodeEvidenceReason, EvidenceConfidence, SourceRole, SymbolKind,
};
use crate::core::code_metadata::CodeMetadata;
use crate::core::local::{
    is_binary_extension, language_from_extension, LocalConfig, LocalFileEntry, LocalMatch,
    LocalSearchRequest, LocalSearchResult, SKIP_DIRS,
};
use crate::core::result::TrustLevel;
use crate::core::sanitize::TrustMarkers;
use crate::core::source_card::{RankReason, SourceCard, SourceKind, SourceMetadata};

/// Local workspace search backend.
///
/// Constructed once at server startup when `[local].enabled = true`.
/// Walks configured roots on each search call, applying bounded scan
/// limits and deterministic scoring.
pub struct LocalWorkspaceBackend {
    config: LocalConfig,
    /// Canonicalized roots with their index.
    roots: Vec<(usize, PathBuf)>,
}

impl std::fmt::Debug for LocalWorkspaceBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalWorkspaceBackend")
            .field("enabled", &self.config.enabled)
            .field("roots", &self.roots.len())
            .finish()
    }
}

impl LocalWorkspaceBackend {
    /// Build a new local workspace backend from config.
    ///
    /// Canonicalizes all configured roots and rejects missing paths.
    /// Returns `Err` if the config is invalid (enabled but no roots,
    /// or roots that don't exist).
    pub fn new(config: LocalConfig) -> Result<Self, String> {
        if !config.enabled {
            return Ok(Self {
                config,
                roots: Vec::new(),
            });
        }
        if config.roots.is_empty() {
            return Err(
                "[local].enabled is true but [local].roots is empty; at least one root is required"
                    .to_string(),
            );
        }
        let mut roots = Vec::new();
        for (i, root) in config.roots.iter().enumerate() {
            let canonical = root
                .canonicalize()
                .map_err(|e| format!("failed to canonicalize root {}: {e}", root.display()))?;
            if !canonical.is_dir() {
                return Err(format!(
                    "root {} is not a directory",
                    root.display()
                ));
            }
            roots.push((i, canonical));
        }
        Ok(Self { config, roots })
    }

    /// Whether local search is enabled and has configured roots.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && !self.roots.is_empty()
    }

    /// Return the canonicalized roots (index, path) pairs.
    pub fn roots(&self) -> Vec<(usize, PathBuf)> {
        self.roots.clone()
    }

    /// Run a local workspace search.
    pub async fn search(&self, req: &LocalSearchRequest) -> LocalSearchResult {
        if !self.is_enabled() {
            return LocalSearchResult::default();
        }

        let timeout_ms = req
            .timeout_ms
            .unwrap_or(self.config.max_indexed_files as u64 / 100);
        let timeout = Duration::from_millis(timeout_ms);
        let max_results = req
            .max_results
            .unwrap_or(10)
            .min(self.config.max_indexed_files);
        let start = Instant::now();

        let config = self.config.clone();
        let roots = self.roots.clone();
        let query = req.query.clone();
        let path_hint = req.path.clone();
        let lang_hint = req.language.clone();
        let file_hint = req.file.clone();
        let symbol_hint = req.symbol.clone();

        let result = tokio::task::spawn_blocking(move || {
            Self::search_sync(
                &config,
                &roots,
                &query,
                path_hint.as_deref(),
                lang_hint.as_deref(),
                file_hint.as_deref(),
                symbol_hint.as_deref(),
                max_results,
                timeout,
                start,
            )
        })
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "local search task panicked");
            LocalSearchResult::default()
        });

        result
    }

    #[allow(clippy::too_many_arguments)]
    fn search_sync(
        config: &LocalConfig,
        roots: &[(usize, PathBuf)],
        query: &str,
        path_hint: Option<&str>,
        lang_hint: Option<&str>,
        file_hint: Option<&str>,
        symbol_hint: Option<&str>,
        max_results: usize,
        timeout: Duration,
        start: Instant,
    ) -> LocalSearchResult {
        let mut matches = Vec::new();
        let mut files_scanned = 0usize;
        let mut truncated = false;
        let mut timed_out = false;

        let query_lower = query.to_lowercase();
        let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

        for &(root_index, ref root_path) in roots {
            if start.elapsed() > timeout {
                timed_out = true;
                break;
            }

            Self::walk_root(
                root_path,
                root_index,
                config,
                &query_lower,
                &query_tokens,
                path_hint,
                lang_hint,
                file_hint,
                symbol_hint,
                &mut matches,
                &mut files_scanned,
                max_results,
                &start,
                timeout,
                &mut timed_out,
                &mut truncated,
            );

            if timed_out {
                break;
            }
        }

        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        if matches.len() > max_results {
            matches.truncate(max_results);
            truncated = true;
        }

        LocalSearchResult {
            matches,
            files_scanned,
            truncated,
            timed_out,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_root(
        root_path: &Path,
        root_index: usize,
        config: &LocalConfig,
        query_lower: &str,
        query_tokens: &[&str],
        path_hint: Option<&str>,
        lang_hint: Option<&str>,
        file_hint: Option<&str>,
        symbol_hint: Option<&str>,
        matches: &mut Vec<LocalMatch>,
        files_scanned: &mut usize,
        max_results: usize,
        start: &Instant,
        timeout: Duration,
        timed_out: &mut bool,
        truncated: &mut bool,
    ) {
        let walk_dir = match std::fs::read_dir(root_path) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(root = %root_path.display(), error = %e, "failed to read root directory");
                return;
            }
        };

        for entry in walk_dir.flatten() {
            if start.elapsed() > timeout {
                *timed_out = true;
                return;
            }

            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if !config.include_hidden && file_name_str.starts_with('.') {
                continue;
            }

            let path = entry.path();

            if path.is_dir() {
                if SKIP_DIRS.contains(&file_name_str.as_ref()) {
                    continue;
                }
                if !config.include_hidden && file_name_str.starts_with('.') {
                    continue;
                }
                Self::walk_dir_recursive(
                    &path,
                    root_path,
                    root_index,
                    config,
                    query_lower,
                    query_tokens,
                    path_hint,
                    lang_hint,
                    file_hint,
                    symbol_hint,
                    matches,
                    files_scanned,
                    max_results,
                    start,
                    timeout,
                    timed_out,
                    truncated,
                );
            } else if path.is_file() {
                *files_scanned += 1;
                if *files_scanned > config.max_indexed_files {
                    *truncated = true;
                    return;
                }

                if is_binary_extension(&file_name_str) {
                    continue;
                }

                let metadata = match std::fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if metadata.len() > config.max_file_bytes as u64 {
                    continue;
                }

                let relative_path = path
                    .strip_prefix(root_path)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                let language = language_from_extension(&relative_path);

                let file_entry = LocalFileEntry {
                    path: path.clone(),
                    relative_path: relative_path.clone(),
                    root_index,
                    size: metadata.len(),
                    language: language.map(|s| s.to_string()),
                };

                if let Some(lang) = lang_hint {
                    if file_entry.language.as_deref() != Some(lang) {
                        continue;
                    }
                }
                if let Some(fh) = file_hint {
                    if !file_name_str.contains(fh) {
                        continue;
                    }
                }
                if let Some(ph) = path_hint {
                    if !relative_path.to_lowercase().contains(&ph.to_lowercase()) {
                        continue;
                    }
                }

                let score = Self::score_file(
                    &file_entry,
                    query_lower,
                    query_tokens,
                    symbol_hint,
                );

                if score > 0.0 {
                    let (snippet, line_start, line_end, matched_symbol, symbol_kind, boosted_score) =
                        if let Some(sym_hint) = symbol_hint {
                            if let Some((name, kind, sym_line)) =
                                Self::find_symbol_match(&path, sym_hint, config.max_file_bytes)
                            {
                                // Re-read a snippet around the definition line
                                let snippet = Self::find_text_match(
                                    &path,
                                    &name,
                                    config.max_file_bytes,
                                )
                                .0;
                                // Boost score for direct symbol definition match
                                let boosted = score + 30.0;
                                (
                                    snippet,
                                    Some(sym_line),
                                    Some(sym_line),
                                    Some(name),
                                    Some(kind),
                                    boosted,
                                )
                            } else if !query_lower.is_empty() {
                                let (s, ls, le) =
                                    Self::find_text_match(&path, query_lower, config.max_file_bytes);
                                (s, ls, le, None, None, score)
                            } else {
                                (None, None, None, None, None, score)
                            }
                        } else if !query_lower.is_empty() {
                            let (s, ls, le) =
                                Self::find_text_match(&path, query_lower, config.max_file_bytes);
                            (s, ls, le, None, None, score)
                        } else {
                            (None, None, None, None, None, score)
                        };

                    matches.push(LocalMatch {
                        file: file_entry,
                        score: boosted_score,
                        line_start,
                        line_end,
                        snippet,
                        matched_symbol,
                        symbol_kind,
                    });
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_dir_recursive(
        dir: &Path,
        root_path: &Path,
        root_index: usize,
        config: &LocalConfig,
        query_lower: &str,
        query_tokens: &[&str],
        path_hint: Option<&str>,
        lang_hint: Option<&str>,
        file_hint: Option<&str>,
        symbol_hint: Option<&str>,
        matches: &mut Vec<LocalMatch>,
        files_scanned: &mut usize,
        _max_results: usize,
        start: &Instant,
        timeout: Duration,
        timed_out: &mut bool,
        truncated: &mut bool,
    ) {
        let read_dir = match std::fs::read_dir(dir) {
            Ok(d) => d,
            Err(_) => return,
        };

        for entry in read_dir.flatten() {
            if start.elapsed() > timeout {
                *timed_out = true;
                return;
            }

            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if !config.include_hidden && file_name_str.starts_with('.') {
                continue;
            }

            let path = entry.path();

            if path.is_dir() {
                if SKIP_DIRS.contains(&file_name_str.as_ref()) {
                    continue;
                }
                if !config.include_hidden && file_name_str.starts_with('.') {
                    continue;
                }
                Self::walk_dir_recursive(
                    &path,
                    root_path,
                    root_index,
                    config,
                    query_lower,
                    query_tokens,
                    path_hint,
                    lang_hint,
                    file_hint,
                    symbol_hint,
                    matches,
                    files_scanned,
                    _max_results,
                    start,
                    timeout,
                    timed_out,
                    truncated,
                );
            } else if path.is_file() {
                *files_scanned += 1;
                if *files_scanned > config.max_indexed_files {
                    *truncated = true;
                    return;
                }

                if is_binary_extension(&file_name_str) {
                    continue;
                }

                let metadata = match std::fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if metadata.len() > config.max_file_bytes as u64 {
                    continue;
                }

                let relative_path = path
                    .strip_prefix(root_path)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                let language = language_from_extension(&relative_path);

                let file_entry = LocalFileEntry {
                    path: path.clone(),
                    relative_path: relative_path.clone(),
                    root_index,
                    size: metadata.len(),
                    language: language.map(|s| s.to_string()),
                };

                if let Some(lang) = lang_hint {
                    if file_entry.language.as_deref() != Some(lang) {
                        continue;
                    }
                }
                if let Some(fh) = file_hint {
                    if !file_name_str.contains(fh) {
                        continue;
                    }
                }
                if let Some(ph) = path_hint {
                    if !relative_path.to_lowercase().contains(&ph.to_lowercase()) {
                        continue;
                    }
                }

                let score = Self::score_file(
                    &file_entry,
                    query_lower,
                    query_tokens,
                    symbol_hint,
                );

                if score > 0.0 {
                    let (snippet, line_start, line_end, matched_symbol, symbol_kind, boosted_score) =
                        if let Some(sym_hint) = symbol_hint {
                            if let Some((name, kind, sym_line)) =
                                Self::find_symbol_match(&path, sym_hint, config.max_file_bytes)
                            {
                                let snippet = Self::find_text_match(
                                    &path,
                                    &name,
                                    config.max_file_bytes,
                                )
                                .0;
                                let boosted = score + 30.0;
                                (
                                    snippet,
                                    Some(sym_line),
                                    Some(sym_line),
                                    Some(name),
                                    Some(kind),
                                    boosted,
                                )
                            } else if !query_lower.is_empty() {
                                let (s, ls, le) =
                                    Self::find_text_match(&path, query_lower, config.max_file_bytes);
                                (s, ls, le, None, None, score)
                            } else {
                                (None, None, None, None, None, score)
                            }
                        } else if !query_lower.is_empty() {
                            let (s, ls, le) =
                                Self::find_text_match(&path, query_lower, config.max_file_bytes);
                            (s, ls, le, None, None, score)
                        } else {
                            (None, None, None, None, None, score)
                        };

                    matches.push(LocalMatch {
                        file: file_entry,
                        score: boosted_score,
                        line_start,
                        line_end,
                        snippet,
                        matched_symbol,
                        symbol_kind,
                    });
                }
            }
        }
    }

    /// Score a file against the query. Returns 0.0 if no match.
    fn score_file(
        file: &LocalFileEntry,
        query_lower: &str,
        query_tokens: &[&str],
        _symbol_hint: Option<&str>,
    ) -> f64 {
        let mut score: f64 = 0.0;
        let path_lower = file.relative_path.to_lowercase();
        let file_name = std::path::Path::new(&file.relative_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if file_name == query_lower {
            score += 100.0;
        }

        for token in query_tokens {
            if path_lower.contains(token) {
                score += 20.0;
            }
        }

        // Language bonus only applies when there is already a path/filename match.
        if score > 0.0 && file.language.is_some() {
            score += 5.0;
        }

        let penalty_extensions = ["lock", "min.js", "min.css", ".map"];
        for ext in &penalty_extensions {
            if file_name.ends_with(ext) {
                score -= 150.0;
            }
        }

        let role = crate::core::code_evidence::infer_source_role(&file.relative_path);
        match role {
            SourceRole::Implementation => score += 10.0,
            SourceRole::Test => score += 5.0,
            SourceRole::Example => score += 5.0,
            SourceRole::Documentation => score += 8.0,
            SourceRole::Readme => score += 12.0,
            _ => {}
        }

        score
    }

    /// Search for a text match in the file content.
    fn find_text_match(
        path: &Path,
        query: &str,
        max_file_bytes: usize,
    ) -> (Option<String>, Option<u32>, Option<u32>) {
        let content = match std::fs::read(path) {
            Ok(bytes) => {
                if bytes.len() > max_file_bytes {
                    return (None, None, None);
                }
                String::from_utf8_lossy(&bytes).to_string()
            }
            Err(_) => return (None, None, None),
        };

        let query_lower = query.to_lowercase();
        for (line_idx, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                let line_num = (line_idx + 1) as u32;
                let snippet = line.trim().to_string();
                let snippet = if snippet.len() > 500 {
                    format!("{}...", &snippet[..500])
                } else {
                    snippet
                };
                return (Some(snippet), Some(line_num), Some(line_num));
            }
        }
        (None, None, None)
    }

    /// Scan file content for symbol definitions matching the hint.
    ///
    /// Returns `(matched_symbol_name, SymbolKind, line_number)` for the
    /// first matching definition, or `None` if no match is found.
    fn find_symbol_match(
        path: &Path,
        symbol_hint: &str,
        max_file_bytes: usize,
    ) -> Option<(String, SymbolKind, u32)> {
        let content = std::fs::read(path).ok()?;
        if content.len() > max_file_bytes {
            return None;
        }
        let text = String::from_utf8_lossy(&content);
        let hint_lower = symbol_hint.to_lowercase();

        // Combined pattern: matches common definition forms across
        // Rust, Python, JavaScript/TypeScript, Go, Java, C/C++.
        // Group 1 = keyword, Group 2 = name.
        // Order matters: check impl blocks before bare struct/enum/trait.
        let patterns: &[(&str, SymbolKind)] = &[
            // Rust: fn name, pub fn name, pub(crate) fn name, async fn name
            (r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)", SymbolKind::Function),
            // Rust: struct Name
            (r"(?:pub\s+)?struct\s+(\w+)", SymbolKind::Struct),
            // Rust: enum Name
            (r"(?:pub\s+)?enum\s+(\w+)", SymbolKind::Enum),
            // Rust: trait Name
            (r"(?:pub\s+)?trait\s+(\w+)", SymbolKind::Trait),
            // Rust: impl blocks (impl Type, impl Trait for Type)
            (r"impl(?:<[^>]*>)?\s+(?:dyn\s+)?(\w+)", SymbolKind::Struct),
            // Rust: type alias
            (r"(?:pub\s+)?type\s+(\w+)", SymbolKind::TypeAlias),
            // Rust: macro_rules!
            (r"macro_rules!\s+(\w+)", SymbolKind::Macro),
            // Rust: const/static
            (r"(?:pub\s+)?(?:const|static)\s+(?:\w+\s*:)?\s*(\w+)", SymbolKind::Constant),
            // Python: def name
            (r"(?:async\s+)?def\s+(\w+)", SymbolKind::Function),
            // Python: class Name
            (r"class\s+(\w+)", SymbolKind::Class),
            // JavaScript/TypeScript: function name, export function name
            (r"(?:export\s+)?(?:async\s+)?function\s+(\w+)", SymbolKind::Function),
            // JavaScript/TypeScript: class Name
            (r"(?:export\s+)?class\s+(\w+)", SymbolKind::Class),
            // JavaScript/TypeScript: interface Name
            (r"(?:export\s+)?interface\s+(\w+)", SymbolKind::Interface),
            // JavaScript/TypeScript: type Name
            (r"(?:export\s+)?type\s+(\w+)", SymbolKind::TypeAlias),
            // Go: func name, func (recv) name
            (r"func\s+(?:\([^)]*\)\s+)?(\w+)", SymbolKind::Function),
            // Go: type Name struct/interface
            (r"type\s+(\w+)\s+(?:struct|interface)", SymbolKind::Struct),
            // Java/C#: public/private/protected ... type Name
            (r"(?:public|private|protected|internal)\s+(?:static\s+)?(?:class|interface|enum)\s+(\w+)", SymbolKind::Class),
            // C/C++: struct/class Name
            (r"(?:typedef\s+)?(?:struct|class)\s+(\w+)", SymbolKind::Struct),
            // C: function-like (return_type name(params)) — only match
            // identifiers followed by '(' on the same line, excluding
            // keywords. This is intentionally coarse.
            (r"^(?:static|inline|extern|const)?\s*\w[\w\s\*]*\b(\w+)\s*\(", SymbolKind::Function),
        ];

        for (line_idx, line) in text.lines().enumerate() {
            let line_lower = line.to_lowercase();
            if !line_lower.contains(&hint_lower) {
                continue;
            }
            for (pattern_str, kind) in patterns {
                if let Ok(re) = Regex::new(pattern_str) {
                    if let Some(caps) = re.captures(line) {
                        if let Some(name_match) = caps.get(1) {
                            let name = name_match.as_str();
                            if name.to_lowercase() == hint_lower {
                                let line_num = (line_idx + 1) as u32;
                                return Some((name.to_string(), *kind, line_num));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Convert local matches into SourceCards.
    pub fn to_source_cards(
        matches: &[LocalMatch],
        roots: &[(usize, PathBuf)],
    ) -> Vec<SourceCard> {
        matches
            .iter()
            .map(|m| {
                let root_name = roots
                    .get(m.file.root_index)
                    .map(|(_, p)| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("workspace")
                            .to_string()
                    })
                    .unwrap_or_else(|| "workspace".to_string());

                let pseudo_url = format!(
                    "workspace://{}/{}",
                    root_name, m.file.relative_path
                );

                let title = m.file.relative_path.clone();

                let language = m.file.language.clone();
                let source_role = crate::core::code_evidence::infer_source_role(&m.file.relative_path);

                let code_metadata = CodeMetadata {
                    host: None,
                    owner: None,
                    repo: Some(root_name.clone()),
                    path: Some(m.file.relative_path.clone()),
                    ref_name: None,
                    language: language.clone(),
                    symbol_hint: m.matched_symbol.clone(),
                    line_start: m.line_start,
                    line_end: m.line_end,
                };

                let code_evidence = CodeEvidence {
                    host: None,
                    owner: None,
                    repo: Some(root_name),
                    ref_name: None,
                    commit_sha: None,
                    path: Some(m.file.relative_path.clone()),
                    language: language.clone(),
                    source_role: Some(source_role),
                    browser_url: None,
                    raw_url: None,
                    permalink_url: None,
                    match_line_start: m.line_start,
                    match_line_end: m.line_end,
                    context_line_start: None,
                    context_line_end: None,
                    matched_symbol: m.matched_symbol.clone(),
                    symbol_kind: m.symbol_kind,
                    enclosing_symbol: None,
                    evidence_confidence: Some(EvidenceConfidence::Strong),
                    evidence_reasons: vec![CodeEvidenceReason::ProviderPathMatch],
                };

                let metadata = SourceMetadata {
                    source_kind: SourceKind::SourceFile,
                    domain: None,
                    rank_reasons: vec![RankReason::HintMatch],
                    code: Some(code_metadata),
                    issue: None,
                    release: None,
                    vulnerability: None,
                    code_evidence: Some(code_evidence),
                };

                let snippet = m.snippet.clone().unwrap_or_else(|| {
                    format!("Local file: {}", m.file.relative_path)
                });

                let trust_markers = TrustMarkers {
                    text_sanitized: false,
                    text_truncated: false,
                    text_framed: false,
                    control_chars_removed: 0,
                    injection_hits: 0,
                };

                SourceCard::new(title, &pseudo_url, vec!["local_workspace".to_string()], Some(m.score), TrustLevel::LocalTrusted)
                    .with_snippet(snippet)
                    .with_trust_markers(trust_markers)
                    .with_metadata(metadata)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("main.rs"), "fn main() {\n    println!(\"hello\");\n}").unwrap();
        fs::write(root.join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();
        fs::write(root.join("README.md"), "# My Project\n\nA test project.").unwrap();
        fs::write(root.join("config.toml"), "[server]\nport = 8080").unwrap();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/engine.rs"), "pub struct Engine {\n    name: String,\n}").unwrap();
        fs::write(root.join("src/utils.rs"), "pub fn helper() -> i32 { 42 }").unwrap();

        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("tests/integration.rs"), "#[test]\nfn test_add() { assert_eq!(1 + 1, 2); }").unwrap();

        fs::write(root.join(".hidden"), "secret").unwrap();
        fs::write(root.join("data.bin"), vec![0u8; 100]).unwrap();

        dir
    }

    #[test]
    fn backend_new_rejects_enabled_without_roots() {
        let cfg = LocalConfig {
            enabled: true,
            roots: Vec::new(),
            ..Default::default()
        };
        assert!(LocalWorkspaceBackend::new(cfg).is_err());
    }

    #[test]
    fn backend_new_accepts_disabled() {
        let cfg = LocalConfig::default();
        let backend = LocalWorkspaceBackend::new(cfg).unwrap();
        assert!(!backend.is_enabled());
    }

    #[test]
    fn backend_new_canonicalizes_roots() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = LocalConfig {
            enabled: true,
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let backend = LocalWorkspaceBackend::new(cfg).unwrap();
        assert!(backend.is_enabled());
        assert_eq!(backend.roots.len(), 1);
    }

    #[test]
    fn backend_new_rejects_missing_root() {
        let cfg = LocalConfig {
            enabled: true,
            roots: vec![PathBuf::from("/nonexistent/path")],
            ..Default::default()
        };
        assert!(LocalWorkspaceBackend::new(cfg).is_err());
    }

    #[test]
    fn score_file_exact_name_match() {
        let file = LocalFileEntry {
            path: PathBuf::from("/test/main.rs"),
            relative_path: "main.rs".to_string(),
            root_index: 0,
            size: 100,
            language: Some("rust".to_string()),
        };
        let score = LocalWorkspaceBackend::score_file(&file, "main.rs", &["main.rs"], None);
        assert!(score >= 100.0, "score should be high for exact match: {score}");
    }

    #[test]
    fn score_file_path_segment_match() {
        let file = LocalFileEntry {
            path: PathBuf::from("/test/src/engine.rs"),
            relative_path: "src/engine.rs".to_string(),
            root_index: 0,
            size: 100,
            language: Some("rust".to_string()),
        };
        let score = LocalWorkspaceBackend::score_file(&file, "engine", &["engine"], None);
        assert!(score > 0.0, "score should be positive for path match: {score}");
    }

    #[test]
    fn score_file_no_match() {
        let file = LocalFileEntry {
            path: PathBuf::from("/test/config.toml"),
            relative_path: "config.toml".to_string(),
            root_index: 0,
            size: 100,
            language: Some("toml".to_string()),
        };
        let score = LocalWorkspaceBackend::score_file(&file, "xyz", &["xyz"], None);
        assert_eq!(score, 0.0, "score should be 0 for no match");
    }

    #[test]
    fn score_file_penalty_for_generated() {
        let file = LocalFileEntry {
            path: PathBuf::from("/test/Cargo.lock"),
            relative_path: "Cargo.lock".to_string(),
            root_index: 0,
            size: 100,
            language: None,
        };
        let score = LocalWorkspaceBackend::score_file(&file, "cargo.lock", &["cargo.lock"], None);
        assert!(score < 0.0, "score should be negative for lock file: {score}");
    }

    #[test]
    fn to_source_cards_produces_cards() {
        let matches = vec![LocalMatch {
            file: LocalFileEntry {
                path: PathBuf::from("/test/main.rs"),
                relative_path: "main.rs".to_string(),
                root_index: 0,
                size: 100,
                language: Some("rust".to_string()),
            },
            score: 100.0,
            line_start: Some(1),
            line_end: Some(1),
            snippet: Some("fn main()".to_string()),
            matched_symbol: None,
            symbol_kind: None,
        }];
        let roots = vec![(0, PathBuf::from("/test"))];
        let cards = LocalWorkspaceBackend::to_source_cards(&matches, &roots);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].trust, TrustLevel::LocalTrusted);
        assert!(cards[0].url.starts_with("workspace://"));
        assert!(cards[0].metadata.code_evidence.is_some());
    }

    #[test]
    fn find_text_match_finds_line() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line one\nline two\nline three\n").unwrap();

        let (snippet, start, end) =
            LocalWorkspaceBackend::find_text_match(&file_path, "line two", 1048576);
        assert_eq!(snippet.as_deref(), Some("line two"));
        assert_eq!(start, Some(2));
        assert_eq!(end, Some(2));
    }

    #[test]
    fn find_text_match_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world\n").unwrap();

        let (snippet, start, end) =
            LocalWorkspaceBackend::find_text_match(&file_path, "xyz", 1048576);
        assert!(snippet.is_none());
        assert!(start.is_none());
        assert!(end.is_none());
    }

    #[test]
    fn find_symbol_match_detects_rust_fn() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("lib.rs");
        fs::write(
            &file_path,
            "pub fn helper(x: i32) -> i32 {\n    x + 1\n}\n\nfn main() {\n    println!(\"hi\");\n}\n",
        )
        .unwrap();

        let result =
            LocalWorkspaceBackend::find_symbol_match(&file_path, "helper", 1048576);
        assert!(result.is_some());
        let (name, kind, line) = result.unwrap();
        assert_eq!(name, "helper");
        assert_eq!(kind, SymbolKind::Function);
        assert_eq!(line, 1);
    }

    #[test]
    fn find_symbol_match_detects_rust_struct() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("model.rs");
        fs::write(
            &file_path,
            "#[derive(Debug)]\npub struct User {\n    name: String,\n}\n",
        )
        .unwrap();

        let result =
            LocalWorkspaceBackend::find_symbol_match(&file_path, "User", 1048576);
        assert!(result.is_some());
        let (name, kind, line) = result.unwrap();
        assert_eq!(name, "User");
        assert_eq!(kind, SymbolKind::Struct);
        assert_eq!(line, 2);
    }

    #[test]
    fn find_symbol_match_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("lib.rs");
        fs::write(&file_path, "fn main() {}\n").unwrap();

        let result =
            LocalWorkspaceBackend::find_symbol_match(&file_path, "nonexistent", 1048576);
        assert!(result.is_none());
    }

    #[test]
    fn find_symbol_match_case_insensitive_hint() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("app.py");
        fs::write(&file_path, "def my_function():\n    pass\n").unwrap();

        // Hint is lowercase "my_function", symbol in code is also "my_function"
        let result =
            LocalWorkspaceBackend::find_symbol_match(&file_path, "my_function", 1048576);
        assert!(result.is_some());
        let (name, kind, _line) = result.unwrap();
        assert_eq!(name, "my_function");
        assert_eq!(kind, SymbolKind::Function);
    }
}
