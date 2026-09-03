//! Gitea/Forgejo Issues Search API provider.
//!
//! Uses the Gitea global search API endpoint with `scope == "issue"`
//! and a personal access token passed via the `Authorization: token`
//! header (Gitea uses `token`, not `Bearer`).

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::code_metadata::CodeHost;
use crate::core::source_card::IssueMetadata;

const ENGINE: &str = "gitea_issues";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

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
    title: Option<String>,
    url: Option<String>,
    body: Option<String>,
    state: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    closed_at: Option<String>,
    repository: Option<GiteaRepo>,
}

#[derive(Debug, Deserialize)]
struct GiteaRepo {
    full_name: Option<String>,
}

fn truncate_body(body: &str, max_chars: usize) -> String {
    crate::core::sanitize::truncate_at_word(body, max_chars)
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
            reason: "base_url is required for gitea_issues (no default)".to_string(),
        });
    };

    let url = format!("{base}/api/v1/search");
    let limit = max_results.clamp(1, 50);

    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(&url)
            .query(&[("q", query), ("limit", &limit.to_string())])
            .header("Authorization", format!("token {api_key}"))
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

    let parsed: GiteaSearchResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed, max_results))
}

fn convert(response: GiteaSearchResponse, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results);
    for scope in &response.data {
        if scope.scope.as_deref() != Some("issue") {
            continue;
        }
        for item in &scope.result {
            if out.len() >= max_results {
                return out;
            }
            let Some(title) = &item.title else {
                continue;
            };
            if title.is_empty() {
                continue;
            }
            let Some(raw_url) = &item.url else {
                continue;
            };
            if raw_url.is_empty() {
                continue;
            }
            if !super::is_http_url(raw_url) {
                continue;
            }

            let url = raw_url.clone();

            let snippet = item
                .body
                .as_deref()
                .map(|b| truncate_body(b, SNIPPET_MAX_CHARS))
                .map(|s| crate::core::sanitize::normalize_whitespace(&s))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            // Gitea issues don't have an `iid` field in global search;
            // use the title as-is.
            let metadata = ResultMetadata::Issue(IssueMetadata {
                host: Some(CodeHost::Unknown),
                owner: item.repository.as_ref().and_then(|r| {
                    r.full_name
                        .as_deref()
                        .and_then(|fn_| fn_.split_once('/').map(|(o, _)| o.to_string()))
                }),
                repo: item.repository.as_ref().and_then(|r| {
                    r.full_name
                        .as_deref()
                        .and_then(|fn_| fn_.split_once('/').map(|(_, r)| r.to_string()))
                }),
                number: None,
                state: item.state.clone(),
                is_pull_request: Some(false),
                labels: item.labels.clone(),
                created_at: item.created_at.clone(),
                updated_at: item.updated_at.clone(),
                closed_at: item.closed_at.clone(),
            });

            out.push(SearchResult {
                title: title.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_extracts_issues() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("issue".to_string()),
                result: vec![
                    GiteaSearchItem {
                        name: None,
                        title: Some("Bug in parser".to_string()),
                        url: Some("https://gitea.example.com/owner/repo/issues/1".to_string()),
                        body: Some("Parser crashes on empty input".to_string()),
                        state: Some("open".to_string()),
                        labels: vec!["bug".to_string()],
                        created_at: Some("2024-01-15T10:30:00Z".to_string()),
                        updated_at: Some("2024-01-20T14:22:00Z".to_string()),
                        closed_at: None,
                        repository: Some(GiteaRepo {
                            full_name: Some("owner/repo".to_string()),
                        }),
                    },
                    GiteaSearchItem {
                        name: None,
                        title: Some("Add feature X".to_string()),
                        url: Some("https://gitea.example.com/owner/repo/issues/2".to_string()),
                        body: None,
                        state: Some("closed".to_string()),
                        labels: vec!["enhancement".to_string()],
                        created_at: None,
                        updated_at: None,
                        closed_at: None,
                        repository: Some(GiteaRepo {
                            full_name: Some("owner/repo".to_string()),
                        }),
                    },
                ],
            }],
        };
        let out = convert(response, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Bug in parser");
        assert_eq!(out[0].url, "https://gitea.example.com/owner/repo/issues/1");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("Parser crashes on empty input")
        );
        assert_eq!(out[0].source_engine, "gitea_issues");
        match &out[0].metadata {
            ResultMetadata::Issue(m) => {
                assert_eq!(m.host, Some(CodeHost::Unknown));
                assert_eq!(m.owner.as_deref(), Some("owner"));
                assert_eq!(m.repo.as_deref(), Some("repo"));
                assert_eq!(m.state.as_deref(), Some("open"));
                assert_eq!(m.labels, vec!["bug"]);
                assert!(m.closed_at.is_none());
            }
            other => panic!("expected Issue metadata, got: {other:?}"),
        }
        assert_eq!(out[1].title, "Add feature X");
    }

    #[test]
    fn test_convert_respects_max_results() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("issue".to_string()),
                result: (0..5)
                    .map(|i| GiteaSearchItem {
                        name: None,
                        title: Some(format!("Issue {i}")),
                        url: Some(format!("https://gitea.example.com/test/repo/issues/{i}")),
                        body: None,
                        state: Some("open".to_string()),
                        labels: vec![],
                        created_at: None,
                        updated_at: None,
                        closed_at: None,
                        repository: Some(GiteaRepo {
                            full_name: Some("test/repo".to_string()),
                        }),
                    })
                    .collect(),
            }],
        };
        let out = convert(response, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_non_issue_scope() {
        let response = GiteaSearchResponse {
            data: vec![
                GiteaScope {
                    scope: Some("code".to_string()),
                    result: vec![GiteaSearchItem {
                        name: None,
                        title: Some("Code result".to_string()),
                        url: Some("https://gitea.example.com/test/repo/issues/1".to_string()),
                        body: None,
                        state: None,
                        labels: vec![],
                        created_at: None,
                        updated_at: None,
                        closed_at: None,
                        repository: None,
                    }],
                },
                GiteaScope {
                    scope: Some("issue".to_string()),
                    result: vec![GiteaSearchItem {
                        name: None,
                        title: Some("Issue result".to_string()),
                        url: Some("https://gitea.example.com/test/repo/issues/2".to_string()),
                        body: None,
                        state: None,
                        labels: vec![],
                        created_at: None,
                        updated_at: None,
                        closed_at: None,
                        repository: None,
                    }],
                },
            ],
        };
        let out = convert(response, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "Issue result");
    }

    #[test]
    fn test_convert_skips_missing_title() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("issue".to_string()),
                result: vec![GiteaSearchItem {
                    name: None,
                    title: None,
                    url: Some("https://gitea.example.com/test/repo/issues/1".to_string()),
                    body: None,
                    state: None,
                    labels: vec![],
                    created_at: None,
                    updated_at: None,
                    closed_at: None,
                    repository: None,
                }],
            }],
        };
        let out = convert(response, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_title() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("issue".to_string()),
                result: vec![GiteaSearchItem {
                    name: None,
                    title: Some(String::new()),
                    url: Some("https://gitea.example.com/test/repo/issues/1".to_string()),
                    body: None,
                    state: None,
                    labels: vec![],
                    created_at: None,
                    updated_at: None,
                    closed_at: None,
                    repository: None,
                }],
            }],
        };
        let out = convert(response, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_missing_url() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("issue".to_string()),
                result: vec![GiteaSearchItem {
                    name: None,
                    title: Some("Title".to_string()),
                    url: None,
                    body: None,
                    state: None,
                    labels: vec![],
                    created_at: None,
                    updated_at: None,
                    closed_at: None,
                    repository: None,
                }],
            }],
        };
        let out = convert(response, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let response = GiteaSearchResponse {
            data: vec![GiteaScope {
                scope: Some("issue".to_string()),
                result: vec![GiteaSearchItem {
                    name: None,
                    title: Some("Title".to_string()),
                    url: Some("https://gitea.example.com/test/repo/issues/1".to_string()),
                    body: Some(String::new()),
                    state: None,
                    labels: vec![],
                    created_at: None,
                    updated_at: None,
                    closed_at: None,
                    repository: None,
                }],
            }],
        };
        let out = convert(response, 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_parse_json_response() {
        let body = r#"{
            "data": [
                {
                    "scope": "issue",
                    "result": [
                        {"title": "Bug report", "url": "/owner/repo/issues/1", "body": "Something broken", "state": "open", "labels": ["bug"], "repository": {"full_name": "owner/repo"}},
                        {"title": "Feature request", "url": "/owner/repo/issues/2", "repository": {"full_name": "owner/repo"}}
                    ]
                }
            ]
        }"#;
        let parsed: GiteaSearchResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.data.len(), 1);
        assert_eq!(parsed.data[0].result.len(), 2);
    }

    #[test]
    fn test_max_results_zero_returns_empty() {
        let response = GiteaSearchResponse { data: vec![] };
        let out = convert(response, 0);
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
                            "scope": "issue",
                            "result": [
                                {"title": "Bug in parser", "url": "https://gitea.example.com/tokio-rs/axum/issues/1", "body": "Parser crashes", "state": "open", "labels": ["bug"], "repository": {"full_name": "tokio-rs/axum"}},
                                {"title": "Add feature", "url": "https://gitea.example.com/tokio-rs/axum/issues/2", "body": "Need feature X", "state": "closed", "repository": {"full_name": "tokio-rs/axum"}}
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
            "bug",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Bug in parser");
        assert_eq!(results[0].source_engine, "gitea_issues");
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
                assert_eq!(engine, "gitea_issues");
                assert_eq!(status, 401);
            }
            other => panic!("expected BadStatus(401), got: {other:?}"),
        }
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
                assert_eq!(engine, "gitea_issues");
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
                assert_eq!(engine, "gitea_issues");
                assert!(reason.contains("base_url is required"), "reason: {reason}");
            }
            other => panic!("expected NetworkError, got: {other:?}"),
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
                assert_eq!(engine, "gitea_issues");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
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

    #[test]
    fn test_provider_descriptor_for_gitea_issues() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("gitea_issues", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "gitea_issues");
        assert_eq!(desc.display_name, "Gitea/Forgejo Issues");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_issue_search);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_release_search);
    }
}
