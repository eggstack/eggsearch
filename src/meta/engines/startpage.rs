use std::time::Duration;

use reqwest::Client;
use scraper::Html;

use super::error::EngineError;
use super::models::SearchResult;

const ENGINE: &str = "startpage";
const STARTPAGE_URL: &str = "https://www.startpage.com/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

pub async fn search(
    client: &Client,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let response = tokio::time::timeout(
        timeout,
        client.get(STARTPAGE_URL).query(&[("q", query)]).send(),
    )
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })?
    .map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;

    if !response.status().is_success() {
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: response.status().as_u16(),
        });
    }

    let bytes = response.bytes().await.map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("response body too large: {} bytes", bytes.len()),
        });
    }
    let body = String::from_utf8_lossy(&bytes).into_owned();

    parse(&body, max_results)
}

fn parse(html: &str, max_results: usize) -> Result<Vec<SearchResult>, EngineError> {
    let document = Html::parse_document(html);

    let result_sel = sel(ENGINE, "div.result")?;
    let link_sel = sel(ENGINE, "a.result-title")?;
    let title_sel = sel(ENGINE, "h2.wgl-title")?;
    let snippet_sel = sel(ENGINE, "p.description")?;

    let mut results = Vec::new();

    for element in document.select(&result_sel) {
        if results.len() >= max_results {
            break;
        }
        let Some(link_el) = element.select(&link_sel).next() else {
            continue;
        };

        let url = link_el.value().attr("href").unwrap_or("").to_string();
        if url.is_empty() || !url.starts_with("http") {
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
            <div class="result css-o7i03b">
                <a class="result-title result-link css-abc" href="https://rust-lang.org/">
                    <h2 class="wgl-title css-xyz">Rust Programming Language</h2>
                </a>
                <p class="description css-def">A fast, memory-safe language.</p>
            </div>
            <div class="result css-o7i03b">
                <a class="result-title result-link css-abc" href="https://en.wikipedia.org/wiki/Rust">
                    <h2 class="wgl-title css-xyz">Rust - Wikipedia</h2>
                </a>
                <p class="description css-def">Rust is a general-purpose programming language.</p>
            </div>
        "#;

        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://rust-lang.org/");
        assert_eq!(results[1].url, "https://en.wikipedia.org/wiki/Rust");
        assert!(results[0].snippet.is_some());
    }

    #[test]
    fn test_parse_respects_max_results() {
        let block = r#"
            <div class="result css-o7i03b">
                <a class="result-title" href="https://example.com">
                    <h2 class="wgl-title">Title</h2>
                </a>
            </div>
        "#;
        let html = block.repeat(5);
        let results = parse(&html, 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_skips_missing_snippet() {
        let html = r#"
            <div class="result css-o7i03b">
                <a class="result-title" href="https://example.com">
                    <h2 class="wgl-title">Title</h2>
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
            <div class="result css-o7i03b">
                <a class="result-title" href="/relative/path">
                    <h2 class="wgl-title">Relative</h2>
                </a>
            </div>
            <div class="result css-o7i03b">
                <a class="result-title" href="https://valid.com">
                    <h2 class="wgl-title">Valid</h2>
                </a>
            </div>
        "#;
        let results = parse(html, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://valid.com");
    }
}
