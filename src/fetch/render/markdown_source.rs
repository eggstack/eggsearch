//! Markdown source file renderer using pulldown-cmark.
//!
//! Parses Markdown content into structured blocks with headings,
//! code blocks, lists, tables, blockquotes, and paragraphs. Extracts
//! headings into the document outline.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::core::document::{BlockKind, DocumentOutlineEntry, RenderedBlock};

/// Result of rendering Markdown into structured blocks.
pub struct RenderedMarkdown {
    /// The rendered content blocks.
    pub blocks: Vec<RenderedBlock>,
    /// Document outline built from headings.
    pub outline: Vec<DocumentOutlineEntry>,
    /// Whether the content was truncated.
    pub text_truncated: bool,
    /// Whether the block list was truncated.
    pub block_truncated: bool,
}

/// Render Markdown source text into structured blocks.
pub fn render_markdown_source(text: &str, max_chars: usize) -> RenderedMarkdown {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(text, options);

    let mut blocks = Vec::new();
    let mut outline = Vec::new();
    let budget = max_chars;
    let mut block_truncated = false;

    let mut in_heading = false;
    let mut heading_level = 0;
    let mut heading_text = String::new();
    let mut in_code_block = false;
    let mut code_language = String::new();
    let mut code_text = String::new();
    let mut code_start_line = 1;
    let mut in_paragraph = false;
    let mut paragraph_text = String::new();
    let mut paragraph_start_line = 1;
    let mut in_item = false;
    let mut item_text = String::new();
    let mut item_start_line = 1;
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut table_row: Vec<String> = Vec::new();
    let mut table_cell: Option<String> = None;
    let mut table_start_line = 1;
    let mut current_line = 1;

    let events: Vec<Event> = parser.collect();

    for event in &events {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                heading_level = *level as usize;
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if !heading_text.trim().is_empty() {
                    let block_index = blocks.len();
                    blocks.push(RenderedBlock {
                        kind: BlockKind::Heading,
                        text: heading_text.trim().to_string(),
                        level: Some(heading_level),
                        anchor: Some(make_slug(&heading_text)),
                        language: None,
                        line_start: Some(current_line),
                        line_end: Some(current_line),
                        page: None,
                    });
                    outline.push(DocumentOutlineEntry {
                        level: heading_level,
                        title: heading_text.trim().to_string(),
                        anchor: Some(make_slug(&heading_text)),
                        block_index: Some(block_index),
                        page: None,
                    });
                }
                in_heading = false;
            }
            Event::Start(Tag::CodeBlock(info)) => {
                in_code_block = true;
                code_language = match info {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code_text.clear();
                code_start_line = current_line;
            }
            Event::End(TagEnd::CodeBlock) => {
                if !code_text.is_empty() {
                    blocks.push(RenderedBlock {
                        kind: BlockKind::Code,
                        text: code_text.trim_end().to_string(),
                        level: None,
                        anchor: None,
                        language: if code_language.is_empty() {
                            None
                        } else {
                            Some(code_language.clone())
                        },
                        line_start: Some(code_start_line),
                        line_end: Some(current_line),
                        page: None,
                    });
                }
                in_code_block = false;
            }
            Event::Start(Tag::Paragraph) => {
                if in_item {
                    if !item_text.is_empty() && !item_text.ends_with(' ') {
                        item_text.push(' ');
                    }
                } else {
                    in_paragraph = true;
                    paragraph_text.clear();
                    paragraph_start_line = current_line;
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if in_item {
                    if !item_text.is_empty() && !item_text.ends_with(' ') {
                        item_text.push(' ');
                    }
                } else {
                    if !paragraph_text.trim().is_empty() {
                        blocks.push(RenderedBlock {
                            kind: BlockKind::Paragraph,
                            text: paragraph_text.trim().to_string(),
                            level: None,
                            anchor: None,
                            language: None,
                            line_start: Some(paragraph_start_line),
                            line_end: Some(current_line),
                            page: None,
                        });
                    }
                    in_paragraph = false;
                }
            }
            Event::Start(Tag::List(_)) => {}
            Event::End(TagEnd::List(_)) => {}
            Event::Start(Tag::Item) => {
                in_item = true;
                item_text.clear();
                item_start_line = current_line;
            }
            Event::End(TagEnd::Item) => {
                if !item_text.trim().is_empty() {
                    blocks.push(RenderedBlock {
                        kind: BlockKind::ListItem,
                        text: item_text.split_whitespace().collect::<Vec<_>>().join(" "),
                        level: None,
                        anchor: None,
                        language: None,
                        line_start: Some(item_start_line),
                        line_end: Some(current_line),
                        page: None,
                    });
                }
                in_item = false;
            }
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table_rows.clear();
                table_start_line = current_line;
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                if !table_rows.is_empty() {
                    let rendered = render_table_rows(&table_rows);
                    if !rendered.is_empty() {
                        blocks.push(RenderedBlock {
                            kind: BlockKind::Table,
                            text: rendered,
                            level: None,
                            anchor: None,
                            language: None,
                            line_start: Some(table_start_line),
                            line_end: Some(current_line),
                            page: None,
                        });
                    }
                }
                table_rows.clear();
            }
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                table_row.clear();
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                if !table_row.is_empty() {
                    table_rows.push(std::mem::take(&mut table_row));
                }
            }
            Event::Start(Tag::TableCell) => {
                table_cell = Some(String::new());
            }
            Event::End(TagEnd::TableCell) => {
                if let Some(cell) = table_cell.take() {
                    table_row.push(cell.trim().to_string());
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {}
            Event::End(TagEnd::BlockQuote(_)) => {}
            Event::Start(Tag::HtmlBlock) => {}
            Event::End(TagEnd::HtmlBlock) => {}
            Event::Rule => {
                blocks.push(RenderedBlock {
                    kind: BlockKind::HorizontalRule,
                    text: String::new(),
                    level: None,
                    anchor: None,
                    language: None,
                    line_start: Some(current_line),
                    line_end: Some(current_line),
                    page: None,
                });
            }
            Event::Text(text) => {
                if in_heading {
                    heading_text.push_str(text);
                } else if in_code_block {
                    code_text.push_str(text);
                } else if in_table {
                    if let Some(cell) = table_cell.as_mut() {
                        cell.push_str(text);
                    }
                } else if in_item {
                    item_text.push_str(text);
                } else if in_paragraph {
                    paragraph_text.push_str(text);
                }
            }
            Event::Code(code) => {
                let target = if in_table {
                    table_cell.as_mut()
                } else if in_item {
                    Some(&mut item_text)
                } else if in_paragraph {
                    Some(&mut paragraph_text)
                } else {
                    None
                };
                if let Some(target) = target {
                    target.push('`');
                    target.push_str(code);
                    target.push('`');
                }
            }
            Event::SoftBreak => {
                if in_paragraph {
                    paragraph_text.push(' ');
                } else if in_item {
                    item_text.push(' ');
                }
                current_line += 1;
            }
            Event::HardBreak => {
                if in_paragraph {
                    paragraph_text.push('\n');
                } else if in_item {
                    item_text.push(' ');
                }
                current_line += 1;
            }
            _ => {}
        }
    }

    if in_paragraph && !paragraph_text.trim().is_empty() {
        blocks.push(RenderedBlock {
            kind: BlockKind::Paragraph,
            text: paragraph_text.trim().to_string(),
            level: None,
            anchor: None,
            language: None,
            line_start: Some(paragraph_start_line),
            line_end: Some(current_line),
            page: None,
        });
    }

    let mut used_chars = 0;
    let mut truncated_at = blocks.len();
    let mut partial_text: Option<String> = None;
    for (idx, block) in blocks.iter().enumerate() {
        let block_chars = block.text.chars().count();
        if used_chars + block_chars > budget {
            if block.kind == BlockKind::Code && budget > used_chars {
                let remaining = budget - used_chars;
                let truncated: String = block.text.chars().take(remaining).collect();
                if let Some(last_nl) = truncated.rfind('\n') {
                    if last_nl > remaining / 2 {
                        partial_text = Some(truncated[..last_nl].to_string());
                        truncated_at = idx + 1;
                        block_truncated = true;
                        break;
                    }
                }
                partial_text = Some(truncated);
                truncated_at = idx + 1;
            } else {
                truncated_at = idx;
            }
            block_truncated = true;
            break;
        }
        used_chars += block_chars;
    }

    if truncated_at < blocks.len() {
        blocks.truncate(truncated_at);
    }
    if let Some(text) = partial_text.take() {
        if truncated_at > 0 && truncated_at <= blocks.len() {
            blocks[truncated_at - 1].text = text;
        }
    }
    outline.retain(|e| e.block_index.is_none_or(|i| i < blocks.len()));

    let text_truncated = block_truncated;

    RenderedMarkdown {
        blocks,
        outline,
        text_truncated,
        block_truncated,
    }
}

