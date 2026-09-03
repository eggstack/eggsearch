use std::time::Duration;

use reqwest::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;

const ENGINE: &str = "mojeek";
const MOJEEK_URL: &str = "https://www.mojeek.com/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(MOJEEK_URL)
            .query(&[("q", query)])
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
            .map_err(|e| EngineError::Http {
                engine: ENGINE,
                source: e,
            })?;
        if !resp.status().is_success() {
            return Err(EngineError::BadStatus {
                engine: ENGINE,
                status: resp.status().as_u16(),
            });
        }
        super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await
    })
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })??;

    let body = String::from_utf8_lossy(&bytes).into_owned();

    parse(&body, max_results)
}

fn parse(html: &str, max_results: usize) -> Result<Vec<SearchResult>, EngineError> {
    let document = Html::parse_document(html);

    let list_sel = sel(ENGINE, "ul.results-standard, ol.results-standard")?;
    let item_sel = sel(ENGINE, "li")?;
    let title_sel = sel(ENGINE, "h2 a.title, h2 a")?;
    let snippet_sel = sel(ENGINE, "p.s")?;

    let mut results = Vec::new();

    let Some(list) = document.select(&list_sel).next() else {
        return Ok(results);
    };

    for element in list.select(&item_sel) {
        if results.len() >= max_results {
            break;
        }

        let Some(title_el) = element.select(&title_sel).next() else {
            continue;
        };

        let title = title_el
            .text()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let title = title.trim().to_string();
        if title.is_empty() {
            continue;
        }

        let href = title_el.value().attr("href").unwrap_or("");
        if !super::is_http_url(href) {
            continue;
        }
        let url = href.to_string();

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
            .map(|s| s.trim().to_string())
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

    Ok(results)
}

fn sel(engine: &'static str, s: &str) -> Result<scraper::Selector, EngineError> {
    scraper::Selector::parse(s).map_err(|e| EngineError::ParseFailed {
        engine,
        reason: format!("invalid selector '{s}': {e:?}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extracts_results() {
        let html = r#"
            <ul class="results-standard">
                <li class="r1">
                    <h2><a class="title" href="https://example.com">Example Site</a></h2>
                    <p class="s">An example website for testing.</p>
                </li>
                <li class="r2">
                    <h2><a class="title" href="https://rust-lang.org">Rust Language</a></h2>
                    <p class="s">Systems programming language.</p>
                </li>
            </ul>
        "#;

        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Example Site");
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(results[1].url, "https://rust-lang.org");
        assert!(results[0].snippet.is_some());
        assert_eq!(results[0].source_engine, "mojeek");
    }

    #[test]
    fn test_parse_collapses_internal_whitespace_in_title() {
        let html = r#"
            <ul class="results-standard">
                <li>
                    <h2><a class="title" href="https://example.com">Rust
                        Programming
                        Language</a></h2>
                </li>
            </ul>
        "#;

        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Language");
    }

    #[test]
    fn test_parse_respects_max_results() {
        let item = r#"
            <li>
                <h2><a class="title" href="https://example.com">Title</a></h2>
                <p class="s">Snippet</p>
            </li>
        "#;
        let html = format!(r#"<ul class="results-standard">{}</ul>"#, item.repeat(5));
        let results = parse(&html, 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_skips_missing_snippet() {
        let html = r#"
            <ul class="results-standard">
                <li>
                    <h2><a class="title" href="https://example.com">No Snippet</a></h2>
                </li>
            </ul>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.is_none());
    }

    #[test]
    fn test_parse_skips_non_http_urls() {
        let html = r#"
            <ul class="results-standard">
                <li>
                    <h2><a class="title" href="/relative/path">Relative</a></h2>
                </li>
                <li>
                    <h2><a class="title" href="https://valid.com">Valid</a></h2>
                </li>
            </ul>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }

    #[test]
    fn test_parse_skips_missing_title_href() {
        let html = r#"
            <ul class="results-standard">
                <li>
                    <h2><a class="title">No href</a></h2>
                </li>
                <li>
                    <h2><a class="title" href="https://valid.com">Valid</a></h2>
                </li>
            </ul>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }

    #[test]
    fn test_parse_returns_empty_when_no_list() {
        let html = "<html><body><p>No results</p></body></html>";
        let results = parse(html, 10).unwrap();
        assert!(results.is_empty());
    }
}
