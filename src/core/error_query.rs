//! Deterministic error-message parser and subquery generator for exact-error search mode.
//!
//! Parses compiler errors, runtime exceptions, linker errors, dependency resolution
//! failures, and opaque toolchain messages into structured query parts that can be
//! used to generate targeted subqueries.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

static API_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:api[_-]?key|token|secret|password)\s*[=:]\s*["']?([a-f0-9]{32,})["']?"#)
        .expect("API token regex is valid")
});
static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"#)
        .expect("UUID regex is valid")
});
static MEMORY_ADDRESS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)0x[0-9a-f]{8,}").expect("memory address regex is valid"));
static LOCAL_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:/[\w.-]+){2,}").expect("local path regex is valid"));

static RUST_ERROR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\b(E0\d{3})\b"#).expect("Rust error regex is valid"));
static TYPESCRIPT_ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(TS\d{4,5})\b"#).expect("TypeScript error regex is valid")
});
static HTTP_STATUS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([45]\d{2})\s+(Not Found|Internal Server Error|Bad Request|Unauthorized|Forbidden|Service Unavailable|Gateway Timeout)")
        .expect("HTTP status regex is valid")
});
static NPM_PACKAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:package |install |resolve )([a-z@][a-z0-9._/-]+(?:@[0-9.]+)?)")
        .expect("npm package regex is valid")
});
static CARGO_PACKAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:package|crate)\s+`([^`]+)`").expect("Cargo package regex is valid")
});
static CARGO_NO_MATCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"no matching package found:\s*(\S+)").expect("Cargo no-match regex is valid")
});
static PIP_PACKAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:Package|Requirement) ([a-z][a-z0-9_-]+)").expect("pip package regex is valid")
});
static PYTHON_FRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"File "([^"]+)", line (\d+), in (\w+)"#).expect("Python frame regex is valid")
});
static RUST_FRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+(?:at\s+)?([\w:/._-]+):(\d+)$").expect("Rust frame regex is valid")
});
static RUST_MODULE_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([\w]+::[\w:]+)\b").expect("Rust module path regex is valid"));
static SOURCE_FILE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([\w/_-]+\.(?:rs|py|js|ts|tsx|jsx|go|java|cpp|c|h|hpp))\b")
        .expect("source file regex is valid")
});
static PRIMARY_ERROR_CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[ET]\d{3,5}\b").expect("primary error code regex is valid")
});

/// A structured error code extracted from the error message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ErrorCode {
    /// The raw error code string (e.g. "E0277", "TS2345", "ERESOLVE").
    pub code: String,
    /// The tool/language that owns this error code.
    pub tool: String,
}

/// A hint about a stack frame from a traceback/stack trace.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StackFrameHint {
    /// Function or method name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// File path (may be redacted to basename).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Line number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// Parsed parts of an exact error query.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ErrorQueryParts {
    /// Original error text, unmodified.
    pub original: String,
    /// Normalized form: collapsed whitespace, trimmed.
    pub normalized: String,
    /// The primary error line(s) suitable for quoted search.
    pub quoted_exact: String,
    /// Extracted error codes.
    #[serde(default)]
    pub error_codes: Vec<ErrorCode>,
    /// Detected tool names (e.g. "cargo", "npm", "rustc").
    #[serde(default)]
    pub tool_names: Vec<String>,
    /// Detected package names.
    #[serde(default)]
    pub package_names: Vec<String>,
    /// Inferred language hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_hint: Option<String>,
    /// Stack frame hints from tracebacks.
    #[serde(default)]
    pub stack_frames: Vec<StackFrameHint>,
    /// Useful path fragments (e.g. crate names, module paths).
    #[serde(default)]
    pub path_fragments: Vec<String>,
    /// Sensitive tokens that were redacted from provider queries.
    #[serde(default)]
    pub redactions_applied: Vec<String>,
}

/// Configuration for exact-error mode behavior.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExactErrorConfig {
    /// Whether exact-error mode is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum number of subqueries to generate.
    #[serde(default = "default_max_subqueries")]
    pub max_subqueries: usize,
    /// Maximum characters accepted in the error query.
    #[serde(default = "default_max_error_chars")]
    pub max_error_chars: usize,
    /// Whether to redact sensitive tokens (paths, usernames, keys).
    #[serde(default = "default_true")]
    pub redact_sensitive_tokens: bool,
}

fn default_true() -> bool {
    true
}
fn default_max_subqueries() -> usize {
    6
}
fn default_max_error_chars() -> usize {
    8000
}

impl Default for ExactErrorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_subqueries: default_max_subqueries(),
            max_error_chars: default_max_error_chars(),
            redact_sensitive_tokens: true,
        }
    }
}

/// An error search context included in the response.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ErrorSearchContext {
    /// Original error text.
    pub original_error: String,
    /// Normalized error text.
    pub normalized_error: String,
    /// Extracted error codes.
    #[serde(default)]
    pub error_codes: Vec<ErrorCode>,
    /// Detected tool names.
    #[serde(default)]
    pub inferred_tools: Vec<String>,
    /// Inferred language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inferred_language: Option<String>,
    /// Redactions applied before provider dispatch.
    #[serde(default)]
    pub redactions_applied: Vec<String>,
    /// Subqueries generated for this error.
    #[serde(default)]
    pub subqueries: Vec<ErrorSubquery>,
    /// Warnings about the error parsing.
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// A subquery generated for the error search.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ErrorSubquery {
    /// Label for this subquery.
    pub label: String,
    /// The query text sent to providers.
    pub query: String,
    /// Target group for this subquery.
    pub target_group: String,
}

/// Parse an error message into structured query parts.
///
/// This is deterministic and does not perform any network calls.
pub fn parse_error_query(error_text: &str) -> ErrorQueryParts {
    let original = error_text.to_string();
    let trimmed = error_text.trim();
    let normalized = collapse_whitespace(trimmed);

    let error_codes = extract_error_codes(trimmed);
    let tool_names = detect_tools(trimmed, &error_codes);
    let language_hint = infer_language(&error_codes, &tool_names);
    let package_names = extract_package_names(trimmed);
    let stack_frames = extract_stack_frames(trimmed);
    let path_fragments = extract_path_fragments(trimmed);
    let quoted_exact = extract_primary_error_line(trimmed);

    ErrorQueryParts {
        original,
        normalized,
        quoted_exact,
        error_codes,
        tool_names,
        package_names,
        language_hint,
        stack_frames,
        path_fragments,
        redactions_applied: Vec::new(),
    }
}

/// Redact sensitive tokens from error query parts.
///
/// Returns a new `ErrorQueryParts` with redacted values and a list of redactions applied.
pub fn redact_error_query(parts: &ErrorQueryParts) -> ErrorQueryParts {
    let mut redactions = Vec::new();
    let mut normalized = parts.normalized.clone();
    let mut quoted_exact = parts.quoted_exact.clone();

    // Redact home directory paths
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();
        if normalized.contains(&home_str) {
            normalized = normalized.replace(&home_str, "~");
            redactions.push(format!("home_directory: {home_str}"));
        }
        if quoted_exact.contains(&home_str) {
            quoted_exact = quoted_exact.replace(&home_str, "~");
        }
    }

    // Redact obvious API keys/tokens (hex strings >= 32 chars that aren't error codes)
    for cap in API_TOKEN_RE.captures_iter(&normalized.clone()) {
        if let Some(matched) = cap.get(1) {
            let token = matched.as_str().to_string();
            replace_in_provider_fields(&mut normalized, &mut quoted_exact, &token, "[REDACTED]");
            redactions.push(format!(
                "api_token: {}...{}",
                &token[..8],
                &token[token.len() - 4..]
            ));
        }
    }

    // Redact UUIDs (not commit SHAs)
    for m in UUID_RE.find_iter(&normalized.clone()) {
        let uuid = m.as_str().to_string();
        replace_in_provider_fields(&mut normalized, &mut quoted_exact, &uuid, "[UUID]");
        redactions.push(format!("uuid: {uuid}"));
    }

    // Redact memory addresses (0x...)
    for m in MEMORY_ADDRESS_RE.find_iter(&normalized.clone()) {
        let addr = m.as_str().to_string();
        replace_in_provider_fields(&mut normalized, &mut quoted_exact, &addr, "[ADDR]");
        redactions.push(format!("memory_address: {addr}"));
    }

    // Redact local absolute paths (but keep basename and useful crate/module segments)
    for m in LOCAL_PATH_RE.find_iter(&normalized.clone()) {
        let path = m.as_str().to_string();
        // Skip paths that look like URLs or error codes
        if path.starts_with("/dev") || path.starts_with("/proc") || path.starts_with("/sys") {
            continue;
        }
        // Keep basename
        let basename = path.rsplit('/').next().unwrap_or(&path);
        // If basename is meaningful (not just a number or generic dir), keep it
        if !basename.chars().all(|c| c.is_ascii_digit()) && basename.len() > 1 {
            replace_in_provider_fields(&mut normalized, &mut quoted_exact, &path, basename);
            redactions.push(format!("local_path: {path}"));
        }
    }

    let mut result = parts.clone();
    result.normalized = normalized;
    result.quoted_exact = quoted_exact;
    result.redactions_applied = redactions;
    result
}

fn replace_in_provider_fields(
    normalized: &mut String,
    quoted_exact: &mut String,
    from: &str,
    to: &str,
) {
    if normalized.contains(from) {
        *normalized = normalized.replace(from, to);
    }
    if quoted_exact.contains(from) {
        *quoted_exact = quoted_exact.replace(from, to);
    }
}

/// Generate bounded subqueries from parsed error parts.
///
/// Returns at most `max_subqueries` subqueries.
pub fn generate_error_subqueries(
    parts: &ErrorQueryParts,
    max_subqueries: usize,
) -> Vec<ErrorSubquery> {
    let mut subqueries = Vec::new();

    // 1. Exact quoted error string
    if !parts.quoted_exact.is_empty() {
        let query = format!("\"{}\"", parts.quoted_exact);
        subqueries.push(ErrorSubquery {
            label: "exact_phrase".to_string(),
            query,
            target_group: "official_docs".to_string(),
        });
    }

    // 2. Error code + tool/language
    for code in &parts.error_codes {
        let mut terms = vec![code.code.clone()];
        if let Some(lang) = &parts.language_hint {
            terms.push(lang.clone());
        } else {
            terms.push(code.tool.clone());
        }
        let query = terms.join(" ");
        subqueries.push(ErrorSubquery {
            label: "error_code".to_string(),
            query,
            target_group: "official_docs".to_string(),
        });
    }

    // 3. Package/repo + error code
    for pkg in &parts.package_names {
        for code in &parts.error_codes {
            let query = format!("{} {}", pkg, code.code);
            subqueries.push(ErrorSubquery {
                label: "package_error".to_string(),
                query,
                target_group: "issues".to_string(),
            });
        }
    }

    // 4. Docs query
    if let Some(lang) = &parts.language_hint {
        for code in &parts.error_codes {
            let query = format!("{} {} docs documentation", code.code, lang);
            subqueries.push(ErrorSubquery {
                label: "docs".to_string(),
                query,
                target_group: "official_docs".to_string(),
            });
        }
    }

    // 5. Issues query
    if !parts.quoted_exact.is_empty() {
        let mut terms = vec![parts.quoted_exact.clone()];
        terms.push("issues".to_string());
        terms.push("github".to_string());
        let query = terms.join(" ");
        subqueries.push(ErrorSubquery {
            label: "issues".to_string(),
            query,
            target_group: "issues".to_string(),
        });
    }

    // 6. Releases/changelog query (if package hints exist)
    for pkg in &parts.package_names {
        for code in &parts.error_codes {
            let query = format!("{} {} release notes changelog", pkg, code.code);
            subqueries.push(ErrorSubquery {
                label: "releases".to_string(),
                query,
                target_group: "releases".to_string(),
            });
        }
    }

    subqueries.truncate(max_subqueries);
    subqueries
}

/// Validate an error query, returning an error if invalid.
pub fn validate_error_query(query: &str, max_chars: usize) -> Result<(), String> {
    if query.trim().is_empty() {
        return Err("error query must not be empty".to_string());
    }
    if query.chars().count() > max_chars {
        return Err(format!(
            "error query must be <= {max_chars} characters, got {}",
            query.chars().count()
        ));
    }
    Ok(())
}

// --- Internal parsing helpers ---

/// Collapse multiple whitespace characters into single spaces.
fn collapse_whitespace(s: &str) -> String {
    crate::core::sanitize::normalize_whitespace(s)
}

/// Extract known compiler/tool error codes.
fn extract_error_codes(text: &str) -> Vec<ErrorCode> {
    let mut codes = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Rust error codes: E0xxx
    for cap in RUST_ERROR_RE.captures_iter(text) {
        let code = cap[1].to_uppercase();
        if seen.insert(code.clone()) {
            codes.push(ErrorCode {
                code,
                tool: "rustc".to_string(),
            });
        }
    }

    // TypeScript error codes: TSxxxx
    for cap in TYPESCRIPT_ERROR_RE.captures_iter(text) {
        let code = cap[1].to_uppercase();
        if seen.insert(code.clone()) {
            codes.push(ErrorCode {
                code,
                tool: "typescript".to_string(),
            });
        }
    }

    // Python exceptions: look for common exception names
    let py_exceptions = [
        "TypeError",
        "ValueError",
        "KeyError",
        "IndexError",
        "AttributeError",
        "ImportError",
        "ModuleNotFoundError",
        "FileNotFoundError",
        "PermissionError",
        "RuntimeError",
        "RecursionError",
        "StopIteration",
        "AssertionError",
        "NotImplementedError",
        "OSError",
        "IOError",
        "SyntaxError",
        "IndentationError",
        "TabError",
        "OverflowError",
        "ZeroDivisionError",
        "MemoryError",
        "UnboundLocalError",
        "NameError",
        "UnicodeDecodeError",
        "UnicodeEncodeError",
        "ConnectionError",
        "TimeoutError",
        "JSONDecodeError",
    ];
    for exc in &py_exceptions {
        if text.contains(exc) {
            let code = exc.to_string();
            if seen.insert(code.clone()) {
                codes.push(ErrorCode {
                    code,
                    tool: "python".to_string(),
                });
            }
        }
    }

    // npm/yarn/pnpm errors
    let npm_errors = [
        "ERESOLVE",
        "ENOENT",
        "EACCES",
        "EEXIST",
        "ENOTDIR",
        "ENOTEMPTY",
    ];
    for err in &npm_errors {
        if text.contains(err) {
            let code = err.to_string();
            if seen.insert(code.clone()) {
                codes.push(ErrorCode {
                    code,
                    tool: "npm".to_string(),
                });
            }
        }
    }

    // Cargo errors - only match cargo-specific patterns, not "error[E" which is rustc
    let cargo_patterns = [
        "failed to select a version",
        "no matching package found",
        "could not find `",
    ];
    for err in &cargo_patterns {
        if text.contains(err) {
            let code = format!("cargo:{}", &err[..err.len().min(30)]);
            if seen.insert(code.clone()) {
                codes.push(ErrorCode {
                    code,
                    tool: "cargo".to_string(),
                });
            }
        }
    }

    // HTTP status errors: "404 Not Found", "500 Internal Server Error", etc.
    for cap in HTTP_STATUS_RE.captures_iter(text) {
        let code = format!("HTTP {}", &cap[1]);
        if seen.insert(code.clone()) {
            codes.push(ErrorCode {
                code,
                tool: "http".to_string(),
            });
        }
    }

    codes
}

