use crate::core::document::{BlockKind, RenderedBlock};

use super::code::RenderedContent;

const MAX_CSV_ROWS: usize = 100;

/// Render CSV/TSV content as a bounded table preview.
pub fn render_csv(text: &str, max_chars: usize) -> RenderedContent {
    let mut blocks = Vec::new();
    let mut char_budget = max_chars;
    let mut block_truncated = false;
    let mut text_truncated = false;

    let mut lines: Vec<&str> = text.lines().take(MAX_CSV_ROWS + 1).collect();
    let total_lines = text.lines().count();
    let row_count = if total_lines > MAX_CSV_ROWS + 1 {
        None
    } else {
        Some(lines.len())
    };

    if lines.len() > MAX_CSV_ROWS {
        text_truncated = true;
        lines.truncate(MAX_CSV_ROWS);
    }

    if let Some(header) = lines.first() {
        let col_count = count_csv_columns(header);
        let meta = match row_count {
            Some(r) => format!("{col_count} columns, {r} rows"),
            None => format!("{col_count} columns, 100+ rows"),
        };
        let (truncated, did_truncate) = truncate_to_budget(&meta, char_budget);
        if let Some(t) = truncated {
            char_budget = char_budget.saturating_sub(t.len());
            blocks.push(RenderedBlock {
                kind: BlockKind::Code,
                text: t,
                level: None,
                anchor: None,
                language: Some("csv".to_string()),
                line_start: None,
                line_end: None,
                page: None,
            });
        }
        if did_truncate {
            block_truncated = true;
        }
    }

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if char_budget == 0 {
            block_truncated = true;
            break;
        }
        let formatted = if i == 0 {
            format!("  1 | {line}")
        } else {
            format!(" {line_num:>3} | {line}")
        };
        let (truncated, did_truncate) = truncate_to_budget(&formatted, char_budget);
        if let Some(t) = truncated {
            char_budget = char_budget.saturating_sub(t.len());
            blocks.push(RenderedBlock {
                kind: BlockKind::Code,
                text: t,
                level: None,
                anchor: None,
                language: Some("csv".to_string()),
                line_start: Some(line_num),
                line_end: Some(line_num),
                page: None,
            });
        }
        if did_truncate {
            block_truncated = true;
            break;
        }
    }

    RenderedContent {
        blocks,
        outline: Vec::new(),
        text_truncated,
        block_truncated,
    }
}

fn count_csv_columns(header: &str) -> usize {
    let mut count = 1;
    let mut in_quotes = false;
    for c in header.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => count += 1,
            _ => {}
        }
    }
    count
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
