//! Deterministic content-type classifier for `web_fetch` responses.
//!
//! Uses Content-Type header, URL path extension, host/path patterns,
//! and lightweight byte heuristics to classify fetched content into
//! a `DocumentKind` with optional language detection.

use crate::core::document::DocumentKind;

/// Result of content classification.
#[derive(Clone, Debug)]
pub struct DetectedContent {
    /// The classified document kind.
    pub kind: DocumentKind,
    /// Detected programming language or content type (e.g. "rust", "python", "json").
    pub language: Option<String>,
    /// Whether this content should use line-preserving rendering.
    pub line_preserving: bool,
}

/// Classify content from all available signals.
///
/// Priority: Content-Type header > URL extension > byte heuristics.
pub fn classify(content_type: Option<&str>, url: &str, body: &[u8]) -> DetectedContent {
    // 1. Try Content-Type first
    if let Some(ct) = content_type {
        if let Some(kind) = detect_from_content_type(ct) {
            let language = language_from_content_type(ct);
            let line_preserving = matches!(
                kind,
                DocumentKind::Code
                    | DocumentKind::Json
                    | DocumentKind::Toml
                    | DocumentKind::Yaml
                    | DocumentKind::Diff
                    | DocumentKind::Patch
                    | DocumentKind::Markdown
            );
            return DetectedContent {
                kind,
                language,
                line_preserving,
            };
        }
    }

    // 2. Try URL extension
    if let Some(kind) = detect_from_url(url) {
        let language = language_from_url(url);
        let line_preserving = matches!(
            kind,
            DocumentKind::Code
                | DocumentKind::Json
                | DocumentKind::Toml
                | DocumentKind::Yaml
                | DocumentKind::Diff
                | DocumentKind::Patch
                | DocumentKind::Markdown
        );
        return DetectedContent {
            kind,
            language,
            line_preserving,
        };
    }

    // 3. Byte heuristics for text/plain or unknown
    let language = detect_language_from_bytes(body);
    let is_code = language.is_some();
    DetectedContent {
        kind: if is_code {
            DocumentKind::Code
        } else {
            DocumentKind::PlainText
        },
        language,
        line_preserving: is_code,
    }
}

/// Detect document kind from Content-Type header.
fn detect_from_content_type(ct: &str) -> Option<DocumentKind> {
    let ct_lower = ct.to_lowercase();
    let ct_base = ct_lower.split(';').next()?.trim();

    match ct_base {
        "text/markdown" | "text/x-markdown" => Some(DocumentKind::Markdown),
        "application/json" | "application/ld+json" => Some(DocumentKind::Json),
        _ if ct_base.ends_with("+json") => Some(DocumentKind::Json),
        "text/toml" | "application/toml" => Some(DocumentKind::Toml),
        "text/x-yaml" | "text/yaml" | "application/x-yaml" | "application/yaml" => {
            Some(DocumentKind::Yaml)
        }
        "text/x-diff" | "text/x-patch" => Some(DocumentKind::Diff),
        "text/x-rust"
        | "text/x-c"
        | "text/x-c++"
        | "text/x-java"
        | "text/x-python"
        | "text/x-shellscript"
        | "text/x-ruby"
        | "text/x-php"
        | "text/x-go"
        | "text/x-kotlin"
        | "text/x-scala"
        | "text/x-sql"
        | "text/x-swift"
        | "text/x-lua"
        | "text/x-typescript"
        | "text/javascript"
        | "text/x-csrc"
        | "application/javascript"
        | "application/x-javascript"
        | "application/typescript"
        | "application/x-sh" => Some(DocumentKind::Code),
        "text/plain" => None,                      // needs further heuristics
        _ if ct_base.starts_with("text/") => None, // unknown text type, needs heuristics
        _ => None,
    }
}

/// Extract language hint from Content-Type (e.g. "text/x-rust" -> "rust").
fn language_from_content_type(ct: &str) -> Option<String> {
    let ct_base = ct.split(';').next()?.trim().to_lowercase();
    match ct_base.as_str() {
        "text/x-rust" => Some("rust".to_string()),
        "text/x-c" | "text/x-csrc" => Some("c".to_string()),
        "text/x-c++" => Some("c++".to_string()),
        "text/x-java" => Some("java".to_string()),
        "text/x-python" => Some("python".to_string()),
        "text/x-shellscript" | "application/x-sh" => Some("bash".to_string()),
        "text/x-ruby" => Some("ruby".to_string()),
        "text/x-php" => Some("php".to_string()),
        "text/x-go" => Some("go".to_string()),
        "text/x-kotlin" => Some("kotlin".to_string()),
        "text/x-scala" => Some("scala".to_string()),
        "text/x-sql" => Some("sql".to_string()),
        "text/x-swift" => Some("swift".to_string()),
        "text/x-lua" => Some("lua".to_string()),
        "text/x-typescript" | "application/typescript" => Some("typescript".to_string()),
        "text/javascript" | "application/javascript" | "application/x-javascript" => {
            Some("javascript".to_string())
        }
        "text/x-yaml" | "text/yaml" | "application/x-yaml" | "application/yaml" => {
            Some("yaml".to_string())
        }
        "text/toml" | "application/toml" => Some("toml".to_string()),
        "application/json" | "application/ld+json" => Some("json".to_string()),
        _ if ct_base.ends_with("+json") => Some("json".to_string()),
        _ if ct_base.ends_with("+yaml") => Some("yaml".to_string()),
        _ if ct_base.ends_with("+toml") => Some("toml".to_string()),
        _ => None,
    }
}