/// Detect tools/compilers from the error text and codes.
fn detect_tools(text: &str, codes: &[ErrorCode]) -> Vec<String> {
    let mut tools = std::collections::BTreeSet::new();

    // From error codes
    for code in codes {
        tools.insert(code.tool.clone());
    }

    // From text patterns
    let text_lower = text.to_lowercase();
    if text_lower.contains("cargo") || text_lower.contains("crate") {
        tools.insert("cargo".to_string());
    }
    if text_lower.contains("rustc") || text_lower.contains("rust") {
        tools.insert("rustc".to_string());
    }
    if text_lower.contains("npm") || text_lower.contains("node") {
        tools.insert("npm".to_string());
    }
    if text_lower.contains("yarn") {
        tools.insert("yarn".to_string());
    }
    if text_lower.contains("pnpm") {
        tools.insert("pnpm".to_string());
    }
    if text_lower.contains("python") || text_lower.contains("pip") {
        tools.insert("python".to_string());
    }
    if text_lower.contains("gcc")
        || text_lower.contains("clang")
        || text_lower.contains("linker")
        || text_lower.contains("ld:")
    {
        tools.insert("linker".to_string());
    }
    if text_lower.contains("docker") {
        tools.insert("docker".to_string());
    }
    if text_lower.contains("typescript") || text_lower.contains("tsc") {
        tools.insert("typescript".to_string());
    }

    tools.into_iter().collect()
}

