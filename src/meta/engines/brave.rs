use std::sync::Arc;
use std::time::Duration;

use eggfetch_core::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;
use super::render::SerpRenderer;
use super::{PageMarker, ScrapeShape};

const ENGINE: &str = "brave";
const BRAVE_URL: &str = "https://search.brave.com/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// The captcha page is a script-rendered shell: its visible text lives in the
/// embedded i18n dictionary, which rendered-text matching deliberately skips.
const STATUS_CHALLENGE_MARKERS: &[&str] = &[
    "Verifying you're not a bot",
    "Switch to traditional captcha",
];

const SHAPE: ScrapeShape = ScrapeShape {
    result_container: "div[data-type='web']",
    empty_markers: &[
        PageMarker::Selector("#bad-results-info-banner"),
        PageMarker::Selector("div.fallback-snippet"),
        PageMarker::Text("Not many great matches came back for your search"),
    ],
    blocked_markers: &[
        PageMarker::Text("Verifying you're not a bot"),
        PageMarker::Text("Quick check before you continue searching."),
        PageMarker::Text("Switch to traditional CAPTCHA"),
        PageMarker::Text("Please verify you are a human"),
    ],
};

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
    renderer: Option<&Arc<dyn SerpRenderer>>,
) -> Result<Vec<SearchResult>, EngineError> {
    search_at(client, BRAVE_URL, query, max_results, timeout, renderer).await
}

pub(super) async fn search_at(
    client: &Client,
    base_url: &str,
    query: &str,
    max_results: usize,
    timeout: Duration,
    renderer: Option<&Arc<dyn SerpRenderer>>,
) -> Result<Vec<SearchResult>, EngineError> {
    let served = match fetch_html(client, base_url, query, timeout).await {
        Ok(body) => parse(&body, max_results),
        Err(error) => Err(error),
    };
    match served {
        Err(error) if super::render::is_browser_gated(&error) => match renderer {
            Some(renderer) => {
                let url = search_url(base_url, query);
                super::render::render_blocked(renderer, &url, timeout, error, max_results, parse)
                    .await
            }
            None => Err(error),
        },
        other => other,
    }
}

async fn fetch_html(
    client: &Client,
    base_url: &str,
    query: &str,
    timeout: Duration,
) -> Result<String, EngineError> {
    let resp = client
        .get(base_url)
        .map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?
        .query("q", query)
        .timeout(super::engine_timeout(timeout))
        .send()
        .await
        .map_err(|e| super::map_request_error(ENGINE, e))?;
    super::read_search_body(
        resp,
        ENGINE,
        &SHAPE,
        MAX_BODY_BYTES,
        STATUS_CHALLENGE_MARKERS,
    )
    .await
}

fn search_url(base_url: &str, query: &str) -> String {
    format!("{base_url}?q={}", urlencoding::encode(query))
}

pub(super) fn parse(html: &str, max_results: usize) -> Result<Vec<SearchResult>, EngineError> {
    let document = Html::parse_document(html);

    let result_sel = super::sel(ENGINE, SHAPE.result_container)?;
    let link_sel = super::sel(ENGINE, "a.l1")?;
    let title_sel = super::sel(ENGINE, "div.search-snippet-title")?;
    let snippet_sel = super::sel(ENGINE, "div.generic-snippet")?;
    let cite_sel = super::sel(ENGINE, "cite.snippet-url")?;

    let mut results = Vec::new();

    for element in document.select(&result_sel) {
        if results.len() >= max_results {
            break;
        }
        let Some(link_el) = element.select(&link_sel).next() else {
            continue;
        };

        let href = link_el.value().attr("href").unwrap_or("");
        let url = if super::is_http_url(href) {
            href.to_string()
        } else {
            element
                .select(&cite_sel)
                .next()
                .map(display_url)
                .unwrap_or_default()
        };
        if !super::is_http_url(&url) {
            continue;
        }

        let title = element
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let snippet = element
            .select(&snippet_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        results.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at: None,
            metadata: Default::default(),
        });
    }

    super::finalize_scrape(ENGINE, &document, &SHAPE, results)
}

