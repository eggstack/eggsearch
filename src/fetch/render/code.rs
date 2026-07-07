//! Line-preserving renderer for code, config, diff, and plain text content.
//!
//! Unlike the HTML renderer which flattens whitespace, this renderer
//! preserves exact line breaks, indentation, and line numbers.

use crate::core::document::{BlockKind, DocumentOutlineEntry, RenderedBlock};

/// Result of rendering non-HTML content into structured blocks.
pub struct RenderedContent {
    /// The rendered content blocks.
    pub blocks: Vec<RenderedBlock>,
    /// Document outline (table of contents).
    pub outline: Vec<DocumentOutlineEntry>,
    /// Whether the content was truncated at character level.
    pub text_truncated: bool,
    /// Whether the block list was truncated.
    pub block_truncated: bool,
}

/// Maximum lines per code block before splitting.
const MAX_LINES_PER_BLOCK: usize = 200;

/// Truncate `text` to fit within `char_budget`, returning the truncated
/// string and a bool indicating truncation occurred. If `char_budget` is
/// zero, returns `None` (caller should stop without pushing a block).
fn truncate_to_budget(text: &str, char_budget: usize) -> Option<(String, bool)> {
    if char_budget == 0 {
        return None;
    }
    let char_count = text.chars().count();
    if char_count <= char_budget {
        Some((text.to_string(), false))
    } else {
        let truncated: String = text.chars().take(char_budget).collect();
        Some((truncated, true))
    }
}

/// Render a code-like document (source code, JSON, TOML, YAML, diff, patch).
///
/// Preserves exact line breaks and indentation. Produces one or more
/// `Code` blocks with line ranges.
pub fn render_code(text: &str, language: Option<&str>, max_chars: usize) -> RenderedContent {
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();

    let mut blocks = Vec::new();
    let mut char_budget = max_chars;
    let mut block_truncated = false;
    let mut text_truncated = false;
    let mut last_line = 0;

    // Split into chunks by line count or character budget
    let mut start_line = 0;
    while start_line < total_lines {
        let mut end_line = start_line;
        let mut chunk_chars = 0;

        while end_line < total_lines {
            let line_chars = lines[end_line].chars().count() + 1; // +1 for newline
            if chunk_chars + line_chars > char_budget
                || end_line - start_line >= MAX_LINES_PER_BLOCK
            {
                break;
            }
            chunk_chars += line_chars;
            end_line += 1;
        }

        if end_line == start_line {
            // Single line exceeds budget - take only what fits
            let line = lines[start_line];
            if let Some((truncated_text, line_truncated)) = truncate_to_budget(line, char_budget) {
                block_truncated = true;
                if line_truncated {
                    text_truncated = true;
                }
                blocks.push(RenderedBlock {
                    kind: BlockKind::Code,
                    text: truncated_text,
                    level: None,
                    anchor: None,
                    language: language.map(|s| s.to_string()),
                    line_start: Some(start_line + 1),
                    line_end: Some(start_line + 1), // single line
                    page: None,
                });
                // Account for the line chars consumed (up to budget + newline)
                let consumed = line.chars().count().min(char_budget) + 1;
                char_budget = char_budget.saturating_sub(consumed);
                if line_truncated {
                    char_budget = 0;
                }
            }
            last_line = start_line + 1;
            start_line += 1;
            if char_budget == 0 {
                block_truncated = true;
                break;
            }
            continue;
        }

        // Check if we've exceeded total budget
        if start_line == 0 && end_line < total_lines && chunk_chars > max_chars {
            end_line = start_line;
            while end_line < total_lines {
                let line_chars = lines[end_line].chars().count() + 1;
                if chunk_chars > max_chars {
                    break;
                }
                chunk_chars += line_chars;
                end_line += 1;
            }
            if end_line == start_line {
                // First line alone exceeds budget - truncate it
                let line = lines[start_line];
                if let Some((truncated_text, line_truncated)) =
                    truncate_to_budget(line, char_budget)
                {
                    block_truncated = true;
                    if line_truncated {
                        text_truncated = true;
                    }
                    blocks.push(RenderedBlock {
                        kind: BlockKind::Code,
                        text: truncated_text,
                        level: None,
                        anchor: None,
                        language: language.map(|s| s.to_string()),
                        line_start: Some(start_line + 1),
                        line_end: Some(start_line + 1),
                        page: None,
                    });
                    if line_truncated {
                        char_budget = 0;
                    }
                }
                last_line = start_line + 1;
                start_line += 1;
                if char_budget == 0 {
                    block_truncated = true;
                    break;
                }
                continue;
            }
            block_truncated = true;
        }

        char_budget = char_budget.saturating_sub(chunk_chars);
        last_line = end_line;

        let block_text = lines[start_line..end_line].join("\n");

        blocks.push(RenderedBlock {
            kind: BlockKind::Code,
            text: block_text,
            level: None,
            anchor: None,
            language: language.map(|s| s.to_string()),
            line_start: Some(start_line + 1), // 1-based
            line_end: Some(end_line),         // inclusive, 1-based
            page: None,
        });

        start_line = end_line;

        if char_budget == 0 {
            block_truncated = true;
            break;
        }
    }

    text_truncated = text_truncated || last_line < total_lines || block_truncated;
    if last_line < total_lines {
        block_truncated = true;
    }

    // No outline for code files (headings don't apply)
    let outline = Vec::new();

    RenderedContent {
        blocks,
        outline,
        text_truncated,
        block_truncated,
    }
}

