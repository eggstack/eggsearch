//! Gitea/Forgejo Code Search API provider.
//!
//! Uses the Gitea global search API endpoint with a personal access
//! token passed via the `Authorization: token` header (Gitea uses
//! `token`, not `Bearer`).

use std::time::Duration;

use eggfetch_core::Client;
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

    let limit = limit.to_string();
    let authorization = format!("token {api_key}");
    let query_params = [("q", query), ("limit", limit.as_str())];
    let headers = [("Authorization", authorization.as_str())];
    let parsed: GiteaSearchResponse = super::get_json_bounded(
        client,
        ENGINE,
        url.as_str(),
        &query_params,
        &headers,
        MAX_BODY_BYTES,
        timeout,
    )
    .await?;

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
                .map(crate::core::sanitize::normalize_whitespace)
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
                excerpts: Vec::new(),
                published_at: None,
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

    // -----------------------------------------------------------------------
    // HTTP-level tests using httpmock
    // -----------------------------------------------------------------------

    use crate::meta::engines::error::EngineError;

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

        let client = crate::meta::engines::build_http_client(None).expect("test client");
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
    async fn test_empty_base_url_returns_error() {
        let client = crate::meta::engines::build_http_client(None).expect("test client");
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
        let client = crate::meta::engines::build_http_client(None).expect("test client");
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
