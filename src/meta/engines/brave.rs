use std::time::Duration;

use eggfetch_core::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;
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
) -> Result<Vec<SearchResult>, EngineError> {
    search_at(client, BRAVE_URL, query, max_results, timeout).await
}

pub(super) async fn search_at(
    client: &Client,
    base_url: &str,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
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
    let body = super::read_search_body(
        resp,
        ENGINE,
        &SHAPE,
        MAX_BODY_BYTES,
        STATUS_CHALLENGE_MARKERS,
    )
    .await?;

    parse(&body, max_results)
}

pub(super) fn parse(html: &str, max_results: usize) -> Result<Vec<SearchResult>, EngineError> {
    let document = Html::parse_document(html);

    let result_sel = super::sel(ENGINE, SHAPE.result_container)?;
    let link_sel = super::sel(ENGINE, "a.l1")?;
    let title_sel = super::sel(ENGINE, "div.search-snippet-title")?;
    let snippet_sel = super::sel(ENGINE, "div.generic-snippet")?;

    let mut results = Vec::new();

    for element in document.select(&result_sel) {
        if results.len() >= max_results {
            break;
        }
        let Some(link_el) = element.select(&link_sel).next() else {
            continue;
        };

        let url = link_el.value().attr("href").unwrap_or("").to_string();
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
        )
        .await
        .expect_err("an empty 500 must not parse as results");
        match error {
            EngineError::BadStatus { status, .. } => assert_eq!(status, 500),
            other => panic!("expected BadStatus, got {other:?}"),
        }
    }
}
