use std::time::Duration;

use eggfetch_core::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;
use super::{PageMarker, ScrapeShape};

const ENGINE: &str = "brave";
const BRAVE_URL: &str = "https://search.brave.com/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

const SHAPE: ScrapeShape = ScrapeShape {
    result_container: "div[data-type='web']",
    empty_markers: &[
        PageMarker::Selector("#bad-results-info-banner"),
        PageMarker::Selector("div.fallback-snippet"),
        PageMarker::Text("Not many great matches came back for your search"),
    ],
    blocked_markers: &[PageMarker::Text("Please verify you are a human")],
};

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let resp = client
        .get(BRAVE_URL)
        .map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?
        .query("q", query)
        .timeout(super::engine_timeout(timeout))
        .send()
        .await
        .map_err(|e| super::map_request_error(ENGINE, e))?;
    if !resp.status().is_success() {
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: resp.status().as_u16(),
        });
    }
    let bytes = super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await?;

    let body = String::from_utf8_lossy(&bytes).into_owned();

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
}
