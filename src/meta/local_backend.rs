//! Local workspace search backend: bounded file walking, scoring, and
//! SourceCard conversion.
//!
//! The `LocalWorkspaceBackend` walks configured filesystem roots,
//! applies ignore rules and extension filters, scores path/text/language
//! matches, and converts results into [`SourceCard`](crate::core::source_card::SourceCard) values with
//! `TrustLevel::LocalTrusted`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::core::code_evidence::{
    CodeEvidence, CodeEvidenceReason, EvidenceConfidence, SourceRole, SymbolKind,
};
use crate::core::code_metadata::CodeMetadata;
use crate::core::local::{
    is_binary_extension, language_from_extension, FreshnessConfidence, InventoryTelemetry,
    LocalConfig, LocalFileEntry, LocalMatch, LocalSearchRequest, LocalSearchResult, SKIP_DIRS,
};
use crate::core::quality::compute_card_quality;
use crate::core::result::TrustLevel;
use crate::core::sanitize::TrustMarkers;
use crate::core::source_card::{RankReason, SourceCard, SourceKind, SourceMetadata};
use crate::meta::local::scan::{
    self, ScanOutcome, GIT_GREP_TIMEOUT, SECOND_PASS_MAX_BYTES, SECOND_PASS_MAX_FILES,
    SECOND_PASS_TIME_BUDGET,
};
use crate::meta::local_ignore::IgnoreStack;
use crate::meta::local_inventory_cache::{
    build_inventory, find_symbols_in_text, has_relevance, needs_rebuild, probe_needs_rebuild,
    score_inventory_entry, validate_entry, InventoryEntry, WorkspaceInventory,
    FRESHNESS_PROBE_INTERVAL, INVENTORY_REBUILD_TTL,
};
use crate::meta::local_symbols::{self, StructuredSymbol, SymbolInventoryCache, SymbolProvenance};
use crate::meta::safe_open::safe_read_file;

/// Local workspace search backend.
///
/// Constructed once at server startup when `[local].enabled = true`.
/// Walks configured roots on each search call, applying bounded scan
/// limits and deterministic scoring.
pub trait SymbolBackend: Send + Sync {
    /// Find symbol definitions matching the hint in the given text.
    fn find_symbols(&self, text: &str, hint: &str) -> Option<(String, SymbolKind, u32)>;
    /// Capability flags for structured operations.
    fn capabilities(&self) -> local_symbols::SymbolBackendCapabilities {
        local_symbols::SymbolBackendCapabilities {
            supports_definitions: false,
            supports_references: true,
            supports_enclosing: false,
            supports_implementors: false,
            structured_languages: Vec::new(),
            regex_fallback_available: true,
        }
    }
    /// Structured definition lookup with provenance.
    fn find_definition(
        &self,
        _relative_path: &str,
        _language: Option<&str>,
        text: &str,
        hint: &str,
    ) -> Option<(StructuredSymbol, SymbolProvenance)> {
        self.find_symbols(text, hint).map(|(name, kind, line)| {
            (
                StructuredSymbol {
                    name,
                    kind,
                    line_start: line,
                    line_end: line,
                    container: None,
                    visibility: None,
                    is_test: false,
                    relationship: None,
                },
                SymbolProvenance::RegexFallback,
            )
        })
    }
    /// Bounded lexical references for a symbol.
    fn find_references(&self, text: &str, symbol: &str, max_results: usize) -> Vec<(u32, String)> {
        local_symbols::find_references(text, symbol, max_results)
    }
}

/// Select and deterministically order a bounded set of inventory candidates.
pub fn select_inventory_candidates<'a>(
    entries: &'a [InventoryEntry],
    query_lower: &str,
    query_tokens: &[&str],
    path_hint_lower: Option<&str>,
    lang_hint: Option<&str>,
    file_hint: Option<&str>,
    max_results: usize,
) -> Vec<&'a InventoryEntry> {
    let budget = max_results.saturating_mul(2);
    if budget == 0 {
        return Vec::new();
    }

    let mut candidates: Vec<(f64, &InventoryEntry)> = entries
        .iter()
        .filter(|entry| {
            if let Some(lang) = lang_hint {
                if entry.language.as_deref() != Some(lang) {
                    return false;
                }
            }
            if let Some(file_hint) = file_hint {
                if !Path::new(&entry.relative_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
                    .contains(file_hint)
                {
                    return false;
                }
            }
            if let Some(path_hint_lower) = path_hint_lower {
                if !entry.relative_path.to_lowercase().contains(path_hint_lower) {
                    return false;
                }
            }
            true
        })
        .map(|entry| {
            (
                score_inventory_entry(entry, query_lower, query_tokens),
                entry,
            )
        })
        .collect();

    let compare = |left: &(f64, &InventoryEntry), right: &(f64, &InventoryEntry)| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.relative_path.cmp(&right.1.relative_path))
    };
    if candidates.len() > budget {
        candidates.select_nth_unstable_by(budget, compare);
        candidates.truncate(budget);
    }
    candidates.sort_unstable_by(compare);
    candidates.into_iter().map(|(_, entry)| entry).collect()
}

/// Regex-based symbol backend using compiled pattern matching.
pub struct RegexSymbolBackend;

impl SymbolBackend for RegexSymbolBackend {
    fn find_symbols(&self, text: &str, hint: &str) -> Option<(String, SymbolKind, u32)> {
        find_symbols_in_text(text, hint)
    }
}

/// Dependency-free structured symbol backend with regex fallback.
pub struct StructuredSymbolBackend {
    max_parse_bytes: usize,
    max_symbols_per_file: usize,
    cache: SymbolInventoryCache,
}

impl StructuredSymbolBackend {
    /// Create a backend from local workspace budgets.
    pub fn new(max_parse_bytes: usize, max_symbols_per_file: usize) -> Self {
        Self {
            max_parse_bytes,
            max_symbols_per_file,
            cache: SymbolInventoryCache::new(),
        }
    }

    /// Create a backend from a full local config.
    pub fn from_config(config: &LocalConfig) -> Self {
        Self::new(config.max_parse_bytes, config.max_symbols_per_file)
    }

    /// Number of cached files (telemetry/debugging).
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }
}

impl Default for StructuredSymbolBackend {
    fn default() -> Self {
        Self::new(
            local_symbols::DEFAULT_MAX_PARSE_BYTES,
            local_symbols::DEFAULT_MAX_SYMBOLS_PER_FILE,
        )
    }
}

impl SymbolBackend for StructuredSymbolBackend {
    fn find_symbols(&self, text: &str, hint: &str) -> Option<(String, SymbolKind, u32)> {
        if text.len() <= self.max_parse_bytes {
            for language in ["rust", "python", "javascript", "typescript", "go"] {
                let (symbols, provenance, _) = local_symbols::symbols_in_file(
                    "",
                    Some(language),
                    text,
                    self.max_parse_bytes,
                    self.max_symbols_per_file,
                );
                if provenance == SymbolProvenance::Structured {
                    if let Some(hit) = local_symbols::find_definition(&symbols, hint) {
                        return Some((hit.name.clone(), hit.kind, hit.line_start));
                    }
                }
            }
        }
        find_symbols_in_text(text, hint)
    }

