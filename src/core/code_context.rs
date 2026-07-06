//! Lightweight line-oriented code context extraction.
//!
//! Extracts imports and enclosing symbol information from source code
//! without requiring a full AST parser. All extraction is bounded by
//! max lines and max chars and operates on the text content of a file.

use serde::{Deserialize, Serialize};

/// Result of lightweight code context extraction.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeContext {
    /// Programming language inferred from extension or content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Top-level imports/use declarations extracted from the file prefix.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    /// The enclosing symbol (function, struct, class, etc.) around the target line range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_symbol: Option<String>,
    /// Kind of the enclosing symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_symbol_kind: Option<String>,
    /// Start line of the enclosing symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_line_start: Option<u32>,
    /// End line of the enclosing symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_line_end: Option<u32>,
}

/// Language identifier for extraction heuristics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractionLanguage {
    /// Rust source code.
    Rust,
    /// Python source code.
    Python,
    /// TypeScript source code.
    TypeScript,
    /// JavaScript source code.
    JavaScript,
    /// Go source code.
    Go,
    /// Unknown or unsupported language.
    Unknown,
}

/// Detect language from file extension.
pub fn detect_language(path: &str) -> ExtractionLanguage {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => ExtractionLanguage::Rust,
        "py" => ExtractionLanguage::Python,
        "ts" | "tsx" => ExtractionLanguage::TypeScript,
        "js" | "jsx" => ExtractionLanguage::JavaScript,
        "go" => ExtractionLanguage::Go,
        _ => ExtractionLanguage::Unknown,
    }
}

/// Detect language from file extension and return a display string.
pub fn detect_language_str(path: &str) -> Option<String> {
    let lang = detect_language(path);
    match lang {
        ExtractionLanguage::Rust => Some("rust".to_string()),
        ExtractionLanguage::Python => Some("python".to_string()),
        ExtractionLanguage::TypeScript => Some("typescript".to_string()),
        ExtractionLanguage::JavaScript => Some("javascript".to_string()),
        ExtractionLanguage::Go => Some("go".to_string()),
        ExtractionLanguage::Unknown => None,
    }
}

/// Maximum number of lines to scan for imports.
const MAX_IMPORT_SCAN_LINES: usize = 50;
/// Maximum chars per import line.
const MAX_IMPORT_LINE_CHARS: usize = 200;
/// Maximum number of imports to extract.
const MAX_IMPORTS: usize = 30;
/// Maximum lines to scan backward/forward for enclosing symbol.
const MAX_ENCLOSING_SCAN_LINES: usize = 200;

/// Extract top-level imports from the beginning of a source file.
///
/// Scans up to `MAX_IMPORT_SCAN_LINES` lines from the top of the file
/// and collects import/use declarations. Each import is bounded to
/// `MAX_IMPORT_LINE_CHARS` and the total count is bounded to `MAX_IMPORTS`.
pub fn extract_imports(text: &str, language: ExtractionLanguage) -> Vec<String> {
    let mut imports = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i >= MAX_IMPORT_SCAN_LINES {
            break;
        }
        if imports.len() >= MAX_IMPORTS {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
        {
            continue;
        }

        let is_import = match language {
            ExtractionLanguage::Rust => {
                trimmed.starts_with("use ") || trimmed.starts_with("pub use ")
            }
            ExtractionLanguage::Python => {
                trimmed.starts_with("import ")
                    || (trimmed.starts_with("from ") && trimmed.contains(" import "))
            }
            ExtractionLanguage::TypeScript | ExtractionLanguage::JavaScript => {
                trimmed.starts_with("import ")
                    || trimmed.starts_with("from ")
                    || trimmed.starts_with("require(")
            }
            ExtractionLanguage::Go => {
                trimmed.starts_with("import ")
                    || (trimmed.starts_with('"') && trimmed.ends_with('"'))
            }
            ExtractionLanguage::Unknown => false,
        };

        if is_import {
            let import_text = if trimmed.len() > MAX_IMPORT_LINE_CHARS {
                &trimmed[..MAX_IMPORT_LINE_CHARS]
            } else {
                trimmed
            };
            imports.push(import_text.to_string());
        }
    }
    imports
}