/// Detect document kind from URL path extension.
fn detect_from_url(url: &str) -> Option<DocumentKind> {
    let path = url::Url::parse(url).ok()?.path().to_string();
    let ext = path.rsplit('.').next()?.to_lowercase();

    match ext.as_str() {
        // Code extensions
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "c" | "h" | "cpp" | "cc" | "cxx"
        | "hpp" | "java" | "kt" | "kts" | "scala" | "sh" | "bash" | "zsh" | "fish" | "sql"
        | "html" | "htm" | "css" | "scss" | "less" | "lua" | "rb" | "php" | "swift" | "m"
        | "mm" | "r" | "dart" | "ex" | "exs" | "erl" | "hs" | "ml" | "fs" | "clj" | "vim"
        | "el" | "lisp" => Some(DocumentKind::Code),

        // Config/data extensions
        "json" | "jsonl" | "geojson" | "ndjson" => Some(DocumentKind::Json),
        "toml" => Some(DocumentKind::Toml),
        "yaml" | "yml" => Some(DocumentKind::Yaml),

        // Diff/patch extensions
        "diff" | "patch" => Some(DocumentKind::Diff),

        // Markdown
        "md" | "mdx" | "markdown" | "mkd" => Some(DocumentKind::Markdown),

        // Log/plain text
        "log" | "txt" | "text" | "cfg" | "conf" | "ini" | "env" | "xml" => None,

        // Binary (not text)
        "pdf" => Some(DocumentKind::Pdf),

        _ => None,
    }
}

/// Extract language hint from URL extension.
fn language_from_url(url: &str) -> Option<String> {
    let path = url::Url::parse(url).ok()?.path().to_string();
    let ext = path.rsplit('.').next()?.to_lowercase();

    match ext.as_str() {
        "rs" => Some("rust".to_string()),
        "py" => Some("python".to_string()),
        "js" | "jsx" => Some("javascript".to_string()),
        "ts" | "tsx" => Some("typescript".to_string()),
        "go" => Some("go".to_string()),
        "c" | "h" => Some("c".to_string()),
        "cpp" | "cc" | "cxx" | "hpp" => Some("c++".to_string()),
        "java" => Some("java".to_string()),
        "kt" | "kts" => Some("kotlin".to_string()),
        "scala" => Some("scala".to_string()),
        "sh" | "bash" | "zsh" | "fish" => Some("bash".to_string()),
        "sql" => Some("sql".to_string()),
        "html" | "htm" => Some("html".to_string()),
        "css" | "scss" | "less" => Some("css".to_string()),
        "lua" => Some("lua".to_string()),
        "rb" => Some("ruby".to_string()),
        "php" => Some("php".to_string()),
        "swift" => Some("swift".to_string()),
        "r" => Some("r".to_string()),
        "dart" => Some("dart".to_string()),
        "ex" | "exs" => Some("elixir".to_string()),
        "erl" => Some("erlang".to_string()),
        "hs" => Some("haskell".to_string()),
        "ml" => Some("ocaml".to_string()),
        "json" | "jsonl" | "geojson" | "ndjson" => Some("json".to_string()),
        "toml" => Some("toml".to_string()),
        "yaml" | "yml" => Some("yaml".to_string()),
        "md" | "mdx" | "markdown" | "mkd" => Some("markdown".to_string()),
        "xml" => Some("xml".to_string()),
        "diff" | "patch" => Some("diff".to_string()),
        _ => None,
    }
}