/// Render a diff or patch document.
///
/// Similar to code rendering but uses `Diff` block kind when available.
pub fn render_diff(text: &str, max_chars: usize) -> RenderedContent {
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();

    let mut blocks = Vec::new();
    let mut char_budget = max_chars;
    let mut block_truncated = false;
    let mut text_truncated = false;
    let mut last_line = 0;

    let mut start_line = 0;
    while start_line < total_lines {
        let mut end_line = start_line;
        let mut chunk_chars = 0;

        while end_line < total_lines {
            let line_chars = lines[end_line].chars().count() + 1;
            if chunk_chars + line_chars > char_budget
                || end_line - start_line >= MAX_LINES_PER_BLOCK
            {
                break;
            }
            chunk_chars += line_chars;
            end_line += 1;
        }

        if end_line == start_line {
            // Single line exceeds budget - take only what fits
            let line = lines[start_line];
            if let Some((truncated_text, line_truncated)) = truncate_to_budget(line, char_budget) {
                block_truncated = true;
                if line_truncated {
                    text_truncated = true;
                }
                blocks.push(RenderedBlock {
                    kind: BlockKind::Code,
                    text: truncated_text,
                    level: None,
                    anchor: None,
                    language: Some("diff".to_string()),
                    line_start: Some(start_line + 1),
                    line_end: Some(start_line + 1),
                    page: None,
                });
                let consumed = line.chars().count().min(char_budget) + 1;
                char_budget = char_budget.saturating_sub(consumed);
                if line_truncated {
                    char_budget = 0;
                }
            }
            last_line = start_line + 1;
            start_line += 1;
            if char_budget == 0 {
                block_truncated = true;
                break;
            }
            continue;
        }

        char_budget = char_budget.saturating_sub(chunk_chars);
        last_line = end_line;

        let block_text = lines[start_line..end_line].join("\n");

        blocks.push(RenderedBlock {
            kind: BlockKind::Code,
            text: block_text,
            level: None,
            anchor: None,
            language: Some("diff".to_string()),
            line_start: Some(start_line + 1),
            line_end: Some(end_line),
            page: None,
        });

        start_line = end_line;

        if char_budget == 0 {
            block_truncated = true;
            break;
        }
    }

    text_truncated = text_truncated || last_line < total_lines || block_truncated;
    if last_line < total_lines {
        block_truncated = true;
    }

    RenderedContent {
        blocks,
        outline: Vec::new(),
        text_truncated,
        block_truncated,
    }
}

