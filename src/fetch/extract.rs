//! HTML content extraction.

use scraper::{Html, Selector};

use crate::core::fetch::ExtractedLink;

/// HTML content extractor.
pub struct HtmlExtractor<'a> {
    html: &'a [u8],
    base_url: &'a str,
}

impl<'a> HtmlExtractor<'a> {
    /// Creates a new HtmlExtractor.
    pub fn new(html: &'a [u8], base_url: &'a str) -> Self {
        Self { html, base_url }
    }

    /// Extracts content from the HTML.
    ///
    /// Returns a tuple of (title, description, body_text, links).
    pub fn extract(
        &self,
        max_chars: usize,
        include_links: bool,
    ) -> (Option<String>, Option<String>, String, Vec<ExtractedLink>) {
        let html_str = std::str::from_utf8(self.html).unwrap_or("");
        let document = Html::parse_document(html_str);

        let title = Selector::parse("title")
            .ok()
            .and_then(|sel| document.select(&sel).next())
            .and_then(|el| el.text().next())
            .map(|s| s.trim().to_string());

        let description = Selector::parse(r#"meta[name="description"]"#)
            .ok()
            .and_then(|sel| document.select(&sel).next())
            .and_then(|el| el.value().attr("content"))
            .map(|s| s.trim().to_string());

        let body_text = Selector::parse("body")
            .ok()
            .and_then(|sel| document.select(&sel).next())
            .map(|body_el| {
                let mut text = String::new();
                extract_text_recursive(&body_el, &mut text);
                text
            })
            .unwrap_or_else(|| document.root_element().text().collect::<String>());

        let normalized: String = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
        let truncated_text: String = normalized.chars().take(max_chars).collect();
        let _was_truncated = normalized.chars().count() > max_chars;

        let links = if include_links {
            extract_links(&document, self.base_url)
        } else {
            Vec::new()
        };

        (title, description, truncated_text, links)
    }
}

fn extract_text_recursive(element: &scraper::ElementRef, out: &mut String) {
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            let s = text.trim();
            if !s.is_empty() {
                out.push_str(s);
                out.push(' ');
            }
        } else if let Some(elem) = child.value().as_element() {
            let tag_name = elem.name();
            let is_block = matches!(
                tag_name,
                "p" | "div" | "br" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            );
            if is_block {
                out.push(' ');
            }
            if let Some(child_elem) = scraper::ElementRef::wrap(child) {
                extract_text_recursive(&child_elem, out);
            }
            if is_block {
                out.push(' ');
            }
        }
    }
}

fn extract_links(document: &scraper::Html, base_url: &str) -> Vec<ExtractedLink> {
    use url::Url;

    let selector = Selector::parse("a[href]").ok();
    let base = Url::parse(base_url).ok();

    selector
        .map(|sel| {
            document
                .select(&sel)
                .filter_map(|el| {
                    let href = el.value().attr("href")?;
                    let text = el.text().collect::<String>().trim().to_string();
                    let resolved = base
                        .as_ref()
                        .and_then(|b| b.join(href).ok())
                        .map(|u| u.to_string());
                    resolved.map(|url| ExtractedLink { text, url })
                })
                .take(100)
                .collect()
        })
        .unwrap_or_default()
}

/// Extracts content from HTML bytes.
///
/// Returns a tuple of (title, description, body_text, links).
pub fn extract_content(
    html: &[u8],
    base_url: &str,
    max_chars: usize,
    include_links: bool,
) -> (Option<String>, Option<String>, String, Vec<ExtractedLink>) {
    let extractor = HtmlExtractor::new(html, base_url);
    extractor.extract(max_chars, include_links)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_title_extraction() {
        let html =
            b"<!DOCTYPE html><html><head><title>Test Page</title></head><body></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (title, _, _, _) = extractor.extract(1000, false);
        assert_eq!(title, Some("Test Page".to_string()));
    }

    #[test]
    fn html_meta_description_extraction() {
        let html = b"<!DOCTYPE html><html><head><meta name=\"description\" content=\"Page description\"></head><body></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, desc, _, _) = extractor.extract(1000, false);
        assert_eq!(desc, Some("Page description".to_string()));
    }

    #[test]
    fn html_truncation() {
        let html = b"<!DOCTYPE html><html><body><p>a b c d e f g h i j k l m n o p q r s t u v w x y z</p></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _) = extractor.extract(10, false);
        assert!(text.chars().count() <= 10);
    }

    #[test]
    fn html_relative_link_resolution() {
        let html = b"<!DOCTYPE html><html><body><a href=\"/path\">Link</a></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/base/");
        let (_, _, _, links) = extractor.extract(1000, true);
        assert!(!links.is_empty());
        assert_eq!(links[0].url, "https://example.com/path");
    }

    #[test]
    fn fetch_response_warning_present() {
        use crate::core::fetch::WebFetchResponse;
        let warning = WebFetchResponse::untrusted_warning();
        assert!(warning.contains("external_untrusted"));
        assert!(warning.contains("data"));
    }
}
