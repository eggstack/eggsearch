//! Brave Search API provider.
//!
//! Uses the official Brave Search API (JSON) with an API key passed via
//! the `X-Subscription-Token` header.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::SearchResult;

const ENGINE: &str = "brave_api";
const DEFAULT_BASE_URL: &str = "https://api.search.brave.com/res/v1/web/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Parsed Brave Search API response.
#[derive(Debug, Deserialize)]
struct BraveApiResponse {
    #[serde(default)]
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
    #[allow(dead_code)]
    age: Option<String>,
}

pub async fn search(
    client: &Client,
    api_key: &str,
    base_url: Option<&str>,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let url = base_url.unwrap_or(DEFAULT_BASE_URL);

    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(url)
            .query(&[("q", query), ("count", &max_results.to_string())])
            .header("Accept", "application/json")
            .header("Accept", "application/x-ndjson")
            .header("X-Subscription-Token", api_key)
            .send()
            .await
            .map_err(|e| EngineError::Http {
                engine: ENGINE,
                source: e,
            })?;
        let status = resp.status();
        if !status.is_success() {
            return Err(EngineError::BadStatus {
                engine: ENGINE,
                status: status.as_u16(),
            });
        }
        super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await
    })
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })??;

    let parsed: BraveApiResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    let raw = parsed.web.map(|w| w.results).unwrap_or_default();
    Ok(convert(raw, max_results))
}