/// Find the enclosing symbol around a target line number.
///
/// Scans backward from `target_line` to find a symbol definition,
/// then scans forward to find the end of that symbol. Returns
/// the symbol name, kind, and line range.
pub fn find_enclosing_symbol(
    text: &str,
    target_line: u32,
    language: ExtractionLanguage,
) -> Option<(String, String, u32, u32)> {
    let lines: Vec<&str> = text.lines().collect();
    let target_idx = (target_line as usize).saturating_sub(1);
    if target_idx >= lines.len() {
        return None;
    }

    // Kinds that represent standalone definitions (not enclosing blocks).
    // These should only match if the target line IS the definition line.
    const LEAF_KINDS: &[&str] = &["constant", "type_alias"];

    // Scan backward to find a definition line whose range contains the target
    let scan_start = target_idx.saturating_sub(MAX_ENCLOSING_SCAN_LINES);

    for i in (scan_start..=target_idx).rev() {
        let line = lines[i].trim();
        if let Some((name, kind)) = extract_symbol_definition(line, language) {
            let end = find_symbol_end(&lines, i, language);
            let is_leaf = LEAF_KINDS.contains(&kind.as_str());
            if is_leaf {
                // Leaf definitions only match if the target is on the exact definition line
                if target_idx == i {
                    return Some((name, kind, (i as u32) + 1, (end as u32) + 1));
                }
            } else if target_idx <= end {
                return Some((name, kind, (i as u32) + 1, (end as u32) + 1));
            }
        }
    }

    None
}

