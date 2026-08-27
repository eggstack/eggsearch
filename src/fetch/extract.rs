//! HTML content extraction.

use std::borrow::Cow;

use scraper::{Html, Selector};

use crate::core::fetch::{ExtractedLink, LinkKind};
use crate::core::sanitize::{bound_text, normalize_whitespace, SNIPPET_MAX_CHARS};

/// Maximum number of links the extractor will collect from a single
/// page. A defensive upper bound to keep response payloads bounded
/// even for link-heavy pages.
pub const MAX_LINKS: usize = 100;

/// Non-UTF-8 warning string. Prepended to `WebFetchResponse.warnings`
/// when the response body cannot be decoded as UTF-8; the extractor
/// falls back to a lossy decode so partial text is still returned.
pub const NON_UTF8_WARNING: &str = "body is not valid UTF-8; extraction may be incomplete";

/// Result of link extraction, including the links and metadata about
/// the extraction process.
#[derive(Clone, Debug)]
pub struct LinkExtractionResult {
    /// Extracted and classified links.
    pub links: Vec<ExtractedLink>,
    /// Total number of `<a href>` links encountered in the HTML.
    pub total_seen: usize,
    /// Whether the link list was truncated at `MAX_LINKS`.
    pub truncated: bool,
}

/// Classifies a link based on URL heuristics relative to the page URL.
fn classify_link(page_url: &url::Url, link_url: &url::Url) -> LinkKind {
    let page_host = page_url.host_str().unwrap_or("");
    let link_host = link_url.host_str().unwrap_or("");

    // Same-page anchor: same host + path + query, only fragment differs.
    if page_host == link_host
        && page_url.path() == link_url.path()
        && page_url.query() == link_url.query()
    {
        return LinkKind::SamePageAnchor;
    }

    let path = link_url.path().to_lowercase();

    // PDF
    if path.ends_with(".pdf") {
        return LinkKind::Pdf;
    }

    // Image
    if path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".gif")
        || path.ends_with(".svg")
        || path.ends_with(".webp")
        || path.ends_with(".ico")
    {
        return LinkKind::Image;
    }

    // SourceCode
    if path.ends_with(".rs")
        || path.ends_with(".py")
        || path.ends_with(".js")
        || path.ends_with(".ts")
        || path.ends_with(".jsx")
        || path.ends_with(".tsx")
        || path.ends_with(".go")
        || path.ends_with(".c")
        || path.ends_with(".cpp")
        || path.ends_with(".h")
        || path.ends_with(".java")
        || path.ends_with(".rb")
        || path.ends_with(".json")
        || path.ends_with(".toml")
        || path.ends_with(".yaml")
        || path.ends_with(".yml")
        || path.ends_with(".xml")
        || path.ends_with(".css")
        || path.ends_with(".scss")
        || path.ends_with(".sh")
        || path.ends_with(".bash")
    {
        return LinkKind::SourceCode;
    }

    // Download
    if path.ends_with(".zip")
        || path.ends_with(".tar")
        || path.ends_with(".gz")
        || path.ends_with(".bz2")
        || path.ends_with(".xz")
        || path.ends_with(".7z")
        || path.ends_with(".rar")
        || path.ends_with(".exe")
        || path.ends_with(".dmg")
        || path.ends_with(".msi")
        || path.ends_with(".deb")
        || path.ends_with(".rpm")
        || path.ends_with(".apk")
        || path.ends_with(".war")
        || path.ends_with(".jar")
    {
        return LinkKind::Download;
    }

    // Feed
    if path.ends_with(".rss")
        || path.ends_with(".atom")
        || path.ends_with("/feed")
        || path.ends_with("/rss")
    {
        return LinkKind::Feed;
    }

    let is_gh_or_gl = is_github_or_gitlab(link_host);

    // Issue
    if is_gh_or_gl && path.contains("/issues/") {
        return LinkKind::Issue;
    }

    // PullRequest
    if is_gh_or_gl && (path.contains("/pull/") || path.contains("/merge_requests/")) {
        return LinkKind::PullRequest;
    }

    // Release
    if is_gh_or_gl && (path.contains("/releases/") || path.contains("/tags/")) {
        return LinkKind::Release;
    }

    // SecurityAdvisory
    if path.contains("/advisories/") || path.contains("/security/") || path.contains("/ghsa/") {
        return LinkKind::SecurityAdvisory;
    }

    // Documentation / ApiReference (check hosts first)
    if is_docs_host(link_host) {
        if path.contains("/api/") || path.contains("/api-reference/") {
            return LinkKind::ApiReference;
        }
        return LinkKind::Documentation;
    }

    // Documentation by path
    if is_docs_path(&path) {
        return LinkKind::Documentation;
    }

    // SameDomain / External
    if page_host == link_host {
        LinkKind::SameDomain
    } else {
        LinkKind::External
    }
}

