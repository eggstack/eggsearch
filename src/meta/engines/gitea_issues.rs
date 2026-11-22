//! Gitea/Forgejo Issues Search API provider.
//!
//! Uses the Gitea global search API endpoint with `scope == "issue"`
//! and a personal access token passed via the `Authorization: token`
//! header (Gitea uses `token`, not `Bearer`).

use std::time::Duration;

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::truncate_body;
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

    // -----------------------------------------------------------------------
    // HTTP-level tests using httpmock
    // -----------------------------------------------------------------------

    use crate::meta::engines::error::EngineError;

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
                assert_eq!(engine, "gitea_issues");
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
                assert_eq!(engine, "gitea_issues");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
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
