use crate::core::document::{BlockKind, RenderedBlock};

/// Render blocks as Markdown.
pub fn render_blocks_markdown(blocks: &[RenderedBlock]) -> String {
    let mut parts = Vec::new();

    for block in blocks {
        match block.kind {
            BlockKind::Heading => {
                let level = block.level.unwrap_or(1);
                parts.push(format!("{} {}", "#".repeat(level), block.text));
                parts.push(String::new());
            }
            BlockKind::Paragraph => {
                parts.push(block.text.clone());
                parts.push(String::new());
            }
            BlockKind::ListItem => {
                parts.push(format!("- {}", block.text));
            }
            BlockKind::Code => {
                if let Some(ref lang) = block.language {
                    parts.push(format!("```{}\n{}\n```", lang, block.text));
                } else {
                    parts.push(format!("```\n{}\n```", block.text));
                }
                parts.push(String::new());
            }
            BlockKind::Table => {
                parts.push(block.text.clone());
                parts.push(String::new());
            }
            BlockKind::BlockQuote => {
                for line in block.text.lines() {
                    parts.push(format!("> {}", line));
                }
                parts.push(String::new());
            }
            BlockKind::Definition => {
                parts.push(format!("**{}**", block.text));
                parts.push(String::new());
            }
            BlockKind::HorizontalRule => {
                parts.push("---".to_string());
                parts.push(String::new());
            }
            _ => {
                if !block.text.is_empty() {
                    parts.push(block.text.clone());
                    parts.push(String::new());
                }
            }
        }
    }

    while parts.last().is_some_and(|s| s.is_empty()) {
        parts.pop();
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_heading_with_hash() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::Heading,
            text: "Title".to_string(),
            level: Some(2),
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        }];
        let md = render_blocks_markdown(&blocks);
        assert_eq!(md, "## Title");
    }

    #[test]
    fn markdown_code_block_with_language() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::Code,
            text: "x = 1".to_string(),
            level: None,
            anchor: None,
            language: Some("python".to_string()),
            line_start: None,
            line_end: None,
            page: None,
        }];
        let md = render_blocks_markdown(&blocks);
        assert!(md.contains("```python"));
        assert!(md.contains("x = 1"));
    }

    #[test]
    fn markdown_definition_block() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::Definition,
            text: "term: definition".to_string(),
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        }];
        let md = render_blocks_markdown(&blocks);
        assert!(md.contains("**term: definition**"));
    }

    #[test]
    fn markdown_list_items() {
        let blocks = vec![
            RenderedBlock {
                kind: BlockKind::ListItem,
                text: "A".to_string(),
                level: None,
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: None,
            },
            RenderedBlock {
                kind: BlockKind::ListItem,
                text: "B".to_string(),
                level: None,
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: None,
            },
        ];
        let md = render_blocks_markdown(&blocks);
        assert!(md.contains("- A"));
        assert!(md.contains("- B"));
    }

    #[test]
    fn markdown_blockquote() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::BlockQuote,
            text: "quoted text".to_string(),
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        }];
        let md = render_blocks_markdown(&blocks);
        assert!(md.contains("> quoted text"));
    }

    #[test]
    fn markdown_table() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::Table,
            text: "| A | B |\n| --- | --- |\n| 1 | 2 |".to_string(),
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        }];
        let md = render_blocks_markdown(&blocks);
        assert!(md.contains("| A | B |"));
        assert!(md.contains("| 1 | 2 |"));
    }

    #[test]
    fn markdown_code_block_preserves_whitespace() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::Code,
            text: "line1\n  line2\n    line3".to_string(),
            level: None,
            anchor: None,
            language: Some("rust".to_string()),
            line_start: None,
            line_end: None,
            page: None,
        }];
        let md = render_blocks_markdown(&blocks);
        assert!(md.contains("```rust\nline1\n  line2\n    line3\n```"));
    }
}