/// Returns `true` if the host is a GitHub or GitLab domain.
fn is_github_or_gitlab(host: &str) -> bool {
    host == "github.com" || host == "gitlab.com"
}

/// Returns `true` if the host is a well-known documentation host.
fn is_docs_host(host: &str) -> bool {
    host == "readthedocs.io"
        || host.ends_with(".readthedocs.io")
        || host == "docs.rs"
        || host == "doc.rust-lang.org"
        || host == "docs.python.org"
        || host == "developer.mozilla.org"
        || host == "pkg.go.dev"
        || host == "docs.npmjs.com"
        || host == "docs.docker.com"
        || host == "docs.github.com"
}

/// Returns `true` if the path contains a documentation-like segment.
fn is_docs_path(path: &str) -> bool {
    path.contains("/docs/")
        || path.contains("/documentation/")
        || path.contains("/guide/")
        || path.contains("/manual/")
        || path.contains("/reference/")
}

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
    /// warnings, text_truncated, links_seen, links_truncated).
    /// The `warnings` vec is empty unless a non-fatal condition
    /// (e.g. non-UTF-8 body) was encountered.
    /// `text_truncated` is `true` when the extracted text exceeded
    /// `max_chars` and was clamped.
    #[allow(clippy::type_complexity)]
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
        usize,
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
                extract_text_recursive(&body_el, &mut text, 0);
                text
            })
            .unwrap_or_else(|| document.root_element().text().collect::<String>());

        let normalized = normalize_html_whitespace(&body_text);
        let text_truncated = normalized.chars().count() > max_chars;
        let truncated_text: String = normalized.chars().take(max_chars).collect();

        let (links, links_seen, links_truncated) = if include_links {
            let result = extract_links(&document, self.base_url);
            (result.links, result.total_seen, result.truncated)
        } else {
            (Vec::new(), 0, false)
        };

        (
            title,
            description,
            truncated_text,
            links,
            warnings,
            text_truncated,
            links_seen,
            links_truncated,
        )
    }
}

const STRIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "svg", "nav", "footer", "header", "form", "aside",
];

fn extract_text_recursive(element: &scraper::ElementRef, out: &mut String, depth: usize) {
    if depth >= super::MAX_TREE_WALK_DEPTH {
        return;
    }
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
            if tag_name == "br" {
                out.push('\n');
                continue;
            }
            let is_block = matches!(
                tag_name,
                "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            );
            if is_block {
                out.push(' ');
            }
            if let Some(child_elem) = scraper::ElementRef::wrap(child) {
                extract_text_recursive(&child_elem, out, depth + 1);
            }
            if is_block {
                out.push(' ');
            }
        }
    }
}

fn normalize_html_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.split('\n') {
        let trimmed = normalize_whitespace(line);
        if !trimmed.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&trimmed);
        }
    }
    out
}