/// Render plain text prose, preserving paragraph breaks.
///
/// Blank-line-separated paragraphs become `Paragraph` blocks.
/// Lines with indentation or code-like markers stay intact.
pub fn render_plaintext(text: &str, max_chars: usize) -> RenderedContent {
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();

    let mut blocks = Vec::new();
    let mut char_budget = max_chars;
    let mut block_truncated = false;
    let mut text_truncated = false;

    // Group into paragraphs by blank lines
    let mut para_start = 0;
    while para_start < total_lines {
        // Skip leading blank lines
        while para_start < total_lines && lines[para_start].trim().is_empty() {
            para_start += 1;
        }
        if para_start >= total_lines {
            break;
        }

        // Find end of paragraph (next blank line or end of input)
        let mut para_end = para_start;
        while para_end < total_lines && !lines[para_end].trim().is_empty() {
            para_end += 1;
        }

        let para_text = lines[para_start..para_end].join("\n");

        let para_chars = para_text.chars().count();
        if para_chars > char_budget {
            // Paragraph exceeds budget - truncate and return what fits
            if let Some((truncated_text, para_truncated)) =
                truncate_to_budget(&para_text, char_budget)
            {
                block_truncated = true;
                if para_truncated {
                    text_truncated = true;
                }
                blocks.push(RenderedBlock {
                    kind: BlockKind::Paragraph,
                    text: truncated_text,
                    level: None,
                    anchor: None,
                    language: None,
                    line_start: Some(para_start + 1),
                    line_end: Some(para_end),
                    page: None,
                });
                if para_truncated {
                    char_budget = 0;
                }
            }
            para_start = para_end;
            break;
        }

        char_budget -= para_chars;

        blocks.push(RenderedBlock {
            kind: BlockKind::Paragraph,
            text: para_text,
            level: None,
            anchor: None,
            language: None,
            line_start: Some(para_start + 1),
            line_end: Some(para_end),
            page: None,
        });

        para_start = para_end;
    }

    text_truncated = text_truncated || (char_budget == 0 && para_start < total_lines);

    RenderedContent {
        blocks,
        outline: Vec::new(),
        text_truncated,
        block_truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_code_single_block() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let result = render_code(code, Some("rust"), 10000);
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].kind, BlockKind::Code);
        assert_eq!(result.blocks[0].language, Some("rust".to_string()));
        assert_eq!(result.blocks[0].line_start, Some(1));
        assert_eq!(result.blocks[0].line_end, Some(3));
        assert!(!result.text_truncated);
        assert!(!result.block_truncated);
    }

    #[test]
    fn render_code_preserves_indentation() {
        let code = "fn main() {\n    let x = 1;\n        let y = 2;\n}";
        let result = render_code(code, Some("rust"), 10000);
        assert!(result.blocks[0].text.contains("    let x"));
        assert!(result.blocks[0].text.contains("        let y"));
    }

    #[test]
    fn render_code_splits_large_file() {
        let lines: Vec<String> = (0..500).map(|i| format!("line {i}")).collect();
        let code = lines.join("\n");
        let result = render_code(code.as_str(), Some("text"), 100000);
        assert!(result.blocks.len() > 1, "should split into multiple blocks");
        // Check line ranges are correct
        assert_eq!(result.blocks[0].line_start, Some(1));
        assert!(result.blocks[0].line_end.unwrap() <= 200);
    }

    #[test]
    fn render_code_truncates_at_line_boundary() {
        let code = "line1\nline2\nline3\nline4\nline5";
        let result = render_code(code, None, 15); // ~15 chars budget
        assert!(result.text_truncated || result.block_truncated);
        // Should not truncate mid-line
        for block in &result.blocks {
            assert!(!block.text.ends_with("li")); // no mid-word truncation
        }
    }

    #[test]
    fn render_diff_preserves_hunks() {
        let diff = "--- a/foo.rs\n+++ b/foo.rs\n@@ -1,3 +1,3 @@\n-old line\n+new line\n context";
        let result = render_diff(diff, 10000);
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].language, Some("diff".to_string()));
        assert!(result.blocks[0].text.contains("@@ -1,3 +1,3 @@"));
    }

    #[test]
    fn render_plaintext_paragraphs() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let result = render_plaintext(text, 10000);
        assert_eq!(result.blocks.len(), 3);
        assert!(result.blocks.iter().all(|b| b.kind == BlockKind::Paragraph));
    }

    #[test]
    fn render_plaintext_preserves_indented_lines() {
        let text = "Normal text.\n    Indented code-like text.\nMore text.";
        let result = render_plaintext(text, 10000);
        assert_eq!(result.blocks.len(), 1);
        assert!(result.blocks[0].text.contains("    Indented"));
    }

    #[test]
    fn render_code_no_language() {
        let code = "just some text";
        let result = render_code(code, None, 10000);
        assert_eq!(result.blocks[0].language, None);
    }

    #[test]
    fn render_diff_truncates() {
        let diff: String = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let result = render_diff(&diff, 100);
        assert!(result.text_truncated || result.block_truncated);
    }

    #[test]
    fn render_code_single_oversized_line_truncated() {
        let code = "a".repeat(500);
        let result = render_code(&code, Some("json"), 100);
        assert_eq!(result.blocks.len(), 1);
        let block_text_chars = result.blocks[0].text.chars().count();
        assert!(
            block_text_chars <= 100,
            "block text ({block_text_chars} chars) must be <= max_chars (100)"
        );
        assert!(result.text_truncated);
        assert!(result.block_truncated);
        // Line range should be preserved as the single line
        assert_eq!(result.blocks[0].line_start, Some(1));
        assert_eq!(result.blocks[0].line_end, Some(1));
    }

    #[test]
    fn render_diff_single_oversized_line_truncated() {
        let diff = format!("+{}", "=".repeat(500));
        let result = render_diff(&diff, 100);
        assert_eq!(result.blocks.len(), 1);
        let block_text_chars = result.blocks[0].text.chars().count();
        assert!(
            block_text_chars <= 100,
            "block text ({block_text_chars} chars) must be <= max_chars (100)"
        );
        assert!(result.text_truncated);
        assert!(result.block_truncated);
    }

    #[test]
    fn render_plaintext_single_oversized_paragraph_truncated() {
        let text = "word ".repeat(200);
        let result = render_plaintext(text.trim(), 100);
        assert_eq!(result.blocks.len(), 1);
        let block_text_chars = result.blocks[0].text.chars().count();
        assert!(
            block_text_chars <= 100,
            "block text ({block_text_chars} chars) must be <= max_chars (100)"
        );
        assert!(result.text_truncated);
        assert!(result.block_truncated);
    }
}
