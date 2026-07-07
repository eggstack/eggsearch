use serde_json::Value;

use crate::core::document::{BlockKind, RenderedBlock};

use super::code::RenderedContent;

const MAX_CELLS: usize = 200;

/// Render a Jupyter notebook (.ipynb) by extracting markdown and code cells.
///
/// Never executes code. Outputs are skipped by default.
pub fn render_notebook(text: &str, max_chars: usize) -> RenderedContent {
    let parsed: Result<Value, _> = serde_json::from_str(text);

    let notebook = match parsed {
        Ok(v) => v,
        Err(_) => {
            return RenderedContent {
                blocks: vec![RenderedBlock {
                    kind: BlockKind::RawText,
                    text: text.to_string(),
                    level: None,
                    anchor: None,
                    language: None,
                    line_start: None,
                    line_end: None,
                    page: None,
                }],
                outline: Vec::new(),
                text_truncated: false,
                block_truncated: false,
            };
        }
    };

    let cells = match notebook.get("cells").and_then(|c| c.as_array()) {
        Some(cells) => cells,
        None => {
            return RenderedContent {
                blocks: vec![RenderedBlock {
                    kind: BlockKind::RawText,
                    text: text.to_string(),
                    level: None,
                    anchor: None,
                    language: None,
                    line_start: None,
                    line_end: None,
                    page: None,
                }],
                outline: Vec::new(),
                text_truncated: false,
                block_truncated: false,
            };
        }
    };

    let mut blocks = Vec::new();
    let mut char_budget = max_chars;
    let mut block_truncated = false;
    let mut text_truncated = false;

    let total_cells = cells.len();
    let iter: Box<dyn Iterator<Item = &Value>> = if total_cells > MAX_CELLS {
        text_truncated = true;
        Box::new(cells.iter().take(MAX_CELLS))
    } else {
        Box::new(cells.iter())
    };

    for (idx, cell) in iter.enumerate() {
        if char_budget == 0 {
            block_truncated = true;
            break;
        }

        let cell_type = cell
            .get("cell_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let source = cell
            .get("source")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        if source.trim().is_empty() {
            continue;
        }

        let (kind, language) = match cell_type {
            "markdown" => (BlockKind::Paragraph, None),
            "code" => (BlockKind::Code, Some("python".to_string())),
            _ => (BlockKind::RawText, None),
        };

        let header = format!("[cell {} ({})]", idx + 1, cell_type,);

        let content = format!("{header}\n{source}");
        let (truncated, did_truncate) = truncate_to_budget(&content, char_budget);
        if let Some(t) = truncated {
            char_budget = char_budget.saturating_sub(t.len());
            blocks.push(RenderedBlock {
                kind,
                text: t,
                level: None,
                anchor: None,
                language,
                line_start: None,
                line_end: None,
                page: None,
            });
        }
        if did_truncate {
            block_truncated = true;
            break;
        }
    }

    let mut outline = Vec::new();
    if let Some(title) = notebook
        .get("metadata")
        .and_then(|m| m.get("kernelspec"))
        .and_then(|k| k.get("display_name"))
        .and_then(|v| v.as_str())
    {
        if !title.is_empty() {
            outline.push(crate::core::document::DocumentOutlineEntry {
                level: 1,
                title: title.to_string(),
                anchor: None,
                block_index: if blocks.is_empty() { None } else { Some(0) },
            });
        }
    }

    RenderedContent {
        blocks,
        outline,
        text_truncated,
        block_truncated,
    }
}

fn truncate_to_budget(text: &str, char_budget: usize) -> (Option<String>, bool) {
    if char_budget == 0 {
        return (None, true);
    }
    let char_count = text.chars().count();
    if char_count <= char_budget {
        (Some(text.to_string()), false)
    } else {
        let truncated: String = text.chars().take(char_budget).collect();
        (Some(truncated), true)
    }
}
