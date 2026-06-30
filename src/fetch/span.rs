//! Symbol/span-aware block expansion for `repo_fetch`.
//!
//! Provides deterministic heuristics for expanding a symbol name, match
//! text, or explicit line range into an enclosing code block. Used by
//! `repo_fetch` when the caller provides a `symbol` or `match_text`
//! instead of (or in addition to) explicit line numbers.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::core::code_evidence::SymbolKind;

/// Confidence level for the span selection.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SpanConfidence {
    /// Exact match (e.g. explicit line range or exact symbol definition).
    Exact,
    /// Strong match (e.g. symbol definition found with high confidence).
    Strong,
    /// Weak match (e.g. brace-matched block in an unsupported language).
    Weak,
    /// Unrecoverable or unclassified confidence.
    #[default]
    Unknown,
}

/// How the span was selected.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SpanSelectionKind {
    /// Caller provided explicit line range, no expansion.
    ExplicitRange,
    /// Caller provided explicit line range, expanded to enclosing block.
    ExpandedExplicitRange,
    /// Matched a symbol definition/declaration.
    SymbolDefinition,
    /// Matched a symbol reference (not a definition).
    SymbolReference,
    /// Matched via free-text match_text search.
    MatchText,
    /// No inputs provided; whole file bounded by max_block_lines.
    #[default]
    WholeFileBounded,
}

/// A selected span of lines with metadata about how it was chosen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SelectedSpan {
    /// Start line (1-indexed, inclusive).
    pub line_start: u32,
    /// End line (1-indexed, inclusive).
    pub line_end: u32,
    /// How this span was selected.
    pub selection_kind: SpanSelectionKind,
    /// Symbol name that was matched (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Kind of the matched symbol (if known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<SymbolKind>,
    /// Confidence in the selection.
    pub confidence: SpanConfidence,
    /// Human-readable reasons explaining the selection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    /// Whether the span was expanded from an explicit range to a block.
    pub expanded: bool,
    /// Whether the block was truncated by max_block_lines.
    pub truncated_by_max_block_lines: bool,
}

// ---------------------------------------------------------------------------
// Compiled regex patterns
// ---------------------------------------------------------------------------

static RUST_FN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)").unwrap());
static RUST_STRUCT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pub\s+)?struct\s+(\w+)").unwrap());
static RUST_ENUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pub\s+)?enum\s+(\w+)").unwrap());
static RUST_TRAIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pub\s+)?trait\s+(\w+)").unwrap());
static RUST_IMPL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"impl(?:<[^>]*>)?\s+(?:dyn\s+)?(\w+)").unwrap());
static RUST_MOD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pub\s+)?mod\s+(\w+)").unwrap());
static RUST_MACRO_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"macro_rules!\s+(\w+)").unwrap());
static RUST_CONST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pub(?:\([^)]*\))?\s+)?(?:const|static)\s+(\w+)").unwrap());
static RUST_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pub\s+)?type\s+(\w+)").unwrap());

static PYTHON_DEF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:async\s+)?def\s+(\w+)").unwrap());
static PYTHON_CLASS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"class\s+(\w+)").unwrap());

static JS_FUNCTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:export\s+)?(?:async\s+)?function\s+(\w+)").unwrap());
static JS_CLASS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:export\s+)?class\s+(\w+)").unwrap());
static JS_INTERFACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:export\s+)?interface\s+(\w+)").unwrap());
static JS_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:export\s+)?type\s+(\w+)").unwrap());
static JS_CONST_ARROW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:export\s+)?(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s+)?(?:\(|[a-zA-Z_$])")
        .unwrap()
});

static GO_FUNC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"func\s+(?:\([^)]*\)\s+)?(\w+)").unwrap());
static GO_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"type\s+(\w+)\s+(?:struct|interface)").unwrap());

static JAVA_KW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:public|private|protected|internal)\s+(?:static\s+)?(?:class|interface|enum)\s+(\w+)",
    )
    .unwrap()
});
static CPP_STRUCT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:typedef\s+)?(?:struct|class)\s+(\w+)").unwrap());