    fn capabilities(&self) -> local_symbols::SymbolBackendCapabilities {
        local_symbols::SymbolBackendCapabilities {
            supports_definitions: true,
            supports_references: true,
            supports_enclosing: true,
            supports_implementors: true,
            structured_languages: vec![
                "rust".to_string(),
                "python".to_string(),
                "javascript".to_string(),
                "typescript".to_string(),
                "go".to_string(),
            ],
            regex_fallback_available: true,
        }
    }

    fn find_definition(
        &self,
        relative_path: &str,
        language: Option<&str>,
        text: &str,
        hint: &str,
    ) -> Option<(StructuredSymbol, SymbolProvenance)> {
        let hash = local_symbols::content_hash_for_symbols(text);
        if let Some((cached, provenance)) = self.cache.get(relative_path, hash) {
            if let Some(hit) = local_symbols::find_definition(&cached, hint) {
                return Some((hit.clone(), provenance));
            }
            if provenance == SymbolProvenance::Structured {
                return self.find_symbols(text, hint).map(|(name, kind, line)| {
                    (
                        StructuredSymbol {
                            name,
                            kind,
                            line_start: line,
                            line_end: line,
                            container: None,
                            visibility: None,
                            is_test: false,
                            relationship: None,
                        },
                        SymbolProvenance::RegexFallback,
                    )
                });
            }
            return None;
        }
        let (symbols, provenance, _) = local_symbols::symbols_in_file(
            relative_path,
            language,
            text,
            self.max_parse_bytes,
            self.max_symbols_per_file,
        );
        if provenance == SymbolProvenance::Structured {
            self.cache.insert(
                relative_path.to_string(),
                hash,
                symbols.clone(),
                provenance,
                1024,
            );
            if let Some(hit) = local_symbols::find_definition(&symbols, hint) {
                return Some((hit.clone(), provenance));
            }
            return self.find_symbols(text, hint).map(|(name, kind, line)| {
                (
                    StructuredSymbol {
                        name,
                        kind,
                        line_start: line,
                        line_end: line,
                        container: None,
                        visibility: None,
                        is_test: false,
                        relationship: None,
                    },
                    SymbolProvenance::RegexFallback,
                )
            });
        }
        self.find_symbols(text, hint).map(|(name, kind, line)| {
            (
                StructuredSymbol {
                    name,
                    kind,
                    line_start: line,
                    line_end: line,
                    container: None,
                    visibility: None,
                    is_test: false,
                    relationship: None,
                },
                SymbolProvenance::RegexFallback,
            )
        })
    }
}

