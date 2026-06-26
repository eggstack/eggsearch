use crate::core::document::{BlockKind, RenderedBlock};

/// Render blocks as plain text.
pub fn render_blocks_text(blocks: &[RenderedBlock]) -> String {
    let mut parts = Vec::new();

    for block in blocks {
        match block.kind {
            BlockKind::Heading => {
                parts.push(block.text.clone());
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
                parts.push(block.text.clone());
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
    fn text_heading_and_paragraph() {
        let blocks = vec![
            RenderedBlock {
                kind: BlockKind::Heading,
                text: "Title".to_string(),
                level: Some(1),
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: None,
            },
            RenderedBlock {
                kind: BlockKind::Paragraph,
                text: "Hello world".to_string(),
                level: None,
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: None,
            },
        ];
        let text = render_blocks_text(&blocks);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn text_code_block_with_language() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::Code,
            text: "fn main() {}".to_string(),
            level: None,
            anchor: None,
            language: Some("rust".to_string()),
            line_start: None,
            line_end: None,
            page: None,
        }];
        let text = render_blocks_text(&blocks);
        assert!(text.contains("```rust"));
        assert!(text.contains("fn main() {}"));
    }

    #[test]
    fn text_list_items() {
        let blocks = vec![
            RenderedBlock {
                kind: BlockKind::ListItem,
                text: "One".to_string(),
                level: None,
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: None,
            },
            RenderedBlock {
                kind: BlockKind::ListItem,
                text: "Two".to_string(),
                level: None,
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: None,
            },
        ];
        let text = render_blocks_text(&blocks);
        assert!(text.contains("- One"));
        assert!(text.contains("- Two"));
    }

    #[test]
    fn text_horizontal_rule() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::HorizontalRule,
            text: String::new(),
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        }];
        let text = render_blocks_text(&blocks);
        assert_eq!(text, "---");
    }

    #[test]
    fn text_blockquote() {
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
        let text = render_blocks_text(&blocks);
        assert!(text.contains("> quoted text"));
    }

    #[test]
    fn text_table() {
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
        let text = render_blocks_text(&blocks);
        assert!(text.contains("| A | B |"));
        assert!(text.contains("| 1 | 2 |"));
    }

    #[test]
    fn text_code_block_preserves_whitespace() {
        let blocks = vec![RenderedBlock {
            kind: BlockKind::Code,
            text: "line1\n  line2\n    line3".to_string(),
            level: None,
            anchor: None,
            language: Some("python".to_string()),
            line_start: None,
            line_end: None,
            page: None,
        }];
        let text = render_blocks_text(&blocks);
        assert!(text.contains("```python\nline1\n  line2\n    line3\n```"));
    }
}
