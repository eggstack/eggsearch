//! HTML content extraction.

use std::borrow::Cow;

use scraper::{Html, Selector};

use crate::core::fetch::ExtractedLink;

/// Maximum number of links the extractor will collect from a single
/// page. A defensive upper bound to keep response payloads bounded
/// even for link-heavy pages.
pub const MAX_LINKS: usize = 100;

/// Non-UTF-8 warning string. Prepended to `WebFetchResponse.warnings`
/// when the response body cannot be decoded as UTF-8; the extractor
/// falls back to a lossy decode so partial text is still returned.
pub const NON_UTF8_WARNING: &str = "body is not valid UTF-8; extraction may be incomplete";

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
    /// Returns a tuple of (title, description, body_text, links,
    /// warnings, text_truncated). The `warnings` vec is empty unless
    /// a non-fatal condition (e.g. non-UTF-8 body) was encountered.
    /// `text_truncated` is `true` when the extracted text exceeded
    /// `max_chars` and was clamped.
    pub fn extract(
        &self,
        max_chars: usize,
        include_links: bool,
    ) -> (
        Option<String>,
        Option<String>,
        String,
        Vec<ExtractedLink>,
        Vec<String>,
        bool,
    ) {
        let (html_str, warnings) = match std::str::from_utf8(self.html) {
            Ok(s) => (Cow::Borrowed(s), Vec::new()),
            Err(_) => {
                tracing::warn!("web_fetch body is not valid UTF-8; falling back to lossy decode");
                (
                    Cow::Owned(String::from_utf8_lossy(self.html).into_owned()),
                    vec![NON_UTF8_WARNING.to_string()],
                )
            }
        };
        let document = Html::parse_document(html_str.as_ref());

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
        let text_truncated = normalized.chars().count() > max_chars;
        let truncated_text: String = normalized.chars().take(max_chars).collect();

        let links = if include_links {
            extract_links(&document, self.base_url)
        } else {
            Vec::new()
        };

        (
            title,
            description,
            truncated_text,
            links,
            warnings,
            text_truncated,
        )
    }
}

const STRIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "svg", "nav", "footer", "header", "form", "aside",
];

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
            if STRIP_TAGS.contains(&tag_name) {
                continue;
            }
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
                .take(MAX_LINKS)
                .collect()
        })
        .unwrap_or_default()
}

/// Extracts content from HTML bytes.
///
/// Returns a tuple of (title, description, body_text, links, warnings,
/// text_truncated).
pub fn extract_content(
    html: &[u8],
    base_url: &str,
    max_chars: usize,
    include_links: bool,
) -> (
    Option<String>,
    Option<String>,
    String,
    Vec<ExtractedLink>,
    Vec<String>,
    bool,
) {
    let extractor = HtmlExtractor::new(html, base_url);
    extractor.extract(max_chars, include_links)
}