/// Local workspace search backend.
///
/// Constructed once at server startup when `[local].enabled = true`.
/// Walks configured roots on each search call, applying bounded scan
/// limits and deterministic scoring.
pub struct LocalWorkspaceBackend {
    config: LocalConfig,
    roots: Vec<(usize, PathBuf)>,
    inventory_cache: Arc<RwLock<Option<Arc<WorkspaceInventory>>>>,
    symbol_backend: Arc<dyn SymbolBackend>,
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
                inventory_cache: Arc::new(RwLock::new(None)),
                symbol_backend: Arc::new(RegexSymbolBackend),
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
                return Err(format!("root {} is not a directory", root.display()));
            }
            roots.push((i, canonical));
        }
        let symbol_backend: Arc<dyn SymbolBackend> = if config.structured_symbols {
            Arc::new(StructuredSymbolBackend::from_config(&config))
        } else {
            Arc::new(RegexSymbolBackend)
        };
        Ok(Self {
            config,
            roots,
            inventory_cache: Arc::new(RwLock::new(None)),
            symbol_backend,
        })
    }

    /// Symbol backend capabilities for diagnostics and evidence metadata.
    pub fn symbol_capabilities(&self) -> local_symbols::SymbolBackendCapabilities {
        self.symbol_backend.capabilities()
    }

    /// Whether local search is enabled and has configured roots.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && !self.roots.is_empty()
    }

    /// Return the canonicalized roots (index, path) pairs.
    pub fn roots(&self) -> Vec<(usize, PathBuf)> {
        self.roots.clone()
    }

    /// Return a reference to the local configuration.
    pub fn config(&self) -> &LocalConfig {
        &self.config
    }

    /// Run a local workspace search.
    pub async fn search(&self, req: &LocalSearchRequest) -> LocalSearchResult {
        if !self.is_enabled() {
            return LocalSearchResult::default();
        }

        let timeout_ms = req
            .timeout_ms
            .unwrap_or((self.config.max_indexed_files as u64 / 100).max(250));
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
        let inventory_cache = self.inventory_cache.clone();
        let symbol_backend = self.symbol_backend.clone();

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
                &inventory_cache,
                symbol_backend.as_ref(),
            )
        })
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "local search task panicked");
            LocalSearchResult::default()
        });

        result
    }

    /// Return the cached workspace inventory, rebuilding if stale or absent.
    pub fn get_or_build_inventory(&self) -> Option<WorkspaceInventory> {
        self.get_or_build_inventory_shared()
            .map(|inventory| (*inventory).clone())
    }

    pub(crate) fn get_or_build_inventory_shared(&self) -> Option<Arc<WorkspaceInventory>> {
        {
            let cache = self.inventory_cache.read().ok()?;
            if let Some(ref inv) = *cache {
                let age = inv.built_at.elapsed();
                if age < FRESHNESS_PROBE_INTERVAL {
                    return Some(Arc::clone(inv));
                }
                if age < INVENTORY_REBUILD_TTL && !probe_needs_rebuild(inv) {
                    return Some(Arc::clone(inv));
                }
                if !needs_rebuild(inv, &self.config, INVENTORY_REBUILD_TTL) {
                    return Some(Arc::clone(inv));
                }
            }
        }
        let inventory = build_inventory(&self.config, &self.roots);
        let inventory = Arc::new(inventory);
        let mut cache = self.inventory_cache.write().ok()?;
        *cache = Some(Arc::clone(&inventory));
        Some(inventory)
    }

    fn score_from_inventory_entry(
        entry: &InventoryEntry,
        query_lower: &str,
        query_tokens: &[&str],
        symbol_hint: Option<&str>,
        content_text: Option<&str>,
        symbol_backend: &dyn SymbolBackend,
    ) -> (f64, Option<SymbolProvenance>, bool) {
        let mut score = score_inventory_entry(entry, query_lower, query_tokens);

        if let Some(text) = content_text {
            let text_lower = text.to_lowercase();
            if !query_lower.is_empty() && text_lower.contains(query_lower) {
                score += 50.0;
            }
            let mut token_score: f64 = 0.0;
            for token in query_tokens {
                if text_lower.contains(token) {
                    token_score += 5.0;
                }
            }
            score += token_score.min(30.0);
        }

        if let (Some(sym), Some(text)) = (symbol_hint, content_text) {
            if let Some((hit, provenance)) = symbol_backend.find_definition(
                &entry.relative_path,
                entry.language.as_deref(),
                text,
                sym,
            ) {
                let exact = hit.name == sym;
                let boost = match provenance {
                    SymbolProvenance::Structured if exact => 80.0,
                    SymbolProvenance::Structured => 50.0,
                    SymbolProvenance::RegexFallback => 30.0,
                };
                score += boost;
                return (score, Some(provenance), exact);
            }
        }

        (score, None, false)
    }

    fn record_symbol_telemetry(
        telemetry: &mut InventoryTelemetry,
        symbol_hint: Option<&str>,
        provenance: Option<SymbolProvenance>,
        content_text: Option<&str>,
        config: &LocalConfig,
    ) {
        if symbol_hint.is_none() {
            return;
        }
        match provenance {
            Some(SymbolProvenance::Structured) => {
                telemetry.structured_files_parsed += 1;
                telemetry.structured_symbols_found += 1;
            }
            Some(SymbolProvenance::RegexFallback) => {
                telemetry.regex_fallback_files += 1;
            }
            None => {}
        }
        if content_text.is_some_and(|text| text.len() > config.max_parse_bytes) {
            telemetry.symbol_budget_breaches += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn inventory_match_for_entry(
        root_path: &Path,
        entry: &InventoryEntry,
        query_lower: &str,
        symbol_hint: Option<&str>,
        config: &LocalConfig,
        score: f64,
        symbol_provenance: Option<SymbolProvenance>,
        is_exact: bool,
        content_text: Option<&str>,
        symbol_backend: &dyn SymbolBackend,
    ) -> LocalMatch {
        let file_entry = LocalFileEntry {
            path: root_path.join(&entry.relative_path),
            relative_path: entry.relative_path.clone(),
            root_index: entry.root_index,
            size: entry.size,
            language: entry.language.clone(),
        };

        let (
            snippet,
            line_start,
            line_end,
            matched_symbol,
            symbol_kind,
            boosted_score,
            enclosing_symbol,
        ) = if let Some(sym_hint) = symbol_hint {
            if let Some(text) = content_text {
                if let Some((hit, provenance)) = symbol_backend.find_definition(
                    &entry.relative_path,
                    entry.language.as_deref(),
                    text,
                    sym_hint,
                ) {
                    let snippet = Self::find_text_match_in_text(text, &hit.name);
                    let extra = match provenance {
                        SymbolProvenance::Structured if hit.name == sym_hint => 0.0,
                        SymbolProvenance::Structured => 0.0,
                        SymbolProvenance::RegexFallback => 30.0,
                    };
                    (
                        snippet,
                        Some(hit.line_start),
                        Some(hit.line_end),
                        Some(hit.name.clone()),
                        Some(hit.kind),
                        score + extra,
                        hit.container.clone(),
                    )
                } else if !query_lower.is_empty() {
                    let (snippet, line_start, line_end) =
                        Self::find_text_match(root_path, &entry.relative_path, query_lower, config);
                    (snippet, line_start, line_end, None, None, score, None)
                } else {
                    (None, None, None, None, None, score, None)
                }
            } else if !query_lower.is_empty() {
                let (snippet, line_start, line_end) =
                    Self::find_text_match(root_path, &entry.relative_path, query_lower, config);
                (snippet, line_start, line_end, None, None, score, None)
            } else {
                (None, None, None, None, None, score, None)
            }
        } else if !query_lower.is_empty() {
            if let Some(text) = content_text {
                (
                    Self::find_text_match_in_text(text, query_lower),
                    None,
                    None,
                    None,
                    None,
                    score,
                    None,
                )
            } else {
                let (snippet, line_start, line_end) =
                    Self::find_text_match(root_path, &entry.relative_path, query_lower, config);
                (snippet, line_start, line_end, None, None, score, None)
            }
        } else {
            (None, None, None, None, None, score, None)
        };

        let (provenance_label, exact_flag) = match symbol_provenance {
            Some(SymbolProvenance::Structured) => (
                Some(SymbolProvenance::Structured.as_str().to_string()),
                Some(is_exact),
            ),
            Some(SymbolProvenance::RegexFallback) => (
                Some(SymbolProvenance::RegexFallback.as_str().to_string()),
                Some(false),
            ),
            None if matched_symbol.is_some() => (
                Some(SymbolProvenance::RegexFallback.as_str().to_string()),
                Some(false),
            ),
            None => (None, None),
        };

        LocalMatch {
            file: file_entry,
            score: boosted_score,
            line_start,
            line_end,
            snippet,
            matched_symbol,
            symbol_kind,
            symbol_provenance: provenance_label,
            enclosing_symbol,
            is_exact_definition: exact_flag,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_sync(
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
        inventory_cache: &RwLock<Option<Arc<WorkspaceInventory>>>,
        symbol_backend: &dyn SymbolBackend,
    ) -> LocalSearchResult {
        let mut matches = Vec::new();
        let mut files_scanned = 0usize;
        let mut scanned_paths: HashSet<(usize, String)> = HashSet::new();
        let mut truncated = false;
        let mut timed_out = false;
        let mut telemetry = InventoryTelemetry {
            used_inventory: false,
            inventory_entries: 0,
            candidates_filtered: 0,
            content_reads: 0,
            inventory_build_time_ms: 0,
            inventory_fresh: false,
            cold_build: false,
            stale_rebuild: false,
            fallback_walk: false,
            uses_git_backend: false,
            untracked_file_count: None,
            freshness_confidence: None,
            structured_files_parsed: 0,
            structured_symbols_found: 0,
            regex_fallback_files: 0,
            symbol_cache_hits: 0,
            symbol_budget_breaches: 0,
        };

        let query_lower = query.to_lowercase();
        let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();
        let path_hint_lower = path_hint.map(str::to_lowercase);

        let inventory = {
            let cache = inventory_cache.read().ok();
            cache.and_then(|c| c.as_ref().map(Arc::clone))
        };
        let mut second_pass_inventory = None;
        let mut scan_start = start;

        if let Some(inv) = inventory
            .as_ref()
            .filter(|inv| !needs_rebuild(inv, config, INVENTORY_REBUILD_TTL))
        {
            second_pass_inventory = Some(Arc::clone(inv));
            telemetry.used_inventory = true;
            telemetry.inventory_fresh = true;
            telemetry.inventory_entries = inv.roots.iter().map(|r| r.entries.len()).sum();
            let inventory_truncated = inv.roots.iter().any(|root| root.truncated);

            let age_secs = inv.built_at.elapsed().as_secs();
            telemetry.freshness_confidence = if age_secs < FRESHNESS_PROBE_INTERVAL.as_secs() {
                Some(FreshnessConfidence::High)
            } else if age_secs < INVENTORY_REBUILD_TTL.as_secs() {
                Some(FreshnessConfidence::Medium)
            } else {
                Some(FreshnessConfidence::Low)
            };

            for root_inv in &inv.roots {
                if scan_start.elapsed() > timeout {
                    timed_out = true;
                    break;
                }

                let candidates = select_inventory_candidates(
                    &root_inv.entries,
                    &query_lower,
                    &query_tokens,
                    path_hint_lower.as_deref(),
                    lang_hint,
                    file_hint,
                    max_results,
                );
                telemetry.candidates_filtered += candidates.len();

                for entry in candidates {
                    if scan_start.elapsed() > timeout {
                        timed_out = true;
                        break;
                    }
                    if truncated {
                        break;
                    }
                    if files_scanned >= config.max_indexed_files {
                        truncated = true;
                        break;
                    }
                    files_scanned += 1;
                    scanned_paths.insert((root_inv.root_index, entry.relative_path.clone()));

                    let root_path = &root_inv.root_path;

                    if !validate_entry(entry, config) {
                        continue;
                    }

                    let content_text = safe_read_file(
                        root_path,
                        &entry.relative_path,
                        config,
                        config.max_file_bytes,
                    )
                    .ok()
                    .and_then(|bytes| {
                        let s = String::from_utf8_lossy(&bytes).to_string();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s)
                        }
                    });

                    if content_text.is_some() {
                        telemetry.content_reads += 1;
                    }

                    let (score, symbol_provenance, is_exact) = Self::score_from_inventory_entry(
                        entry,
                        &query_lower,
                        &query_tokens,
                        symbol_hint,
                        content_text.as_deref(),
                        symbol_backend,
                    );
                    Self::record_symbol_telemetry(
                        &mut telemetry,
                        symbol_hint,
                        symbol_provenance,
                        content_text.as_deref(),
                        config,
                    );

                    if score <= 0.0
                        || !has_relevance(
                            &entry.relative_path,
                            &query_lower,
                            &query_tokens,
                            content_text.as_deref(),
                            path_hint_lower.as_deref(),
                            file_hint,
                            symbol_provenance.is_some(),
                        )
                    {
                        continue;
                    }

                    let file_entry = LocalFileEntry {
                        path: root_path.join(&entry.relative_path),
                        relative_path: entry.relative_path.clone(),
                        root_index: root_inv.root_index,
                        size: entry.size,
                        language: entry.language.clone(),
                    };

                    let (
                        snippet,
                        line_start,
                        line_end,
                        matched_symbol,
                        symbol_kind,
                        boosted_score,
                        enclosing_symbol,
                    ) = if let Some(sym_hint) = symbol_hint {
                        if let Some(ref text) = content_text {
                            if let Some((hit, provenance)) = symbol_backend.find_definition(
                                &entry.relative_path,
                                entry.language.as_deref(),
                                text,
                                sym_hint,
                            ) {
                                let snippet = Self::find_text_match_in_text(text, &hit.name);
                                let extra = match provenance {
                                    SymbolProvenance::Structured if hit.name == sym_hint => 0.0,
                                    SymbolProvenance::Structured => 0.0,
                                    SymbolProvenance::RegexFallback => 30.0,
                                };
                                let boosted = score + extra;
                                (
                                    snippet,
                                    Some(hit.line_start),
                                    Some(hit.line_end),
                                    Some(hit.name.clone()),
                                    Some(hit.kind),
                                    boosted,
                                    hit.container.clone(),
                                )
                            } else if !query_lower.is_empty() {
                                let (s, ls, le) = Self::find_text_match(
                                    root_path,
                                    &entry.relative_path,
                                    &query_lower,
                                    config,
                                );
                                (s, ls, le, None, None, score, None)
                            } else {
                                (None, None, None, None, None, score, None)
                            }
                        } else if !query_lower.is_empty() {
                            let (s, ls, le) = Self::find_text_match(
                                root_path,
                                &entry.relative_path,
                                &query_lower,
                                config,
                            );
                            (s, ls, le, None, None, score, None)
                        } else {
                            (None, None, None, None, None, score, None)
                        }
                    } else if !query_lower.is_empty() {
                        if let Some(ref text) = content_text {
                            let snippet = Self::find_text_match_in_text(text, &query_lower);
                            (snippet, None, None, None, None, score, None)
                        } else {
                            let (s, ls, le) = Self::find_text_match(
                                root_path,
                                &entry.relative_path,
                                &query_lower,
                                config,
                            );
                            (s, ls, le, None, None, score, None)
                        }
                    } else {
                        (None, None, None, None, None, score, None)
                    };

                    let (provenance_label, exact_flag) = match symbol_provenance {
                        Some(SymbolProvenance::Structured) => (
                            Some(SymbolProvenance::Structured.as_str().to_string()),
                            Some(is_exact),
                        ),
                        Some(SymbolProvenance::RegexFallback) => (
                            Some(SymbolProvenance::RegexFallback.as_str().to_string()),
                            Some(false),
                        ),
                        None if matched_symbol.is_some() => (
                            Some(SymbolProvenance::RegexFallback.as_str().to_string()),
                            Some(false),
                        ),
                        None => (None, None),
                    };

                    matches.push(LocalMatch {
                        file: file_entry,
                        score: boosted_score,
                        line_start,
                        line_end,
                        snippet,
                        matched_symbol,
                        symbol_kind,
                        symbol_provenance: provenance_label,
                        enclosing_symbol,
                        is_exact_definition: exact_flag,
                    });
                }
            }
            if inventory_truncated {
                truncated = true;
            }
        } else {
            let is_stale = inventory
                .as_ref()
                .is_some_and(|inv| needs_rebuild(inv, config, INVENTORY_REBUILD_TTL));
            let is_cold = inventory.is_none();

            let build_start = Instant::now();
            let new_inventory = Arc::new(build_inventory(config, roots));
            let build_time = build_start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            scan_start = Instant::now();

            let total_entries: usize = new_inventory.roots.iter().map(|r| r.entry_count).sum();
            let inventory_truncated = new_inventory.roots.iter().any(|root| root.truncated);

            if total_entries == 0 {
                telemetry.fallback_walk = true;
                telemetry.inventory_build_time_ms = build_time;

                for &(root_index, ref root_path) in roots {
                    if scan_start.elapsed() > timeout {
                        timed_out = true;
                        break;
                    }

                    let ignore_stack = if config.respect_gitignore {
                        IgnoreStack::build(root_path, root_path)
                    } else {
                        IgnoreStack::new()
                    };

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
                        &ignore_stack,
                        &mut matches,
                        &mut files_scanned,
                        max_results,
                        &scan_start,
                        timeout,
                        &mut timed_out,
                        &mut truncated,
                    );

                    if timed_out || truncated {
                        break;
                    }
                }
                if inventory_truncated {
                    truncated = true;
                }
            } else {
                {
                    if let Ok(mut cache) = inventory_cache.write() {
                        *cache = Some(Arc::clone(&new_inventory));
                    }
                }

                let inv = new_inventory;
                second_pass_inventory = Some(Arc::clone(&inv));
                telemetry.used_inventory = true;
                telemetry.inventory_fresh = false;
                telemetry.cold_build = is_cold;
                telemetry.stale_rebuild = is_stale;
                telemetry.inventory_build_time_ms = build_time;
                telemetry.inventory_entries = inv.roots.iter().map(|r| r.entries.len()).sum();
                telemetry.uses_git_backend = inv.roots.iter().any(|r| r.uses_git_backend);
                let total_untracked: usize = inv.roots.iter().map(|r| r.untracked_count).sum();
                if total_untracked > 0 {
                    telemetry.untracked_file_count = Some(total_untracked);
                }
                for root_inv in &inv.roots {
                    if scan_start.elapsed() > timeout {
                        timed_out = true;
                        break;
                    }

                    let candidates = select_inventory_candidates(
                        &root_inv.entries,
                        &query_lower,
                        &query_tokens,
                        path_hint_lower.as_deref(),
                        lang_hint,
                        file_hint,
                        max_results,
                    );
                    telemetry.candidates_filtered += candidates.len();

                    for entry in candidates {
                        if scan_start.elapsed() > timeout {
                            timed_out = true;
                            break;
                        }
                        if truncated {
                            break;
                        }
                        if files_scanned >= config.max_indexed_files {
                            truncated = true;
                            break;
                        }
                        files_scanned += 1;
                        scanned_paths.insert((root_inv.root_index, entry.relative_path.clone()));

                        let root_path = &root_inv.root_path;

                        if !validate_entry(entry, config) {
                            continue;
                        }

                        let content_text = safe_read_file(
                            root_path,
                            &entry.relative_path,
                            config,
                            config.max_file_bytes,
                        )
                        .ok()
                        .and_then(|bytes| {
                            let s = String::from_utf8_lossy(&bytes).to_string();
                            if s.is_empty() {
                                None
                            } else {
                                Some(s)
                            }
                        });

                        if content_text.is_some() {
                            telemetry.content_reads += 1;
                        }

                        let (score, symbol_provenance, is_exact) = Self::score_from_inventory_entry(
                            entry,
                            &query_lower,
                            &query_tokens,
                            symbol_hint,
                            content_text.as_deref(),
                            symbol_backend,
                        );
                        Self::record_symbol_telemetry(
                            &mut telemetry,
                            symbol_hint,
                            symbol_provenance,
                            content_text.as_deref(),
                            config,
                        );

                        if score <= 0.0
                            || !has_relevance(
                                &entry.relative_path,
                                &query_lower,
                                &query_tokens,
                                content_text.as_deref(),
                                path_hint_lower.as_deref(),
                                file_hint,
                                symbol_provenance.is_some(),
                            )
                        {
                            continue;
                        }

                        let file_entry = LocalFileEntry {
                            path: root_path.join(&entry.relative_path),
                            relative_path: entry.relative_path.clone(),
                            root_index: root_inv.root_index,
                            size: entry.size,
                            language: entry.language.clone(),
                        };

                        let (
                            snippet,
                            line_start,
                            line_end,
                            matched_symbol,
                            symbol_kind,
                            boosted_score,
                            enclosing_symbol,
                        ) = if let Some(sym_hint) = symbol_hint {
                            if let Some(ref text) = content_text {
                                if let Some((hit, provenance)) = symbol_backend.find_definition(
                                    &entry.relative_path,
                                    entry.language.as_deref(),
                                    text,
                                    sym_hint,
                                ) {
                                    let snippet = Self::find_text_match_in_text(text, &hit.name);
                                    let extra = match provenance {
                                        SymbolProvenance::Structured => 0.0,
                                        SymbolProvenance::RegexFallback => 30.0,
                                    };
                                    let boosted = score + extra;
                                    (
                                        snippet,
                                        Some(hit.line_start),
                                        Some(hit.line_end),
                                        Some(hit.name.clone()),
                                        Some(hit.kind),
                                        boosted,
                                        hit.container.clone(),
                                    )
                                } else if !query_lower.is_empty() {
                                    let (s, ls, le) = Self::find_text_match(
                                        root_path,
                                        &entry.relative_path,
                                        &query_lower,
                                        config,
                                    );
                                    (s, ls, le, None, None, score, None)
                                } else {
                                    (None, None, None, None, None, score, None)
                                }
                            } else if !query_lower.is_empty() {
                                let (s, ls, le) = Self::find_text_match(
                                    root_path,
                                    &entry.relative_path,
                                    &query_lower,
                                    config,
                                );
                                (s, ls, le, None, None, score, None)
                            } else {
                                (None, None, None, None, None, score, None)
                            }
                        } else if !query_lower.is_empty() {
                            if let Some(ref text) = content_text {
                                let snippet = Self::find_text_match_in_text(text, &query_lower);
                                (snippet, None, None, None, None, score, None)
                            } else {
                                let (s, ls, le) = Self::find_text_match(
                                    root_path,
                                    &entry.relative_path,
                                    &query_lower,
                                    config,
                                );
                                (s, ls, le, None, None, score, None)
                            }
                        } else {
                            (None, None, None, None, None, score, None)
                        };

                        let (provenance_label, exact_flag) = match symbol_provenance {
                            Some(SymbolProvenance::Structured) => (
                                Some(SymbolProvenance::Structured.as_str().to_string()),
                                Some(is_exact),
                            ),
                            Some(SymbolProvenance::RegexFallback) => (
                                Some(SymbolProvenance::RegexFallback.as_str().to_string()),
                                Some(false),
                            ),
                            None if matched_symbol.is_some() => (
                                Some(SymbolProvenance::RegexFallback.as_str().to_string()),
                                Some(false),
                            ),
                            None => (None, None),
                        };

                        matches.push(LocalMatch {
                            file: file_entry,
                            score: boosted_score,
                            line_start,
                            line_end,
                            snippet,
                            matched_symbol,
                            symbol_kind,
                            symbol_provenance: provenance_label,
                            enclosing_symbol,
                            is_exact_definition: exact_flag,
                        });
                    }
                }

                if inventory_truncated {
                    truncated = true;
                }
            }
        }

        if scan::should_run_second_pass(
            matches.len(),
            max_results,
            timed_out,
            files_scanned,
            config.max_indexed_files,
            &query_lower,
            path_hint,
            file_hint,
            symbol_hint,
        ) {
            if let Some(inv) = second_pass_inventory.as_ref() {
                if Self::run_second_pass(
                    config,
                    inv,
                    &query_lower,
                    &query_tokens,
                    path_hint_lower.as_deref(),
                    path_hint,
                    lang_hint,
                    file_hint,
                    symbol_hint,
                    max_results,
                    timeout,
                    &scan_start,
                    &mut scanned_paths,
                    &mut matches,
                    &mut files_scanned,
                    &mut timed_out,
                    &mut telemetry,
                    symbol_backend,
                ) == ScanOutcome::CutShort
                {
                    truncated = true;
                }
            }
        }

        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if matches.len() > max_results {
            matches.truncate(max_results);
            truncated = true;
        }

        LocalSearchResult {
            matches,
            files_scanned,
            truncated,
            timed_out,
            telemetry: Some(telemetry),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_second_pass(
        config: &LocalConfig,
        inventory: &WorkspaceInventory,
        query_lower: &str,
        query_tokens: &[&str],
        path_hint_lower: Option<&str>,
        path_hint: Option<&str>,
        lang_hint: Option<&str>,
        file_hint: Option<&str>,
        symbol_hint: Option<&str>,
        max_results: usize,
        timeout: Duration,
        start: &Instant,
        scanned_paths: &mut HashSet<(usize, String)>,
        matches: &mut Vec<LocalMatch>,
        files_scanned: &mut usize,
        timed_out: &mut bool,
        telemetry: &mut InventoryTelemetry,
        symbol_backend: &dyn SymbolBackend,
    ) -> ScanOutcome {
        let pass_start = Instant::now();
        let mut read_files = 0usize;
        let mut read_bytes = 0usize;
        for root_inv in &inventory.roots {
            if matches.len() >= max_results {
                return ScanOutcome::Exhausted;
            }
            if start.elapsed() > timeout {
                *timed_out = true;
                return ScanOutcome::CutShort;
            }
            if pass_start.elapsed() > SECOND_PASS_TIME_BUDGET {
                return ScanOutcome::CutShort;
            }

            let mut entries: Vec<_> = root_inv.entries.iter().collect();
            entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
            let unscanned = entries
                .iter()
                .filter(|entry| {
                    !scanned_paths.contains(&(root_inv.root_index, entry.relative_path.clone()))
                })
                .count();
            if unscanned == 0 {
                continue;
            }

            let hint_paths = if scan::should_use_git_hint(unscanned) {
                let pass_remaining = SECOND_PASS_TIME_BUDGET.saturating_sub(pass_start.elapsed());
                let request_remaining = timeout.saturating_sub(start.elapsed());
                let git_timeout = GIT_GREP_TIMEOUT
                    .min(SECOND_PASS_TIME_BUDGET / 2)
                    .min(pass_remaining)
                    .min(request_remaining);
                if git_timeout.is_zero() {
                    if start.elapsed() > timeout {
                        *timed_out = true;
                    }
                    return ScanOutcome::CutShort;
                }
                scan::git_literal_hint(
                    &root_inv.root_path,
                    query_lower,
                    query_tokens,
                    symbol_hint,
                    config.respect_gitignore,
                    git_timeout,
                )
            } else {
                None
            };
            if start.elapsed() > timeout {
                *timed_out = true;
                return ScanOutcome::CutShort;
            }
            if pass_start.elapsed() > SECOND_PASS_TIME_BUDGET {
                return ScanOutcome::CutShort;
            }

            for entry in entries {
                if matches.len() >= max_results {
                    return ScanOutcome::Exhausted;
                }
                if start.elapsed() > timeout {
                    *timed_out = true;
                    return ScanOutcome::CutShort;
                }
                if pass_start.elapsed() > SECOND_PASS_TIME_BUDGET {
                    return ScanOutcome::CutShort;
                }
                if scanned_paths.contains(&(root_inv.root_index, entry.relative_path.clone()))
                    || scan::should_skip_git_candidate(
                        &entry.relative_path,
                        query_tokens,
                        hint_paths.as_ref(),
                        path_hint,
                        file_hint,
                    )
                {
                    continue;
                }
                if lang_hint.is_some_and(|language| entry.language.as_deref() != Some(language)) {
                    continue;
                }
                if file_hint.is_some_and(|hint| {
                    !Path::new(&entry.relative_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("")
                        .contains(hint)
                }) {
                    continue;
                }
                if path_hint_lower
                    .is_some_and(|hint| !entry.relative_path.to_lowercase().contains(hint))
                {
                    continue;
                }
                if !validate_entry(entry, config) {
                    continue;
                }
                if *files_scanned >= config.max_indexed_files || read_files >= SECOND_PASS_MAX_FILES
                {
                    return ScanOutcome::CutShort;
                }

                let remaining_bytes = SECOND_PASS_MAX_BYTES.saturating_sub(read_bytes);
                if usize::try_from(entry.size).unwrap_or(usize::MAX) > remaining_bytes {
                    return ScanOutcome::CutShort;
                }
                let max_read_bytes = config.max_file_bytes.min(remaining_bytes);
                *files_scanned += 1;
                read_files += 1;
                scanned_paths.insert((root_inv.root_index, entry.relative_path.clone()));
                let content_bytes = match safe_read_file(
                    &root_inv.root_path,
                    &entry.relative_path,
                    config,
                    max_read_bytes,
                ) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        if matches!(
                            error,
                            crate::meta::safe_open::SafeOpenError::FileContentLimitExceeded(..)
                        ) {
                            return ScanOutcome::CutShort;
                        }
                        if start.elapsed() > timeout {
                            *timed_out = true;
                            return ScanOutcome::CutShort;
                        }
                        if pass_start.elapsed() > SECOND_PASS_TIME_BUDGET {
                            return ScanOutcome::CutShort;
                        }
                        continue;
                    }
                };
                read_bytes = read_bytes.saturating_add(content_bytes.len());
                telemetry.content_reads += 1;
                if start.elapsed() > timeout {
                    *timed_out = true;
                    return ScanOutcome::CutShort;
                }
                if pass_start.elapsed() > SECOND_PASS_TIME_BUDGET {
                    return ScanOutcome::CutShort;
                }
                let content_text = if content_bytes.is_empty() {
                    None
                } else {
                    Some(String::from_utf8_lossy(&content_bytes).to_string())
                };
                let (score, symbol_provenance, is_exact) = Self::score_from_inventory_entry(
                    entry,
                    query_lower,
                    query_tokens,
                    symbol_hint,
                    content_text.as_deref(),
                    symbol_backend,
                );
                Self::record_symbol_telemetry(
                    telemetry,
                    symbol_hint,
                    symbol_provenance,
                    content_text.as_deref(),
                    config,
                );

                if score > 0.0
                    && has_relevance(
                        &entry.relative_path,
                        query_lower,
                        query_tokens,
                        content_text.as_deref(),
                        path_hint_lower,
                        file_hint,
                        symbol_provenance.is_some(),
                    )
                {
                    matches.push(Self::inventory_match_for_entry(
                        &root_inv.root_path,
                        entry,
                        query_lower,
                        symbol_hint,
                        config,
                        score,
                        symbol_provenance,
                        is_exact,
                        content_text.as_deref(),
                        symbol_backend,
                    ));
                }
            }
        }

        ScanOutcome::Exhausted
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
        ignore_stack: &IgnoreStack,
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

        let mut entries: Vec<_> = walk_dir.flatten().collect();
        entries.sort_by_key(|a| a.file_name());

        for entry in entries {
            if start.elapsed() > timeout {
                *timed_out = true;
                return;
            }
            if *truncated {
                return;
            }

            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if !config.include_hidden && file_name_str.starts_with('.') {
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

            if config.respect_gitignore {
                let is_dir = path.is_dir();
                if ignore_stack.is_ignored(root_path, &path, is_dir) {
                    continue;
                }
            }

            if path.is_dir() {
                if SKIP_DIRS.contains(&file_name_str.as_ref()) {
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
                    ignore_stack,
                    matches,
                    files_scanned,
                    max_results,
                    start,
                    timeout,
                    timed_out,
                    truncated,
                );
                if *truncated {
                    return;
                }
            } else if path.is_file()
                && Self::consider_file(
                    &path,
                    &file_name_str,
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
                    truncated,
                )
            {
                return;
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
        ignore_stack: &IgnoreStack,
        matches: &mut Vec<LocalMatch>,
        files_scanned: &mut usize,
        max_results: usize,
        start: &Instant,
        timeout: Duration,
        timed_out: &mut bool,
        truncated: &mut bool,
    ) {
        let read_dir = match std::fs::read_dir(dir) {
            Ok(d) => d,
            Err(_) => return,
        };

        let mut entries: Vec<_> = read_dir.flatten().collect();
        entries.sort_by_key(|a| a.file_name());

        for entry in entries {
            if start.elapsed() > timeout {
                *timed_out = true;
                return;
            }
            if *truncated || matches.len() > max_results {
                *truncated = true;
                return;
            }

            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if !config.include_hidden && file_name_str.starts_with('.') {
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

            if config.respect_gitignore {
                let is_dir = path.is_dir();
                if ignore_stack.is_ignored(root_path, &path, is_dir) {
                    continue;
                }
            }

            if path.is_dir() {
                if SKIP_DIRS.contains(&file_name_str.as_ref()) {
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
                    ignore_stack,
                    matches,
                    files_scanned,
                    max_results,
                    start,
                    timeout,
                    timed_out,
                    truncated,
                );
                if *truncated {
                    return;
                }
            } else if path.is_file()
                && Self::consider_file(
                    &path,
                    &file_name_str,
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
                    truncated,
                )
            {
                return;
            }
        }
    }

    /// Score, snippet-extract, and possibly emit a match for a single file.
    ///
    /// Returns `true` when the caller should stop iterating (files-scanned
    /// cap hit and `truncated` was set); returns `false` to continue.
    #[allow(clippy::too_many_arguments)]
    fn consider_file(
        path: &Path,
        file_name_str: &str,
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
        truncated: &mut bool,
    ) -> bool {
        if *files_scanned >= config.max_indexed_files {
            *truncated = true;
            return true;
        }
        *files_scanned += 1;

        if is_binary_extension(file_name_str) {
            return false;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return false,
        };
        if metadata.len() > config.max_file_bytes as u64 {
            return false;
        }

        let relative_path = path
            .strip_prefix(root_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let language = language_from_extension(&relative_path);

        let file_entry = LocalFileEntry {
            path: path.to_path_buf(),
            relative_path: relative_path.clone(),
            root_index,
            size: metadata.len(),
            language: language.map(|s| s.to_string()),
        };

        if let Some(lang) = lang_hint {
            if file_entry.language.as_deref() != Some(lang) {
                return false;
            }
        }
        if let Some(fh) = file_hint {
            if !file_name_str.contains(fh) {
                return false;
            }
        }
        if let Some(ph) = path_hint {
            if !relative_path.to_lowercase().contains(&ph.to_lowercase()) {
                return false;
            }
        }

        let content_text = safe_read_file(root_path, &relative_path, config, config.max_file_bytes)
            .ok()
            .and_then(|bytes| {
                let s = String::from_utf8_lossy(&bytes).to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            });

        let score = Self::score_file(
            &file_entry,
            query_lower,
            query_tokens,
            symbol_hint,
            content_text.as_deref(),
        );

        if score <= 0.0 {
            return false;
        }

        let (snippet, line_start, line_end, matched_symbol, symbol_kind, boosted_score) =
            if let Some(sym_hint) = symbol_hint {
                if let Some(ref text) = content_text {
                    if let Some((hit, _)) = StructuredSymbolBackend::default().find_definition(
                        &relative_path,
                        language,
                        text,
                        sym_hint,
                    ) {
                        let snippet = Self::find_text_match_in_text(text, &hit.name);
                        let boosted = score + 30.0;
                        (
                            snippet,
                            Some(hit.line_start),
                            Some(hit.line_end),
                            Some(hit.name),
                            Some(hit.kind),
                            boosted,
                        )
                    } else if !query_lower.is_empty() {
                        let (s, ls, le) =
                            Self::find_text_match(root_path, &relative_path, query_lower, config);
                        (s, ls, le, None, None, score)
                    } else {
                        (None, None, None, None, None, score)
                    }
                } else if !query_lower.is_empty() {
                    let (s, ls, le) =
                        Self::find_text_match(root_path, &relative_path, query_lower, config);
                    (s, ls, le, None, None, score)
                } else {
                    (None, None, None, None, None, score)
                }
            } else if !query_lower.is_empty() {
                if let Some(ref text) = content_text {
                    let snippet = Self::find_text_match_in_text(text, query_lower);
                    (snippet, None, None, None, None, score)
                } else {
                    let (s, ls, le) =
                        Self::find_text_match(root_path, &relative_path, query_lower, config);
                    (s, ls, le, None, None, score)
                }
            } else {
                (None, None, None, None, None, score)
            };

        if !has_relevance(
            &relative_path,
            query_lower,
            query_tokens,
            content_text.as_deref(),
            path_hint,
            file_hint,
            matched_symbol.is_some(),
        ) {
            return false;
        }

        let has_symbol = matched_symbol.is_some();
        let provenance = if has_symbol {
            Some(SymbolProvenance::RegexFallback.as_str().to_string())
        } else {
            None
        };

        matches.push(LocalMatch {
            file: file_entry,
            score: boosted_score,
            line_start,
            line_end,
            snippet,
            matched_symbol,
            symbol_kind,
            symbol_provenance: provenance,
            enclosing_symbol: None,
            is_exact_definition: if has_symbol { Some(false) } else { None },
        });

        false
    }

    /// Score a file against the query. Returns 0.0 if no match.
    ///
    /// When `content_text` is provided, also scores content matches
    /// (exact full-query and per-token) so that files containing the
    /// query text are found even when path/name tokens don't match.
    fn score_file(
        file: &LocalFileEntry,
        query_lower: &str,
        query_tokens: &[&str],
        _symbol_hint: Option<&str>,
        content_text: Option<&str>,
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

        // Content-based scoring: when file text is available, search
        // for the full query and individual tokens. This ensures files
        // containing the query text are found even when path tokens
        // don't match.
        if let Some(text) = content_text {
            let text_lower = text.to_lowercase();

            // Exact full-query content match
            if !query_lower.is_empty() && text_lower.contains(query_lower) {
                score += 50.0;
            }

            // Per-token content matches (capped to avoid huge files dominating)
            let mut token_score: f64 = 0.0;
            for token in query_tokens {
                if text_lower.contains(token) {
                    token_score += 5.0;
                }
            }
            // Cap token content score at +30
            score += token_score.min(30.0);
        }

        let penalty_extensions = [".lock", "min.js", "min.css", ".map"];
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
        root_path: &Path,
        relative_path: &str,
        query: &str,
        config: &LocalConfig,
    ) -> (Option<String>, Option<u32>, Option<u32>) {
        let content = match safe_read_file(root_path, relative_path, config, config.max_file_bytes)
        {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(_) => return (None, None, None),
        };

        let query_lower = query.to_lowercase();
        let snippet = Self::find_text_match_in_text_lower(&content, &query_lower);
        let line_num = snippet.as_ref().and_then(|_| {
            content
                .lines()
                .enumerate()
                .find(|(_, line)| line.to_lowercase().contains(&query_lower))
                .map(|(i, _)| (i + 1) as u32)
        });
        (snippet, line_num, line_num)
    }

    /// Search for a text match in already-read content. Returns a
    /// bounded snippet of the first matching line.
    fn find_text_match_in_text(content: &str, query: &str) -> Option<String> {
        let query_lower = query.to_lowercase();
        Self::find_text_match_in_text_lower(content, &query_lower)
    }

    fn find_text_match_in_text_lower(content: &str, query_lower: &str) -> Option<String> {
        for line in content.lines() {
            if line.to_lowercase().contains(query_lower) {
                let snippet = line.trim().to_string();
                let snippet = if snippet.len() > 500 {
                    let truncated: String = snippet.chars().take(500).collect();
                    format!("{truncated}...")
                } else {
                    snippet
                };
                return Some(snippet);
            }
        }
        None
    }

    /// Convert local matches into SourceCards.
    ///
    /// When `sanitize_output` is `true`, snippets are scanned for
    /// prompt-injection markers and control characters are stripped.
    ///
    /// When `repo_identity` is `Some`, each card's metadata includes
    /// `local_repo_match` with the matched repository identity and
    /// worktree state.
    pub fn to_source_cards(
        matches: &[LocalMatch],
        roots: &[(usize, PathBuf)],
        sanitize_output: bool,
        repo_identity: Option<&crate::meta::local_inventory::LocalRepoIdentity>,
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

                let pseudo_url = format!("workspace://{}/{}", root_name, m.file.relative_path);

                let title = m.file.relative_path.clone();

                let language = m.file.language.clone();
                let source_role =
                    crate::core::code_evidence::infer_source_role(&m.file.relative_path);

                let is_generated = matches!(source_role, SourceRole::Generated);
                let is_vendor = matches!(source_role, SourceRole::Vendor);
                let is_test = matches!(source_role, SourceRole::Test);
                let is_example = matches!(source_role, SourceRole::Example);
                let is_config = matches!(source_role, SourceRole::Configuration);
                let is_lockfile = matches!(source_role, SourceRole::Lockfile);

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

                let (evidence_confidence, mut evidence_reasons) =
                    match m.symbol_provenance.as_deref() {
                        Some("structured") if m.is_exact_definition == Some(true) => (
                            EvidenceConfidence::Exact,
                            vec![
                                CodeEvidenceReason::ProviderPathMatch,
                                CodeEvidenceReason::ProviderSymbolMatch,
                            ],
                        ),
                        Some("structured") => (
                            EvidenceConfidence::Exact,
                            vec![
                                CodeEvidenceReason::ProviderPathMatch,
                                CodeEvidenceReason::ProviderSymbolMatch,
                            ],
                        ),
                        Some(_) if m.matched_symbol.is_some() => (
                            EvidenceConfidence::Strong,
                            vec![
                                CodeEvidenceReason::ProviderPathMatch,
                                CodeEvidenceReason::ProviderTextMatch,
                            ],
                        ),
                        _ => (
                            EvidenceConfidence::Strong,
                            vec![CodeEvidenceReason::ProviderPathMatch],
                        ),
                    };
                if m.enclosing_symbol.is_some() {
                    evidence_reasons.push(CodeEvidenceReason::SourceRoleInferred);
                }

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
                    raw_permalink_url: None,
                    match_line_start: m.line_start,
                    match_line_end: m.line_end,
                    context_line_start: None,
                    context_line_end: None,
                    matched_symbol: m.matched_symbol.clone(),
                    symbol_kind: m.symbol_kind,
                    enclosing_symbol: m.enclosing_symbol.clone(),
                    evidence_confidence: Some(evidence_confidence),
                    evidence_reasons,
                    imports: Vec::new(),
                };

                let local_repo_match = repo_identity.map(|rid| {
                    let mut reasons = Vec::new();
                    let confidence = if rid.matched_host.is_some()
                        && rid.matched_owner.is_some()
                        && rid.matched_repo.is_some()
                    {
                        reasons.push("host_owner_repo_match".to_string());
                        EvidenceConfidence::Exact
                    } else if rid.remotes.is_empty() {
                        reasons.push("no_remotes_configured".to_string());
                        EvidenceConfidence::Weak
                    } else {
                        reasons.push("partial_remote_match".to_string());
                        EvidenceConfidence::Strong
                    };
                    crate::core::source_card::LocalRepoMatch {
                        matched: true,
                        remote_host: rid
                            .matched_host
                            .as_ref()
                            .map(|h| format!("{h:?}").to_lowercase()),
                        remote_owner: rid.matched_owner.clone(),
                        remote_repo: rid.matched_repo.clone(),
                        branch: rid.current_branch.clone(),
                        commit: rid.current_commit.clone(),
                        dirty_state: Some(rid.dirty_state.to_string()),
                        root_name: Some(rid.root_name.clone()),
                        root_path: Some(rid.root_path.display().to_string()),
                        match_confidence: Some(confidence),
                        reasons,
                        freshness_confidence: None,
                    }
                });

                let metadata = SourceMetadata {
                    source_kind: SourceKind::SourceFile,
                    domain: None,
                    rank_reasons: vec![RankReason::HintMatch],
                    code: Some(code_metadata),
                    issue: None,
                    release: None,
                    vulnerability: None,
                    code_evidence: Some(code_evidence),
                    local_repo_match,
                    is_generated: Some(is_generated),
                    is_vendor: Some(is_vendor),
                    is_test: Some(is_test),
                    is_example: Some(is_example),
                    is_config: Some(is_config),
                    is_lockfile: Some(is_lockfile),
                    evidence_role: None,
                    published_at: None,
                };

                let raw_snippet = m
                    .snippet
                    .clone()
                    .unwrap_or_else(|| format!("Local file: {}", m.file.relative_path));

                // Sanitize snippet: strip control chars and scan for
                // injection markers. Do NOT frame — source lines must
                // remain intact for agent copy-paste.
                let (snippet, trust_markers) = if sanitize_output {
                    let (cleaned, control_removed) =
                        crate::core::sanitize::strip_control_chars(&raw_snippet);
                    let hits = crate::core::sanitize::scan_injection_markers(&cleaned);
                    let mut tm = TrustMarkers {
                        text_sanitized: control_removed > 0,
                        text_truncated: false,
                        text_framed: false,
                        control_chars_removed: control_removed,
                        injection_hits: hits.len(),
                    };
                    // Also scan raw snippet for markers that may have
                    // been in control chars — but strip_control_chars
                    // only removes non-printable chars, so scan both.
                    if hits.is_empty() {
                        let raw_hits = crate::core::sanitize::scan_injection_markers(&raw_snippet);
                        tm.injection_hits = raw_hits.len();
                    }
                    (cleaned, tm)
                } else {
                    (raw_snippet, TrustMarkers::default())
                };

                let mut card = SourceCard::new(
                    title,
                    &pseudo_url,
                    vec!["local_workspace".to_string()],
                    Some(m.score),
                    TrustLevel::LocalTrusted,
                )
                .with_snippet(snippet)
                .with_trust_markers(trust_markers)
                .with_metadata(metadata);
                card.quality = Some(compute_card_quality(&card));
                card
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "local_backend_tests.rs"]
mod tests;