/// Rebuild the destination from the URL the page displays.
///
/// A rendered Brave result links to an opaque `/go/v1#` token rather than to
/// the destination, and the displayed URL is the only readable statement of
/// where the result points. Breadcrumb segments are joined as path segments
/// while they stay URI-shaped, and the walk stops at the first human-readable
/// one, so a truncated or prose-like display degrades to its exact prefix
/// instead of inventing a path.
fn display_url(cite: scraper::ElementRef<'_>) -> String {
    let text = cite.text().collect::<String>();
    let mut url = String::new();
    for segment in text.split('›') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if url.is_empty() {
            url = if segment.starts_with("http") {
                segment.to_string()
            } else {
                format!("https://{segment}")
            };
            continue;
        }
        if !is_path_segment(segment) {
            break;
        }
        if !url.ends_with('/') {
            url.push('/');
        }
        url.push_str(segment);
    }
    url
}

fn is_path_segment(segment: &str) -> bool {
    segment.chars().all(|c| {
        c.is_ascii_graphic() && !matches!(c, '"' | '<' | '>' | '\\' | '^' | '`' | '{' | '|' | '}')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extracts_results() {
        let html = r#"
            <div class="snippet" data-type="web">
                <a class="l1" href="https://example.com">
                    <div class="search-snippet-title">Example Site</div>
                </a>
                <div class="generic-snippet">An example website for testing.</div>
            </div>
            <div class="snippet" data-type="web">
                <a class="l1" href="https://rust-lang.org">
                    <div class="search-snippet-title">Rust Language</div>
                </a>
                <div class="generic-snippet">Systems programming language.</div>
            </div>
        "#;

        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Example Site");
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(results[1].url, "https://rust-lang.org");
        assert!(results[0].snippet.is_some());
    }

    #[test]
    fn test_parse_respects_max_results() {
        let result_html = r#"
            <div class="snippet" data-type="web">
                <a class="l1" href="https://example.com">
                    <div class="search-snippet-title">T</div>
                </a>
            </div>
        "#;
        let html = result_html.repeat(5);
        let results = parse(&html, 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_skips_missing_snippet() {
        let html = r#"
            <div class="snippet" data-type="web">
                <a class="l1" href="https://example.com">
                    <div class="search-snippet-title">Title</div>
                </a>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.is_none());
    }

    #[test]
    fn rendered_result_with_an_opaque_link_uses_the_displayed_url() {
        let html = r#"
            <div data-type="web">
                <a class="l1" href="/go/v1#AbAyDuQ8">
                    <cite class="snippet-url">https://en.wikipedia.org <span>&rsaquo; wiki</span> <span>&rsaquo; Rust_(programming_language)</span></cite>
                    <div class="search-snippet-title">Rust</div>
                </a>
                <div class="generic-snippet">Systems language.</div>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(
            results[0].url,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        );
    }

    #[test]
    fn a_readable_breadcrumb_degrades_to_its_exact_prefix() {
        let html = r#"
            <div data-type="web">
                <a class="l1" href="/go/v1#x">
                    <cite class="snippet-url">https://coursera.org &rsaquo; coursera articles &rsaquo; data</cite>
                    <div class="search-snippet-title">Course</div>
                </a>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results[0].url, "https://coursera.org");
    }

    #[test]
    fn test_parse_skips_non_http_urls() {
        let html = r#"
            <div class="snippet" data-type="web">
                <a class="l1" href="/relative">
                    <div class="search-snippet-title">Relative</div>
                </a>
            </div>
            <div class="snippet" data-type="web">
                <a class="l1" href="https://valid.com">
                    <div class="search-snippet-title">Valid</div>
                </a>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }

    #[tokio::test]
    async fn challenge_served_with_429_is_blocked_not_a_status_failure() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/search").query_param("q", "rust");
            then.status(429)
                .header("content-type", "text/html")
                .body(
                    r#"<html><head><script>const dict = {"Verifying you're not a bot":"Verifying you're not a bot"};</script></head><body><div id="app"></div></body></html>"#,
                );
        });
        let client = crate::meta::engines::build_http_client(None).expect("test client");

        let error = search_at(
            &client,
            &server.url("/search"),
            "rust",
            10,
            Duration::from_secs(5),
            None,
        )
        .await
        .expect_err("a challenge page must not parse as results");
        mock.assert();
        match error {
            EngineError::Blocked { reason, .. } => {
                assert!(reason.contains("challenge page detected"), "{reason}");
                assert!(reason.contains("HTTP 429"), "{reason}");
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_500_stays_a_status_failure() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search").query_param("q", "rust");
            then.status(500);
        });
        let client = crate::meta::engines::build_http_client(None).expect("test client");

        let error = search_at(
            &client,
            &server.url("/search"),
            "rust",
            10,
            Duration::from_secs(5),
            None,
        )
        .await
        .expect_err("an empty 500 must not parse as results");
        match error {
            EngineError::BadStatus { status, .. } => assert_eq!(status, 500),
            other => panic!("expected BadStatus, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_challenge_is_served_by_the_browser_when_one_is_installed() {
        use crate::meta::engines::render::test_support::FixtureRenderer;
        use crate::meta::engines::render::SerpRenderer;
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search").query_param("q", "rust");
            then.status(429).header("content-type", "text/html").body(
                r#"<html><body><script>const d = {"Verifying you're not a bot":"Verifying you're not a bot"};</script></body></html>"#,
            );
        });
        let client = crate::meta::engines::build_http_client(None).expect("test client");
        let renderer = FixtureRenderer::serving(
            r#"<div data-type="web"><a class="l1" href="https://example.com"><div class="search-snippet-title">Rendered</div></a><div class="generic-snippet">Served by the browser.</div></div>"#,
        );
        let installed: std::sync::Arc<dyn SerpRenderer> = renderer.clone();

        let results = search_at(
            &client,
            &server.url("/search"),
            "rust",
            10,
            Duration::from_secs(5),
            Some(&installed),
        )
        .await
        .expect("the rendered page parses");

        assert_eq!(renderer.calls(), 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://example.com");
    }

    #[tokio::test]
    async fn a_render_that_cannot_run_keeps_the_challenge_reason() {
        use crate::meta::engines::render::test_support::FixtureRenderer;
        use crate::meta::engines::render::SerpRenderer;
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search").query_param("q", "rust");
            then.status(429).header("content-type", "text/html").body(
                r#"<html><body><script>const d = {"Verifying you're not a bot":"x"};</script></body></html>"#,
            );
        });
        let client = crate::meta::engines::build_http_client(None).expect("test client");
        let renderer = FixtureRenderer::unavailable();
        let installed: std::sync::Arc<dyn SerpRenderer> = renderer.clone();

        let error = search_at(
            &client,
            &server.url("/search"),
            "rust",
            10,
            Duration::from_secs(5),
            Some(&installed),
        )
        .await
        .expect_err("an unavailable browser must not invent results");

        assert_eq!(renderer.calls(), 1);
        match error {
            EngineError::Blocked { reason, .. } => {
                assert!(reason.contains("challenge page detected"), "{reason}");
                assert!(reason.contains("browser render unavailable"), "{reason}");
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_served_page_never_consults_the_browser() {
        use crate::meta::engines::render::test_support::FixtureRenderer;
        use crate::meta::engines::render::SerpRenderer;
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search").query_param("q", "rust");
            then.status(200).header("content-type", "text/html").body(
                r#"<div data-type="web"><a class="l1" href="https://http.example"><div class="search-snippet-title">Http</div></a></div>"#,
            );
        });
        let client = crate::meta::engines::build_http_client(None).expect("test client");
        let renderer = FixtureRenderer::serving(
            r#"<div data-type="web"><a class="l1" href="https://browser.example"><div class="search-snippet-title">Browser</div></a></div>"#,
        );
        let installed: std::sync::Arc<dyn SerpRenderer> = renderer.clone();

        let results = search_at(
            &client,
            &server.url("/search"),
            "rust",
            10,
            Duration::from_secs(5),
            Some(&installed),
        )
        .await
        .expect("the HTTP page parses");

        assert_eq!(renderer.calls(), 0);
        assert_eq!(results[0].url, "https://http.example");
    }
}
