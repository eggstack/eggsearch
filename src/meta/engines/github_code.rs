//! GitHub Code Search API provider.
//!
//! Uses the GitHub REST API `/search/code` endpoint with a personal
//! access token passed via the `Authorization: Bearer` header.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{CodeSearchMetadata, ResultMetadata, SearchResult};

const ENGINE: &str = "github_code";
const DEFAULT_BASE_URL: &str = "https://api.github.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Parsed GitHub code search API response.
#[derive(Debug, Deserialize)]
struct GithubCodeResponse {
    #[serde(default)]
    items: Vec<GithubCodeItem>,
}

#[derive(Debug, Deserialize)]
struct GithubCodeItem {
    #[allow(dead_code)]
    name: Option<String>,
    path: Option<String>,
    html_url: Option<String>,
    repository: Option<GithubRepo>,
    #[allow(dead_code)]
    score: Option<f64>,
    #[serde(default)]
    text_matches: Vec<TextMatch>,
}

/// A text-match fragment returned by the GitHub Code Search API
/// when using the `application/vnd.github.text-match+json` media type.
#[derive(Debug, Deserialize)]
struct TextMatch {
    fragment: Option<String>,
    #[serde(default)]
    matches: Vec<TextMatchItem>,
}

#[derive(Debug, Deserialize)]
struct TextMatchItem {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRepo {
    full_name: Option<String>,
    description: Option<String>,
}

pub async fn search(
    client: &Client,
    api_key: &str,
    base_url: Option<&str>,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    if max_results == 0 {
        return Ok(Vec::new());
    }

    let base = base_url.unwrap_or(DEFAULT_BASE_URL);
    let url = format!("{base}/search/code");

    let per_page = max_results.clamp(1, 100);

    let response = tokio::time::timeout(
        timeout,
        client
            .get(&url)
            .query(&[("q", query), ("per_page", &per_page.to_string())])
            .header("Accept", "application/vnd.github.text-match+json")
            .header("Authorization", format!("Bearer {api_key}"))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send(),
    )
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })?
    .map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;

    let status = response.status();
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

    let parsed: GithubCodeResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed.items, max_results))
}

fn convert(items: Vec<GithubCodeItem>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(items.len()));
    for item in items {
        if out.len() >= max_results {
            break;
        }
        let Some(html_url) = &item.html_url else {
            continue;
        };
        if html_url.is_empty() || !html_url.starts_with("http") {
            continue;
        }
        let title = build_title(item.path.as_deref(), item.repository.as_ref());
        let Some(title) = title else {
            continue;
        };
        let snippet = item
            .repository
            .as_ref()
            .and_then(|r| r.description.as_deref())
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        // Extract matched symbol and fragment from text_matches.
        let (matched_symbol, text_fragment) = extract_text_match(&item.text_matches);

        let metadata = if matched_symbol.is_some() || text_fragment.is_some() {
            ResultMetadata::CodeSearch(CodeSearchMetadata {
                matched_symbol,
                text_fragment,
            })
        } else {
            ResultMetadata::None
        };

        out.push(SearchResult {
            title,
            url: html_url.clone(),
            snippet,
            source_engine: ENGINE.to_string(),
            metadata,
        });
    }
    out
}

/// Extract the first matched symbol and text fragment from text matches.
fn extract_text_match(text_matches: &[TextMatch]) -> (Option<String>, Option<String>) {
    for tm in text_matches {
        let matched_text = tm
            .matches
            .iter()
            .find_map(|m| m.text.clone())
            .or_else(|| tm.fragment.clone());
        if matched_text.is_some() {
            let fragment = tm.fragment.clone();
            return (matched_text, fragment);
        }
    }
    (None, None)
}

