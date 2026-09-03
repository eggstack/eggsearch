use std::time::Duration;

use reqwest::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;

const ENGINE: &str = "duckduckgo";
const DDG_URL: &str = "https://html.duckduckgo.com/html/";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(DDG_URL)
            .query(&[("q", query)])
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

    let result_sel = sel(ENGINE, "div.result")?;
    let title_sel = sel(ENGINE, "a.result__a")?;
    let snippet_sel = sel(ENGINE, "a.result__snippet")?;

    let mut results = Vec::new();

    for element in document.select(&result_sel) {
        if results.len() >= max_results {
            break;
        }

        let Some(title_el) = element.select(&title_sel).next() else {
            continue;
        };

        let title = title_el.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let href = title_el.value().attr("href").unwrap_or("");
        let Some(url) = extract_destination_url(href) else {
            continue;
        };
        if url.is_empty() {
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

    Ok(results)
}

fn extract_destination_url(href: &str) -> Option<String> {
    let full = format!("https://html.duckduckgo.com{href}");
    let parsed = url::Url::parse(&full).ok()?;
    parsed
        .query_pairs()
        .find(|(k, _)| k == "uddg")
        .map(|(_, v)| v.into_owned())
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
    fn test_extract_destination_url() {
        let href = "/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F&rut=abc123";
        assert_eq!(
            extract_destination_url(href),
            Some("https://www.rust-lang.org/".to_string())
        );
    }

    #[test]
    fn test_parse_extracts_results() {
        let html = r#"
            <div class="result">
                <a class="result__a" href="/l/?uddg=https%3A%2F%2Fexample.com">Example Site</a>
                <a class="result__snippet">An example website for testing.</a>
            </div>
            <div class="result">
                <a class="result__a" href="/l/?uddg=https%3A%2F%2Frust-lang.org">Rust</a>
                <a class="result__snippet">Systems programming language.</a>
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
        let result_html = r#"<div class="result"><a class="result__a" href="/l/?uddg=https%3A%2F%2Fexample.com">T</a></div>"#;
        let html = result_html.repeat(5);
        let results = parse(&html, 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_skips_missing_snippet() {
        let html = r#"
            <div class="result">
                <a class="result__a" href="/l/?uddg=https%3A%2F%2Fexample.com">Title</a>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.is_none());
    }

    #[test]
    fn test_parse_skips_invalid_url() {
        let html = r#"
            <div class="result">
                <a class="result__a" href="/relative/path">Title</a>
                <a class="result__snippet">Snippet text.</a>
            </div>
            <div class="result">
                <a class="result__a" href="/l/?uddg=https%3A%2F%2Fvalid.com">Valid</a>
                <a class="result__snippet">Valid snippet.</a>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }
}
