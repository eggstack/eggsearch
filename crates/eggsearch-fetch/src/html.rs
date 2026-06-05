//! HTML extraction: title, headings, main content, link extraction.

use scraper::{Html, Selector};
use std::collections::HashSet;

const NOISE_TAGS: &[&str] = &[
    "script", "style", "noscript", "iframe", "svg", "canvas", "video", "audio",
    "form", "button", "input", "select", "textarea", "object", "embed",
];

const NOISE_ROLES: &[&str] = &["navigation", "banner", "contentinfo", "search"];

pub fn extract_title(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("title").ok()?;
    doc.select(&sel)
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn strip_noise(html: &str) -> String {
    let doc = Html::parse_document(html);
    let mut out = String::with_capacity(html.len() / 2);
    let body_sel = Selector::parse("body").ok();
    if let Some(body) = body_sel.and_then(|s| doc.select(&s).next()) {
        walk_element(&body, &mut out);
    }
    out
}

fn is_noise(el: &scraper::ElementRef) -> bool {
    let tag = el.value().name().to_lowercase();
    if NOISE_TAGS.contains(&tag.as_str()) {
        return true;
    }
    let role = el.value().attr("role").unwrap_or("").to_lowercase();
    if NOISE_ROLES.contains(&role.as_str()) {
        return true;
    }
    let id_attr = el.value().attr("id").unwrap_or("").to_lowercase();
    let cls = el.value().attr("class").unwrap_or("").to_lowercase();
    id_attr.contains("nav")
        || id_attr.contains("footer")
        || id_attr.contains("header")
        || id_attr.contains("sidebar")
        || id_attr.contains("ad")
        || cls.contains("nav")
        || cls.contains("footer")
        || cls.contains("sidebar")
        || cls.contains("ad-")
        || cls.contains("advert")
}

fn walk_element(el: &scraper::ElementRef, out: &mut String) {
    if is_noise(el) {
        return;
    }
    let tag = el.value().name().to_lowercase();
    out.push('<');
    out.push_str(&tag);
    for (k, v) in el.value().attrs() {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&html_escape_attr(v));
        out.push('"');
    }
    out.push('>');
    for child in el.children() {
        match child.value() {
            scraper::node::Node::Text(t) => {
                out.push_str(&html_escape_text(&t.to_string()));
            }
            scraper::node::Node::Element(_) => {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    walk_element(&child_el, out);
                }
            }
            _ => {}
        }
    }
    out.push_str("</");
    out.push_str(&tag);
    out.push('>');
}

fn html_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
}

/// Extract a flat list of <a href> links.
pub fn extract_links(html: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let sel = match Selector::parse("a[href]") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for a in doc.select(&sel) {
        if let Some(h) = a.value().attr("href") {
            let s = h.trim().to_string();
            if s.is_empty() || s.starts_with('#') || s.starts_with("javascript:") {
                continue;
            }
            if seen.insert(s.clone()) {
                out.push(s);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_extracted() {
        let html = "<html><head><title>Hello &amp; World</title></head><body></body></html>";
        // The HTML parser decodes &amp; back to & when collecting text.
        assert_eq!(extract_title(html).as_deref(), Some("Hello & World"));
    }

    #[test]
    fn strips_scripts_and_nav() {
        let html = r#"
            <html><body>
              <nav id="primary-nav"><a href="/x">x</a></nav>
              <script>alert(1)</script>
              <main><p>Hello <b>world</b>.</p></main>
              <footer class="site-footer">legal</footer>
            </body></html>
        "#;
        let s = strip_noise(html);
        assert!(!s.contains("alert(1)"));
        assert!(!s.contains("primary-nav"));
        assert!(!s.contains("site-footer"));
        assert!(s.contains("Hello"));
        assert!(s.contains("world"));
    }

    #[test]
    fn extract_links_basic() {
        let html = r#"<a href="/a">A</a><a href="https://b.com">B</a><a href="">empty</a><a href="javascript:void(0)">js</a>"#;
        let links = extract_links(html);
        assert_eq!(links, vec!["/a".to_string(), "https://b.com".to_string()]);
    }
}