/// Extract a symbol definition from a single line.
///
/// Returns (symbol_name, symbol_kind) if the line contains a definition.
fn extract_symbol_definition(line: &str, language: ExtractionLanguage) -> Option<(String, String)> {
    match language {
        ExtractionLanguage::Rust => {
            // fn name, pub fn name, pub(crate) fn name, async fn name
            if let Some(name) = extract_rust_fn(line) {
                return Some((name, "function".to_string()));
            }
            // struct Name
            if let Some(name) = extract_word_after(line, "struct ") {
                return Some((name, "struct".to_string()));
            }
            // enum Name
            if let Some(name) = extract_word_after(line, "enum ") {
                return Some((name, "enum".to_string()));
            }
            // trait Name
            if let Some(name) = extract_word_after(line, "trait ") {
                return Some((name, "trait".to_string()));
            }
            // impl blocks: impl Type { or impl Trait for Type {
            if line.contains("impl ") && line.contains('{') {
                let after_impl = line[line.find("impl ")? + 5..].trim();
                let name = after_impl.split('{').next()?.trim().to_string();
                if !name.is_empty() {
                    return Some((name, "impl".to_string()));
                }
            }
            // mod name
            if let Some(name) = extract_word_after(line, "mod ") {
                return Some((name, "module".to_string()));
            }
            // const NAME or static NAME
            if line.starts_with("pub const ")
                || line.starts_with("const ")
                || line.starts_with("pub static ")
                || line.starts_with("static ")
            {
                let stripped = line.strip_prefix("pub ").unwrap_or(line);
                let after_kind = if let Some(rest) = stripped.strip_prefix("const ") {
                    rest
                } else {
                    stripped.strip_prefix("static ")?
                };
                let name = after_kind.split(':').next()?.trim().to_string();
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Some((name, "constant".to_string()));
                }
            }
            // type Name
            if let Some(name) = extract_word_after(line, "type ") {
                return Some((name, "type_alias".to_string()));
            }
            // macro_rules! name
            if let Some(pos) = line.find("macro_rules!") {
                let after = line[pos + 12..].trim();
                let name = after.split('(').next()?.trim().to_string();
                if !name.is_empty() {
                    return Some((name, "macro".to_string()));
                }
            }
            None
        }
        ExtractionLanguage::Python => {
            // def name( or async def name(
            if line.contains("def ") {
                let trimmed = line.trim_start();
                let (is_async, after_def) = if let Some(rest) = trimmed.strip_prefix("async ") {
                    (true, rest.trim_start())
                } else {
                    (false, trimmed)
                };
                if let Some(rest) = after_def.strip_prefix("def ") {
                    let name = rest.split('(').next()?.trim().to_string();
                    if !name.is_empty() {
                        let kind = if is_async {
                            "async_function"
                        } else {
                            "function"
                        };
                        return Some((name, kind.to_string()));
                    }
                }
            }
            // class Name
            if let Some(name) = extract_word_after(line.trim_start(), "class ") {
                return Some((name, "class".to_string()));
            }
            None
        }
        ExtractionLanguage::TypeScript | ExtractionLanguage::JavaScript => {
            // function name(, export function name(
            if line.contains("function ") {
                let trimmed = line.trim_start();
                let after_export = trimmed
                    .strip_prefix("export ")
                    .unwrap_or(trimmed)
                    .trim_start();
                let after_async = after_export
                    .strip_prefix("async ")
                    .unwrap_or(after_export)
                    .trim_start();
                if let Some(rest) = after_async.strip_prefix("function ") {
                    let name = rest
                        .split('(')
                        .next()?
                        .split('<')
                        .next()?
                        .trim()
                        .to_string();
                    if !name.is_empty() && !name.starts_with('(') {
                        return Some((name, "function".to_string()));
                    }
                }
            }
            // class Name, export class Name
            if line.contains("class ") {
                let trimmed = line.trim_start();
                let after_export = trimmed
                    .strip_prefix("export ")
                    .unwrap_or(trimmed)
                    .trim_start();
                if let Some(rest) = after_export.strip_prefix("class ") {
                    let name = rest
                        .split('{')
                        .next()?
                        .split('<')
                        .next()?
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        return Some((name, "class".to_string()));
                    }
                }
            }
            // const Name = (...) => or let Name = (...) =>
            let trimmed = line.trim_start();
            for prefix in &["const ", "let ", "var "] {
                if let Some(rest) = trimmed.strip_prefix(*prefix) {
                    if let Some(eq_pos) = rest.find('=') {
                        let name = rest[..eq_pos].trim();
                        let after_eq = rest[eq_pos + 1..].trim();
                        // Match arrow functions: (...) => or async (...) =>
                        let is_arrow = after_eq.starts_with("async ")
                            || after_eq.starts_with('(')
                            || after_eq.starts_with('{');
                        if !name.is_empty()
                            && name
                                .chars()
                                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                            && is_arrow
                        {
                            return Some((name.to_string(), "function".to_string()));
                        }
                    }
                }
            }
            // interface Name
            if let Some(name) = extract_word_after(trimmed, "interface ") {
                return Some((name, "interface".to_string()));
            }
            // type Name =
            if let Some(name) = extract_word_after(trimmed, "type ") {
                return Some((name, "type_alias".to_string()));
            }
            None
        }
        ExtractionLanguage::Go => {
            // func Name( or func (receiver) Name(
            if line.contains("func ") {
                let trimmed = line.trim();
                let after_func = trimmed.strip_prefix("func ")?;
                if after_func.starts_with('(') {
                    // Method: func (r Type) Name(
                    if let Some(close) = after_func.find(')') {
                        let after_receiver = after_func[close + 1..].trim();
                        let name = after_receiver.split('(').next()?.trim().to_string();
                        if !name.is_empty() {
                            return Some((name, "method".to_string()));
                        }
                    }
                } else {
                    let name = after_func.split('(').next()?.trim().to_string();
                    if !name.is_empty() {
                        return Some((name, "function".to_string()));
                    }
                }
            }
            // type Name struct or type Name interface
            if line.contains("type ") && (line.contains("struct") || line.contains("interface")) {
                let trimmed = line.trim();
                let after_type = trimmed.strip_prefix("type ")?;
                let name = if line.contains("struct") {
                    after_type.split("struct").next()?.trim()
                } else {
                    after_type.split("interface").next()?.trim()
                };
                let name = name.to_string();
                if !name.is_empty() {
                    let kind = if line.contains("struct") {
                        "struct"
                    } else {
                        "interface"
                    };
                    return Some((name, kind.to_string()));
                }
            }
            // var Name or const Name
            let trimmed = line.trim();
            for prefix in &["var ", "const "] {
                if let Some(rest) = trimmed.strip_prefix(*prefix) {
                    let name = rest
                        .split('=')
                        .next()?
                        .split_whitespace()
                        .next()?
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        return Some((name, "constant".to_string()));
                    }
                }
            }
            None
        }
        ExtractionLanguage::Unknown => None,
    }
}