/// Infer language from error codes and tool names.
fn infer_language(codes: &[ErrorCode], tools: &[String]) -> Option<String> {
    // Check error codes first
    for code in codes {
        match code.tool.as_str() {
            "rustc" | "cargo" => return Some("rust".to_string()),
            "typescript" => return Some("typescript".to_string()),
            "python" => return Some("python".to_string()),
            "npm" | "yarn" | "pnpm" => return Some("javascript".to_string()),
            _ => {}
        }
    }

    // Check tool names
    for tool in tools {
        match tool.as_str() {
            "rustc" | "cargo" => return Some("rust".to_string()),
            "typescript" => return Some("typescript".to_string()),
            "python" => return Some("python".to_string()),
            "npm" | "yarn" | "pnpm" | "node" => return Some("javascript".to_string()),
            _ => {}
        }
    }

    None
}

/// Extract package names from the error text.
fn extract_package_names(text: &str) -> Vec<String> {
    let mut packages = std::collections::BTreeSet::new();

    // npm package patterns: "package-name@version" or "npm ERR! could not install package-name"
    for cap in NPM_PACKAGE_RE.captures_iter(text) {
        let pkg = cap[1].to_string();
        if !pkg.starts_with('@') && pkg.len() > 1 {
            packages.insert(pkg.split('@').next().unwrap_or(&pkg).to_string());
        }
    }

    // Cargo package patterns: "package `foo`" or "crate `foo`"
    for cap in CARGO_PACKAGE_RE.captures_iter(text) {
        packages.insert(cap[1].to_string());
    }

    // Cargo "no matching package found" pattern
    for cap in CARGO_NO_MATCH_RE.captures_iter(text) {
        packages.insert(cap[1].to_string());
    }

    // PyPI/pip patterns
    for cap in PIP_PACKAGE_RE.captures_iter(text) {
        packages.insert(cap[1].to_string());
    }

    packages.into_iter().collect()
}