fn render_table_rows(rows: &[Vec<String>]) -> String {
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if max_cols == 0 {
        return String::new();
    }
    let mut lines = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let mut line = String::from("|");
        for cell in row {
            line.push(' ');
            line.push_str(&cell.replace('|', "\\|"));
            line.push_str(" |");
        }
        for _ in row.len()..max_cols {
            line.push_str(" |");
        }
        lines.push(line);
        if index == 0 {
            lines.push(format!("|{}", " --- |".repeat(max_cols)));
        }
    }
    lines.join("\n")
}

/// Generate a URL-safe slug from heading text.
fn make_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_markdown_headings() {
        let md = "# Title\n\n## Section\n\nText here.";
        let result = render_markdown_source(md, 10000);
        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.blocks[0].kind, BlockKind::Heading);
        assert_eq!(result.blocks[0].level, Some(1));
        assert_eq!(result.blocks[0].text, "Title");
        assert_eq!(result.blocks[1].kind, BlockKind::Heading);
        assert_eq!(result.blocks[1].level, Some(2));
        assert_eq!(result.outline.len(), 2);
        assert_eq!(result.outline[0].title, "Title");
        assert_eq!(result.outline[1].title, "Section");
    }

    #[test]
    fn render_markdown_code_blocks() {
        let md = "```rust\nfn main() {\n    println!(\"hello\");\n}\n```";
        let result = render_markdown_source(md, 10000);
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].kind, BlockKind::Code);
        assert_eq!(result.blocks[0].language, Some("rust".to_string()));
        assert!(result.blocks[0].text.contains("fn main()"));
    }

    #[test]
    fn render_markdown_paragraphs() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let result = render_markdown_source(md, 10000);
        assert_eq!(result.blocks.len(), 2);
        assert!(result.blocks.iter().all(|b| b.kind == BlockKind::Paragraph));
    }

    #[test]
    fn render_markdown_inline_code() {
        let md = "Use `cargo build` to compile.";
        let result = render_markdown_source(md, 10000);
        assert_eq!(result.blocks.len(), 1);
        assert!(result.blocks[0].text.contains("`cargo build`"));
    }

    #[test]
    fn render_markdown_truncation() {
        let md = "# Title\n\n## Other\n\nText.";
        let result = render_markdown_source(md, 5);
        assert!(result.text_truncated || result.block_truncated);
    }

    #[test]
    fn make_slug_basic() {
        assert_eq!(make_slug("Hello World"), "hello-world");
        assert_eq!(make_slug("Special!@#"), "special");
    }

    #[test]
    fn render_markdown_thematic_break() {
        let md = "Before\n\n---\n\nAfter.";
        let result = render_markdown_source(md, 10000);
        assert!(result
            .blocks
            .iter()
            .any(|b| b.kind == BlockKind::HorizontalRule));
    }

    #[test]
    fn render_markdown_table_preserves_cells() {
        let md = "| Metric | Value |\n| --- | --- |\n| retries | 3 |\n| budget | 5s |";
        let result = render_markdown_source(md, 10000);
        let table = result
            .blocks
            .iter()
            .find(|b| b.kind == BlockKind::Table)
            .expect("table block");
        assert!(table.text.contains("| Metric | Value |"), "{}", table.text);
        assert!(table.text.contains("| retries | 3 |"), "{}", table.text);
        assert!(table.text.contains("| budget | 5s |"), "{}", table.text);
    }

    #[test]
    fn render_markdown_tight_list_items_are_kept() {
        let md = "- first item\n- second item\n- third item";
        let result = render_markdown_source(md, 10000);
        let items: Vec<&str> = result
            .blocks
            .iter()
            .filter(|b| b.kind == BlockKind::ListItem)
            .map(|b| b.text.as_str())
            .collect();
        assert_eq!(items, vec!["first item", "second item", "third item"]);
    }

    #[test]
    fn render_markdown_loose_list_items_are_kept() {
        let md = "1. alpha\n\n2. beta";
        let result = render_markdown_source(md, 10000);
        let items: Vec<&str> = result
            .blocks
            .iter()
            .filter(|b| b.kind == BlockKind::ListItem)
            .map(|b| b.text.as_str())
            .collect();
        assert_eq!(items, vec!["alpha", "beta"]);
    }
}