fn convert(raw: Vec<BraveResult>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results);
    for r in raw {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = r.url else { continue };
        if !super::is_http_url(&url) {
            continue;
        }
        let title = r
            .title
            .map(|t| crate::core::sanitize::normalize_whitespace(&t))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let Some(title) = title else { continue };
        let snippet = r
            .description
            .map(|s| crate::core::sanitize::normalize_whitespace(&s))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        out.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            metadata: Default::default(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_extracts_results() {
        let raw = vec![
            BraveResult {
                title: Some("Example Site".to_string()),
                url: Some("https://example.com".to_string()),
                description: Some("An example website for testing.".to_string()),
                age: Some("2024-01-15".to_string()),
            },
            BraveResult {
                title: Some("Rust Language".to_string()),
                url: Some("https://rust-lang.org".to_string()),
                description: Some("Systems programming language.".to_string()),
                age: None,
            },
        ];
        let out = convert(raw, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Example Site");
        assert_eq!(out[0].url, "https://example.com");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("An example website for testing.")
        );
        assert_eq!(out[0].source_engine, "brave_api");
    }

    #[test]
    fn test_convert_respects_max_results() {
        let raw: Vec<BraveResult> = (0..5)
            .map(|i| BraveResult {
                title: Some(format!("T{i}")),
                url: Some(format!("https://example.com/{i}")),
                description: None,
                age: None,
            })
            .collect();
        let out = convert(raw, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_url() {
        let raw = vec![BraveResult {
            title: Some("No URL".to_string()),
            url: None,
            description: None,
            age: None,
        }];
        let out = convert(raw, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_url() {
        let raw = vec![BraveResult {
            title: Some("Empty".to_string()),
            url: Some(String::new()),
            description: None,
            age: None,
        }];
        let out = convert(raw, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let raw = vec![
            BraveResult {
                title: Some("Relative".to_string()),
                url: Some("/relative".to_string()),
                description: None,
                age: None,
            },
            BraveResult {
                title: Some("Valid".to_string()),
                url: Some("https://valid.com".to_string()),
                description: None,
                age: None,
            },
        ];
        let out = convert(raw, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "https://valid.com");
    }

    #[test]
    fn test_convert_skips_missing_title() {
        let raw = vec![BraveResult {
            title: None,
            url: Some("https://example.com".to_string()),
            description: None,
            age: None,
        }];
        let out = convert(raw, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let raw = vec![BraveResult {
            title: Some("Title".to_string()),
            url: Some("https://example.com".to_string()),
            description: Some(String::new()),
            age: None,
        }];
        let out = convert(raw, 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_parse_json_response_full() {
        let body = r#"{
            "web": {
                "results": [
                    {"title": "Rust Lang", "url": "https://rust-lang.org", "description": "A language", "age": "2024-01-15"},
                    {"title": "Wikipedia", "url": "https://en.wikipedia.org/wiki/Rust", "description": "Article"}
                ]
            }
        }"#;
        let parsed: BraveApiResponse = serde_json::from_str(body).unwrap();
        let web = parsed.web.expect("web results");
        assert_eq!(web.results.len(), 2);
    }

    #[test]
    fn test_parse_json_response_empty_web() {
        let body = r#"{}"#;
        let parsed: BraveApiResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.web.is_none());
    }

    // -----------------------------------------------------------------------
    // HTTP-level tests using httpmock
    // -----------------------------------------------------------------------

    use crate::meta::engines::error::EngineError;

    #[tokio::test]
    async fn test_successful_response_with_multiple_results() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .header("X-Subscription-Token", "test-api-key");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{
                    "web": {
                        "results": [
                            {"title": "Rust Lang", "url": "https://rust-lang.org", "description": "A language"},
                            {"title": "Wikipedia", "url": "https://en.wikipedia.org/wiki/Rust", "description": "Article"},
                            {"title": "Docs", "url": "https://docs.rs", "description": "Documentation"}
                        ]
                    }
                }"#);
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-api-key",
            Some(&server.url("/search")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].title, "Rust Lang");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].source_engine, "brave_api");
        assert_eq!(results[1].title, "Wikipedia");
        assert_eq!(results[2].title, "Docs");
    }

    #[tokio::test]
    async fn test_empty_result_set() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-api-key",
            Some(&server.url("/search")),
            "xyznonexistent",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_empty_web_object() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{}"#);
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-api-key",
            Some(&server.url("/search")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_invalid_api_key_401() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(401).body("Unauthorized");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "bad-key",
            Some(&server.url("/search")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 401");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "brave_api");
                assert_eq!(status, 401);
            }
            other => panic!("expected BadStatus(401), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_invalid_api_key_403() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(403).body("Forbidden");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "bad-key",
            Some(&server.url("/search")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 403");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "brave_api");
                assert_eq!(status, 403);
            }
            other => panic!("expected BadStatus(403), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_rate_limited_429() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(429).body("Too Many Requests");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-api-key",
            Some(&server.url("/search")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 429");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "brave_api");
                assert_eq!(status, 429);
            }
            other => panic!("expected BadStatus(429), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_malformed_json() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(200)
                .header("content-type", "application/json")
                .body("this is not json");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-api-key",
            Some(&server.url("/search")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with parse error");

        match err {
            EngineError::ParseFailed { engine, reason } => {
                assert_eq!(engine, "brave_api");
                assert!(reason.contains("invalid JSON"), "reason: {reason}");
            }
            other => panic!("expected ParseFailed, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_server_error_500() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search");
            then.status(500).body("Internal Server Error");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-api-key",
            Some(&server.url("/search")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 500");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "brave_api");
                assert_eq!(status, 500);
            }
            other => panic!("expected BadStatus(500), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_respects_max_results() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search").query_param("count", "2");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "web": {
                        "results": [
                            {"title": "A", "url": "https://a.com", "description": "a"},
                            {"title": "B", "url": "https://b.com", "description": "b"},
                            {"title": "C", "url": "https://c.com", "description": "c"}
                        ]
                    }
                }"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-api-key",
            Some(&server.url("/search")),
            "rust",
            2,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "A");
        assert_eq!(results[1].title, "B");
    }

    #[tokio::test]
    async fn test_api_key_sent_in_header() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .header("X-Subscription-Token", "my-secret-key");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });

        let client = reqwest::Client::new();
        // Should succeed because the mock verifies the header
        search(
            &client,
            "my-secret-key",
            Some(&server.url("/search")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed with correct API key header");
    }

    #[test]
    fn test_provider_descriptor_for_brave_api() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("brave_api", true, false, true, false, None, None)
            .unwrap();
        assert_eq!(desc.id, "brave_api");
        assert_eq!(desc.display_name, "Brave Search API");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        // Brave API adapter only forwards q and count; safe_search,
        // freshness, language, and region are not passed through.
        assert!(!desc.capabilities.supports_safe_search);
        assert!(!desc.capabilities.supports_freshness);
        assert!(!desc.capabilities.supports_language);
        assert!(!desc.capabilities.supports_region);
    }

    #[test]
    fn test_provider_descriptor_brave_api_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("brave_api", false, false, true, false, None, None)
            .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