/// Extract stack frame hints from tracebacks.
fn extract_stack_frames(text: &str) -> Vec<StackFrameHint> {
    let mut frames = Vec::new();

    // Python traceback: "  File \"path\", line N, in func"
    for cap in PYTHON_FRAME_RE.captures_iter(text) {
        frames.push(StackFrameHint {
            function: Some(cap[3].to_string()),
            file: Some(cap[1].to_string()),
            line: cap[2].parse().ok(),
        });
    }

    // Rust backtrace: "  at path:line" or " path::function"
    for line in text.lines() {
        if let Some(cap) = RUST_FRAME_RE.captures(line) {
            frames.push(StackFrameHint {
                function: None,
                file: Some(cap[1].to_string()),
                line: cap[2].parse().ok(),
            });
        }
    }

    frames
}

/// Extract useful path fragments (crate names, module paths).
fn extract_path_fragments(text: &str) -> Vec<String> {
    let mut fragments = std::collections::BTreeSet::new();

    // Rust module paths: "crate::module::function" or "foo::bar::baz"
    for cap in RUST_MODULE_PATH_RE.captures_iter(text) {
        let path = cap[1].to_string();
        // Only keep if it looks like a Rust path (at least one ::)
        if path.contains("::") && path.len() > 5 {
            fragments.insert(path);
        }
    }

    // File paths with extensions
    for cap in SOURCE_FILE_RE.captures_iter(text) {
        fragments.insert(cap[1].to_string());
    }

    fragments.into_iter().collect()
}