/// Heuristic language detection from byte content.
/// Returns Some(language) if content looks like code.
fn detect_language_from_bytes(bytes: &[u8]) -> Option<String> {
    // Take first 8192 bytes for heuristic analysis
    let sample_len = bytes.len().min(8192);
    let sample = &bytes[..sample_len];

    let text = String::from_utf8_lossy(sample);
    let lines: Vec<&str> = text.lines().take(200).collect();

    if lines.is_empty() {
        return None;
    }

    // Count heuristic signals
    let mut shebang_lang = None;
    let mut import_count = 0;
    let mut fn_def_count = 0;
    let mut struct_count = 0;
    let mut brace_depth = 0;
    let mut _has_long_lines = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Shebang detection (first line only)
        if i == 0 && trimmed.starts_with("#!") {
            if trimmed.contains("python") {
                shebang_lang = Some("python");
            } else if trimmed.contains("node") || trimmed.contains("bash") || trimmed.contains("sh")
            {
                shebang_lang = Some("bash");
            } else if trimmed.contains("ruby") {
                shebang_lang = Some("ruby");
            } else if trimmed.contains("perl") {
                shebang_lang = Some("perl");
            } else if trimmed.contains("env") {
                // #!/usr/bin/env <lang>
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 3 {
                    shebang_lang = Some(parts[2]);
                }
            }
        }

        // Import/require patterns
        if trimmed.starts_with("use ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("from ") && trimmed.contains("import")
            || trimmed.starts_with("require(")
            || trimmed.starts_with("include!")
        {
            import_count += 1;
        }

        // Function definition patterns
        if trimmed.starts_with("fn ")
            || trimmed.starts_with("func ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("async fn ")
        {
            fn_def_count += 1;
        }

        // Struct/class/type patterns
        if trimmed.starts_with("struct ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("interface ")
        {
            struct_count += 1;
        }

        // Track brace depth (rough C-family heuristic)
        for c in trimmed.chars() {
            match c {
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                _ => {}
            }
        }

        // Long non-blank lines suggest code
        if trimmed.len() > 120 && !trimmed.starts_with('#') {
            _has_long_lines = true;
        }
    }

    // If shebang detected something, trust it
    if let Some(lang) = shebang_lang {
        return Some(lang.to_string());
    }

    // Score-based detection
    let total_signals = import_count + fn_def_count + struct_count;

    if total_signals >= 2 || (fn_def_count >= 2 && brace_depth > 0) {
        // Likely code - try to determine language
        // Check for Rust-specific patterns
        let text_full = String::from_utf8_lossy(sample);
        if text_full.contains("fn main()")
            || text_full.contains("fn ")
                && text_full.contains("-> ")
                && text_full.contains("Option<")
        {
            return Some("rust".to_string());
        }
        if text_full.contains("def ")
            && text_full.contains(":")
            && (text_full.contains("self") || text_full.contains("__init__"))
        {
            return Some("python".to_string());
        }
        if text_full.contains("func ") && text_full.contains(":=") {
            return Some("go".to_string());
        }
        if text_full.contains("class ") && text_full.contains("extends") {
            return Some("typescript".to_string());
        }
        if text_full.contains("public class ") || text_full.contains("private ") {
            return Some("java".to_string());
        }
        // Default to "code" if we can't determine specific language
        return Some("code".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_json_content_type() {
        let det = classify(
            Some("application/json; charset=utf-8"),
            "https://example.com/data",
            b"{}",
        );
        assert_eq!(det.kind, DocumentKind::Json);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_markdown_content_type() {
        let det = classify(
            Some("text/markdown"),
            "https://example.com/readme",
            b"# Hello",
        );
        assert_eq!(det.kind, DocumentKind::Markdown);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_json_url_extension() {
        let det = classify(None, "https://example.com/config.json", b"{}");
        assert_eq!(det.kind, DocumentKind::Json);
    }

    #[test]
    fn classify_rust_url_extension() {
        let det = classify(None, "https://example.com/main.rs", b"fn main() {}");
        assert_eq!(det.kind, DocumentKind::Code);
        assert_eq!(det.language, Some("rust".to_string()));
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_diff_content_type() {
        let det = classify(
            Some("text/x-diff"),
            "https://example.com/changes",
            b"--- a/foo\n+++ b/foo\n@@ -1 +1 @@\n-old\n+new",
        );
        assert_eq!(det.kind, DocumentKind::Diff);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_unknown_with_code_heuristics() {
        let code =
            b"use std::collections::HashMap;\n\nfn main() {\n    let map = HashMap::new();\n}\n";
        let det = classify(None, "https://example.com/script", code);
        assert_eq!(det.kind, DocumentKind::Code);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_plain_text_fallback() {
        let det = classify(
            None,
            "https://example.com/notes",
            b"Just some plain text notes here.",
        );
        assert_eq!(det.kind, DocumentKind::PlainText);
        assert!(!det.line_preserving);
    }

    #[test]
    fn detect_from_content_type_toml() {
        assert_eq!(
            detect_from_content_type("text/toml"),
            Some(DocumentKind::Toml)
        );
    }

    #[test]
    fn detect_from_content_type_yaml() {
        assert_eq!(
            detect_from_content_type("text/yaml"),
            Some(DocumentKind::Yaml)
        );
        assert_eq!(
            detect_from_content_type("application/x-yaml"),
            Some(DocumentKind::Yaml)
        );
    }

    #[test]
    fn detect_from_url_markdown() {
        assert_eq!(
            detect_from_url("https://example.com/README.md"),
            Some(DocumentKind::Markdown)
        );
    }

    #[test]
    fn detect_from_url_diff() {
        assert_eq!(
            detect_from_url("https://example.com/changes.diff"),
            Some(DocumentKind::Diff)
        );
    }

    #[test]
    fn language_from_url_rust() {
        assert_eq!(
            language_from_url("https://example.com/main.rs"),
            Some("rust".to_string())
        );
    }

    #[test]
    fn language_from_url_python() {
        assert_eq!(
            language_from_url("https://example.com/app.py"),
            Some("python".to_string())
        );
    }

    #[test]
    fn classify_application_javascript() {
        let det = classify(
            Some("application/javascript"),
            "https://example.com/script.js",
            b"function foo() { return 1; }",
        );
        assert_eq!(det.kind, DocumentKind::Code);
        assert_eq!(det.language, Some("javascript".to_string()));
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_javascript_with_charset() {
        let det = classify(
            Some("application/javascript; charset=utf-8"),
            "https://example.com/script.js",
            b"const x = 1;",
        );
        assert_eq!(det.kind, DocumentKind::Code);
        assert_eq!(det.language, Some("javascript".to_string()));
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_x_javascript() {
        let det = classify(
            Some("application/x-javascript"),
            "https://example.com/legacy.js",
            b"var x = 1;",
        );
        assert_eq!(det.kind, DocumentKind::Code);
        assert_eq!(det.language, Some("javascript".to_string()));
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_typescript() {
        let det = classify(
            Some("application/typescript"),
            "https://example.com/app.ts",
            b"const x: number = 1;",
        );
        assert_eq!(det.kind, DocumentKind::Code);
        assert_eq!(det.language, Some("typescript".to_string()));
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_x_sh() {
        let det = classify(
            Some("application/x-sh"),
            "https://example.com/run.sh",
            b"#!/bin/bash\necho hello",
        );
        assert_eq!(det.kind, DocumentKind::Code);
        assert_eq!(det.language, Some("bash".to_string()));
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_json() {
        let det = classify(
            Some("application/json"),
            "https://example.com/data",
            b"{\"key\": \"value\"}",
        );
        assert_eq!(det.kind, DocumentKind::Json);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_ld_json() {
        let det = classify(
            Some("application/ld+json"),
            "https://example.com/schema",
            b"{\"@type\": \"Thing\"}",
        );
        assert_eq!(det.kind, DocumentKind::Json);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_wildcard_json() {
        let det = classify(
            Some("application/vnd.api+json"),
            "https://example.com/api",
            b"{\"data\": []}",
        );
        assert_eq!(det.kind, DocumentKind::Json);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_toml() {
        let det = classify(
            Some("application/toml"),
            "https://example.com/config.toml",
            b"[package]\nname = \"foo\"",
        );
        assert_eq!(det.kind, DocumentKind::Toml);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_yaml() {
        let det = classify(
            Some("application/yaml"),
            "https://example.com/config.yaml",
            b"name: foo",
        );
        assert_eq!(det.kind, DocumentKind::Yaml);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_application_x_yaml() {
        let det = classify(
            Some("application/x-yaml"),
            "https://example.com/config.yaml",
            b"name: foo",
        );
        assert_eq!(det.kind, DocumentKind::Yaml);
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_text_javascript() {
        let det = classify(
            Some("text/javascript"),
            "https://example.com/script.js",
            b"function foo() {}",
        );
        assert_eq!(det.kind, DocumentKind::Code);
        assert_eq!(det.language, Some("javascript".to_string()));
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_text_x_typescript() {
        let det = classify(
            Some("text/x-typescript"),
            "https://example.com/app.ts",
            b"const x: number = 1;",
        );
        assert_eq!(det.kind, DocumentKind::Code);
        assert_eq!(det.language, Some("typescript".to_string()));
        assert!(det.line_preserving);
    }

    #[test]
    fn classify_unknown_text_type_falls_through() {
        let det = classify(
            Some("text/x-custom-thing"),
            "https://example.com/thing",
            b"just some text",
        );
        assert_eq!(det.kind, DocumentKind::PlainText);
        assert!(!det.line_preserving);
    }

    #[test]
    fn classify_text_plain_falls_through_to_heuristics() {
        let code = b"use std::collections::HashMap;\nfn main() { HashMap::new(); }\n";
        let det = classify(Some("text/plain"), "https://example.com/script", code);
        assert_eq!(det.kind, DocumentKind::Code);
        assert!(det.line_preserving);
    }
}
