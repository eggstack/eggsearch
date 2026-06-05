//! Markdown-style HTML extraction (lightweight).

use crate::extract::extract_text;

/// Convert HTML to a Markdown-ish text representation.
/// We don't implement a full HTML->MD converter; this is enough for
/// capturing structure (headings + paragraphs) and is resilient to
/// malformed input.
pub fn html_to_markdown(html: &str) -> String {
    // For MVP we reuse readability extraction and just return the result.
    // A more complete converter can be slotted in later.
    extract_text(html)
}
