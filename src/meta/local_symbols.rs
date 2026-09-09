//! Deterministic structured code intelligence for the local workspace backend.
//!
//! Provides a dependency-free structured symbol parser for Rust, Python,
//! JavaScript/TypeScript, and Go behind the established `SymbolBackend`
//! abstraction (see `crate::meta::local_backend`). The regex backend remains the fallback when parsing is
//! disabled, unsupported, fails, or exceeds budget. No repository code is
//! executed and no native plugins are loaded.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::core::code_evidence::SymbolKind;

/// Default maximum file bytes accepted by the structured parser.
pub const DEFAULT_MAX_PARSE_BYTES: usize = 262_144;
/// Default maximum symbols retained per file.
pub const DEFAULT_MAX_SYMBOLS_PER_FILE: usize = 256;
/// Default maximum files parsed with structure per request.
pub const DEFAULT_MAX_STRUCTURED_FILES: usize = 200;
/// Default maximum total symbols retained per request.
pub const DEFAULT_MAX_TOTAL_SYMBOLS: usize = 5_000;
/// Default maximum symbols retained per file for repo-map summaries.
pub const DEFAULT_MAX_SYMBOLS_PER_REPO_MAP_FILE: usize = 32;
/// Default cap for total repo-map structural entries.
pub const DEFAULT_REPO_MAP_STRUCTURE_CAP: usize = 500;
/// Schema version for cached symbol records.
pub const SYMBOL_RECORD_VERSION: u32 = 1;

/// Where a symbol match came from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolProvenance {
    /// Deterministic structured parser produced the match.
    #[default]
    Structured,
    /// Regex fallback produced the match.
    RegexFallback,
}

impl SymbolProvenance {
    /// Stable string label for telemetry and evidence metadata.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structured => "structured",
            Self::RegexFallback => "regex_fallback",
        }
    }
}

/// Capability flags for a symbol backend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolBackendCapabilities {
    /// Backend can enumerate definitions within a file.
    pub supports_definitions: bool,
    /// Backend can return bounded lexical references.
    pub supports_references: bool,
    /// Backend can resolve the enclosing symbol for a line.
    pub supports_enclosing: bool,
    /// Backend can list implementors where semantics support it.
    pub supports_implementors: bool,
    /// Languages with structured (non-regex) support.
    pub structured_languages: Vec<String>,
    /// Whether the regex fallback remains available.
    pub regex_fallback_available: bool,
}

/// A single structured symbol extracted from source text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructuredSymbol {
    /// Symbol name as written in source.
    pub name: String,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// Definition start line (1-indexed).
    pub line_start: u32,
    /// Definition end line (1-indexed, bounded estimate).
    pub line_end: u32,
    /// Enclosing module/impl/class, when reliably known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    /// Visibility prefix as written (`pub`, `export`, etc.), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    /// Whether this symbol is a test by syntax or container.
    #[serde(default)]
    pub is_test: bool,
    /// Relationship hint (`impl:Type`, `impl Trait for Type`, `import`, `module`, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
}

/// Cached symbol record keyed by stable file identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolRecord {
    /// Root-relative path.
    pub path: String,
    /// Detected language.
    pub language: Option<String>,
    /// Symbol name.
    pub name: String,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// Definition start line.
    pub line_start: u32,
    /// Definition end line.
    pub line_end: u32,
    /// Enclosing module/container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    /// Visibility as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    /// Relationship hints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationship_hints: Vec<String>,
    /// Content hash of the file the record was derived from.
    pub content_hash: u64,
    /// Record schema version.
    pub version: u32,
}

/// Confidence for a source-to-test relationship hint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestHintConfidence {
    /// Syntax-level test module or function.
    Syntax,
    /// Path convention (`tests/`, `*_test.*`, `test_*.py`, etc.).
    Path,
    /// Same symbol name referenced in a test file.
    NameReference,
    /// Same manifest/package boundary only.
    Package,
}

/// A deterministic related-test hint. Heuristic, never a coverage claim.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelatedTestHint {
    /// Source file the hint applies to.
    pub source_path: String,
    /// Candidate test file.
    pub test_path: String,
    /// Hint confidence.
    pub confidence: TestHintConfidence,
    /// Human-readable reasons for the hint.
    pub reasons: Vec<String>,
}

