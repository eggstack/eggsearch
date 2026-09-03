//! Brave Search API provider.
//!
//! Uses the official Brave Search API (JSON) with an API key passed via
//! the `X-Subscription-Token` header.

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::SearchResult;
use super::request::EngineSearchRequest;
use crate::core::query::{Freshness, SafeSearch, SearchIntent};

const ENGINE: &str = "brave_api";
const DEFAULT_WEB_URL: &str = "https://api.search.brave.com/res/v1/web/search";
const DEFAULT_NEWS_URL: &str = "https://api.search.brave.com/res/v1/news/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const BRAVE_MAX_COUNT: usize = 20;

/// Parsed Brave Search API response.
#[derive(Debug, Deserialize)]
struct BraveApiResponse {
    #[serde(default)]
    web: Option<BraveWebResults>,
    #[serde(default)]
    news: Option<BraveNewsResults>,
    #[serde(default)]
    results: Option<Vec<BraveResult>>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveNewsResults {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
    age: Option<String>,
    extra_snippets: Option<Vec<String>>,
}

fn brave_endpoint(intent: SearchIntent) -> &'static str {
    match intent {
        SearchIntent::News => DEFAULT_NEWS_URL,
        _ => DEFAULT_WEB_URL,
    }
}

fn resolve_brave_url(base_url: Option<&str>, intent: SearchIntent) -> String {
    match base_url {
        None => brave_endpoint(intent).to_string(),
        Some(url) => {
            if intent == SearchIntent::News && url.contains("/web/search") {
                url.replace("/web/search", "/news/search")
            } else {
                url.to_string()
            }
        }
    }
}

fn map_freshness(request: &EngineSearchRequest) -> Option<String> {
    if let Some(range) = &request.date_range {
        let start = range.start.trim();
        let end = range.end.trim();
        if !start.is_empty() && !end.is_empty() {
            return Some(format!("{start}to{end}"));
        }
        return None;
    }
    match request.freshness {
        Freshness::Any => None,
        Freshness::Day => Some("pd".to_string()),
        Freshness::Week => Some("pw".to_string()),
        Freshness::Month => Some("pm".to_string()),
        Freshness::Year => Some("py".to_string()),
    }
}

fn map_safe_search(value: Option<SafeSearch>) -> Option<String> {
    value.map(|v| match v {
        SafeSearch::Off => "off".to_string(),
        SafeSearch::Moderate => "moderate".to_string(),
        SafeSearch::Strict => "strict".to_string(),
    })
}

fn map_language(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    let normalized = raw.replace('_', "-");
    let parts: Vec<&str> = normalized.split('-').collect();
    match parts.as_slice() {
        [primary]
            if (2..=3).contains(&primary.len())
                && primary.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            Some(primary.to_ascii_lowercase())
        }
        [primary, region]
            if (2..=3).contains(&primary.len())
                && region.len() == 2
                && primary.chars().all(|c| c.is_ascii_alphabetic())
                && region.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            Some(format!(
                "{}-{}",
                primary.to_ascii_lowercase(),
                region.to_ascii_uppercase()
            ))
        }
        _ => None,
    }
}

fn map_country(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.len() == 2 && raw.chars().all(|c| c.is_ascii_alphabetic()) {
        return Some(raw.to_ascii_uppercase());
    }
    None
}

pub async fn search(
    client: &Client,
    api_key: &str,
    base_url: Option<&str>,
    request: &EngineSearchRequest,
) -> Result<Vec<SearchResult>, EngineError> {
    let url = resolve_brave_url(base_url, request.intent);
    let count = request.max_results.clamp(1, BRAVE_MAX_COUNT).to_string();

    let mut params: Vec<(String, String)> = vec![
        ("q".to_string(), request.query.clone()),
        ("count".to_string(), count),
    ];
    if let Some(safe) = map_safe_search(request.safe_search) {
        params.push(("safesearch".to_string(), safe));
    }
    if let Some(fresh) = map_freshness(request) {
        params.push(("freshness".to_string(), fresh));
    }
    if let Some(lang) = map_language(request.language.as_deref()) {
        params.push(("search_lang".to_string(), lang));
    }
    if let Some(country) = map_country(request.region.as_deref()) {
        params.push(("country".to_string(), country));
    }
    if request.wants_excerpts() {
        params.push(("extra_snippets".to_string(), "true".to_string()));
    }

    let timeout = request.timeout;
    let max_results = request.max_results;
    let excerpt_count = request
        .excerpt_count
        .min(crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT);
    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(url)
            .query(&params)
            .header("Accept", "application/json")
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

    let mut raw: Vec<BraveResult> = Vec::new();
    if let Some(web) = parsed.web {
        raw.extend(web.results);
    }
    if let Some(news) = parsed.news {
        raw.extend(news.results);
    }
    if let Some(results) = parsed.results {
        raw.extend(results);
    }
    Ok(convert(raw, max_results, excerpt_count))
}

