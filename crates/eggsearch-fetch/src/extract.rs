//! Content extraction for `eggsearch-fetch`.

use scraper::{Html, Selector};

use crate::html::strip_noise;

/// Strip noise tags and return visible text. Whitespace is collapsed.
pub fn extract_text(html: &str) -> String {
    let stripped = strip_noise(html);
    let doc = Html::parse_document(&stripped);
    let body_sel = Selector::parse("body").ok();
    let mut out = String::new();
    if let Some(body) = body_sel.and_then(|s| doc.select(&s).next()) {
        for t in body.text() {
            let s = t.split_whitespace().collect::<Vec<_>>().join(" ");
            if !s.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&s);
            }
        }
    }
    out
}

/// Readability-style extraction: preserve headings, paragraphs, and list
/// items. Produces plain text with simple structural markers.
pub fn extract_html(html: &str) -> String {
    let stripped = strip_noise(html);
    let doc = Html::parse_document(&stripped);
    let block_sel = Selector::parse("h1, h2, h3, h4, h5, h6, p, li, blockquote, pre, code, br").ok();
    let body_sel = Selector::parse("body").ok();
    let mut out = String::new();

    fn emit(node: &scraper::ElementRef, out: &mut String) {
        let tag = node.value().name().to_lowercase();
        let text: String = node.text().collect::<Vec<_>>().join(" ").trim().to_string();
        if text.is_empty() {
            return;
        }
        match tag.as_str() {
            "h1" => {
                out.push_str("\n# ");
                out.push_str(&text);
                out.push('\n');
            }
            "h2" => {
                out.push_str("\n## ");
                out.push_str(&text);
                out.push('\n');
            }
            "h3" => {
                out.push_str("\n### ");
                out.push_str(&text);
                out.push('\n');
            }
            "h4" | "h5" | "h6" => {
                let level: usize = tag.trim_start_matches('h').parse().unwrap_or(4);
                out.push('\n');
                for _ in 0..(level + 1) {
                    out.push('#');
                }
                out.push(' ');
                out.push_str(&text);
                out.push('\n');
            }
            "p" => {
                out.push_str("\n");
                out.push_str(&text);
                out.push('\n');
            }
            "li" => {
                out.push_str("\n- ");
                out.push_str(&text);
                out.push('\n');
            }
            "blockquote" => {
                out.push_str("\n> ");
                out.push_str(&text);
                out.push('\n');
            }
            "pre" | "code" => {
                out.push_str("\n```\n");
                out.push_str(&text);
                out.push_str("\n```\n");
            }
            "br" => {
                out.push('\n');
            }
            _ => {
                out.push_str(&text);
                out.push(' ');
            }
        }
    }

    if let (Some(body), Some(bs)) = (body_sel.and_then(|s| doc.select(&s).next()), block_sel) {
        for node in body.select(&bs) {
            emit(&node, &mut out);
        }
    }
    out.trim().to_string()
}

pub use crate::fetch::ExtractMode;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_text_strips_tags() {
        let html = r#"<html><body><p>Hello <b>world</b>!</p><script>evil()</script></body></html>"#;
        let t = extract_text(html);
        assert!(t.contains("Hello"));
        assert!(t.contains("world"));
        assert!(!t.contains("evil"));
    }

    #[test]
    fn extract_html_preserves_headings() {
        let html = "<html><body><h1>Title</h1><p>Body text.</p><h2>Subhead</h2><li>Item 1</li></body></html>";
        let t = extract_html(html);
        assert!(t.contains("# Title"));
        assert!(t.contains("Body text."));
        assert!(t.contains("## Subhead"));
        assert!(t.contains("- Item 1"));
    }
}