/// Bounded symbol inventory cache keyed by path and content hash.
#[derive(Debug, Default)]
pub struct SymbolInventoryCache {
    inner: RwLock<HashMap<String, CachedFileSymbols>>,
}

/// Cached symbols for one file.
#[derive(Clone, Debug)]
struct CachedFileSymbols {
    content_hash: u64,
    symbols: Vec<StructuredSymbol>,
    provenance: SymbolProvenance,
}

impl SymbolInventoryCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Look up cached symbols when the content hash still matches.
    pub fn get(
        &self,
        path: &str,
        content_hash: u64,
    ) -> Option<(Vec<StructuredSymbol>, SymbolProvenance)> {
        let inner = self.inner.read().ok()?;
        let entry = inner.get(path)?;
        if entry.content_hash != content_hash {
            return None;
        }
        Some((entry.symbols.clone(), entry.provenance))
    }

    /// Store symbols for a path, evicting an arbitrary entry when oversized.
    pub fn insert(
        &self,
        path: String,
        content_hash: u64,
        symbols: Vec<StructuredSymbol>,
        provenance: SymbolProvenance,
        max_entries: usize,
    ) {
        let Ok(mut inner) = self.inner.write() else {
            return;
        };
        if inner.len() >= max_entries.max(1) && !inner.contains_key(&path) {
            if let Some(key) = inner.keys().next().cloned() {
                inner.remove(&key);
            }
        }
        inner.insert(
            path,
            CachedFileSymbols {
                content_hash,
                symbols,
                provenance,
            },
        );
    }

    /// Number of cached files.
    pub fn len(&self) -> usize {
        self.inner.read().ok().map_or(0, |m| m.len())
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Whether a language has structured parser support.
pub fn language_has_structured_support(language: &str) -> bool {
    matches!(
        language,
        "rust" | "python" | "javascript" | "typescript" | "go"
    )
}

/// Content hash for cache keys. Deterministic and model-free.
pub fn content_hash_for_symbols(text: &str) -> u64 {
    xxhash_rust::xxh3::xxh3_64(text.as_bytes())
}

/// Parse symbols for a file with budget enforcement.
///
/// Returns the symbols, the provenance, and whether a budget cap truncated
/// the result. Unsupported languages yield an empty structured result so the
/// caller can apply the regex fallback.
pub fn symbols_in_file(
    relative_path: &str,
    language: Option<&str>,
    text: &str,
    max_bytes: usize,
    max_symbols: usize,
) -> (Vec<StructuredSymbol>, SymbolProvenance, bool) {
    let Some(lang) = language else {
        return (Vec::new(), SymbolProvenance::RegexFallback, false);
    };
    if !language_has_structured_support(lang) {
        return (Vec::new(), SymbolProvenance::RegexFallback, false);
    }
    if text.len() > max_bytes {
        return (Vec::new(), SymbolProvenance::RegexFallback, true);
    }
    if text.is_empty() {
        return (Vec::new(), SymbolProvenance::Structured, false);
    }
    let mut symbols = match lang {
        "rust" => parse_rust(text),
        "python" => parse_python(text),
        "javascript" | "typescript" => parse_javascript(text, relative_path),
        "go" => parse_go(text),
        _ => Vec::new(),
    };
    let truncated = symbols.len() > max_symbols;
    if truncated {
        symbols.truncate(max_symbols);
    }
    (symbols, SymbolProvenance::Structured, truncated)
}

/// Find an exact definition for a symbol name (case-sensitive first, then insensitive).
pub fn find_definition<'a>(
    symbols: &'a [StructuredSymbol],
    name: &str,
) -> Option<&'a StructuredSymbol> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    symbols.iter().find(|s| s.name == trimmed).or_else(|| {
        let lower = trimmed.to_lowercase();
        symbols.iter().find(|s| s.name.to_lowercase() == lower)
    })
}

/// Find the innermost symbol containing a 1-indexed line.
pub fn find_enclosing_symbol(symbols: &[StructuredSymbol], line: u32) -> Option<&StructuredSymbol> {
    let mut best: Option<&StructuredSymbol> = None;
    for symbol in symbols {
        if line < symbol.line_start || line > symbol.line_end {
            continue;
        }
        let span = symbol.line_end.saturating_sub(symbol.line_start);
        let best_span = best.map_or(u32::MAX, |b| b.line_end.saturating_sub(b.line_start));
        if span < best_span {
            best = Some(symbol);
        }
    }
    best
}