fn extract_links(document: &scraper::Html, base_url: &str) -> LinkExtractionResult {
    use url::Url;

    let selector = Selector::parse("a[href]").ok();
    let base = Url::parse(base_url).ok();

    let mut total_seen: usize = 0;
    let mut links: Vec<ExtractedLink> = Vec::new();

    if let Some(sel) = selector {
        for el in document.select(&sel) {
            total_seen += 1;
            if links.len() >= MAX_LINKS {
                continue;
            }
            let href = match el.value().attr("href") {
                Some(h) => h,
                None => continue,
            };
            let text = el.text().collect::<String>().trim().to_string();
            let rel = el
                .value()
                .attr("rel")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let page_url = match base.as_ref() {
                Some(u) => u,
                None => continue,
            };
            let url = match page_url.join(href) {
                Ok(u) => u,
                Err(_) => continue,
            };
            let url_string = url.to_string();
            if url_string.chars().count() > SNIPPET_MAX_CHARS {
                continue;
            }
            let (text, _) = bound_text(&text, SNIPPET_MAX_CHARS);
            let same_domain = Some(page_url.host_str() == url.host_str());
            let link_kind = classify_link(page_url, &url);
            links.push(ExtractedLink {
                text,
                url: url_string,
                link_kind,
                rel,
                same_domain,
            });
        }
    }

    LinkExtractionResult {
        links,
        total_seen,
        truncated: total_seen > MAX_LINKS,
    }
}

/// Extracts content from HTML bytes.
///
/// Returns a tuple of (title, description, body_text, links, warnings,
/// text_truncated, links_seen, links_truncated).
#[allow(clippy::type_complexity)]
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
    usize,
    bool,
) {
    let extractor = HtmlExtractor::new(html, base_url);
    extractor.extract(max_chars, include_links)
}