/// Extract the primary error line suitable for quoted search.
///
/// Strategy: find the line containing the error code or error message,
/// excluding lines that are just stack frames or context.
fn extract_primary_error_line(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    // Look for lines containing error codes
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Lines with error codes are good candidates
        if PRIMARY_ERROR_CODE_RE.is_match(trimmed) {
            return collapse_whitespace(trimmed);
        }
        // Lines with "error" or "Error" are good candidates
        if trimmed.to_lowercase().contains("error") && trimmed.len() > 10 && trimmed.len() < 200 {
            return collapse_whitespace(trimmed);
        }
    }

    // Fallback: use the first non-empty, non-stack-frame line
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("  at ") || trimmed.starts_with("  File ") {
            continue;
        }
        if trimmed.len() > 10 && trimmed.len() < 200 {
            return collapse_whitespace(trimmed);
        }
    }

    // Last resort: use the whole text (truncated)
    collapse_whitespace(text).chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rust_error_codes() {
        let parts = parse_error_query(
            "error[E0277]: the trait bound `Foo: Bar` is not satisfied\n  --> src/main.rs:10:5",
        );
        assert_eq!(parts.error_codes.len(), 1);
        assert_eq!(parts.error_codes[0].code, "E0277");
        assert_eq!(parts.error_codes[0].tool, "rustc");
        assert_eq!(parts.language_hint.as_deref(), Some("rust"));
    }

    #[test]
    fn parse_typescript_error_codes() {
        let parts = parse_error_query(
            "error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'.",
        );
        assert_eq!(parts.error_codes.len(), 1);
        assert_eq!(parts.error_codes[0].code, "TS2345");
        assert_eq!(parts.error_codes[0].tool, "typescript");
        assert_eq!(parts.language_hint.as_deref(), Some("typescript"));
    }

    #[test]
    fn parse_python_exception() {
        let parts = parse_error_query(
            "Traceback (most recent call last):\n  File \"app.py\", line 42, in main\n    result = data[key]\nKeyError: 'missing_key'",
        );
        assert!(parts.error_codes.iter().any(|c| c.code == "KeyError"));
        assert_eq!(parts.language_hint.as_deref(), Some("python"));
        assert!(!parts.stack_frames.is_empty());
    }

    #[test]
    fn parse_npm_eresolve() {
        let parts = parse_error_query(
            "npm ERR! ERESOLVE could not resolve dependency tree\nnpm ERR! Found: react@17.0.2",
        );
        assert!(parts.error_codes.iter().any(|c| c.code == "ERESOLVE"));
        assert_eq!(parts.language_hint.as_deref(), Some("javascript"));
    }

    #[test]
    fn parse_multiple_error_codes() {
        let parts = parse_error_query("error[E0277] and error[E0382]");
        assert_eq!(parts.error_codes.len(), 2);
        assert!(parts.error_codes.iter().any(|c| c.code == "E0277"));
        assert!(parts.error_codes.iter().any(|c| c.code == "E0382"));
    }

    #[test]
    fn collapse_whitespace_basic() {
        assert_eq!(collapse_whitespace("  hello   world  "), "hello world");
        assert_eq!(collapse_whitespace("a\n\nb\nc"), "a b c");
    }

    #[test]
    fn extract_primary_error_line_with_error_code() {
        let text = "   Compiling foo v0.1.0\nerror[E0277]: the trait bound is not satisfied\n  --> src/main.rs:10:5";
        let primary = extract_primary_error_line(text);
        assert!(primary.contains("E0277"));
    }

    #[test]
    fn detect_tools_from_text() {
        let parts = parse_error_query("cargo build failed: could not find package `foo`");
        assert!(parts.tool_names.contains(&"cargo".to_string()));
    }

    #[test]
    fn extract_package_names_cargo() {
        let parts = parse_error_query("package `tokio` not found");
        assert!(parts.package_names.contains(&"tokio".to_string()));
    }

    #[test]
    fn extract_package_names_npm() {
        let parts = parse_error_query("npm ERR! could not install express");
        assert!(parts.package_names.contains(&"express".to_string()));
    }

    #[test]
    fn redact_home_directory() {
        let parts = parse_error_query("error in /Users/john/project/src/main.rs");
        let redacted = redact_error_query(&parts);
        assert!(!redacted.normalized.contains("/Users/john"));
        assert!(!redacted.redactions_applied.is_empty());
    }

    #[test]
    fn redact_memory_addresses() {
        let parts = parse_error_query("segfault at 0x7fff5fbff8d0");
        let redacted = redact_error_query(&parts);
        assert!(!redacted.normalized.contains("0x7fff5fbff8d0"));
    }

    #[test]
    fn redact_provider_facing_exact_phrase() {
        let secret = "ABCDEF0123456789ABCDEF0123456789";
        let parts = parse_error_query(&format!(
            "error: token={secret} failed in /Users/john/project/src/main.rs at 0xDEADBEEF1234"
        ));
        let redacted = redact_error_query(&parts);

        assert!(!redacted.normalized.contains(secret));
        assert!(!redacted.quoted_exact.contains(secret));
        assert!(!redacted
            .normalized
            .contains("/Users/john/project/src/main.rs"));
        assert!(!redacted
            .quoted_exact
            .contains("/Users/john/project/src/main.rs"));
        assert!(!redacted.normalized.contains("0xDEADBEEF1234"));
        assert!(!redacted.quoted_exact.contains("0xDEADBEEF1234"));
        assert!(redacted.quoted_exact.contains("[REDACTED]"));
        assert!(redacted.quoted_exact.contains("main.rs"));
        assert!(redacted.quoted_exact.contains("[ADDR]"));
    }

    #[test]
    fn generated_exact_phrase_uses_redacted_text() {
        let secret = "abcdef0123456789abcdef0123456789";
        let parts = parse_error_query(&format!("error: api_key={secret} failed"));
        let redacted = redact_error_query(&parts);
        let subqueries = generate_error_subqueries(&redacted, 6);
        let exact = subqueries
            .iter()
            .find(|s| s.label == "exact_phrase")
            .expect("exact phrase subquery should exist");

        assert!(!exact.query.contains(secret));
        assert!(exact.query.contains("[REDACTED]"));
    }

    #[test]
    fn redact_uuids() {
        let parts = parse_error_query("request id: 550e8400-e29b-41d4-a716-446655440000");
        let redacted = redact_error_query(&parts);
        assert!(!redacted
            .normalized
            .contains("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn generate_subqueries_respects_cap() {
        let parts = parse_error_query(
            "error[E0277]: the trait bound `Foo: Bar` is not satisfied\npackage `tokio` not found",
        );
        let subqueries = generate_error_subqueries(&parts, 3);
        assert!(subqueries.len() <= 3);
    }

    #[test]
    fn generate_subqueries_preserves_quotes() {
        let parts = parse_error_query("error[E0277]: the trait bound is not satisfied");
        let subqueries = generate_error_subqueries(&parts, 6);
        // The exact_phrase subquery should have quotes
        let exact = subqueries.iter().find(|s| s.label == "exact_phrase");
        assert!(exact.is_some());
        assert!(exact.unwrap().query.starts_with('"'));
        assert!(exact.unwrap().query.ends_with('"'));
    }

    #[test]
    fn generate_subqueries_includes_error_code() {
        let parts = parse_error_query("error[E0277]: the trait bound is not satisfied");
        let subqueries = generate_error_subqueries(&parts, 6);
        let code = subqueries.iter().find(|s| s.label == "error_code");
        assert!(code.is_some());
        assert!(code.unwrap().query.contains("E0277"));
    }

    #[test]
    fn validate_error_query_rejects_empty() {
        assert!(validate_error_query("", 8000).is_err());
        assert!(validate_error_query("   ", 8000).is_err());
    }

    #[test]
    fn validate_error_query_rejects_oversized() {
        let big = "a".repeat(9000);
        assert!(validate_error_query(&big, 8000).is_err());
    }

    #[test]
    fn validate_error_query_accepts_valid() {
        assert!(validate_error_query("error[E0277]: foo", 8000).is_ok());
    }

    #[test]
    fn error_search_context_roundtrip() {
        let ctx = ErrorSearchContext {
            original_error: "test error".to_string(),
            normalized_error: "test error".to_string(),
            error_codes: vec![ErrorCode {
                code: "E0001".to_string(),
                tool: "rustc".to_string(),
            }],
            inferred_tools: vec!["rustc".to_string()],
            inferred_language: Some("rust".to_string()),
            redactions_applied: vec![],
            subqueries: vec![ErrorSubquery {
                label: "test".to_string(),
                query: "test query".to_string(),
                target_group: "official_docs".to_string(),
            }],
            warnings: vec![],
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: ErrorSearchContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.original_error, "test error");
        assert_eq!(parsed.error_codes.len(), 1);
        assert_eq!(parsed.subqueries.len(), 1);
    }

    #[test]
    fn error_config_default() {
        let cfg = ExactErrorConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_subqueries, 6);
        assert_eq!(cfg.max_error_chars, 8000);
        assert!(cfg.redact_sensitive_tokens);
    }

    #[test]
    fn error_config_roundtrip() {
        let cfg = ExactErrorConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: ExactErrorConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_subqueries, 6);
    }

    #[test]
    fn cargo_error_detection() {
        let parts = parse_error_query(
            "error: failed to select a version for `tokio`\n  required by package foo v0.1.0",
        );
        assert!(parts.tool_names.contains(&"cargo".to_string()));
    }

    #[test]
    fn linker_error_detection() {
        let parts = parse_error_query("ld: Undefined symbols:\n  referenced from main");
        assert!(parts.tool_names.contains(&"linker".to_string()));
    }

    #[test]
    fn http_status_error() {
        let parts = parse_error_query("HTTP 404 Not Found for /api/users");
        assert!(parts.error_codes.iter().any(|c| c.code == "HTTP 404"));
        assert_eq!(parts.tool_names, vec!["http".to_string()]);
    }

    #[test]
    fn stack_frames_from_python_traceback() {
        let parts = parse_error_query(
            "Traceback (most recent call last):\n  File \"app.py\", line 42, in main\n    x = foo()\n  File \"utils.py\", line 10, in foo\n    return bar()",
        );
        assert!(parts.stack_frames.len() >= 2);
        assert_eq!(parts.stack_frames[0].function.as_deref(), Some("main"));
        assert_eq!(parts.stack_frames[0].file.as_deref(), Some("app.py"));
    }

    #[test]
    fn path_fragments_from_rust_paths() {
        let parts = parse_error_query("error in tokio::runtime::Worker::spawn");
        assert!(parts
            .path_fragments
            .iter()
            .any(|p| p.contains("tokio::runtime")));
    }

    #[test]
    fn subquery_docs_group() {
        let parts = parse_error_query("error[E0277]: the trait bound is not satisfied");
        let subqueries = generate_error_subqueries(&parts, 6);
        let docs = subqueries.iter().find(|s| s.label == "docs");
        assert!(docs.is_some());
        assert!(docs.unwrap().target_group == "official_docs");
    }

    #[test]
    fn subquery_issues_group() {
        let parts = parse_error_query("error[E0277]: the trait bound is not satisfied");
        let subqueries = generate_error_subqueries(&parts, 6);
        let issues = subqueries.iter().find(|s| s.label == "issues");
        assert!(issues.is_some());
        assert!(issues.unwrap().target_group == "issues");
    }
}
