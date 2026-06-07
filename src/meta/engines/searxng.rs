use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::SearchResult;

const ENGINE: &str = "searxng";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngResult>,
}

#[derive(Debug, Deserialize)]
struct SearxngResult {
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
}

pub async fn search(
    client: &Client,
    base_url: &str,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let endpoint = build_endpoint(base_url);

    let response = tokio::time::timeout(
        timeout,
        client
            .get(&endpoint)
            .query(&[
                ("q", query),
                ("format", "json"),
                ("categories", "general"),
                ("language", "en-US"),
            ])
            .header("Accept", "application/json")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send(),
    )
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })?
    .map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;

    let status = response.status();
    if status.as_u16() == 403 || status.as_u16() == 400 {
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: status.as_u16(),
        });
    }
    if !status.is_success() {
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: status.as_u16(),
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

    let parsed: SearxngResponse = serde_json::from_slice(&bytes).map_err(|e| {
        EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        }
    })?;

    Ok(convert(parsed.results, max_results))
}

fn build_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    format!("{trimmed}/search")
}

fn convert(raw: Vec<SearxngResult>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results);
    for r in raw {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = r.url else { continue };
        if url.is_empty() || !url.starts_with("http") {
            continue;
        }
        let title = r
            .title
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let Some(title) = title else { continue };
        let snippet = r
            .content
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        out.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_endpoint_trims_trailing_slash() {
        assert_eq!(
            build_endpoint("https://searx.example.org/"),
            "https://searx.example.org/search"
        );
        assert_eq!(
            build_endpoint("https://searx.example.org"),
            "https://searx.example.org/search"
        );
    }

    #[test]
    fn test_convert_extracts_results() {
        let raw = vec![
            SearxngResult {
                title: Some("Example Site".to_string()),
                url: Some("https://example.com".to_string()),
                content: Some("An example website for testing.".to_string()),
            },
            SearxngResult {
                title: Some("Rust Language".to_string()),
                url: Some("https://rust-lang.org".to_string()),
                content: Some("Systems programming language.".to_string()),
            },
        ];
        let out = convert(raw, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Example Site");
        assert_eq!(out[0].url, "https://example.com");
        assert_eq!(out[0].snippet.as_deref(), Some("An example website for testing."));
        assert_eq!(out[0].source_engine, "searxng");
    }

    #[test]
    fn test_convert_respects_max_results() {
        let raw: Vec<SearxngResult> = (0..5)
            .map(|i| SearxngResult {
                title: Some(format!("T{i}")),
                url: Some(format!("https://example.com/{i}")),
                content: None,
            })
            .collect();
        let out = convert(raw, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_url() {
        let raw = vec![SearxngResult {
            title: Some("No URL".to_string()),
            url: None,
            content: None,
        }];
        let out = convert(raw, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_url() {
        let raw = vec![SearxngResult {
            title: Some("Empty".to_string()),
            url: Some(String::new()),
            content: None,
        }];
        let out = convert(raw, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let raw = vec![
            SearxngResult {
                title: Some("Relative".to_string()),
                url: Some("/relative".to_string()),
                content: None,
            },
            SearxngResult {
                title: Some("Valid".to_string()),
                url: Some("https://valid.com".to_string()),
                content: None,
            },
        ];
        let out = convert(raw, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "https://valid.com");
    }

    #[test]
    fn test_convert_skips_missing_title() {
        let raw = vec![SearxngResult {
            title: None,
            url: Some("https://example.com".to_string()),
            content: None,
        }];
        let out = convert(raw, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let raw = vec![SearxngResult {
            title: Some("Title".to_string()),
            url: Some("https://example.com".to_string()),
            content: Some(String::new()),
        }];
        let out = convert(raw, 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_parse_json_response_full() {
        let body = r#"{
            "query": "rust",
            "results": [
                {"title": "Rust Lang", "url": "https://rust-lang.org", "content": "A language"},
                {"title": "Wikipedia", "url": "https://en.wikipedia.org/wiki/Rust", "content": "Article"}
            ],
            "engines": ["bing", "google"]
        }"#;
        let parsed: SearxngResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.results.len(), 2);
    }
}