/// Extracts links from HTML bytes.
///
/// Parses the HTML and extracts all `<a href>` links, resolving
/// relative URLs against `base_url`.
pub fn extract_links_from_html(html: &[u8], base_url: &str) -> LinkExtractionResult {
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
        let (title, _, _, _, _, _, _, _) = extractor.extract(1000, false);
        assert_eq!(title, Some("Test Page".to_string()));
    }

    #[test]
    fn html_meta_description_extraction() {
        let html = b"<!DOCTYPE html><html><head><meta name=\"description\" content=\"Page description\"></head><body></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, desc, _, _, _, _, _, _) = extractor.extract(1000, false);
        assert_eq!(desc, Some("Page description".to_string()));
    }

    #[test]
    fn html_truncation() {
        let html = b"<!DOCTYPE html><html><body><p>a b c d e f g h i j k l m n o p q r s t u v w x y z</p></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _, _, truncated, _, _) = extractor.extract(10, false);
        assert!(text.chars().count() <= 10);
        assert!(truncated);
    }

    #[test]
    fn html_no_truncation_when_within_limit() {
        let html = b"<!DOCTYPE html><html><body><p>short</p></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _, _, truncated, _, _) = extractor.extract(1000, false);
        assert!(!truncated);
        assert!(text.contains("short"));
    }

    #[test]
    fn html_break_preserves_line_break() {
        let html = b"<!DOCTYPE html><html><body><p>before<br>after</p></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, text, _, _, _, _, _) = extractor.extract(1000, false);
        assert_eq!(text, "before\nafter");
    }

    #[test]
    fn html_relative_link_resolution() {
        let html = b"<!DOCTYPE html><html><body><a href=\"/path\">Link</a></body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/base/");
        let (_, _, _, links, _, _, _, _) = extractor.extract(1000, true);
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
        let (_, _, text, _, _, _, _, _) = extractor.extract(1000, false);
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
        let (_, _, text, _, _, _, _, _) = extractor.extract(1000, false);
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
        let (_, _, text, _, _, _, _, _) = extractor.extract(1000, false);
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
        let (title, _, text, _, warnings, _, _, _) = extractor.extract(1000, false);
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
        let (_, _, _, _, warnings, _, _, _) = extractor.extract(1000, false);
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

    #[test]
    fn classify_same_page_anchor() {
        let page = url::Url::parse("https://example.com/page?q=1#section").unwrap();
        let link = url::Url::parse("https://example.com/page?q=1#other").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::SamePageAnchor);
    }

    #[test]
    fn classify_pdf_extension() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://example.com/doc.pdf").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::Pdf);
    }

    #[test]
    fn classify_image_extensions() {
        let page = url::Url::parse("https://example.com/").unwrap();
        for ext in &[".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".ico"] {
            let url = format!("https://example.com/photo{ext}");
            let link = url::Url::parse(&url).unwrap();
            assert_eq!(classify_link(&page, &link), LinkKind::Image);
        }
    }

    #[test]
    fn classify_source_code_extensions() {
        let page = url::Url::parse("https://example.com/").unwrap();
        for ext in &[
            ".rs", ".py", ".js", ".ts", ".go", ".c", ".json", ".toml", ".yaml",
        ] {
            let url = format!("https://example.com/file{ext}");
            let link = url::Url::parse(&url).unwrap();
            assert_eq!(classify_link(&page, &link), LinkKind::SourceCode);
        }
    }

    #[test]
    fn classify_download_extensions() {
        let page = url::Url::parse("https://example.com/").unwrap();
        for ext in &[".zip", ".tar", ".gz", ".exe", ".dmg", ".deb", ".rpm"] {
            let url = format!("https://example.com/archive{ext}");
            let link = url::Url::parse(&url).unwrap();
            assert_eq!(classify_link(&page, &link), LinkKind::Download);
        }
    }

    #[test]
    fn classify_feed_extensions() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let _rss = url::Url::parse("https://example.com/feed.xml").unwrap();
        // .rss extension
        let rss_ext = url::Url::parse("https://example.com/blog.rss").unwrap();
        let _atom_ext = url::Url::parse("https://example.com/atom.xml").unwrap();
        // /feed path
        let feed_path = url::Url::parse("https://example.com/feed").unwrap();
        let rss_path = url::Url::parse("https://example.com/rss").unwrap();
        assert_eq!(classify_link(&page, &rss_ext), LinkKind::Feed);
        assert_eq!(classify_link(&page, &feed_path), LinkKind::Feed);
        assert_eq!(classify_link(&page, &rss_path), LinkKind::Feed);
    }

    #[test]
    fn classify_github_issue() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://github.com/rust-lang/rust/issues/12345").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::Issue);
    }

    #[test]
    fn classify_gitlab_merge_request() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://gitlab.com/group/project/merge_requests/42").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::PullRequest);
    }

    #[test]
    fn classify_github_pull_request() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://github.com/rust-lang/rust/pull/99999").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::PullRequest);
    }

    #[test]
    fn classify_github_release() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://github.com/rust-lang/rust/releases/tag/1.75").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::Release);
    }

    #[test]
    fn classify_security_advisory() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let ghsa = url::Url::parse("https://github.com/advisories/GHSA-xxxx").unwrap();
        let security = url::Url::parse("https://example.com/security/cve-2024-1234").unwrap();
        let advisories = url::Url::parse("https://example.com/advisories/123").unwrap();
        assert_eq!(classify_link(&page, &ghsa), LinkKind::SecurityAdvisory);
        assert_eq!(classify_link(&page, &security), LinkKind::SecurityAdvisory);
        assert_eq!(
            classify_link(&page, &advisories),
            LinkKind::SecurityAdvisory
        );
    }

    #[test]
    fn classify_documentation_by_host() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let rtd = url::Url::parse("https://myproject.readthedocs.io/en/latest/").unwrap();
        let docs_rs = url::Url::parse("https://docs.rs/serde/latest/serde/").unwrap();
        let mdn = url::Url::parse("https://developer.mozilla.org/en-US/docs/Web").unwrap();
        assert_eq!(classify_link(&page, &rtd), LinkKind::Documentation);
        assert_eq!(classify_link(&page, &docs_rs), LinkKind::Documentation);
        assert_eq!(classify_link(&page, &mdn), LinkKind::Documentation);
    }

    #[test]
    fn classify_api_reference_by_host() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link =
            url::Url::parse("https://docs.rs/serde/latest/serde/struct.Serializer.html").unwrap();
        // docs.rs is a docs host but doesn't have /api/ in path, so Documentation.
        assert_eq!(classify_link(&page, &link), LinkKind::Documentation);
        let api_link =
            url::Url::parse("https://docs.rs/crate/serde/latest/serde/api/struct.Foo.html")
                .unwrap();
        assert_eq!(classify_link(&page, &api_link), LinkKind::ApiReference);
    }

    #[test]
    fn classify_documentation_by_path() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://example.com/docs/getting-started").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::Documentation);
        let guide = url::Url::parse("https://example.com/guide/intro").unwrap();
        assert_eq!(classify_link(&page, &guide), LinkKind::Documentation);
    }

    #[test]
    fn classify_same_domain() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://example.com/about").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::SameDomain);
    }

    #[test]
    fn classify_external() {
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://other.com/page").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::External);
    }

    #[test]
    fn classify_link_populates_same_domain_field() {
        let html = b"<!DOCTYPE html><html><body>\
            <a href=\"/same\">same</a>\
            <a href=\"https://other.com/\">other</a>\
        </body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, _, links, _, _, _, _) = extractor.extract(1000, true);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].same_domain, Some(true));
        assert_eq!(links[0].link_kind, LinkKind::SameDomain);
        assert_eq!(links[1].same_domain, Some(false));
        assert_eq!(links[1].link_kind, LinkKind::External);
    }

    #[test]
    fn classify_link_populates_rel_field() {
        let html = b"<!DOCTYPE html><html><body>\
            <a href=\"/page\" rel=\"nofollow\">link</a>\
            <a href=\"/other\">plain</a>\
        </body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, _, links, _, _, _, _) = extractor.extract(1000, true);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].rel, Some("nofollow".to_string()));
        assert_eq!(links[1].rel, None);
    }

    #[test]
    fn extract_links_returns_total_seen_and_truncated() {
        // Build HTML with exactly 3 links.
        let html = b"<!DOCTYPE html><html><body>\
            <a href=\"/a\">a</a>\
            <a href=\"/b\">b</a>\
            <a href=\"/c\">c</a>\
        </body></html>";
        let extractor = HtmlExtractor::new(html, "https://example.com/");
        let (_, _, _, _, _, _, links_seen, links_truncated) = extractor.extract(1000, true);
        assert_eq!(links_seen, 3);
        assert!(!links_truncated);
    }

    #[test]
    fn extracted_links_bound_text_and_reject_oversized_urls() {
        let long_text = "x".repeat(SNIPPET_MAX_CHARS + 1);
        let html = format!(
            "<html><body><a href=\"/ok\">{long_text}</a><a href=\"https://example.com/{}\">ok</a></body></html>",
            "x".repeat(SNIPPET_MAX_CHARS)
        );
        let result = extract_links_from_html(html.as_bytes(), "https://example.com/");

        assert_eq!(result.total_seen, 2);
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.links[0].text.chars().count(), SNIPPET_MAX_CHARS);
        assert!(result.links[0].text.ends_with('…'));
    }

    #[test]
    fn classify_github_not_issues_is_same_domain() {
        // A GitHub link that is NOT /issues/ should not be Issue.
        let page = url::Url::parse("https://example.com/").unwrap();
        let link =
            url::Url::parse("https://github.com/rust-lang/rust/blob/main/src/main.rs").unwrap();
        // /blob/ is not /issues/, /pull/, /releases/, etc.
        assert_eq!(classify_link(&page, &link), LinkKind::SourceCode);
    }

    #[test]
    fn classify_non_github_issues_is_not_issue_kind() {
        // Issues path on a non-GitHub/GitLab host should not be classified as Issue.
        let page = url::Url::parse("https://example.com/").unwrap();
        let link = url::Url::parse("https://example.com/issues/123").unwrap();
        assert_eq!(classify_link(&page, &link), LinkKind::SameDomain);
    }
}