/// Extracts links from HTML bytes.
///
/// Parses the HTML and extracts all `<a href>` links, resolving
/// relative URLs against `base_url`.
pub fn extract_links_from_html(html: &[u8], base_url: &str) -> Vec<ExtractedLink> {
    let html_str = match std::str::from_utf8(html) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(String::from_utf8_lossy(html).into_owned()),
    };
    let document = Html::parse_document(html_str.as_ref());
    extract_links(&document, base_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_title_extraction() {
        let html =
            b"<!DOCTYPE html><html><head><title>Test Page</title></head><body></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (title, _, _, _, _, _) = extractor.extract(1000, false);
        assert_eq!(title, Some("Test Page".to_string()));
    }

    #[test]
    fn html_meta_description_extraction() {
        let html = b"<!DOCTYPE html><html><head><meta name=\"description\" content=\"Page description\"></head><body></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, desc, _, _, _, _) = extractor.extract(1000, false);
        assert_eq!(desc, Some("Page description".to_string()));
    }

    #[test]
    fn html_truncation() {
        let html = b"<!DOCTYPE html><html><body><p>a b c d e f g h i j k l m n o p q r s t u v w x y z</p></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _, _, truncated) = extractor.extract(10, false);
        assert!(text.chars().count() <= 10);
        assert!(truncated);
    }

    #[test]
    fn html_no_truncation_when_within_limit() {
        let html = b"<!DOCTYPE html><html><body><p>short</p></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _, _, truncated) = extractor.extract(1000, false);
        assert!(!truncated);
        assert!(text.contains("short"));
    }

    #[test]
    fn html_relative_link_resolution() {
        let html = b"<!DOCTYPE html><html><body><a href=\"/path\">Link</a></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/base/");
        let (_, _, _, links, _, _) = extractor.extract(1000, true);
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

    #[test]
    fn html_strips_script_and_style() {
        let html = b"<!DOCTYPE html><html><body>\
            <p>visible</p>\
            <script>alert('evil');</script>\
            <style>body{color:red}</style>\
            <p>after</p>\
        </body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _, _, _) = extractor.extract(1000, false);
        assert!(text.contains("visible"), "got: {text:?}");
        assert!(text.contains("after"), "got: {text:?}");
        assert!(!text.contains("alert"), "script content leaked: {text:?}");
        assert!(
            !text.contains("color:red"),
            "style content leaked: {text:?}"
        );
        assert!(!text.contains("body{"), "css leaked: {text:?}");
    }

    #[test]
    fn html_strips_nav_footer_header_aside() {
        let html = b"<!DOCTYPE html><html><body>\
            <header>top chrome</header>\
            <nav>nav links</nav>\
            <main><p>main content</p></main>\
            <aside>sidebar</aside>\
            <footer>bottom chrome</footer>\
        </body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _, _, _) = extractor.extract(1000, false);
        assert!(text.contains("main content"), "got: {text:?}");
        assert!(!text.contains("top chrome"), "header leaked: {text:?}");
        assert!(!text.contains("nav links"), "nav leaked: {text:?}");
        assert!(!text.contains("sidebar"), "aside leaked: {text:?}");
        assert!(!text.contains("bottom chrome"), "footer leaked: {text:?}");
    }

    #[test]
    fn html_strips_noscript_and_svg() {
        let html = b"<!DOCTYPE html><html><body>\
            <p>before</p>\
            <noscript>enable js</noscript>\
            <svg><text>x</text></svg>\
            <p>after</p>\
        </body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _, _, _) = extractor.extract(1000, false);
        assert!(text.contains("before"), "got: {text:?}");
        assert!(text.contains("after"), "got: {text:?}");
        assert!(!text.contains("enable js"), "noscript leaked: {text:?}");
        assert!(!text.contains("svg"), "svg leaked: {text:?}");
    }

    #[test]
    fn non_utf8_body_emits_warning_and_decodes_lossy() {
        // Valid HTML wrapping with invalid UTF-8 bytes in the middle.
        // The lossy decoder should turn 0xFF 0xFE into U+FFFD
        // replacement characters, and the surrounding text should
        // still be extractable.
        let html: &[u8] = b"<html><body><p>before</p>\xff\xfe<p>after</p></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (title, _, text, _, warnings, _) = extractor.extract(1000, false);
        assert!(
            warnings.iter().any(|w| w == NON_UTF8_WARNING),
            "expected non-UTF-8 warning, got: {warnings:?}"
        );
        // Surrounding text should still be extractable despite the
        // invalid bytes.
        assert!(text.contains("before"), "got: {text:?}");
        assert!(text.contains("after"), "got: {text:?}");
        assert!(title.is_none());
    }

    #[test]
    fn valid_utf8_body_has_no_warnings() {
        let html = b"<!DOCTYPE html><html><body><p>hello</p></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, _, _, warnings, _) = extractor.extract(1000, false);
        assert!(
            warnings.is_empty(),
            "expected no warnings, got: {warnings:?}"
        );
    }

    #[test]
    fn max_links_constant_is_reasonable() {
        // Sanity check the constant is set to a reasonable value.
        const {
            assert!(MAX_LINKS >= 1);
            assert!(MAX_LINKS <= 1000);
        }
    }
}