/// Find implementors for a type or trait name where parser semantics support it.
pub fn find_implementors<'a>(
    symbols: &'a [StructuredSymbol],
    name: &str,
) -> Vec<&'a StructuredSymbol> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    symbols
        .iter()
        .filter(|s| {
            s.relationship.as_deref().is_some_and(|rel| {
                rel == format!("impl:{trimmed}")
                    || rel.contains(&format!("impl {trimmed} for"))
                    || rel.contains(&format!("for {trimmed}"))
                    || rel == format!("implements:{trimmed}")
            })
        })
        .collect()
}

/// Bounded lexical reference scan for a symbol name.
pub fn find_references(text: &str, symbol: &str, max_results: usize) -> Vec<(u32, String)> {
    let trimmed = symbol.trim();
    if trimmed.is_empty() || max_results == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if out.len() >= max_results {
            break;
        }
        if contains_word(line, trimmed) {
            let snippet: String = line.trim().chars().take(500).collect();
            if snippet.is_empty() {
                continue;
            }
            out.push((idx as u32 + 1, snippet));
        }
    }
    out
}

fn contains_word(line: &str, word: &str) -> bool {
    if !line.contains(word) {
        return false;
    }
    let bytes = line.as_bytes();
    let needle = word.as_bytes();
    if needle.is_empty() || needle.len() > bytes.len() {
        return false;
    }
    for start in 0..=(bytes.len() - needle.len()) {
        if &bytes[start..start + needle.len()] != needle {
            continue;
        }
        let before_ok = start == 0 || !is_word_byte(bytes[start - 1]);
        let after = start + needle.len();
        let after_ok = after >= bytes.len() || !is_word_byte(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn estimate_line_end(lines: &[&str], start_idx: usize) -> u32 {
    let start_line = lines[start_idx];
    if !start_line.contains('{') {
        return start_idx as u32 + 1;
    }
    let mut depth: i32 = 0;
    let cap = (start_idx + 200).min(lines.len());
    for (offset, line) in lines[start_idx..cap].iter().enumerate() {
        let code = strip_line_comment(line);
        for ch in code.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        if depth <= 0 && offset > 0 {
            return (start_idx + offset) as u32 + 1;
        }
    }
    cap as u32
}

fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn is_test_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    let file = lower.rsplit('/').next().unwrap_or(&lower);
    lower.starts_with("tests/")
        || lower.contains("/tests/")
        || file.starts_with("test_")
        || file.ends_with("_test.rs")
        || file.ends_with("_test.py")
        || file.ends_with("_test.go")
        || file.ends_with(".test.ts")
        || file.ends_with(".test.js")
        || file.ends_with(".spec.ts")
        || file.ends_with(".spec.js")
}

fn parse_rust(text: &str) -> Vec<StructuredSymbol> {
    let lines: Vec<&str> = text.lines().collect();
    let mut symbols = Vec::new();
    let mut modules: Vec<String> = Vec::new();
    let mut brace_depth: Vec<usize> = Vec::new();
    let mut impl_stack: Vec<(String, usize)> = Vec::new();
    let mut pending_test_attr = false;
    let mut pending_cfg_test = false;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "#[test]" || trimmed.starts_with("#[test(") {
            pending_test_attr = true;
            continue;
        }
        if trimmed.contains("cfg(test)") {
            pending_cfg_test = true;
        }
        let depth = line.chars().take_while(|c| c.is_whitespace()).count();
        while brace_depth.last().is_some_and(|d| *d >= depth) {
            brace_depth.pop();
            modules.pop();
        }
        while impl_stack.last().is_some_and(|(_, d)| *d >= depth) {
            impl_stack.pop();
        }

        if let Some(name) = parse_rust_mod(trimmed) {
            modules.push(name.clone());
            brace_depth.push(depth);
            symbols.push(StructuredSymbol {
                name,
                kind: SymbolKind::Module,
                line_start: idx as u32 + 1,
                line_end: idx as u32 + 1,
                container: modules.iter().rev().nth(1).cloned(),
                visibility: visibility_of(trimmed),
                is_test: pending_cfg_test,
                relationship: Some("module".to_string()),
            });
            continue;
        }

        if let Some((impl_target, relationship)) = parse_rust_impl(trimmed) {
            impl_stack.push((impl_target, depth));
            let line_end = estimate_line_end(&lines, idx);
            symbols.push(StructuredSymbol {
                name: impl_stack
                    .last()
                    .map(|(target, _)| target.clone())
                    .unwrap_or_default(),
                kind: SymbolKind::Struct,
                line_start: idx as u32 + 1,
                line_end,
                container: modules.last().cloned(),
                visibility: None,
                is_test: false,
                relationship: Some(relationship),
            });
            continue;
        }

        if let Some((kind, name)) = parse_rust_item(trimmed) {
            let container = impl_stack
                .last()
                .map(|(target, _)| format!("impl {target}"))
                .or_else(|| modules.last().cloned());
            let effective_kind = if impl_stack.last().is_some() && kind == SymbolKind::Function {
                SymbolKind::Method
            } else {
                kind
            };
            let is_test = pending_test_attr
                || (effective_kind == SymbolKind::Function
                    && (name.starts_with("test_") || pending_cfg_test));
            pending_test_attr = false;
            let line_end = estimate_line_end(&lines, idx);
            symbols.push(StructuredSymbol {
                name,
                kind: effective_kind,
                line_start: idx as u32 + 1,
                line_end,
                container,
                visibility: visibility_of(trimmed),
                is_test,
                relationship: None,
            });
            continue;
        }

        if trimmed.starts_with("use ") && trimmed.ends_with(';') {
            let import = trimmed
                .trim_start_matches("pub ")
                .trim_start_matches("use ")
                .trim_end_matches(';')
                .trim()
                .to_string();
            if !import.is_empty() && symbols.len() < 10_000 {
                symbols.push(StructuredSymbol {
                    name: import.clone(),
                    kind: SymbolKind::Module,
                    line_start: idx as u32 + 1,
                    line_end: idx as u32 + 1,
                    container: modules.last().cloned(),
                    visibility: visibility_of(trimmed),
                    is_test: false,
                    relationship: Some("import".to_string()),
                });
            }
        }
        if !trimmed.starts_with("#[") {
            pending_test_attr = false;
        }
    }
    symbols
}

fn visibility_of(trimmed: &str) -> Option<String> {
    if trimmed.starts_with("pub(") {
        let end = trimmed.find(')').map(|i| i + 1).unwrap_or(3);
        Some(trimmed[..end].to_string())
    } else if trimmed.starts_with("pub ") {
        Some("pub".to_string())
    } else if trimmed.starts_with("export ") {
        Some("export".to_string())
    } else {
        None
    }
}

fn parse_rust_mod(trimmed: &str) -> Option<String> {
    let rest = trimmed
        .strip_prefix("pub mod ")
        .or_else(|| trimmed.strip_prefix("mod "))
        .or_else(|| {
            trimmed
                .strip_prefix("pub(crate) mod ")
                .or_else(|| trimmed.strip_prefix("pub(super) mod "))
        })?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn parse_rust_impl(trimmed: &str) -> Option<(String, String)> {
    let rest = trimmed.strip_prefix("impl")?;
    if !rest.starts_with([' ', '<', '(']) {
        return None;
    }
    let rest = rest.trim();
    if rest.starts_with("for ") || rest.is_empty() {
        return None;
    }
    let target = rest
        .split('{')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches("where")
        .trim()
        .to_string();
    if target.is_empty() {
        return None;
    }
    let relationship = if target.contains(" for ") {
        format!("impl {target}")
    } else {
        format!("impl:{target}")
    };
    let short = target
        .rsplit(" for ")
        .next()
        .unwrap_or(&target)
        .split('<')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .last()
        .unwrap_or(&target)
        .to_string();
    Some((short, relationship))
}

fn parse_rust_item(trimmed: &str) -> Option<(SymbolKind, String)> {
    let code = trimmed
        .strip_prefix("pub(crate) ")
        .or_else(|| trimmed.strip_prefix("pub(super) "))
        .or_else(|| trimmed.strip_prefix("pub "))
        .unwrap_or(trimmed);
    let code = code
        .strip_prefix("async ")
        .unwrap_or(code)
        .strip_prefix("unsafe ")
        .unwrap_or(code)
        .strip_prefix("extern ")
        .unwrap_or(code);
    if let Some(rest) = code.strip_prefix("fn ") {
        return take_ident(rest).map(|name| (SymbolKind::Function, name));
    }
    if let Some(rest) = code.strip_prefix("struct ") {
        return take_ident(rest).map(|name| (SymbolKind::Struct, name));
    }
    if let Some(rest) = code.strip_prefix("enum ") {
        return take_ident(rest).map(|name| (SymbolKind::Enum, name));
    }
    if let Some(rest) = code.strip_prefix("trait ") {
        return take_ident(rest).map(|name| (SymbolKind::Trait, name));
    }
    if let Some(rest) = code.strip_prefix("type ") {
        return take_ident(rest).map(|name| (SymbolKind::TypeAlias, name));
    }
    if let Some(rest) = code.strip_prefix("macro_rules! ") {
        return take_ident(rest).map(|name| (SymbolKind::Macro, name));
    }
    if let Some(rest) = code.strip_prefix("const ") {
        return take_ident(rest.split(':').next().unwrap_or(""))
            .map(|name| (SymbolKind::Constant, name));
    }
    if let Some(rest) = code.strip_prefix("static ") {
        return take_ident(rest.split(':').next().unwrap_or(""))
            .map(|name| (SymbolKind::Constant, name));
    }
    None
}

fn take_ident(rest: &str) -> Option<String> {
    let name: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() || name.chars().next().is_some_and(|c| c.is_numeric()) {
        None
    } else {
        Some(name)
    }
}

fn parse_python(text: &str) -> Vec<StructuredSymbol> {
    let lines: Vec<&str> = text.lines().collect();
    let mut symbols = Vec::new();
    let mut class_stack: Vec<(String, usize)> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        while class_stack.last().is_some_and(|(_, d)| *d >= indent) {
            class_stack.pop();
        }
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("async def ")
            .or_else(|| trimmed.strip_prefix("def "))
        {
            if let Some(name) = take_ident(rest) {
                let container = class_stack.last().map(|(c, _)| c.clone());
                let kind = if container.is_some() {
                    SymbolKind::Method
                } else {
                    SymbolKind::Function
                };
                symbols.push(StructuredSymbol {
                    name: name.clone(),
                    kind,
                    line_start: idx as u32 + 1,
                    line_end: python_block_end(&lines, idx, indent),
                    container,
                    visibility: None,
                    is_test: name.starts_with("test_"),
                    relationship: None,
                });
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some(name) = take_ident(rest) {
                class_stack.push((name.clone(), indent));
                symbols.push(StructuredSymbol {
                    name: name.clone(),
                    kind: SymbolKind::Class,
                    line_start: idx as u32 + 1,
                    line_end: python_block_end(&lines, idx, indent),
                    container: None,
                    visibility: None,
                    is_test: name.starts_with("Test"),
                    relationship: None,
                });
            }
            continue;
        }
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            symbols.push(StructuredSymbol {
                name: trimmed.to_string(),
                kind: SymbolKind::Module,
                line_start: idx as u32 + 1,
                line_end: idx as u32 + 1,
                container: class_stack.last().map(|(c, _)| c.clone()),
                visibility: None,
                is_test: false,
                relationship: Some("import".to_string()),
            });
        }
    }
    symbols
}

fn python_block_end(lines: &[&str], start_idx: usize, indent: usize) -> u32 {
    let cap = (start_idx + 200).min(lines.len());
    for (offset, line) in lines[start_idx + 1..cap].iter().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let next_indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        if next_indent <= indent {
            return (start_idx + offset) as u32 + 1;
        }
    }
    cap as u32
}