/// Find the end line of a symbol starting at `start_idx`.
fn find_symbol_end(lines: &[&str], start_idx: usize, language: ExtractionLanguage) -> usize {
    match language {
        ExtractionLanguage::Rust
        | ExtractionLanguage::TypeScript
        | ExtractionLanguage::JavaScript
        | ExtractionLanguage::Go => {
            // Count braces
            let mut depth = 0i32;
            let mut found_open = false;
            let end = (start_idx + MAX_ENCLOSING_SCAN_LINES).min(lines.len());
            for (i, line) in lines.iter().enumerate().take(end).skip(start_idx) {
                for ch in line.chars() {
                    match ch {
                        '{' => {
                            depth += 1;
                            found_open = true;
                        }
                        '}' => {
                            depth -= 1;
                        }
                        _ => {}
                    }
                }
                if found_open && depth == 0 {
                    return i;
                }
            }
            lines
                .len()
                .saturating_sub(1)
                .min(start_idx + MAX_ENCLOSING_SCAN_LINES)
        }
        ExtractionLanguage::Python => {
            // Indentation-based: find where indentation returns to or below the def level.
            // Blank lines between definitions are skipped — they don't close a block.
            // The end line is the last non-blank line before the indent drops.
            if start_idx >= lines.len() {
                return start_idx;
            }
            let base_indent = lines[start_idx].len() - lines[start_idx].trim_start().len();
            let end = (start_idx + MAX_ENCLOSING_SCAN_LINES).min(lines.len());
            let mut found_body = false;
            let mut last_body_idx = start_idx;
            for (i, line) in lines.iter().enumerate().take(end).skip(start_idx + 1) {
                if line.trim().is_empty() {
                    continue;
                }
                let indent = line.len() - line.trim_start().len();
                if indent > base_indent {
                    found_body = true;
                    last_body_idx = i;
                } else if found_body {
                    return last_body_idx;
                }
            }
            if found_body {
                last_body_idx
            } else {
                lines
                    .len()
                    .saturating_sub(1)
                    .min(start_idx + MAX_ENCLOSING_SCAN_LINES)
            }
        }
        ExtractionLanguage::Unknown => start_idx,
    }
}

