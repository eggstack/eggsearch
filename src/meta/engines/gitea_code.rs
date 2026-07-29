//! Gitea/Forgejo Code Search API provider.
//!
//! Uses the Gitea global search API endpoint with a personal access
//! token passed via the `Authorization: token` header (Gitea uses
//! `token`, not `Bearer`).

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{CodeSearchMetadata, ResultMetadata, SearchResult};

const ENGINE: &str = "gitea_code";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Parsed Gitea global search API response.
///
/// The global search endpoint returns a `data` array of scope objects,
/// each containing a `scope` string and a `result` array of items.
#[derive(Debug, Deserialize)]
struct GiteaSearchResponse {
    #[serde(default)]
    data: Vec<GiteaScope>,
}

#[derive(Debug, Deserialize)]
struct GiteaScope {
    scope: Option<String>,
    #[serde(default)]
    result: Vec<GiteaSearchItem>,
}

#[derive(Debug, Deserialize)]
struct GiteaSearchItem {
    #[allow(dead_code)]
    name: Option<String>,
    path: Option<String>,
    url: Option<String>,
    repository: Option<GiteaRepo>,
}

#[derive(Debug, Deserialize)]
struct GiteaRepo {
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

    let Some(base) = base_url.filter(|s| !s.is_empty()) else {
        return Err(EngineError::NetworkError {
            engine: ENGINE,
            reason: "base_url is required for gitea_code (no default)".to_string(),
        });
    };

    let url = format!("{base}/api/v1/search");
    let limit = max_results.clamp(1, 50);

    let response = tokio::time::timeout(
        timeout,
        client
            .get(&url)
            .query(&[("q", query), ("limit", &limit.to_string())])
            .header("Authorization", format!("token {api_key}"))
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

    let parsed: GiteaSearchResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed, base, max_results))
}

fn convert(response: GiteaSearchResponse, base_url: &str, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results);
    for scope in &response.data {
        if scope.scope.as_deref() != Some("code") {
            continue;
        }
        for item in &scope.result {
            if out.len() >= max_results {
                return out;
            }
            let Some(path) = &item.path else {
                continue;
            };
            let Some(raw_url) = &item.url else {
                continue;
            };
            if raw_url.is_empty() {
                continue;
            }

            let url = if super::is_http_url(raw_url) {
                raw_url.clone()
            } else {
                format!("{base_url}{raw_url}")
            };

            let title = build_title(path, item.repository.as_ref());
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

            let metadata = ResultMetadata::CodeSearch(CodeSearchMetadata {
                matched_symbol: None,
                text_fragment: snippet.clone(),
            });

            out.push(SearchResult {
                title,
                url,
                snippet,
                source_engine: ENGINE.to_string(),
                metadata,
            });
        }
    }
    out
}