fn parse_javascript(text: &str, path: &str) -> Vec<StructuredSymbol> {
    let lines: Vec<&str> = text.lines().collect();
    let mut symbols = Vec::new();
    let path_test = is_test_path(path);

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("import ")
            || trimmed.starts_with("export ") && trimmed.contains(" from ")
            || trimmed.starts_with("const ") && trimmed.contains("require(")
        {
            symbols.push(StructuredSymbol {
                name: trimmed.chars().take(120).collect(),
                kind: SymbolKind::Module,
                line_start: idx as u32 + 1,
                line_end: idx as u32 + 1,
                container: None,
                visibility: visibility_of(trimmed),
                is_test: false,
                relationship: Some("import".to_string()),
            });
            continue;
        }
        let code = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let code = code.strip_prefix("default ").unwrap_or(code);
        if let Some(rest) = code
            .strip_prefix("async function ")
            .or_else(|| code.strip_prefix("function "))
        {
            if let Some(name) = take_ident(rest) {
                symbols.push(StructuredSymbol {
                    name,
                    kind: SymbolKind::Function,
                    line_start: idx as u32 + 1,
                    line_end: estimate_line_end(&lines, idx),
                    container: None,
                    visibility: visibility_of(trimmed),
                    is_test: path_test,
                    relationship: None,
                });
                continue;
            }
        }
        if let Some(rest) = code.strip_prefix("class ") {
            if let Some(name) = take_ident(rest) {
                symbols.push(StructuredSymbol {
                    name,
                    kind: SymbolKind::Class,
                    line_start: idx as u32 + 1,
                    line_end: estimate_line_end(&lines, idx),
                    container: None,
                    visibility: visibility_of(trimmed),
                    is_test: path_test,
                    relationship: None,
                });
                continue;
            }
        }
        if let Some(rest) = code.strip_prefix("interface ") {
            if let Some(name) = take_ident(rest) {
                symbols.push(StructuredSymbol {
                    name,
                    kind: SymbolKind::Interface,
                    line_start: idx as u32 + 1,
                    line_end: estimate_line_end(&lines, idx),
                    container: None,
                    visibility: visibility_of(trimmed),
                    is_test: false,
                    relationship: None,
                });
                continue;
            }
        }
        if let Some(rest) = code.strip_prefix("type ") {
            if let Some(name) = take_ident(rest) {
                symbols.push(StructuredSymbol {
                    name,
                    kind: SymbolKind::TypeAlias,
                    line_start: idx as u32 + 1,
                    line_end: idx as u32 + 1,
                    container: None,
                    visibility: visibility_of(trimmed),
                    is_test: false,
                    relationship: None,
                });
                continue;
            }
        }
        if let Some(name) = parse_js_const_fn(code) {
            symbols.push(StructuredSymbol {
                name,
                kind: SymbolKind::Function,
                line_start: idx as u32 + 1,
                line_end: estimate_line_end(&lines, idx),
                container: None,
                visibility: visibility_of(trimmed),
                is_test: path_test,
                relationship: None,
            });
        }
    }
    symbols
}