/// Helper: extract a word after a prefix string.
fn extract_word_after(line: &str, prefix: &str) -> Option<String> {
    if let Some(pos) = line.find(prefix) {
        let rest = &line[pos + prefix.len()..];
        let word: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
            .collect();
        if !word.is_empty() {
            // For Rust paths like `module::Type`, take just the last segment
            let name = word.rsplit("::").next().unwrap_or(&word);
            // Strip trailing colon (Python class/def syntax)
            let name = name.strip_suffix(':').unwrap_or(name);
            Some(name.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

/// Helper: extract a Rust fn name from various forms.
fn extract_rust_fn(line: &str) -> Option<String> {
    let trimmed = line.trim();
    // Skip if it's a macro or type definition
    if trimmed.contains("macro_rules!")
        || trimmed.contains("struct ")
        || trimmed.contains("enum ")
        || trimmed.contains("trait ")
        || trimmed.contains("impl ")
        || trimmed.contains("type ")
    {
        return None;
    }
    // Find "fn " keyword
    let fn_pos = trimmed.find(" fn ")?;
    let after_fn = &trimmed[fn_pos + 4..];
    let name = after_fn
        .split('(')
        .next()?
        .split('<')
        .next()?
        .trim()
        .to_string();
    // Skip common false positives
    if name.is_empty()
        || name.starts_with("impl")
        || name.starts_with("dyn")
        || name == "where"
        || name == "for"
        || name == "if"
        || name == "match"
        || name == "while"
        || name == "loop"
    {
        return None;
    }
    Some(name)
}

/// Extract code context from text content.
///
/// This is the main entry point. Given the full text of a file and an
/// optional target line range, returns a `CodeContext` with imports
/// and enclosing symbol information.
pub fn extract_code_context(text: &str, path: &str, target_line: Option<u32>) -> CodeContext {
    let language = detect_language(path);
    let language_str = detect_language_str(path);
    let imports = extract_imports(text, language);

    let (enclosing_symbol, enclosing_symbol_kind, enclosing_line_start, enclosing_line_end) =
        if let Some(line) = target_line {
            if let Some((name, kind, start, end)) = find_enclosing_symbol(text, line, language) {
                (Some(name), Some(kind), Some(start), Some(end))
            } else {
                (None, None, None, None)
            }
        } else {
            (None, None, None, None)
        };

    CodeContext {
        language: language_str,
        imports,
        enclosing_symbol,
        enclosing_symbol_kind,
        enclosing_line_start,
        enclosing_line_end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Language detection tests ---

    #[test]
    fn detect_language_rust() {
        assert_eq!(detect_language("src/main.rs"), ExtractionLanguage::Rust);
    }

    #[test]
    fn detect_language_python() {
        assert_eq!(detect_language("lib/module.py"), ExtractionLanguage::Python);
    }

    #[test]
    fn detect_language_typescript() {
        assert_eq!(
            detect_language("src/index.ts"),
            ExtractionLanguage::TypeScript
        );
    }

    #[test]
    fn detect_language_javascript() {
        assert_eq!(
            detect_language("src/app.js"),
            ExtractionLanguage::JavaScript
        );
    }

    #[test]
    fn detect_language_go() {
        assert_eq!(detect_language("cmd/server.go"), ExtractionLanguage::Go);
    }

    #[test]
    fn detect_language_unknown() {
        assert_eq!(detect_language("data.xyz"), ExtractionLanguage::Unknown);
    }

    // --- Import extraction tests ---

    #[test]
    fn extract_imports_rust() {
        let text = r#"use axum::Router;
use tokio::sync::mpsc;
use crate::error::Error;

pub fn handler() {}
"#;
        let imports = extract_imports(text, ExtractionLanguage::Rust);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0], "use axum::Router;");
        assert_eq!(imports[1], "use tokio::sync::mpsc;");
        assert_eq!(imports[2], "use crate::error::Error;");
    }

    #[test]
    fn extract_imports_python() {
        let text = r#"import os
from pathlib import Path
from typing import Optional

def main():
    pass
"#;
        let imports = extract_imports(text, ExtractionLanguage::Python);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0], "import os");
        assert_eq!(imports[1], "from pathlib import Path");
        assert_eq!(imports[2], "from typing import Optional");
    }

    #[test]
    fn extract_imports_javascript() {
        let text = r#"import React from 'react';
import { useState } from 'react';
const fs = require('fs');

export default function App() {}
"#;
        let imports = extract_imports(text, ExtractionLanguage::JavaScript);
        assert!(imports.len() >= 2);
    }

    #[test]
    fn extract_imports_go() {
        let text = r#"package main

import (
    "fmt"
    "net/http"
)

func main() {}
"#;
        let imports = extract_imports(text, ExtractionLanguage::Go);
        assert!(!imports.is_empty());
    }

    #[test]
    fn extract_imports_skips_comments() {
        let text = r#"// use this;  <- comment
# import os  <- comment
use actual_module;
"#;
        let imports = extract_imports(text, ExtractionLanguage::Rust);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], "use actual_module;");
    }

    #[test]
    fn extract_imports_respects_limit() {
        let text = (0..100)
            .map(|i| format!("use module_{i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let imports = extract_imports(&text, ExtractionLanguage::Rust);
        assert_eq!(imports.len(), MAX_IMPORTS);
    }

    // --- Enclosing symbol tests ---

    #[test]
    fn find_enclosing_symbol_rust_fn() {
        let text = r#"pub fn helper() {}

pub fn main() {
    let x = 42;
}

pub fn other() {}
"#;
        let result = find_enclosing_symbol(text, 3, ExtractionLanguage::Rust);
        assert!(result.is_some());
        let (name, kind, _start, _end) = result.unwrap();
        assert_eq!(name, "main");
        assert_eq!(kind, "function");
    }

    #[test]
    fn find_enclosing_symbol_rust_struct() {
        let text = r#"pub struct Config {
    name: String,
    value: i32,
}

impl Config {
    pub fn new() -> Self {}
}
"#;
        let result = find_enclosing_symbol(text, 2, ExtractionLanguage::Rust);
        assert!(result.is_some());
        let (name, kind, _start, _end) = result.unwrap();
        assert_eq!(name, "Config");
        assert_eq!(kind, "struct");
    }

    #[test]
    fn find_enclosing_symbol_python_class() {
        let text = r#"class MyService:
    def __init__(self):
        self.name = "test"
    
    def process(self):
        return self.name
"#;
        let result = find_enclosing_symbol(text, 4, ExtractionLanguage::Python);
        assert!(result.is_some());
        let (name, kind, _start, _end) = result.unwrap();
        assert_eq!(name, "MyService");
        assert_eq!(kind, "class");
    }

    #[test]
    fn find_enclosing_symbol_typescript_function() {
        let text = r#"export function processData(items: Item[]): Result {
    const mapped = items.map(transform);
    return { items: mapped };
}
"#;
        let result = find_enclosing_symbol(text, 2, ExtractionLanguage::TypeScript);
        assert!(result.is_some());
        let (name, kind, _start, _end) = result.unwrap();
        assert_eq!(name, "processData");
        assert_eq!(kind, "function");
    }

    #[test]
    fn find_enclosing_symbol_go_method() {
        let text = r#"func (s *Server) Handle(w http.ResponseWriter, r *http.Request) {
    fmt.Fprintf(w, "hello")
}
"#;
        let result = find_enclosing_symbol(text, 2, ExtractionLanguage::Go);
        assert!(result.is_some());
        let (name, kind, _start, _end) = result.unwrap();
        assert_eq!(name, "Handle");
        assert_eq!(kind, "method");
    }

    #[test]
    fn find_enclosing_symbol_out_of_range() {
        let text = "fn main() {}";
        let result = find_enclosing_symbol(text, 100, ExtractionLanguage::Rust);
        assert!(result.is_none());
    }

    // --- extract_code_context integration test ---

    #[test]
    fn extract_code_context_rust() {
        let text = r#"use axum::Router;
use tokio::net::TcpListener;

pub struct AppState {
    pub count: std::sync::atomic::AtomicU64,
}

pub fn create_router() -> Router {
    Router::new()
}
"#;
        let ctx = extract_code_context(text, "src/main.rs", Some(8));
        assert_eq!(ctx.language.as_deref(), Some("rust"));
        assert_eq!(ctx.imports.len(), 2);
        // Line 8 is the Router::new() line, which is inside create_router
        assert!(ctx.enclosing_symbol.is_some());
    }

    // --- Serialization roundtrip ---

    #[test]
    fn code_context_roundtrip() {
        let ctx = CodeContext {
            language: Some("rust".to_string()),
            imports: vec!["use foo;".to_string()],
            enclosing_symbol: Some("main".to_string()),
            enclosing_symbol_kind: Some("function".to_string()),
            enclosing_line_start: Some(5),
            enclosing_line_end: Some(10),
        };
        let json_str = serde_json::to_string(&ctx).unwrap();
        let deserialized: CodeContext = serde_json::from_str(&json_str).unwrap();
        assert_eq!(ctx, deserialized);
    }

    #[test]
    fn code_context_default_is_empty() {
        let ctx = CodeContext::default();
        let json = serde_json::to_value(&ctx).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("language"));
        assert!(!obj.contains_key("imports"));
        assert!(!obj.contains_key("enclosing_symbol"));
    }
}