fn build_title(path: &str, repo: Option<&GiteaRepo>) -> Option<String> {
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
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("code".to_string()),
                result: vec![
                    GiteaSearchItem {
                        name: Some("lib.rs".to_string()),
                        path: Some("src/lib.rs".to_string()),
                        url: Some("/owner/repo/src/branch/main/src/lib.rs".to_string()),
                        repository: Some(GiteaRepo {
                            full_name: Some("owner/repo".to_string()),
                            description: Some("A test repo".to_string()),
                        }),
                    },
                    GiteaSearchItem {
                        name: Some("main.rs".to_string()),
                        path: Some("src/main.rs".to_string()),
                        url: Some("/owner/repo/src/branch/main/src/main.rs".to_string()),
                        repository: Some(GiteaRepo {
                            full_name: Some("owner/repo".to_string()),
                            description: None,
                        }),
                    },
                ],
            }],
        };
        let out = convert(response, "https://gitea.example.com", 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "src/lib.rs - owner/repo");
        assert_eq!(
            out[0].url,
            "https://gitea.example.com/owner/repo/src/branch/main/src/lib.rs"
        );
        assert_eq!(out[0].snippet.as_deref(), Some("A test repo"));
        assert_eq!(out[0].source_engine, "gitea_code");
        assert_eq!(out[1].title, "src/main.rs - owner/repo");
        assert!(out[1].snippet.is_none());
    }

    #[test]
    fn test_convert_respects_max_results() {
        let items: Vec<GiteaSearchItem> = (0..5)
            .map(|i| GiteaSearchItem {
                name: Some(format!("f{i}.rs")),
                path: Some(format!("src/f{i}.rs")),
                url: Some(format!("/test/repo/src/branch/main/src/f{i}.rs")),
                repository: Some(GiteaRepo {
                    full_name: Some("test/repo".to_string()),
                    description: None,
                }),
            })
            .collect();
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("code".to_string()),
                result: items,
            }],
        };
        let out = convert(response, "https://gitea.example.com", 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_path() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("code".to_string()),
                result: vec![GiteaSearchItem {
                    name: Some("lib.rs".to_string()),
                    path: None,
                    url: Some("/owner/repo/src/branch/main/lib.rs".to_string()),
                    repository: Some(GiteaRepo {
                        full_name: Some("owner/repo".to_string()),
                        description: None,
                    }),
                }],
            }],
        };
        let out = convert(response, "https://gitea.example.com", 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_missing_url() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("code".to_string()),
                result: vec![GiteaSearchItem {
                    name: Some("lib.rs".to_string()),
                    path: Some("src/lib.rs".to_string()),
                    url: None,
                    repository: Some(GiteaRepo {
                        full_name: Some("owner/repo".to_string()),
                        description: None,
                    }),
                }],
            }],
        };
        let out = convert(response, "https://gitea.example.com", 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_url() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("code".to_string()),
                result: vec![GiteaSearchItem {
                    name: Some("lib.rs".to_string()),
                    path: Some("src/lib.rs".to_string()),
                    url: Some(String::new()),
                    repository: Some(GiteaRepo {
                        full_name: Some("owner/repo".to_string()),
                        description: None,
                    }),
                }],
            }],
        };
        let out = convert(response, "https://gitea.example.com", 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_code_scope() {
        let response = GiteaSearchResponse {
            data: vec![
                GiteaScope {
                    scope: Some("repository".to_string()),
                    result: vec![GiteaSearchItem {
                        name: Some("repo".to_string()),
                        path: Some("owner/repo".to_string()),
                        url: Some("/owner/repo".to_string()),
                        repository: Some(GiteaRepo {
                            full_name: Some("owner/repo".to_string()),
                            description: None,
                        }),
                    }],
                },
                GiteaScope {
                    scope: Some("code".to_string()),
                    result: vec![GiteaSearchItem {
                        name: Some("lib.rs".to_string()),
                        path: Some("src/lib.rs".to_string()),
                        url: Some("/owner/repo/src/branch/main/src/lib.rs".to_string()),
                        repository: Some(GiteaRepo {
                            full_name: Some("owner/repo".to_string()),
                            description: None,
                        }),
                    }],
                },
            ],
        };
        let out = convert(response, "https://gitea.example.com", 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "src/lib.rs - owner/repo");
    }

    #[test]
    fn test_convert_prepends_base_url_to_relative_path() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("code".to_string()),
                result: vec![GiteaSearchItem {
                    name: Some("lib.rs".to_string()),
                    path: Some("src/lib.rs".to_string()),
                    url: Some("/owner/repo/src/branch/main/src/lib.rs".to_string()),
                    repository: Some(GiteaRepo {
                        full_name: Some("owner/repo".to_string()),
                        description: None,
                    }),
                }],
            }],
        };
        let out = convert(response, "https://gitea.example.com", 10);
        assert_eq!(
            out[0].url,
            "https://gitea.example.com/owner/repo/src/branch/main/src/lib.rs"
        );
    }

    #[test]
    fn test_convert_does_not_double_prefix_absolute_url() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("code".to_string()),
                result: vec![GiteaSearchItem {
                    name: Some("lib.rs".to_string()),
                    path: Some("src/lib.rs".to_string()),
                    url: Some(
                        "https://gitea.example.com/owner/repo/src/branch/main/src/lib.rs"
                            .to_string(),
                    ),
                    repository: Some(GiteaRepo {
                        full_name: Some("owner/repo".to_string()),
                        description: None,
                    }),
                }],
            }],
        };
        let out = convert(response, "https://gitea.example.com", 10);
        assert_eq!(
            out[0].url,
            "https://gitea.example.com/owner/repo/src/branch/main/src/lib.rs"
        );
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("code".to_string()),
                result: vec![GiteaSearchItem {
                    name: Some("lib.rs".to_string()),
                    path: Some("src/lib.rs".to_string()),
                    url: Some("/owner/repo/src/branch/main/src/lib.rs".to_string()),
                    repository: Some(GiteaRepo {
                        full_name: Some("owner/repo".to_string()),
                        description: Some(String::new()),
                    }),
                }],
            }],
        };
        let out = convert(response, "https://gitea.example.com", 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_build_title_with_path_and_repo() {
        let repo = GiteaRepo {
            full_name: Some("owner/repo".to_string()),
            description: None,
        };
        assert_eq!(
            build_title("src/lib.rs", Some(&repo)).unwrap(),
            "src/lib.rs - owner/repo"
        );
    }

    #[test]
    fn test_build_title_fallback_when_no_repo() {
        assert_eq!(build_title("lib.rs", None).unwrap(), "lib.rs - unknown");
    }

    #[test]
    fn test_parse_json_response_full() {
        let body = r#"{
            "data": [
                {
                    "scope": "code",
                    "result": [
                        {"name": "lib.rs", "path": "src/lib.rs", "url": "/owner/repo/src/branch/main/src/lib.rs", "repository": {"full_name": "owner/repo", "description": "A repo"}},
                        {"name": "main.rs", "path": "src/main.rs", "url": "/owner/repo/src/branch/main/src/main.rs", "repository": {"full_name": "owner/repo"}}
                    ]
                }
            ]
        }"#;
        let parsed: GiteaSearchResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.data.len(), 1);
        assert_eq!(parsed.data[0].result.len(), 2);
        assert_eq!(
            parsed.data[0].result[0].url.as_deref(),
            Some("/owner/repo/src/branch/main/src/lib.rs")
        );
    }

    #[test]
    fn test_parse_json_response_empty() {
        let body = r#"{"data": []}"#;
        let parsed: GiteaSearchResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn test_max_results_zero_returns_empty() {
        let response = GiteaSearchResponse { data: vec![] };
        let out = convert(response, "https://gitea.example.com", 0);
        assert!(out.is_empty());
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
                .path("/api/v1/search")
                .header("Authorization", "token test-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "data": [
                        {
                            "scope": "code",
                            "result": [
                                {"name": "lib.rs", "path": "src/lib.rs", "url": "/tokio-rs/axum/src/branch/main/src/lib.rs", "repository": {"full_name": "tokio-rs/axum", "description": "A web framework"}},
                                {"name": "main.rs", "path": "src/main.rs", "url": "/tokio-rs/axum/src/branch/main/src/main.rs", "repository": {"full_name": "tokio-rs/axum"}},
                                {"name": "handler.rs", "path": "src/handler.rs", "url": "/tokio-rs/axum/src/branch/main/src/handler.rs", "repository": {"full_name": "tokio-rs/axum"}}
                            ]
                        }
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
            format!(
                "{}/tokio-rs/axum/src/branch/main/src/lib.rs",
                server.url("")
            )
        );
        assert_eq!(results[0].snippet.as_deref(), Some("A web framework"));
        assert_eq!(results[0].source_engine, "gitea_code");
    }

    #[tokio::test]
    async fn test_empty_result_set() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/search");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"data": []}"#);
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
            when.method(GET).path("/api/v1/search");
            then.status(401).body("unauthorized");
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
                assert_eq!(engine, "gitea_code");
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
            when.method(GET).path("/api/v1/search");
            then.status(403).body("forbidden");
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
                assert_eq!(engine, "gitea_code");
                assert_eq!(status, 403);
            }
            other => panic!("expected BadStatus(403), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_server_error_500() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/search");
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
                assert_eq!(engine, "gitea_code");
                assert_eq!(status, 500);
            }
            other => panic!("expected BadStatus(500), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_malformed_json() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/search");
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
                assert_eq!(engine, "gitea_code");
                assert!(reason.contains("invalid JSON"), "reason: {reason}");
            }
            other => panic!("expected ParseFailed, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_timeout() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/search");
            then.status(200)
                .header("content-type", "application/json")
                .delay(std::time::Duration::from_secs(10))
                .body(r#"{"data": []}"#);
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
                assert_eq!(engine, "gitea_code");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_respects_max_results() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/api/v1/search")
                .query_param("limit", "2");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "data": [
                        {
                            "scope": "code",
                            "result": [
                                {"name": "a.rs", "path": "src/a.rs", "url": "/test/repo/src/branch/main/src/a.rs", "repository": {"full_name": "test/repo"}},
                                {"name": "b.rs", "path": "src/b.rs", "url": "/test/repo/src/branch/main/src/b.rs", "repository": {"full_name": "test/repo"}},
                                {"name": "c.rs", "path": "src/c.rs", "url": "/test/repo/src/branch/main/src/c.rs", "repository": {"full_name": "test/repo"}}
                            ]
                        }
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
                .path("/api/v1/search")
                .header("Authorization", "token my-secret-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"data": []}"#);
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
    async fn test_empty_base_url_returns_error() {
        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            None,
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with missing base_url");

        match err {
            EngineError::NetworkError { engine, reason } => {
                assert_eq!(engine, "gitea_code");
                assert!(reason.contains("base_url is required"), "reason: {reason}");
            }
            other => panic!("expected NetworkError, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_empty_string_base_url_returns_error() {
        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(""),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with empty base_url");

        match err {
            EngineError::NetworkError { engine, reason } => {
                assert_eq!(engine, "gitea_code");
                assert!(reason.contains("base_url is required"), "reason: {reason}");
            }
            other => panic!("expected NetworkError, got: {other:?}"),
        }
    }

    #[test]
    fn test_provider_descriptor_for_gitea_code() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("gitea_code", true, false, true, false, None, None)
            .unwrap();
        assert_eq!(desc.id, "gitea_code");
        assert_eq!(desc.display_name, "Gitea/Forgejo Code Search");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_repo_filter);
        assert!(!desc.capabilities.supports_path_filter);
    }

    #[test]
    fn test_provider_descriptor_gitea_code_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("gitea_code", false, false, true, false, None, None)
                .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