fn parse_js_const_fn(code: &str) -> Option<String> {
    let rest = code.strip_prefix("const ")?.trim();
    let name = take_ident(rest)?;
    let after = rest[name.len()..].trim_start();
    let after = after.strip_prefix('=')?.trim_start();
    if after.starts_with("async (")
        || after.starts_with('(')
        || after.starts_with("async ")
        || after.starts_with('<')
        || after.contains("=>")
    {
        Some(name)
    } else {
        None
    }
}

fn parse_go(text: &str) -> Vec<StructuredSymbol> {
    let lines: Vec<&str> = text.lines().collect();
    let mut symbols = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("package ") {
            let name = trimmed
                .strip_prefix("package ")
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                symbols.push(StructuredSymbol {
                    name,
                    kind: SymbolKind::Module,
                    line_start: idx as u32 + 1,
                    line_end: idx as u32 + 1,
                    container: None,
                    visibility: None,
                    is_test: false,
                    relationship: Some("module".to_string()),
                });
            }
            continue;
        }
        if trimmed.starts_with("import ") {
            symbols.push(StructuredSymbol {
                name: trimmed.chars().take(120).collect(),
                kind: SymbolKind::Module,
                line_start: idx as u32 + 1,
                line_end: idx as u32 + 1,
                container: None,
                visibility: None,
                is_test: false,
                relationship: Some("import".to_string()),
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("func ") {
            let rest = rest.trim();
            if rest.starts_with('(') {
                if let Some(close) = rest.find(')') {
                    let receiver = rest[1..close].trim().to_string();
                    let after = rest[close + 1..].trim();
                    if let Some(name) = take_ident(after) {
                        symbols.push(StructuredSymbol {
                            name,
                            kind: SymbolKind::Method,
                            line_start: idx as u32 + 1,
                            line_end: estimate_line_end(&lines, idx),
                            container: Some(receiver),
                            visibility: None,
                            is_test: false,
                            relationship: None,
                        });
                        continue;
                    }
                }
            } else if let Some(name) = take_ident(rest) {
                let is_test = name.starts_with("Test")
                    || name.starts_with("Example")
                    || name.starts_with("Benchmark");
                symbols.push(StructuredSymbol {
                    name,
                    kind: SymbolKind::Function,
                    line_start: idx as u32 + 1,
                    line_end: estimate_line_end(&lines, idx),
                    container: None,
                    visibility: None,
                    is_test,
                    relationship: None,
                });
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("type ") {
            let rest = rest.trim();
            if let Some(name) = take_ident(rest) {
                let after = rest[name.len()..].trim_start();
                let kind = if after.starts_with("interface") {
                    SymbolKind::Interface
                } else if after.starts_with("struct") {
                    SymbolKind::Struct
                } else {
                    SymbolKind::TypeAlias
                };
                let relationship = if kind == SymbolKind::Interface {
                    Some(format!("interface:{name}"))
                } else {
                    None
                };
                symbols.push(StructuredSymbol {
                    name,
                    kind,
                    line_start: idx as u32 + 1,
                    line_end: estimate_line_end(&lines, idx),
                    container: None,
                    visibility: None,
                    is_test: false,
                    relationship,
                });
            }
        }
    }
    symbols
}

/// Deterministic related-test hints for a source file.
///
/// Confidence order: syntax-level test items, path conventions, same-symbol
/// name references in test files, then package-boundary fallback.
pub fn related_test_hints(
    source_path: &str,
    source_symbol_names: &[String],
    candidate_test_files: &[String],
    candidate_test_texts: &HashMap<String, String>,
) -> Vec<RelatedTestHint> {
    let mut hints = Vec::new();
    let source_file = source_path.rsplit('/').next().unwrap_or(source_path);
    let source_stem = source_file
        .rsplit('.')
        .nth(1)
        .unwrap_or(source_file)
        .to_string();
    let source_dir = source_path
        .rfind('/')
        .map(|i| &source_path[..i])
        .unwrap_or("");

    for test_path in candidate_test_files {
        if test_path == source_path {
            continue;
        }
        let mut reasons = Vec::new();
        let mut confidence: Option<TestHintConfidence> = None;

        let text = candidate_test_texts.get(test_path);
        let has_syntax_test = text.is_some_and(|t| {
            t.contains("#[test]")
                || t.contains("def test_")
                || t.contains("func Test")
                || t.contains("describe(")
                || t.contains("it(")
        });
        if has_syntax_test {
            confidence = Some(TestHintConfidence::Syntax);
            reasons.push("syntax_test_item".to_string());
        }

        let lower = test_path.to_lowercase();
        let file = lower.rsplit('/').next().unwrap_or(&lower);
        let path_convention = lower.starts_with("tests/")
            || lower.contains("/tests/")
            || file.starts_with("test_")
            || file.ends_with("_test.rs")
            || file.ends_with("_test.py")
            || file.ends_with("_test.go")
            || file.ends_with(".test.ts")
            || file.ends_with(".test.js")
            || file.ends_with(".spec.ts")
            || file.ends_with(".spec.js");
        if path_convention {
            if confidence.is_none() {
                confidence = Some(TestHintConfidence::Path);
            }
            reasons.push("path_convention".to_string());
        }

        let same_stem = test_path
            .to_lowercase()
            .contains(&source_stem.to_lowercase())
            && source_stem.len() >= 3;
        if same_stem {
            if confidence.is_none() {
                confidence = Some(TestHintConfidence::NameReference);
            }
            reasons.push("name_reference".to_string());
        }

        let name_hit = text.is_some_and(|t| {
            source_symbol_names
                .iter()
                .any(|name| !name.is_empty() && contains_word(t, name))
        });
        if name_hit {
            if confidence.is_none() || confidence == Some(TestHintConfidence::Path) {
                confidence = Some(TestHintConfidence::NameReference);
            }
            reasons.push("symbol_reference".to_string());
        }

        let same_package = !source_dir.is_empty()
            && (test_path.starts_with(&format!("{source_dir}/")) || test_path == source_path);
        if confidence.is_none() && same_package {
            confidence = Some(TestHintConfidence::Package);
            reasons.push("package_boundary".to_string());
        }

        if let Some(level) = confidence {
            reasons.sort();
            reasons.dedup();
            hints.push(RelatedTestHint {
                source_path: source_path.to_string(),
                test_path: test_path.clone(),
                confidence: level,
                reasons,
            });
        }
    }

    hints.sort_by(|a, b| {
        rank_confidence(a.confidence)
            .cmp(&rank_confidence(b.confidence))
            .then_with(|| a.test_path.cmp(&b.test_path))
    });
    hints
}

fn rank_confidence(confidence: TestHintConfidence) -> u8 {
    match confidence {
        TestHintConfidence::Syntax => 0,
        TestHintConfidence::Path => 1,
        TestHintConfidence::NameReference => 2,
        TestHintConfidence::Package => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_extracts_fn_struct_trait_impl() {
        let text = "mod api;\npub struct Engine {\n    name: String,\n}\npub trait Runner {\n    fn run(&self);\n}\nimpl Engine {\n    pub fn start(&self) {}\n}\npub fn helper() {}";
        let symbols = parse_rust(text);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Engine"));
        assert!(names.contains(&"Runner"));
        assert!(names.contains(&"helper"));
        assert!(names.contains(&"start"));
        let method = symbols.iter().find(|s| s.name == "start").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
    }

    #[test]
    fn python_extracts_class_function_import() {
        let text = "import os\nclass Store:\n    def save(self):\n        pass\ndef load():\n    pass\ndef test_save():\n    pass";
        let symbols = parse_python(text);
        assert!(symbols
            .iter()
            .any(|s| s.name == "Store" && s.kind == SymbolKind::Class));
        assert!(symbols
            .iter()
            .any(|s| s.name == "save" && s.kind == SymbolKind::Method));
        assert!(symbols.iter().any(|s| s.name == "load"));
        let test_fn = symbols.iter().find(|s| s.name == "test_save").unwrap();
        assert!(test_fn.is_test);
    }

    #[test]
    fn javascript_extracts_class_function() {
        let text = "import x from './x';\nexport class Api {\n}\nexport function fetchData() {}\nconst handler = async () => {};";
        let symbols = parse_javascript(text, "src/api.ts");
        assert!(symbols.iter().any(|s| s.name == "Api"));
        assert!(symbols.iter().any(|s| s.name == "fetchData"));
        assert!(symbols.iter().any(|s| s.name == "handler"));
    }

    #[test]
    fn go_extracts_func_method_type() {
        let text = "package store\ntype Store struct {}\nfunc New() {}\nfunc (s Store) Save() {}\nfunc TestSave() {}";
        let symbols = parse_go(text);
        assert!(symbols.iter().any(|s| s.name == "Store"));
        assert!(symbols.iter().any(|s| s.name == "New"));
        let method = symbols.iter().find(|s| s.name == "Save").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
    }

    #[test]
    fn unsupported_language_falls_back() {
        let (symbols, provenance, truncated) =
            symbols_in_file("data.xyz", Some("unknown"), "content", 1024, 10);
        assert!(symbols.is_empty());
        assert_eq!(provenance, SymbolProvenance::RegexFallback);
        assert!(!truncated);
    }

    #[test]
    fn oversized_file_degrades_to_fallback() {
        let text = "fn a() {}".repeat(100);
        let (symbols, provenance, truncated) = symbols_in_file("a.rs", Some("rust"), &text, 10, 10);
        assert!(symbols.is_empty());
        assert_eq!(provenance, SymbolProvenance::RegexFallback);
        assert!(truncated);
    }

    #[test]
    fn enclosing_symbol_prefers_innermost() {
        let symbols = vec![
            StructuredSymbol {
                name: "outer".to_string(),
                kind: SymbolKind::Function,
                line_start: 1,
                line_end: 20,
                container: None,
                visibility: None,
                is_test: false,
                relationship: None,
            },
            StructuredSymbol {
                name: "inner".to_string(),
                kind: SymbolKind::Function,
                line_start: 5,
                line_end: 8,
                container: None,
                visibility: None,
                is_test: false,
                relationship: None,
            },
        ];
        let enclosing = find_enclosing_symbol(&symbols, 6).unwrap();
        assert_eq!(enclosing.name, "inner");
    }

    #[test]
    fn related_hints_prefer_syntax_over_path() {
        let mut texts = HashMap::new();
        texts.insert(
            "tests/api.rs".to_string(),
            "#[test]\nfn test_save() {}".to_string(),
        );
        let hints = related_test_hints(
            "src/api.rs",
            &["save".to_string()],
            &["tests/api.rs".to_string()],
            &texts,
        );
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].confidence, TestHintConfidence::Syntax);
    }
}