fn build_title(path: Option<&str>, repo: Option<&GithubRepo>) -> Option<String> {
    let path = path?;
    let repo_name = repo
        .as_ref()
        .and_then(|r| r.full_name.as_deref())
        .unwrap_or("unknown");
    Some(format!("{path} - {repo_name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_extracts_results() {
        let items = vec![
            GithubCodeItem {
                name: Some("lib.rs".to_string()),
                path: Some("src/lib.rs".to_string()),
                html_url: Some("https://github.com/tokio-rs/axum/blob/main/src/lib.rs".to_string()),
                repository: Some(GithubRepo {
                    full_name: Some("tokio-rs/axum".to_string()),
                    description: Some("A web framework".to_string()),
                }),
                score: Some(1.0),
                text_matches: vec![],
            },
            GithubCodeItem {
                name: Some("main.rs".to_string()),
                path: Some("src/main.rs".to_string()),
                html_url: Some(
                    "https://github.com/tokio-rs/axum/blob/main/src/main.rs".to_string(),
                ),
                repository: Some(GithubRepo {
                    full_name: Some("tokio-rs/axum".to_string()),
                    description: None,
                }),
                score: Some(0.8),
                text_matches: vec![],
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "src/lib.rs - tokio-rs/axum");
        assert_eq!(
            out[0].url,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs"
        );
        assert_eq!(out[0].snippet.as_deref(), Some("A web framework"));
        assert_eq!(out[0].source_engine, "github_code");
        assert_eq!(out[1].title, "src/main.rs - tokio-rs/axum");
        assert!(out[1].snippet.is_none());
    }

    #[test]
    fn test_convert_respects_max_results() {
        let items: Vec<GithubCodeItem> = (0..5)
            .map(|i| GithubCodeItem {
                name: Some(format!("f{i}.rs")),
                path: Some(format!("src/f{i}.rs")),
                html_url: Some(format!(
                    "https://github.com/test/repo/blob/main/src/f{i}.rs"
                )),
                repository: Some(GithubRepo {
                    full_name: Some("test/repo".to_string()),
                    description: None,
                }),
                score: None,
                text_matches: vec![],
            })
            .collect();
        let out = convert(items, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_html_url() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: None,
            repository: None,
            score: None,
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_html_url() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: Some(String::new()),
            repository: None,
            score: None,
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let items = vec![
            GithubCodeItem {
                name: Some("a.rs".to_string()),
                path: Some("a.rs".to_string()),
                html_url: Some("ftp://example.com/a.rs".to_string()),
                repository: None,
                score: None,
                text_matches: vec![],
            },
            GithubCodeItem {
                name: Some("b.rs".to_string()),
                path: Some("b.rs".to_string()),
                html_url: Some("https://github.com/test/repo/blob/main/b.rs".to_string()),
                repository: Some(GithubRepo {
                    full_name: Some("test/repo".to_string()),
                    description: None,
                }),
                score: None,
                text_matches: vec![],
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "b.rs - test/repo");
    }

    #[test]
    fn test_convert_skips_missing_path() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: None,
            html_url: Some("https://github.com/test/repo/blob/main/lib.rs".to_string()),
            repository: Some(GithubRepo {
                full_name: Some("test/repo".to_string()),
                description: None,
            }),
            score: None,
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: Some("https://github.com/test/repo/blob/main/src/lib.rs".to_string()),
            repository: Some(GithubRepo {
                full_name: Some("test/repo".to_string()),
                description: Some(String::new()),
            }),
            score: None,
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_build_title_with_path_and_repo() {
        let repo = GithubRepo {
            full_name: Some("tokio-rs/axum".to_string()),
            description: None,
        };
        assert_eq!(
            build_title(Some("src/lib.rs"), Some(&repo)).unwrap(),
            "src/lib.rs - tokio-rs/axum"
        );
    }

    #[test]
    fn test_build_title_none_when_no_path() {
        assert!(build_title(None, None).is_none());
    }

    #[test]
    fn test_build_title_fallback_when_no_repo() {
        assert_eq!(
            build_title(Some("lib.rs"), None).unwrap(),
            "lib.rs - unknown"
        );
    }

    #[test]
    fn test_parse_json_response_full() {
        let body = r#"{
            "items": [
                {"name": "lib.rs", "path": "src/lib.rs", "html_url": "https://github.com/test/repo/blob/main/src/lib.rs", "repository": {"full_name": "test/repo", "description": "A test"}, "score": 1.0},
                {"name": "main.rs", "path": "src/main.rs", "html_url": "https://github.com/test/repo/blob/main/src/main.rs", "repository": {"full_name": "test/repo"}, "score": 0.5}
            ]
        }"#;
        let parsed: GithubCodeResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(
            parsed.items[0].html_url.as_deref(),
            Some("https://github.com/test/repo/blob/main/src/lib.rs")
        );
    }

    #[test]
    fn test_parse_json_response_empty_items() {
        let body = r#"{"items": []}"#;
        let parsed: GithubCodeResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.items.is_empty());
    }

    #[test]
    fn test_max_results_zero_returns_empty() {
        let out = convert(vec![], 0);
        assert!(out.is_empty());
    }

    // --- Text-match extraction tests ---

    #[test]
    fn test_extract_text_match_with_matches() {
        let matches = vec![TextMatch {
            fragment: Some("pub fn router() -> Router {\n}".to_string()),
            matches: vec![TextMatchItem {
                text: Some("router".to_string()),
            }],
        }];
        let (symbol, fragment) = extract_text_match(&matches);
        assert_eq!(symbol.as_deref(), Some("router"));
        assert!(fragment.is_some());
    }

    #[test]
    fn test_extract_text_match_fragment_only() {
        let matches = vec![TextMatch {
            fragment: Some("fn main() {}".to_string()),
            matches: vec![],
        }];
        let (symbol, fragment) = extract_text_match(&matches);
        assert_eq!(symbol.as_deref(), Some("fn main() {}"));
        assert!(fragment.is_some());
    }

    #[test]
    fn test_extract_text_match_empty() {
        let (symbol, fragment) = extract_text_match(&[]);
        assert!(symbol.is_none());
        assert!(fragment.is_none());
    }

    #[test]
    fn test_convert_with_text_matches_produces_code_search_metadata() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: Some("https://github.com/test/repo/blob/main/src/lib.rs".to_string()),
            repository: Some(GithubRepo {
                full_name: Some("test/repo".to_string()),
                description: None,
            }),
            score: Some(1.0),
            text_matches: vec![TextMatch {
                fragment: Some("pub fn router() -> Router {\n}".to_string()),
                matches: vec![TextMatchItem {
                    text: Some("router".to_string()),
                }],
            }],
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        match &out[0].metadata {
            ResultMetadata::CodeSearch(m) => {
                assert_eq!(m.matched_symbol.as_deref(), Some("router"));
                assert!(m.text_fragment.is_some());
            }
            other => panic!("expected CodeSearch metadata, got {other:?}"),
        }
    }

    #[test]
    fn test_convert_without_text_matches_produces_none_metadata() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: Some("https://github.com/test/repo/blob/main/src/lib.rs".to_string()),
            repository: Some(GithubRepo {
                full_name: Some("test/repo".to_string()),
                description: None,
            }),
            score: Some(1.0),
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].metadata, ResultMetadata::None);
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
                .path("/search/code")
                .header("Authorization", "Bearer test-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "items": [
                        {"name": "lib.rs", "path": "src/lib.rs", "html_url": "https://github.com/tokio-rs/axum/blob/main/src/lib.rs", "repository": {"full_name": "tokio-rs/axum", "description": "A web framework"}, "score": 1.0},
                        {"name": "main.rs", "path": "src/main.rs", "html_url": "https://github.com/tokio-rs/axum/blob/main/src/main.rs", "repository": {"full_name": "tokio-rs/axum"}, "score": 0.8},
                        {"name": "handler.rs", "path": "src/handler.rs", "html_url": "https://github.com/tokio-rs/axum/blob/main/src/handler.rs", "repository": {"full_name": "tokio-rs/axum"}, "score": 0.5}
                    ]
                }"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "Router",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].title, "src/lib.rs - tokio-rs/axum");
        assert_eq!(
            results[0].url,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs"
        );
        assert_eq!(results[0].snippet.as_deref(), Some("A web framework"));
        assert_eq!(results[0].source_engine, "github_code");
    }

    #[tokio::test]
    async fn test_empty_result_set() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/code");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"items": []}"#);
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "xyznonexistent",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_unauthorized_401() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/code");
            then.status(401).body("Bad credentials");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "bad-token",
            Some(&server.url("")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 401");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_code");
                assert_eq!(status, 401);
            }
            other => panic!("expected BadStatus(401), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_forbidden_403() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/code");
            then.status(403).body("rate limit exceeded");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 403");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_code");
                assert_eq!(status, 403);
            }
            other => panic!("expected BadStatus(403), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_invalid_query_422() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/code");
            then.status(422).body("Validation Failed");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "bad:query:++",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 422");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_code");
                assert_eq!(status, 422);
            }
            other => panic!("expected BadStatus(422), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_malformed_json() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/code");
            then.status(200)
                .header("content-type", "application/json")
                .body("this is not json");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with parse error");

        match err {
            EngineError::ParseFailed { engine, reason } => {
                assert_eq!(engine, "github_code");
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
            when.method(GET).path("/search/code");
            then.status(500).body("Internal Server Error");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 500");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_code");
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
            when.method(GET)
                .path("/search/code")
                .query_param("per_page", "2");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "items": [
                        {"name": "a.rs", "path": "src/a.rs", "html_url": "https://github.com/test/repo/blob/main/src/a.rs", "repository": {"full_name": "test/repo"}},
                        {"name": "b.rs", "path": "src/b.rs", "html_url": "https://github.com/test/repo/blob/main/src/b.rs", "repository": {"full_name": "test/repo"}},
                        {"name": "c.rs", "path": "src/c.rs", "html_url": "https://github.com/test/repo/blob/main/src/c.rs", "repository": {"full_name": "test/repo"}}
                    ]
                }"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "rust",
            2,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "src/a.rs - test/repo");
        assert_eq!(results[1].title, "src/b.rs - test/repo");
    }

    #[tokio::test]
    async fn test_api_key_sent_in_header() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/search/code")
                .header("Authorization", "Bearer my-secret-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"items": []}"#);
        });

        let client = reqwest::Client::new();
        search(
            &client,
            "my-secret-token",
            Some(&server.url("")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed with correct auth header");
    }

    #[tokio::test]
    async fn test_timeout() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/code");
            then.status(200)
                .header("content-type", "application/json")
                .delay(std::time::Duration::from_secs(10))
                .body(r#"{"items": []}"#);
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "rust",
            10,
            Duration::from_millis(50),
        )
        .await
        .expect_err("should fail with timeout");

        match err {
            EngineError::Timeout { engine } => {
                assert_eq!(engine, "github_code");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[test]
    fn test_provider_descriptor_for_github_code() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("github_code", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "github_code");
        assert_eq!(desc.display_name, "GitHub Code Search");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_code_search);
        assert!(desc.capabilities.supports_repo_filter);
        assert!(desc.capabilities.supports_org_filter);
        assert!(desc.capabilities.supports_path_filter);
        assert!(desc.capabilities.supports_language_filter);
        assert!(desc.capabilities.supports_symbol_hint);
    }

    #[test]
    fn test_provider_descriptor_github_code_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("github_code", false, false, true, false, None, None)
                .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