static MARKDOWN_HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.*)").unwrap());
static TOML_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[([^\]]+)\]").unwrap());

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Select a span of lines based on symbol, match text, or explicit range.
///
/// Returns `None` when an explicit input (symbol, match_text) is
/// provided but not found. When nothing is provided, returns
/// `WholeFileBounded` spanning the full file.
#[allow(clippy::too_many_arguments)]
pub fn select_span(
    all_lines: &[String],
    language: Option<&str>,
    symbol: Option<&str>,
    _symbol_kind: Option<SymbolKind>,
    match_text: Option<&str>,
    explicit_line_start: Option<u32>,
    explicit_line_end: Option<u32>,
    expand_to_block: bool,
    max_block_lines: Option<usize>,
) -> Option<SelectedSpan> {
    if all_lines.is_empty() {
        return None;
    }

    let total = all_lines.len() as u32;

    // 1. Explicit line range without expansion.
    if let (Some(start), Some(end)) = (explicit_line_start, explicit_line_end) {
        if !expand_to_block {
            let s = start.clamp(1, total);
            let e = end.clamp(s, total);
            return Some(SelectedSpan {
                line_start: s,
                line_end: e,
                selection_kind: SpanSelectionKind::ExplicitRange,
                symbol: None,
                symbol_kind: None,
                confidence: SpanConfidence::Exact,
                reasons: vec![format!("explicit line range {s}-{e}")],
                expanded: false,
                truncated_by_max_block_lines: false,
            });
        }

        // 2. Explicit line range with expansion.
        let midpoint = ((start + end) / 2).saturating_sub(1) as usize;
        let midpoint = midpoint.min(all_lines.len().saturating_sub(1));
        let lang = language.unwrap_or("");
        let (block_start, block_end) = expand_to_enclosing_block(all_lines, midpoint, lang);
        let mut reasons = vec![format!(
            "explicit range {start}-{end} expanded to enclosing block"
        )];
        let confidence = SpanConfidence::Strong;
        let (ls, le, truncated) =
            clamp_and_truncate(block_start, block_end, max_block_lines, &mut reasons);
        return Some(SelectedSpan {
            line_start: ls,
            line_end: le,
            selection_kind: SpanSelectionKind::ExpandedExplicitRange,
            symbol: None,
            symbol_kind: None,
            confidence,
            reasons,
            expanded: true,
            truncated_by_max_block_lines: truncated,
        });
    }

    // 3. Symbol search.
    let mut symbol_not_found = false;
    if let Some(sym) = symbol {
        let lang = language.unwrap_or("");
        if let Some((line_idx, kind, conf, mut reasons)) =
            find_symbol_line(all_lines, sym, lang)
        {
            let (block_start, block_end) = expand_to_enclosing_block(all_lines, line_idx, lang);
            let sel_kind = if kind == SymbolKind::Unknown {
                SpanSelectionKind::SymbolReference
            } else {
                SpanSelectionKind::SymbolDefinition
            };
            let selection_kind = sel_kind;
            if selection_kind == SpanSelectionKind::SymbolDefinition {
                reasons.insert(
                    0,
                    format!("symbol '{}' definition found at line {}", sym, line_idx + 1),
                );
            } else {
                reasons.insert(
                    0,
                    format!("symbol '{}' reference found at line {}", sym, line_idx + 1),
                );
            }
            let (ls, le, truncated) =
                clamp_and_truncate(block_start, block_end, max_block_lines, &mut reasons);
            return Some(SelectedSpan {
                line_start: ls,
                line_end: le,
                selection_kind,
                symbol: Some(sym.to_string()),
                symbol_kind: Some(kind),
                confidence: conf,
                reasons,
                expanded: true,
                truncated_by_max_block_lines: truncated,
            });
        }
        // Symbol not found — fall through to match_text.
        symbol_not_found = true;
    }

    // 4. Match text search — return the matched line directly (no block expansion).
    if let Some(text) = match_text {
        if let Some((line_idx, mut reasons)) = find_match_text_line(all_lines, text) {
            reasons.insert(
                0,
                format!("match_text '{}' found at line {}", text, line_idx + 1),
            );
            let ls = (line_idx as u32) + 1;
            let le = ls;
            return Some(SelectedSpan {
                line_start: ls,
                line_end: le,
                selection_kind: SpanSelectionKind::MatchText,
                symbol: None,
                symbol_kind: None,
                confidence: SpanConfidence::Strong,
                reasons,
                expanded: false,
                truncated_by_max_block_lines: false,
            });
        }
    }

    // 5. Whole file bounded — only when no explicit inputs were given.
    if symbol_not_found || match_text.is_some() {
        // Symbol was provided but not found, or match_text was provided
        // but not found. Return None to signal no match.
        return None;
    }
    let mut reasons = vec!["whole file (no symbol or match_text provided)".to_string()];
    let (ls, le, truncated) =
        clamp_and_truncate(0, all_lines.len().saturating_sub(1), max_block_lines, &mut reasons);
    Some(SelectedSpan {
        line_start: ls,
        line_end: le,
        selection_kind: SpanSelectionKind::WholeFileBounded,
        symbol: None,
        symbol_kind: None,
        confidence: SpanConfidence::Unknown,
        reasons,
        expanded: false,
        truncated_by_max_block_lines: truncated,
    })
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Clamp `line_start`..=`line_end` to `max_block_lines` if specified.
///
/// Returns `(line_start_1indexed, line_end_1indexed, truncated)`.
fn clamp_and_truncate(
    block_start: usize,
    block_end: usize,
    max_block_lines: Option<usize>,
    reasons: &mut Vec<String>,
) -> (u32, u32, bool) {
    let ls = (block_start as u32) + 1;
    let mut le = (block_end as u32) + 1;
    let mut truncated = false;
    if let Some(max) = max_block_lines {
        let max = max as u32;
        if le - ls + 1 > max {
            le = ls + max - 1;
            truncated = true;
            reasons.push(format!("truncated to {max} lines"));
        }
    }
    (ls, le, truncated)
}

/// Scan lines for a definition/declaration matching `symbol`.
///
/// Returns `(line_index, SymbolKind, confidence, reasons)`.
fn find_symbol_line(
    all_lines: &[String],
    symbol: &str,
    language: &str,
) -> Option<(usize, SymbolKind, SpanConfidence, Vec<String>)> {
    let lang = classify_language(language);

    for (line_idx, line) in all_lines.iter().enumerate() {
        let trimmed = line.trim_start();
        // Skip pure comments for definition matching but not doc comments.
        if trimmed.starts_with("//") && !trimmed.starts_with("///") && !trimmed.starts_with("//!")
        {
            continue;
        }

        if let Some(result) = try_match_symbol_in_line(line, symbol, lang) {
            let (kind, conf, mut reasons) = result;
            reasons.push(format!(
                "matched {} pattern on line {}",
                kind_name(kind),
                line_idx + 1
            ));
            return Some((line_idx, kind, conf, reasons));
        }
    }
    None
}

/// Classify a language string into a family for pattern selection.
fn classify_language(lang: &str) -> LanguageFamily {
    let lower = lang.to_lowercase();
    match lower.as_str() {
        "rust" | "rs" => LanguageFamily::Rust,
        "python" | "py" => LanguageFamily::Python,
        "javascript" | "js" | "typescript" | "ts" | "tsx" | "jsx" => LanguageFamily::JavaScript,
        "go" => LanguageFamily::Go,
        "java" => LanguageFamily::Java,
        "kotlin" | "kt" => LanguageFamily::Kotlin,
        "c" | "cpp" | "cc" | "cxx" | "c++" | "h" | "hpp" => LanguageFamily::Cpp,
        "csharp" | "cs" => LanguageFamily::CSharp,
        "ruby" | "rb" => LanguageFamily::Ruby,
        "php" => LanguageFamily::Php,
        "swift" => LanguageFamily::Swift,
        "markdown" | "md" => LanguageFamily::Markdown,
        "toml" => LanguageFamily::Toml,
        "yaml" | "yml" => LanguageFamily::Yaml,
        "json" => LanguageFamily::Json,
        "xml" | "html" | "htm" => LanguageFamily::Xml,
        "css" | "scss" | "less" => LanguageFamily::Css,
        _ => LanguageFamily::Unknown,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LanguageFamily {
    Rust,
    Python,
    JavaScript,
    Go,
    Java,
    Kotlin,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Swift,
    Markdown,
    Toml,
    Yaml,
    Json,
    Xml,
    Css,
    Unknown,
}

/// Try to match a symbol name in a single line.
///
/// Returns `(SymbolKind, SpanConfidence, reasons)` on match.
fn try_match_symbol_in_line(
    line: &str,
    symbol: &str,
    lang: LanguageFamily,
) -> Option<(SymbolKind, SpanConfidence, Vec<String>)> {
    match lang {
        LanguageFamily::Rust => try_match_rust(line, symbol),
        LanguageFamily::Python => try_match_python(line, symbol),
        LanguageFamily::JavaScript => try_match_javascript(line, symbol),
        LanguageFamily::Go => try_match_go(line, symbol),
        LanguageFamily::Java
        | LanguageFamily::Kotlin
        | LanguageFamily::Cpp
        | LanguageFamily::CSharp
        | LanguageFamily::Ruby
        | LanguageFamily::Php
        | LanguageFamily::Swift => try_match_generic_brace(line, symbol),
        LanguageFamily::Markdown => None,
        LanguageFamily::Toml
        | LanguageFamily::Yaml
        | LanguageFamily::Json
        | LanguageFamily::Xml
        | LanguageFamily::Css
        | LanguageFamily::Unknown => None,
    }
}

fn try_match_rust(
    line: &str,
    symbol: &str,
) -> Option<(SymbolKind, SpanConfidence, Vec<String>)> {
    let checks: &[(&Regex, SymbolKind, &str)] = &[
        (&RUST_FN_RE, SymbolKind::Function, "rust fn"),
        (&RUST_STRUCT_RE, SymbolKind::Struct, "rust struct"),
        (&RUST_ENUM_RE, SymbolKind::Enum, "rust enum"),
        (&RUST_TRAIT_RE, SymbolKind::Trait, "rust trait"),
        (&RUST_IMPL_RE, SymbolKind::Struct, "rust impl"),
        (&RUST_MOD_RE, SymbolKind::Module, "rust mod"),
        (&RUST_MACRO_RE, SymbolKind::Macro, "rust macro_rules"),
        (&RUST_CONST_RE, SymbolKind::Constant, "rust const/static"),
        (&RUST_TYPE_RE, SymbolKind::TypeAlias, "rust type"),
    ];

    for (re, kind, label) in checks {
        if let Some(caps) = re.captures(line) {
            if let Some(name_match) = caps.get(1) {
                if name_match.as_str().eq_ignore_ascii_case(symbol) {
                    let conf = if *kind == SymbolKind::Function
                        || *kind == SymbolKind::Struct
                        || *kind == SymbolKind::Enum
                        || *kind == SymbolKind::Trait
                    {
                        SpanConfidence::Exact
                    } else {
                        SpanConfidence::Strong
                    };
                    return Some((*kind, conf, vec![label.to_string()]));
                }
            }
        }
    }
    None
}

fn try_match_python(
    line: &str,
    symbol: &str,
) -> Option<(SymbolKind, SpanConfidence, Vec<String>)> {
    // Class
    if let Some(caps) = PYTHON_CLASS_RE.captures(line) {
        if let Some(m) = caps.get(1) {
            if m.as_str().eq_ignore_ascii_case(symbol) {
                return Some((
                    SymbolKind::Class,
                    SpanConfidence::Exact,
                    vec!["python class".to_string()],
                ));
            }
        }
    }
    // Function / async def
    if let Some(caps) = PYTHON_DEF_RE.captures(line) {
        if let Some(m) = caps.get(1) {
            if m.as_str().eq_ignore_ascii_case(symbol) {
                return Some((
                    SymbolKind::Function,
                    SpanConfidence::Exact,
                    vec!["python def".to_string()],
                ));
            }
        }
    }
    None
}

fn try_match_javascript(
    line: &str,
    symbol: &str,
) -> Option<(SymbolKind, SpanConfidence, Vec<String>)> {
    let checks: &[(&Regex, SymbolKind, &str)] = &[
        (&JS_FUNCTION_RE, SymbolKind::Function, "js function"),
        (&JS_CLASS_RE, SymbolKind::Class, "js class"),
        (&JS_INTERFACE_RE, SymbolKind::Interface, "ts interface"),
        (&JS_TYPE_RE, SymbolKind::TypeAlias, "ts type"),
        (&JS_CONST_ARROW_RE, SymbolKind::Function, "js const arrow"),
    ];

    for (re, kind, label) in checks {
        if let Some(caps) = re.captures(line) {
            if let Some(m) = caps.get(1) {
                if m.as_str().eq_ignore_ascii_case(symbol) {
                    let conf = if *kind == SymbolKind::Function || *kind == SymbolKind::Class {
                        SpanConfidence::Exact
                    } else {
                        SpanConfidence::Strong
                    };
                    return Some((*kind, conf, vec![label.to_string()]));
                }
            }
        }
    }
    None
}

fn try_match_go(
    line: &str,
    symbol: &str,
) -> Option<(SymbolKind, SpanConfidence, Vec<String>)> {
    // type X struct/interface
    if let Some(caps) = GO_TYPE_RE.captures(line) {
        if let Some(m) = caps.get(1) {
            if m.as_str() == symbol {
                return Some((
                    SymbolKind::Struct,
                    SpanConfidence::Exact,
                    vec!["go type".to_string()],
                ));
            }
        }
    }
    // func
    if let Some(caps) = GO_FUNC_RE.captures(line) {
        if let Some(m) = caps.get(1) {
            if m.as_str() == symbol {
                return Some((
                    SymbolKind::Function,
                    SpanConfidence::Exact,
                    vec!["go func".to_string()],
                ));
            }
        }
    }
    None
}

fn try_match_generic_brace(
    line: &str,
    symbol: &str,
) -> Option<(SymbolKind, SpanConfidence, Vec<String>)> {
    // Java/Kotlin/C++ class/interface/struct
    if let Some(caps) = JAVA_KW_RE.captures(line) {
        if let Some(m) = caps.get(1) {
            if m.as_str().eq_ignore_ascii_case(symbol) {
                return Some((
                    SymbolKind::Class,
                    SpanConfidence::Strong,
                    vec!["generic class/interface".to_string()],
                ));
            }
        }
    }
    if let Some(caps) = CPP_STRUCT_RE.captures(line) {
        if let Some(m) = caps.get(1) {
            if m.as_str().eq_ignore_ascii_case(symbol) {
                return Some((
                    SymbolKind::Struct,
                    SpanConfidence::Weak,
                    vec!["generic struct/class".to_string()],
                ));
            }
        }
    }
    None
}

/// Human-readable name for a SymbolKind.
fn kind_name(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Class => "class",
        SymbolKind::Interface => "interface",
        SymbolKind::Module => "module",
        SymbolKind::Constant => "constant",
        SymbolKind::TypeAlias => "type_alias",
        SymbolKind::Macro => "macro",
        SymbolKind::Unknown => "unknown",
    }
}

/// Find the first line containing `match_text`.
fn find_match_text_line(all_lines: &[String], match_text: &str) -> Option<(usize, Vec<String>)> {
    let needle = match_text.to_lowercase();
    for (idx, line) in all_lines.iter().enumerate() {
        if line.to_lowercase().contains(&needle) {
            return Some((idx, vec![]));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Block expansion
// ---------------------------------------------------------------------------

/// From a line inside a block, find the enclosing block boundaries.
///
/// Returns `(start_index, end_index)` as 0-based indices inclusive.
/// Includes doc comments, attributes, and decorators immediately above.
fn expand_to_enclosing_block(
    all_lines: &[String],
    line_idx: usize,
    language: &str,
) -> (usize, usize) {
    let lang = classify_language(language);
    match lang {
        LanguageFamily::Rust => expand_brace_block_with_attrs(all_lines, line_idx),
        LanguageFamily::Python => expand_indentation_block(all_lines, line_idx),
        LanguageFamily::JavaScript => expand_brace_block(all_lines, line_idx),
        LanguageFamily::Go => expand_brace_block(all_lines, line_idx),
        LanguageFamily::Java
        | LanguageFamily::Kotlin
        | LanguageFamily::Cpp
        | LanguageFamily::CSharp
        | LanguageFamily::Ruby
        | LanguageFamily::Php
        | LanguageFamily::Swift => expand_brace_block(all_lines, line_idx),
        LanguageFamily::Markdown => expand_markdown_heading(all_lines, line_idx),
        LanguageFamily::Toml => expand_toml_table(all_lines, line_idx),
        LanguageFamily::Yaml => expand_yaml_key(all_lines, line_idx),
        _ => expand_brace_block(all_lines, line_idx),
    }
}

/// Expand to an enclosing `{}` block, then include `#[...]` attributes
/// and `///`/`//!` doc comments immediately above.
fn expand_brace_block_with_attrs(all_lines: &[String], line_idx: usize) -> (usize, usize) {
    let (mut start, end) = expand_brace_block(all_lines, line_idx);

    // Walk backwards from start to include attributes and doc comments.
    while start > 0 {
        let prev = start - 1;
        let trimmed = all_lines[prev].trim();
        if trimmed.starts_with("#[")
            || trimmed.starts_with("#![")
            || trimmed.starts_with("///")
            || trimmed.starts_with("//!")
        {
            start = prev;
        } else {
            break;
        }
    }

    (start, end)
}

/// Expand to an enclosing `{}` block using brace counting.
fn expand_brace_block(all_lines: &[String], line_idx: usize) -> (usize, usize) {
    if all_lines.is_empty() {
        return (0, 0);
    }

    let total = all_lines.len();

    // Scan backwards to find the opening `{`.
    let mut depth: i32 = 0;
    let mut start = line_idx;
    for i in (0..=line_idx).rev() {
        let line = &all_lines[i];
        for ch in line.chars().rev() {
            match ch {
                '}' => depth += 1,
                '{' => {
                    depth -= 1;
                    if depth < 0 {
                        start = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth < 0 {
            break;
        }
    }

    // Scan forwards to find the matching closing `}`.
    let mut depth: i32 = 0;
    let mut end = start;
    for (i, line) in all_lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth == 0 {
            break;
        }
    }

    // If we never balanced, use the whole file from start.
    if depth != 0 {
        end = total.saturating_sub(1);
    }

    (start, end)
}

/// Expand to an indentation-based block (Python).
///
/// Includes `@decorator` lines immediately above.
fn expand_indentation_block(all_lines: &[String], line_idx: usize) -> (usize, usize) {
    if all_lines.is_empty() {
        return (0, 0);
    }

    let base_indent = count_indent(&all_lines[line_idx]);

    // Find the block start: walk backwards until we hit a line with
    // equal or lesser indentation (or a decorator).
    let mut start = line_idx;
    for i in (0..line_idx).rev() {
        let line = &all_lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('@') {
            start = i;
            continue;
        }
        let indent = count_indent(line);
        if indent > base_indent {
            continue;
        }
        if indent == base_indent
            && (trimmed.starts_with("def ") || trimmed.starts_with("async ") || trimmed.starts_with("class "))
        {
            start = i;
            break;
        }
        start = i + 1;
        break;
    }

    // Include decorators above start.
    while start > 0 {
        let prev = start - 1;
        let trimmed = all_lines[prev].trim();
        if trimmed.starts_with('@') {
            start = prev;
        } else {
            break;
        }
    }

    // Find the block end: walk forward until we hit a line with
    // equal or lesser indentation (that is not blank).
    let mut end = line_idx;
    let mut last_content = line_idx;
    for (i, line) in all_lines.iter().enumerate().skip(line_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = count_indent(line);
        if indent <= base_indent {
            end = last_content;
            return (start, end);
        }
        last_content = i;
        end = i;
    }

    if end < start {
        end = start;
    }

    (start, end)
}

/// Count leading spaces (not tabs) for indentation.
fn count_indent(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Expand to a Markdown heading section.
///
/// A section runs from the heading line to (but not including) the
/// next heading of the same or higher level.
fn expand_markdown_heading(all_lines: &[String], line_idx: usize) -> (usize, usize) {
    if all_lines.is_empty() {
        return (0, 0);
    }

    let total = all_lines.len();

    let current_level = {
        let caps = MARKDOWN_HEADING_RE.captures(&all_lines[line_idx]);
        caps.map(|c| c.get(1).unwrap().as_str().len())
            .unwrap_or(6)
    };

    let start = line_idx;

    let mut end = total.saturating_sub(1);
    for (i, line) in all_lines.iter().enumerate().skip(line_idx + 1) {
        if let Some(caps) = MARKDOWN_HEADING_RE.captures(line) {
            let level = caps.get(1).unwrap().as_str().len();
            if level <= current_level {
                end = i - 1;
                break;
            }
        }
    }

    (start, end)
}

/// Expand to a TOML table section.
///
/// Walks backwards to find the enclosing `[table]` header, then forward
/// to the next table header or EOF. Excludes trailing blank lines.
fn expand_toml_table(all_lines: &[String], line_idx: usize) -> (usize, usize) {
    if all_lines.is_empty() {
        return (0, 0);
    }

    let total = all_lines.len();

    // Find the table header above or at line_idx.
    let mut start = line_idx;
    for i in (0..=line_idx).rev() {
        let trimmed = all_lines[i].trim();
        if TOML_TABLE_RE.is_match(trimmed) {
            start = i;
            break;
        }
        // If we hit a non-blank, non-table line before the header, keep going.
    }

    // Find end: next table header after start.
    let mut end = total.saturating_sub(1);
    for (i, line) in all_lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim();
        if TOML_TABLE_RE.is_match(trimmed) {
            end = i - 1;
            break;
        }
    }

    // Exclude trailing blank lines.
    while end > start && all_lines[end].trim().is_empty() {
        end -= 1;
    }

    (start, end)
}

/// Expand to a YAML key section (lines with greater indentation).
///
/// Walks backwards to find the parent key, then forward to find the
/// last content line with greater indentation.
fn expand_yaml_key(all_lines: &[String], line_idx: usize) -> (usize, usize) {
    if all_lines.is_empty() {
        return (0, 0);
    }

    // Walk backwards to find the parent key (a line with less indentation).
    let mut start = line_idx;
    for i in (0..=line_idx).rev() {
        let line = &all_lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = count_indent(line);
        let original_indent = count_indent(&all_lines[line_idx]);
        if indent < original_indent {
            start = i;
            break;
        }
        // At same indent, this might be the key itself.
        if indent == original_indent && i < line_idx {
            // Another key at same level — our block starts after it.
            start = i + 1;
            break;
        }
    }

    // Now find the base indent from the start line.
    let base_indent = count_indent(&all_lines[start]);

    // Find the block end: walk forward past all lines with greater indentation.
    let mut end = start;
    let mut last_content = start;
    for (i, line) in all_lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = count_indent(line);
        if indent <= base_indent {
            end = last_content;
            return (start, end);
        }
        last_content = i;
        end = i;
    }

    // Exclude trailing blank lines.
    while end > start && all_lines[end].trim().is_empty() {
        end -= 1;
    }

    (start, end)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(String::from).collect()
    }

    // --- Rust function block expansion with attributes/doc comments ---

    #[test]
    fn rust_fn_with_attrs_and_docs() {
        let input = lines(
            "#[cfg(test)]\n\
             /// A test helper.\n\
             /// Does important things.\n\
             fn helper() {\n\
             \x20   let x = 1;\n\
             \x20   let y = 2;\n\
             }\n\
             \n\
             fn other() {}",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("helper"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1); // #[cfg(test)]
        assert_eq!(span.line_end, 7); // }
        assert_eq!(span.selection_kind, SpanSelectionKind::SymbolDefinition);
        assert_eq!(span.symbol.as_deref(), Some("helper"));
        assert_eq!(span.symbol_kind, Some(SymbolKind::Function));
        assert_eq!(span.confidence, SpanConfidence::Exact);
        assert!(span.expanded);
    }

    #[test]
    fn rust_struct_expansion() {
        let input = lines(
            "/// My struct.\n\
             #[derive(Debug)]\n\
             struct Foo {\n\
             \x20   field: i32,\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("Foo"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1); // doc comment
        assert_eq!(span.line_end, 5); // }
        assert_eq!(span.selection_kind, SpanSelectionKind::SymbolDefinition);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Struct));
    }

    #[test]
    fn rust_impl_method() {
        let input = lines(
            "impl MyStruct {\n\
             \x20   fn method(&self) {\n\
             \x20       let x = 1;\n\
             \x20   }\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("method"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 2); // fn method
        assert_eq!(span.line_end, 4); // }
        assert_eq!(span.symbol.as_deref(), Some("method"));
    }

    #[test]
    fn rust_trait_expansion() {
        let input = lines(
            "/// Trait docs.\n\
             trait Foo {\n\
             \x20   fn bar(&self);\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("Foo"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 4);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Trait));
    }

    #[test]
    fn rust_macro_rules() {
        let input = lines(
            "macro_rules! my_macro {\n\
             \x20   ($($t:tt)*) => {\n\
             \x20       // ...\n\
             \x20   };\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("my_macro"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 5);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Macro));
    }

    #[test]
    fn rust_const_expansion() {
        let input = lines("const MAX_SIZE: usize = 1024;");
        let span = select_span(
            &input,
            Some("rust"),
            Some("MAX_SIZE"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 1);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Constant));
    }

    // --- Python class/function indentation expansion ---

    #[test]
    fn python_class_expansion() {
        let input = lines(
            "@dataclass\n\
             class User:\n\
             \x20   name: str\n\
             \x20   age: int\n\
             \n\
             \x20   def greet(self) -> str:\n\
             \x20       return f\"Hello, {self.name}\"",
        );
        let span = select_span(
            &input,
            Some("python"),
            Some("User"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1); // @dataclass
        assert_eq!(span.line_end, 7); // return ...
        assert_eq!(span.selection_kind, SpanSelectionKind::SymbolDefinition);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Class));
    }

    #[test]
    fn python_def_expansion() {
        let input = lines(
            "def hello():\n\
             \x20   print('hi')\n\
             \x20   return True\n\
             \n\
             def world():\n\
             \x20   print('world')",
        );
        let span = select_span(
            &input,
            Some("python"),
            Some("hello"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Function));
    }

    #[test]
    fn python_async_def() {
        let input = lines(
            "async def fetch_data():\n\
             \x20   async with aiohttp.get(url) as resp:\n\
             \x20       return await resp.json()",
        );
        let span = select_span(
            &input,
            Some("python"),
            Some("fetch_data"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
    }

    #[test]
    fn python_multi_decorator() {
        let input = lines(
            "@staticmethod\n\
             @retry(max_attempts=3)\n\
             def fetchData():\n\
             \x20   return requests.get(url)",
        );
        let span = select_span(
            &input,
            Some("python"),
            Some("fetchData"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1); // @staticmethod
        assert_eq!(span.line_end, 4); // return ...
    }

    #[test]
    fn python_nested_indentation() {
        let input = lines(
            "class Outer:\n\
             \x20   def method(self):\n\
             \x20       if True:\n\
             \x20           x = 1\n\
             \x20           y = 2\n\
             \x20   def other(self):\n\
             \x20       pass",
        );
        let span = select_span(
            &input,
            Some("python"),
            Some("method"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 2);
        // The "if True:" body is at deeper indent, but the "def method" block
        // ends at the last content line before "def other" at same indent.
        assert_eq!(span.line_end, 5);
    }

    // --- JS/TS function and arrow function expansion ---

    #[test]
    fn js_function_expansion() {
        let input = lines(
            "export function handler(req, res) {\n\
             \x20   const data = req.body;\n\
             \x20   res.send(data);\n\
             }",
        );
        let span = select_span(
            &input,
            Some("javascript"),
            Some("handler"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 4);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Function));
    }

    #[test]
    fn js_arrow_function_const() {
        let input = lines(
            "const handleClick = (event) => {\n\
             \x20   event.preventDefault();\n\
             \x20   console.log('clicked');\n\
             };",
        );
        let span = select_span(
            &input,
            Some("javascript"),
            Some("handleClick"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 4);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Function));
    }

    #[test]
    fn ts_interface_expansion() {
        let input = lines(
            "export interface Config {\n\
             \x20   host: string;\n\
             \x20   port: number;\n\
             }",
        );
        let span = select_span(
            &input,
            Some("typescript"),
            Some("Config"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 4);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Interface));
    }

    #[test]
    fn js_class_expansion() {
        let input = lines(
            "class EventEmitter {\n\
             \x20   constructor() {\n\
             \x20       this.listeners = [];\n\
             \x20   }\n\
             \n\
             \x20   on(event, fn) {\n\
             \x20       this.listeners.push(fn);\n\
             \x20   }\n\
             }",
        );
        let span = select_span(
            &input,
            Some("javascript"),
            Some("EventEmitter"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 9);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Class));
    }

    // --- Go function expansion ---

    #[test]
    fn go_func_expansion() {
        let input = lines(
            "func serve(addr string) error {\n\
             \x20   ln, err := net.Listen(\"tcp\", addr)\n\
             \x20   if err != nil {\n\
             \x20       return err\n\
             \x20   }\n\
             \x20   return http.Serve(ln, nil)\n\
             }",
        );
        let span = select_span(
            &input,
            Some("go"),
            Some("serve"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 7);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Function));
    }

    #[test]
    fn go_type_struct_expansion() {
        let input = lines(
            "type Server struct {\n\
             \x20   addr string\n\
             \x20   port int\n\
             }",
        );
        let span = select_span(
            &input,
            Some("go"),
            Some("Server"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 4);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Struct));
    }

    #[test]
    fn go_method_expansion() {
        let input = lines(
            "func (s *Server) Listen() error {\n\
             \x20   ln, err := net.Listen(\"tcp\", s.addr)\n\
             \x20   return err\n\
             }",
        );
        let span = select_span(
            &input,
            Some("go"),
            Some("Listen"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 4);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Function));
    }

    // --- Markdown heading-section expansion ---

    #[test]
    fn markdown_heading_section() {
        let input = lines(
            "# Title\n\
             Some intro.\n\
             \n\
             ## Section A\n\
             Content A.\n\
             More content.\n\
             \n\
             ## Section B\n\
             Content B.",
        );
        let span = select_span(
            &input,
            Some("markdown"),
            None,
            None,
            None,
            Some(4), // "## Section A"
            Some(4),
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 4);
        assert_eq!(span.line_end, 7); // before "## Section B"
        assert_eq!(
            span.selection_kind,
            SpanSelectionKind::ExpandedExplicitRange
        );
    }

    #[test]
    fn markdown_heading_to_eof() {
        let input = lines(
            "# Title\n\
             \n\
             ## Last Section\n\
             Content here.\n\
             More content.",
        );
        let span = select_span(
            &input,
            Some("markdown"),
            None,
            None,
            None,
            Some(3),
            Some(3),
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 3);
        assert_eq!(span.line_end, 5);
    }

    #[test]
    fn markdown_nested_heading_level() {
        let input = lines(
            "# Top\n\
             content\n\
             ## Sub A\n\
             sub content\n\
             ## Sub B\n\
             more content\n\
             # Top Two\n\
             other",
        );
        let span = select_span(
            &input,
            Some("markdown"),
            None,
            None,
            None,
            Some(3),
            Some(3),
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 3);
        assert_eq!(span.line_end, 4);
    }

    // --- Match-text context fallback ---

    #[test]
    fn match_text_fallback() {
        let input = lines(
            "line one\n\
             line two\n\
             the answer is 42 here\n\
             line four",
        );
        let span = select_span(
            &input,
            Some("rust"),
            None,
            None,
            Some("answer is 42"),
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 3);
        assert_eq!(span.line_end, 3);
        assert_eq!(span.selection_kind, SpanSelectionKind::MatchText);
        assert_eq!(span.confidence, SpanConfidence::Strong);
    }

    #[test]
    fn match_text_case_insensitive() {
        let input = lines("Hello World\nfoo bar");
        let span = select_span(
            &input,
            None,
            None,
            None,
            Some("hello world"),
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 1);
    }

    // --- max_block_lines truncation ---

    #[test]
    fn max_block_lines_truncation() {
        let input = lines(
            "fn big() {\n\
             \x20   let a = 1;\n\
             \x20   let b = 2;\n\
             \x20   let c = 3;\n\
             \x20   let d = 4;\n\
             \x20   let e = 5;\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("big"),
            None,
            None,
            None,
            None,
            true,
            Some(3),
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3); // 1 + 3 - 1 = 3
        assert!(span.truncated_by_max_block_lines);
        assert!(span.reasons.iter().any(|r| r.contains("truncated")));
    }

    #[test]
    fn max_block_lines_no_truncation_when_within_limit() {
        let input = lines(
            "fn small() {\n\
             \x20   let x = 1;\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("small"),
            None,
            None,
            None,
            None,
            true,
            Some(10),
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
        assert!(!span.truncated_by_max_block_lines);
    }

    #[test]
    fn whole_file_respects_max_block_lines() {
        let input = lines("a\nb\nc\nd\ne");
        let span = select_span(
            &input,
            None,
            None,
            None,
            None,
            None,
            None,
            true,
            Some(3),
        )
        .unwrap();
        assert_eq!(span.selection_kind, SpanSelectionKind::WholeFileBounded);
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
        assert!(span.truncated_by_max_block_lines);
    }

    // --- Explicit range unchanged when expand_to_block is false ---

    #[test]
    fn explicit_range_no_expansion() {
        let input = lines(
            "fn foo() {\n\
             \x20   let a = 1;\n\
             \x20   let b = 2;\n\
             \x20   let c = 3;\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            None,
            None,
            None,
            Some(2),
            Some(3),
            false,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 2);
        assert_eq!(span.line_end, 3);
        assert_eq!(span.selection_kind, SpanSelectionKind::ExplicitRange);
        assert!(!span.expanded);
        assert_eq!(span.confidence, SpanConfidence::Exact);
    }

    // --- Explicit range expanded when expand_to_block is true ---

    #[test]
    fn explicit_range_with_expansion() {
        let input = lines(
            "fn foo() {\n\
             \x20   let a = 1;\n\
             \x20   let b = 2;\n\
             \x20   let c = 3;\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            None,
            None,
            None,
            Some(2),
            Some(3),
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 5);
        assert_eq!(
            span.selection_kind,
            SpanSelectionKind::ExpandedExplicitRange
        );
        assert!(span.expanded);
    }

    #[test]
    fn explicit_range_clamped_to_file_bounds() {
        let input = lines("a\nb\nc");
        let span = select_span(
            &input,
            None,
            None,
            None,
            None,
            Some(0),
            Some(99),
            false,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
    }

    // --- Missing symbol returns WholeFileBounded ---

    #[test]
    fn missing_symbol_returns_whole_file() {
        let input = lines(
            "fn foo() {\n\
             \x20   let x = 1;\n\
             }",
        );
        let result = select_span(
            &input,
            Some("rust"),
            Some("nonexistent"),
            None,
            None,
            None,
            None,
            true,
            None,
        );
        // Symbol provided but not found → returns None.
        assert!(result.is_none());
    }

    #[test]
    fn missing_match_text_returns_whole_file() {
        let input = lines("hello\nworld");
        // match_text provided but not found → returns None.
        let result = select_span(
            &input,
            None,
            None,
            None,
            Some("zzz not found"),
            None,
            None,
            true,
            None,
        );
        assert!(result.is_none());
    }

    #[test]
    fn no_inputs_returns_whole_file() {
        let input = lines("line 1\nline 2\nline 3");
        let span = select_span(
            &input,
            None,
            None,
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.selection_kind, SpanSelectionKind::WholeFileBounded);
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
    }

    #[test]
    fn symbol_not_found_falls_through_to_match_text() {
        let input = lines(
            "fn foo() {\n\
             \x20   let x = 42;\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("nonexistent"),
            None,
            Some("let x = 42"),
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.selection_kind, SpanSelectionKind::MatchText);
        assert_eq!(span.line_start, 2);
        assert_eq!(span.line_end, 2);
    }

    // --- Empty file ---

    #[test]
    fn empty_file_returns_none() {
        let input: Vec<String> = vec![];
        let result = select_span(
            &input,
            Some("rust"),
            Some("foo"),
            None,
            None,
            None,
            None,
            true,
            None,
        );
        assert!(result.is_none());
    }

    // --- TOML table expansion ---

    #[test]
    fn toml_table_expansion() {
        let input = lines(
            "[package]\n\
             name = \"foo\"\n\
             version = \"1.0\"\n\
             \n\
             [dependencies]\n\
             serde = \"1\"",
        );
        let span = select_span(
            &input,
            Some("toml"),
            None,
            None,
            None,
            Some(1),
            Some(1),
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
    }

    #[test]
    fn toml_nested_table() {
        let input = lines(
            "[package]\n\
             name = \"foo\"\n\
             \n\
             [dependencies]\n\
             serde = \"1\"",
        );
        let span = select_span(
            &input,
            Some("toml"),
            None,
            None,
            None,
            Some(2),
            Some(2),
            true,
            None,
        )
        .unwrap();
        // line 2 is "name = foo" inside [package]
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 2);
    }

    // --- YAML key expansion ---

    #[test]
    fn yaml_key_expansion() {
        let input = lines(
            "server:\n\
             \x20  host: localhost\n\
             \x20  port: 8080\n\
             \n\
             logging:\n\
             \x20  level: info",
        );
        let span = select_span(
            &input,
            Some("yaml"),
            None,
            None,
            None,
            Some(1),
            Some(1),
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
    }

    #[test]
    fn yaml_nested_key() {
        let input = lines(
            "server:\n\
             \x20  host: localhost\n\
             \x20  port: 8080\n\
             logging:\n\
             \x20  level: info",
        );
        let span = select_span(
            &input,
            Some("yaml"),
            None,
            None,
            None,
            Some(2),
            Some(2),
            true,
            None,
        )
        .unwrap();
        // line 2 is "host: localhost" under server
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 3);
    }

    // --- Java class expansion ---

    #[test]
    fn java_class_expansion() {
        let input = lines(
            "public class Server {\n\
             \x20   private int port;\n\
             \n\
             \x20   public void start() {\n\
             \x20       // ...\n\
             \x20   }\n\
             }",
        );
        let span = select_span(
            &input,
            Some("java"),
            Some("Server"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 7);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Class));
        assert_eq!(span.confidence, SpanConfidence::Strong);
    }

    // --- Unrecognized language falls back to brace matching ---

    #[test]
    fn unknown_language_returns_none_for_symbol() {
        let input = lines(
            "fn foo() {\n\
             \x20   let x = 1;\n\
             }",
        );
        // Unknown language has no symbol patterns → returns None.
        let result = select_span(
            &input,
            Some("unknown_lang"),
            Some("foo"),
            None,
            None,
            None,
            None,
            true,
            None,
        );
        assert!(result.is_none());
    }

    // --- Nested brace blocks ---

    #[test]
    fn rust_nested_braces() {
        let input = lines(
            "fn outer() {\n\
             \x20   if true {\n\
             \x20       fn inner() {\n\
             \x20           let x = 1;\n\
             \x20       }\n\
             \x20   }\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("inner"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 3);
        assert_eq!(span.line_end, 5);
    }

    // --- Rust enum expansion ---

    #[test]
    fn rust_enum_expansion() {
        let input = lines(
            "#[derive(Clone)]\n\
             enum Color {\n\
             \x20   Red,\n\
             \x20   Green,\n\
             \x20   Blue,\n\
             }",
        );
        let span = select_span(
            &input,
            Some("rust"),
            Some("Color"),
            None,
            None,
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 6);
        assert_eq!(span.symbol_kind, Some(SymbolKind::Enum));
    }

    // --- Selection kind serialization ---

    #[test]
    fn selected_span_serializes_snake_case() {
        let span = SelectedSpan {
            line_start: 1,
            line_end: 10,
            selection_kind: SpanSelectionKind::SymbolDefinition,
            symbol: Some("foo".to_string()),
            symbol_kind: Some(SymbolKind::Function),
            confidence: SpanConfidence::Exact,
            reasons: vec!["test".to_string()],
            expanded: true,
            truncated_by_max_block_lines: false,
        };
        let json = serde_json::to_value(&span).unwrap();
        assert_eq!(json["selection_kind"], "symbol_definition");
        assert_eq!(json["confidence"], "exact");
        assert_eq!(json["symbol"], "foo");
        assert_eq!(json["symbol_kind"], "function");
        assert_eq!(json["expanded"], true);
        assert_eq!(json["truncated_by_max_block_lines"], false);
    }

    #[test]
    fn selected_span_default_unknown() {
        let span = SelectedSpan {
            line_start: 1,
            line_end: 1,
            selection_kind: SpanSelectionKind::default(),
            symbol: None,
            symbol_kind: None,
            confidence: SpanConfidence::default(),
            reasons: vec![],
            expanded: false,
            truncated_by_max_block_lines: false,
        };
        assert_eq!(span.selection_kind, SpanSelectionKind::WholeFileBounded);
        assert_eq!(span.confidence, SpanConfidence::Unknown);
    }
}