fn convert(raw: Vec<BraveResult>, max_results: usize, excerpt_count: usize) -> Vec<SearchResult> {
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
        let published_at = r
            .age
            .as_deref()
            .and_then(crate::core::source_card::parse_result_timestamp);
        let mut excerpts = Vec::new();
        if excerpt_count > 0 {
            if let Some(extra) = r.extra_snippets {
                for text in extra {
                    if excerpts.len() >= excerpt_count {
                        break;
                    }
                    let text = crate::core::sanitize::normalize_whitespace(&text)
                        .trim()
                        .to_string();
                    if text.is_empty() {
                        continue;
                    }
                    excerpts.push(crate::core::source_card::SourceExcerpt {
                        text,
                        score: None,
                        provenance: crate::core::source_card::ExcerptProvenance::ProviderSnippet,
                    });
                }
            }
        }
        out.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            excerpts,
            published_at,
            metadata: Default::default(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn simple_req(query: &str, max_results: usize) -> EngineSearchRequest {
        EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
    }

    #[test]
    fn test_convert_extracts_results() {
        let raw = vec![
            BraveResult {
                title: Some("Example Site".to_string()),
                url: Some("https://example.com".to_string()),
                description: Some("An example website for testing.".to_string()),
                age: Some("2024-01-15".to_string()),
                extra_snippets: None,
            },
            BraveResult {
                title: Some("Rust Language".to_string()),
                url: Some("https://rust-lang.org".to_string()),
                description: Some("Systems programming language.".to_string()),
                age: None,
                extra_snippets: None,
            },
        ];
        let out = convert(raw, 10, 0);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Example Site");
        assert_eq!(out[0].url, "https://example.com");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("An example website for testing.")
        );
        assert_eq!(out[0].source_engine, "brave_api");
        assert_eq!(
            out[0].published_at.as_deref(),
            Some("2024-01-15T00:00:00+00:00")
        );
        assert!(out[0].excerpts.is_empty());
        assert!(out[1].published_at.is_none());
    }

    #[test]
    fn test_convert_rejects_unparseable_age() {
        let raw = vec![BraveResult {
            title: Some("T".to_string()),
            url: Some("https://example.com".to_string()),
            description: None,
            age: Some("2 days ago".to_string()),
            extra_snippets: None,
        }];
        let out = convert(raw, 10, 0);
        assert_eq!(out.len(), 1);
        assert!(out[0].published_at.is_none());
    }

    #[test]
    fn test_convert_populates_excerpts_only_when_requested() {
        let make = || BraveResult {
            title: Some("T".to_string()),
            url: Some("https://example.com".to_string()),
            description: Some("primary".to_string()),
            age: None,
            extra_snippets: Some(vec![
                "first alternate".to_string(),
                "second alternate".to_string(),
                String::new(),
                "third alternate".to_string(),
                "fourth alternate".to_string(),
            ]),
        };
        let without = convert(vec![make()], 10, 0);
        assert!(without[0].excerpts.is_empty());
        let with = convert(vec![make()], 10, 2);
        assert_eq!(with.len(), 1);
        assert_eq!(with[0].excerpts.len(), 2);
        assert_eq!(with[0].excerpts[0].text, "first alternate");
        assert_eq!(with[0].excerpts[1].text, "second alternate");
        assert!(matches!(
            with[0].excerpts[0].provenance,
            crate::core::source_card::ExcerptProvenance::ProviderSnippet
        ));
    }

    #[test]
    fn test_convert_respects_max_results() {
        let raw: Vec<BraveResult> = (0..5)
            .map(|i| BraveResult {
                title: Some(format!("T{i}")),
                url: Some(format!("https://example.com/{i}")),
                description: None,
                age: None,
                extra_snippets: None,
            })
            .collect();
        let out = convert(raw, 2, 0);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_url() {
        let raw = vec![BraveResult {
            title: Some("No URL".to_string()),
            url: None,
            description: None,
            age: None,
            extra_snippets: None,
        }];
        let out = convert(raw, 10, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_url() {
        let raw = vec![BraveResult {
            title: Some("Empty".to_string()),
            url: Some(String::new()),
            description: None,
            age: None,
            extra_snippets: None,
        }];
        let out = convert(raw, 10, 0);
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
                extra_snippets: None,
            },
            BraveResult {
                title: Some("Valid".to_string()),
                url: Some("https://valid.com".to_string()),
                description: None,
                age: None,
                extra_snippets: None,
            },
        ];
        let out = convert(raw, 10, 0);
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
            extra_snippets: None,
        }];
        let out = convert(raw, 10, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let raw = vec![BraveResult {
            title: Some("Title".to_string()),
            url: Some("https://example.com".to_string()),
            description: Some(String::new()),
            age: None,
            extra_snippets: None,
        }];
        let out = convert(raw, 10, 0);
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

    #[test]
    fn test_parse_news_response() {
        let body = r#"{
            "news": {
                "results": [
                    {"title": "Breaking", "url": "https://example.com/news", "description": "News story"}
                ]
            }
        }"#;
        let parsed: BraveApiResponse = serde_json::from_str(body).unwrap();
        let news = parsed.news.expect("news results");
        assert_eq!(news.results.len(), 1);
        let out = convert(news.results, 10, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "Breaking");
    }

    #[test]
    fn endpoint_selection_is_intent_driven() {
        assert_eq!(brave_endpoint(SearchIntent::News), DEFAULT_NEWS_URL);
        assert_eq!(brave_endpoint(SearchIntent::Web), DEFAULT_WEB_URL);
        assert_eq!(brave_endpoint(SearchIntent::Code), DEFAULT_WEB_URL);
        assert_eq!(
            resolve_brave_url(None, SearchIntent::News),
            DEFAULT_NEWS_URL.to_string()
        );
        assert_eq!(
            resolve_brave_url(None, SearchIntent::Web),
            DEFAULT_WEB_URL.to_string()
        );
    }

    #[test]
    fn resolve_rewrites_web_override_to_news_for_news_intent() {
        let web_override = "http://127.0.0.1:1/web/search";
        let resolved = resolve_brave_url(Some(web_override), SearchIntent::News);
        assert_eq!(resolved, "http://127.0.0.1:1/news/search");
        let kept = resolve_brave_url(Some(web_override), SearchIntent::Web);
        assert_eq!(kept, web_override.to_string());
    }

    #[test]
    fn map_freshness_relative() {
        let mut req = simple_req("q", 10);
        req.freshness = Freshness::Day;
        assert_eq!(map_freshness(&req).as_deref(), Some("pd"));
        req.freshness = Freshness::Week;
        assert_eq!(map_freshness(&req).as_deref(), Some("pw"));
        req.freshness = Freshness::Month;
        assert_eq!(map_freshness(&req).as_deref(), Some("pm"));
        req.freshness = Freshness::Year;
        assert_eq!(map_freshness(&req).as_deref(), Some("py"));
        req.freshness = Freshness::Any;
        assert_eq!(map_freshness(&req), None);
    }

    #[test]
    fn map_freshness_exact_range() {
        let mut req = simple_req("q", 10);
        req.freshness = Freshness::Any;
        req.date_range = Some(crate::core::query::SearchDateRange::new(
            "2024-01-01",
            "2024-01-31",
        ));
        assert_eq!(
            map_freshness(&req).as_deref(),
            Some("2024-01-01to2024-01-31")
        );
    }

    #[test]
    fn map_language_supported_and_unsupported() {
        assert_eq!(map_language(Some("en")).as_deref(), Some("en"));
        assert_eq!(map_language(Some("en-US")).as_deref(), Some("en-US"));
        assert_eq!(map_language(Some("en_US")).as_deref(), Some("en-US"));
        assert_eq!(map_language(Some("not-a-locale!!!")).as_deref(), None);
        assert_eq!(map_language(None), None);
    }

    #[test]
    fn map_country_supported_and_unsupported() {
        assert_eq!(map_country(Some("us")).as_deref(), Some("US"));
        assert_eq!(map_country(Some("US")).as_deref(), Some("US"));
        assert_eq!(map_country(Some("USA")), None);
        assert_eq!(map_country(Some("u1")), None);
        assert_eq!(map_country(None), None);
    }

    #[test]
    fn map_safe_search_values() {
        assert_eq!(
            map_safe_search(Some(SafeSearch::Off)).as_deref(),
            Some("off")
        );
        assert_eq!(
            map_safe_search(Some(SafeSearch::Moderate)).as_deref(),
            Some("moderate")
        );
        assert_eq!(
            map_safe_search(Some(SafeSearch::Strict)).as_deref(),
            Some("strict")
        );
        assert_eq!(map_safe_search(None), None);
    }

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
            &simple_req("rust", 10),
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
            &simple_req("xyznonexistent", 10),
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
            &simple_req("rust", 10),
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
            &simple_req("rust", 10),
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
            &simple_req("rust", 10),
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
            &simple_req("rust", 10),
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
            &simple_req("rust", 10),
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
            "bad-key",
            Some(&server.url("/search")),
            &simple_req("rust", 10),
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
            &simple_req("rust", 2),
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
        search(
            &client,
            "my-secret-key",
            Some(&server.url("/search")),
            &simple_req("rust", 10),
        )
        .await
        .expect("search should succeed with correct API key header");
    }

    #[tokio::test]
    async fn test_web_request_contains_expected_params() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .query_param("count", "5")
                .query_param("safesearch", "strict")
                .query_param("freshness", "pw")
                .query_param("search_lang", "en")
                .query_param("country", "US");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });

        let client = reqwest::Client::new();
        let mut req = simple_req("rust", 5);
        req.safe_search = Some(SafeSearch::Strict);
        req.freshness = Freshness::Week;
        req.language = Some("en".to_string());
        req.region = Some("US".to_string());
        search(&client, "k", Some(&server.url("/search")), &req)
            .await
            .expect("search should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn test_exact_date_range_sent_as_freshness() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("freshness", "2024-01-01to2024-01-31");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });

        let client = reqwest::Client::new();
        let mut req = simple_req("rust", 5);
        req.date_range = Some(crate::core::query::SearchDateRange::new(
            "2024-01-01",
            "2024-01-31",
        ));
        search(&client, "k", Some(&server.url("/search")), &req)
            .await
            .expect("search should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn test_unsupported_locale_is_omitted() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .query_param("count", "5");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });

        let client = reqwest::Client::new();
        let mut req = simple_req("rust", 5);
        req.language = Some("not-a-locale!!!".to_string());
        req.region = Some("USA".to_string());
        let url = server.url("/search");
        search(&client, "k", Some(&url), &req)
            .await
            .expect("search should succeed");
        mock.assert();
        assert_eq!(map_language(Some("not-a-locale!!!")), None);
        assert_eq!(map_country(Some("USA")), None);
    }

    #[tokio::test]
    async fn test_news_intent_hits_news_endpoint() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let web_mock = server.mock(|when, then| {
            when.method(GET).path("/web/search");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });
        let news_mock = server.mock(|when, then| {
            when.method(GET).path("/news/search");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{"news": {"results": [{"title": "N", "url": "https://example.com/n", "description": "d"}]}}"#,
                );
        });

        let client = reqwest::Client::new();
        let web_base = server.url("/web/search");
        let mut news_req = simple_req("election", 5);
        news_req.intent = SearchIntent::News;
        let results = search(&client, "k", Some(&web_base), &news_req)
            .await
            .expect("news search should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "N");
        news_mock.assert();
        assert_eq!(web_mock.hits(), 0);

        let web_req = simple_req("election", 5);
        search(&client, "k", Some(&web_base), &web_req)
            .await
            .expect("web search should succeed");
        assert!(web_mock.hits() >= 1);
    }

    #[tokio::test]
    async fn test_extra_snippets_param_only_when_requested() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let with_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search")
                .query_param("q", "rust")
                .query_param("extra_snippets", "true");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });
        let without_mock = server.mock(|when, then| {
            when.method(GET).path("/plain");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });

        let client = reqwest::Client::new();
        let mut req = simple_req("rust", 5);
        req.excerpt_count = 2;
        search(&client, "k", Some(&server.url("/search")), &req)
            .await
            .expect("search should succeed");
        with_mock.assert();

        let plain = simple_req("rust", 5);
        assert_eq!(plain.excerpt_count, 0);
        let snippet_probe = server.mock(|when, then| {
            when.method(GET)
                .path("/plain")
                .query_param("extra_snippets", "true");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"web": {"results": []}}"#);
        });
        search(&client, "k", Some(&server.url("/plain")), &plain)
            .await
            .expect("search should succeed");
        without_mock.assert();
        assert_eq!(
            snippet_probe.hits(),
            0,
            "extra_snippets must not be sent without excerpt demand"
        );
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
        assert!(desc.capabilities.supports_safe_search);
        assert!(desc.capabilities.supports_freshness);
        assert!(desc.capabilities.supports_language);
        assert!(desc.capabilities.supports_region);
        assert!(desc.capabilities.supports_news);
        assert!(!desc.capabilities.supports_domain_filters);
        assert!(desc.capabilities.supports_result_timestamps);
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
