use std::sync::Arc;
use std::time::Duration;

use eggfetch_core::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;
use super::render::SerpRenderer;
use super::{PageMarker, ScrapeShape};

const ENGINE: &str = "yahoo";
const YAHOO_URL: &str = "https://search.yahoo.com/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

const SHAPE: ScrapeShape = ScrapeShape {
    result_container: "div.algo-sr",
    empty_markers: &[PageMarker::Text("We did not find results for")],
    blocked_markers: &[PageMarker::Text("Before you continue to Yahoo")],
};

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
    renderer: Option<&Arc<dyn SerpRenderer>>,
) -> Result<Vec<SearchResult>, EngineError> {
    search_at(client, YAHOO_URL, query, max_results, timeout, renderer).await
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
        .query("p", query)
        .header("Cookie", "sB=v=1&vm=p&fl=1&vl=lang_en&pn=10")
        .timeout(super::engine_timeout(timeout))
        .send()
        .await
        .map_err(|e| super::map_request_error(ENGINE, e))?;
    super::read_search_body(resp, ENGINE, &SHAPE, MAX_BODY_BYTES, &[]).await
}

fn search_url(base_url: &str, query: &str) -> String {
    format!("{base_url}?p={}", urlencoding::encode(query))
}

pub(super) fn parse(html: &str, max_results: usize) -> Result<Vec<SearchResult>, EngineError> {
    let document = Html::parse_document(html);

    let result_sel = super::sel(ENGINE, SHAPE.result_container)?;
    let link_sel = super::sel(ENGINE, "div.compTitle a")?;
    let title_sel = super::sel(ENGINE, "div.compTitle a h3 span")?;
    let snippet_sel = super::sel(ENGINE, "div.compText")?;

    let mut results = Vec::new();

    for element in document.select(&result_sel) {
        if results.len() >= max_results {
            break;
        }

        let Some(link_el) = element.select(&link_sel).next() else {
            continue;
        };

        let raw_href = link_el.value().attr("href").unwrap_or("");
        let url = parse_yahoo_url(raw_href);
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
            .map(|el| {
                el.text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
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

fn parse_yahoo_url(href: &str) -> String {
    let start = match href.find("/RU=") {
        Some(pos) => {
            let after = pos + 4;
            match href[after..].find("http") {
                Some(off) => after + off,
                None => return href.to_string(),
            }
        }
        None => return href.to_string(),
    };

    let slice = &href[start..];
    let end = ["/RS=", "/RK="]
        .iter()
        .filter_map(|marker| slice.find(marker))
        .min()
        .unwrap_or(slice.len());

    urlencoding::decode(&slice[..end])
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| slice[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yahoo_url_redirect() {
        let href =
            "https://r.search.yahoo.com/_ylt=abc/RU=https%3A%2F%2Fwww.rust-lang.org%2F/RS=xyz";
        assert_eq!(parse_yahoo_url(href), "https://www.rust-lang.org/");
    }

    #[test]
    fn test_parse_yahoo_url_direct() {
        let href = "https://example.com";
        assert_eq!(parse_yahoo_url(href), "https://example.com");
    }

    #[test]
    fn test_parse_yahoo_url_rk_ending() {
        let href = "https://r.search.yahoo.com/_ylt=abc/RU=https%3A%2F%2Fexample.com/RK=2/RS=xyz";
        assert_eq!(parse_yahoo_url(href), "https://example.com");
    }

    #[test]
    fn test_parse_extracts_results() {
        let html = r#"
            <div class="algo-sr">
                <div class="compTitle">
                    <a href="https://r.search.yahoo.com/_ylt=abc/RU=https%3A%2F%2Fexample.com/RS=xyz">
                        <h3><span>Example Site</span></h3>
                    </a>
                </div>
                <div class="compText">An example website for testing.</div>
            </div>
            <div class="algo-sr">
                <div class="compTitle">
                    <a href="https://r.search.yahoo.com/_ylt=abc/RU=https%3A%2F%2Frust-lang.org/RS=xyz">
                        <h3><span>Rust Language</span></h3>
                    </a>
                </div>
                <div class="compText">Systems programming language.</div>
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
        let item = r#"
            <div class="algo-sr">
                <div class="compTitle">
                    <a href="https://r.search.yahoo.com/_ylt=x/RU=https%3A%2F%2Fexample.com/RS=y">
                        <h3><span>Title</span></h3>
                    </a>
                </div>
            </div>
        "#;
        let html = item.repeat(5);
        let results = parse(&html, 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_snippet_optional() {
        let html = r#"
            <div class="algo-sr">
                <div class="compTitle">
                    <a href="https://r.search.yahoo.com/_ylt=x/RU=https%3A%2F%2Fexample.com/RS=y">
                        <h3><span>No Snippet</span></h3>
                    </a>
                </div>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.is_none());
    }

    #[test]
    fn test_parse_skips_non_http_urls() {
        let html = r#"
            <div class="algo-sr">
                <div class="compTitle">
                    <a href="/relative-link">
                        <h3><span>Relative</span></h3>
                    </a>
                </div>
            </div>
            <div class="algo-sr">
                <div class="compTitle">
                    <a href="https://r.search.yahoo.com/_ylt=x/RU=https%3A%2F%2Fvalid.com/RS=y">
                        <h3><span>Valid</span></h3>
                    </a>
                </div>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }

    #[test]
    fn test_drifted_result_container_is_parse_failed_not_empty() {
        let html = r#"
            <div class="dd fst algo algo-sr relsrch Sr">
                <div class="compTitleRenamed">
                    <a href="https://rust-lang.org/"><h3><span>Rust</span></h3></a>
                </div>
            </div>
        "#;
        let err = parse(html, 10).expect_err("a result container with no parseable row is drift");
        assert!(
            matches!(err, EngineError::ParseFailed { .. }),
            "unexpected error: {err:?}"
        );
    }

    #[tokio::test]
    async fn an_empty_failure_status_is_served_by_the_browser_when_one_is_installed() {
        use crate::meta::engines::render::test_support::FixtureRenderer;
        use crate::meta::engines::render::SerpRenderer;
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search").query_param("p", "rust");
            then.status(500);
        });
        let client = crate::meta::engines::build_http_client(None).expect("test client");
        let renderer = FixtureRenderer::serving(
            r#"<div class="algo-sr"><div class="compTitle"><a href="https://example.com"><h3><span>Rendered</span></h3></a></div><div class="compText">Served by the browser.</div></div>"#,
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
}
