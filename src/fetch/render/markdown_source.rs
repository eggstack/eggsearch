//! Markdown source file renderer using pulldown-cmark.
//!
//! Parses Markdown content into structured blocks with headings,
//! code blocks, lists, blockquotes, and paragraphs. Extracts
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
    let char_budget = max_chars;
    let mut block_truncated = false;

    // State for accumulating events into blocks
    let mut in_heading = false;
    let mut heading_level = 0;
    let mut heading_text = String::new();
    let mut in_code_block = false;
    let mut code_language = String::new();
    let mut code_text = String::new();
    let mut code_start_line = 1;
    let mut current_line = 1;
    let mut in_paragraph = false;
    let mut paragraph_text = String::new();
    let mut paragraph_start_line = 1;

    let events: Vec<Event> = parser.collect();
    let mut i = 0;

    while i < events.len() {
        match &events[i] {
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
                in_paragraph = true;
                paragraph_text.clear();
                paragraph_start_line = current_line;
            }
            Event::End(TagEnd::Paragraph) => {
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
            Event::Start(Tag::BlockQuote(_)) => {
                // Collect block quote content
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                // Block quotes are rendered as paragraphs in the MVP
            }
            Event::Start(Tag::List(_)) => {}
            Event::End(TagEnd::List(_)) => {}
            Event::Start(Tag::Item) => {}
            Event::End(TagEnd::Item) => {
                // List items are rendered as paragraphs in the MVP
            }
            Event::Start(Tag::Table(_)) => {}
            Event::End(TagEnd::Table) => {
                // Tables rendered as raw text in MVP
            }
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
                } else if in_paragraph {
                    paragraph_text.push_str(text);
                }
            }
            Event::Code(code) => {
                if in_paragraph {
                    paragraph_text.push('`');
                    paragraph_text.push_str(code);
                    paragraph_text.push('`');
                }
            }
            Event::SoftBreak => {
                if in_paragraph {
                    paragraph_text.push(' ');
                }
                current_line += 1;
            }
            Event::HardBreak => {
                if in_paragraph {
                    paragraph_text.push('\n');
                }
                current_line += 1;
            }
            Event::Start(Tag::Emphasis) => {}
            Event::End(TagEnd::Emphasis) => {}
            Event::Start(Tag::Strong) => {}
            Event::End(TagEnd::Strong) => {}
            Event::Start(Tag::Link { .. }) => {}
            Event::End(TagEnd::Link) => {}
            Event::Start(Tag::Image { .. }) => {}
            Event::End(TagEnd::Image) => {}
            _ => {}
        }
        i += 1;
    }

    // Flush any remaining paragraph
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

    // Apply char budget truncation
    let mut used_chars = 0;
    let mut truncated_at = blocks.len();
    for (idx, block) in blocks.iter().enumerate() {
        let block_chars = block.text.chars().count();
        if used_chars + block_chars > char_budget {
            truncated_at = idx;
            block_truncated = true;
            break;
        }
        used_chars += block_chars;
    }

    if truncated_at < blocks.len() {
        blocks.truncate(truncated_at);
    }

    let text_truncated = block_truncated;

    RenderedMarkdown {
        blocks,
        outline,
        text_truncated,
        block_truncated,
    }
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
        let result = render_markdown_source(md, 5); // very small budget
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
}
