//! GitLab Issues API provider.
//!
//! Uses the GitLab REST API `/api/v4/projects/:id/issues` (project-scoped)
//! or `/api/v4/search?scope=issues` (global) with a personal access
//! token passed via the `PRIVATE-TOKEN` header.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::code_metadata::CodeHost;
use crate::core::source_card::IssueMetadata;

const ENGINE: &str = "gitlab_issues";
const DEFAULT_BASE_URL: &str = "https://gitlab.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

/// Parsed GitLab issue item.
#[derive(Debug, Deserialize)]
struct GitlabIssueItem {
    iid: Option<u64>,
    title: Option<String>,
    web_url: Option<String>,
    description: Option<String>,
    state: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    closed_at: Option<String>,
}

/// UTF-8-safe snippet truncation that preserves the historical
/// word-boundary-trim semantics without ever slicing by byte offset
/// inside a multi-byte code point.
fn truncate_body(body: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let body_char_len = body.chars().count();
    if body_char_len <= max_chars {
        return body.to_string();
    }
    let truncated: String = body.chars().take(max_chars).collect();
    match truncated.rfind(char::is_whitespace) {
        Some(pos) if pos > 0 => truncated[..pos].to_string(),
        _ => truncated,
    }
}

pub async fn search(
    client: &Client,
    api_key: &str,
    base_url: Option<&str>,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    search_with_project(client, api_key, base_url, None, query, max_results, timeout).await
}

/// Search for issues, optionally scoped to a specific project.
///
/// When `project_id` is `Some`, uses the project-scoped endpoint:
/// `GET {base}/api/v4/projects/{encoded_id}/issues?search={query}`
///
/// When `project_id` is `None`, uses the global endpoint:
/// `GET {base}/api/v4/search?scope=issues&search={query}`
pub async fn search_with_project(
    client: &Client,
    api_key: &str,
    base_url: Option<&str>,
    project_id: Option<&str>,
    query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    if max_results == 0 {
        return Ok(Vec::new());
    }

    let base = base_url.unwrap_or(DEFAULT_BASE_URL);
    let per_page = max_results.clamp(1, 100);

    let url = match project_id {
        Some(pid) => {
            let encoded = urlencoding::encode(pid);
            format!("{base}/api/v4/projects/{encoded}/issues")
        }
        None => format!("{base}/api/v4/search?scope=issues"),
    };

    let response = tokio::time::timeout(
        timeout,
        client
            .get(&url)
            .query(&[("search", query), ("per_page", &per_page.to_string())])
            .header("PRIVATE-TOKEN", api_key)
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

    let parsed: Vec<GitlabIssueItem> =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed, max_results))
}

fn convert(items: Vec<GitlabIssueItem>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(items.len()));
    for item in items {
        if out.len() >= max_results {
            break;
        }
        let Some(web_url) = &item.web_url else {
            continue;
        };
        if web_url.is_empty() || !web_url.starts_with("http") {
            continue;
        }
        let Some(title) = &item.title else {
            continue;
        };
        if title.is_empty() {
            continue;
        }
        let iid_str = item.iid.map(|n| n.to_string()).unwrap_or_default();
        let title = format!("#{iid_str} {title}");

        let snippet = item
            .description
            .as_deref()
            .map(|b| truncate_body(b, SNIPPET_MAX_CHARS))
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let metadata = ResultMetadata::Issue(IssueMetadata {
            host: Some(CodeHost::Gitlab),
            owner: None,
            repo: None,
            number: item.iid,
            state: item.state.clone(),
            is_pull_request: Some(false),
            labels: item.labels.clone(),
            created_at: item.created_at.clone(),
            updated_at: item.updated_at.clone(),
            closed_at: item.closed_at.clone(),
        });

        out.push(SearchResult {
            title,
            url: web_url.clone(),
            snippet,
            source_engine: ENGINE.to_string(),
            metadata,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_extracts_results() {
        let items = vec![
            GitlabIssueItem {
                iid: Some(123),
                title: Some("Bug in parser".to_string()),
                web_url: Some("https://gitlab.com/tokio-rs/axum/-/issues/123".to_string()),
                description: Some("The parser crashes".to_string()),
                state: Some("opened".to_string()),
                labels: vec!["bug".to_string(), "p0".to_string()],
                created_at: Some("2024-01-15T10:30:00Z".to_string()),
                updated_at: Some("2024-01-20T14:22:00Z".to_string()),
                closed_at: None,
            },
            GitlabIssueItem {
                iid: Some(124),
                title: Some("Add feature X".to_string()),
                web_url: Some("https://gitlab.com/tokio-rs/axum/-/issues/124".to_string()),
                description: Some("We need feature X".to_string()),
                state: Some("closed".to_string()),
                labels: vec!["enhancement".to_string()],
                created_at: Some("2024-02-01T08:00:00Z".to_string()),
                updated_at: Some("2024-02-05T12:00:00Z".to_string()),
                closed_at: Some("2024-02-05T12:00:00Z".to_string()),
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "#123 Bug in parser");
        assert_eq!(out[0].url, "https://gitlab.com/tokio-rs/axum/-/issues/123");
        assert_eq!(out[0].snippet.as_deref(), Some("The parser crashes"));
        assert_eq!(out[0].source_engine, "gitlab_issues");
        match &out[0].metadata {
            ResultMetadata::Issue(m) => {
                assert_eq!(m.host, Some(CodeHost::Gitlab));
                assert!(m.owner.is_none());
                assert!(m.repo.is_none());
                assert_eq!(m.number, Some(123));
                assert_eq!(m.state.as_deref(), Some("opened"));
                assert_eq!(m.is_pull_request, Some(false));
                assert_eq!(m.labels, vec!["bug", "p0"]);
                assert!(m.closed_at.is_none());
            }
            other => panic!("expected Issue metadata, got: {other:?}"),
        }
        assert_eq!(out[1].title, "#124 Add feature X");
    }

    #[test]
    fn test_convert_respects_max_results() {
        let items: Vec<GitlabIssueItem> = (0..5)
            .map(|i| GitlabIssueItem {
                iid: Some(i),
                title: Some(format!("Issue {i}")),
                web_url: Some(format!("https://gitlab.com/test/repo/-/issues/{i}")),
                description: None,
                state: Some("opened".to_string()),
                labels: vec![],
                created_at: None,
                updated_at: None,
                closed_at: None,
            })
            .collect();
        let out = convert(items, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_web_url() {
        let items = vec![GitlabIssueItem {
            iid: Some(1),
            title: Some("Title".to_string()),
            web_url: None,
            description: None,
            state: Some("opened".to_string()),
            labels: vec![],
            created_at: None,
            updated_at: None,
            closed_at: None,
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_web_url() {
        let items = vec![GitlabIssueItem {
            iid: Some(1),
            title: Some("Title".to_string()),
            web_url: Some(String::new()),
            description: None,
            state: Some("opened".to_string()),
            labels: vec![],
            created_at: None,
            updated_at: None,
            closed_at: None,
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let items = vec![
            GitlabIssueItem {
                iid: Some(1),
                title: Some("Title".to_string()),
                web_url: Some("ftp://example.com/1".to_string()),
                description: None,
                state: Some("opened".to_string()),
                labels: vec![],
                created_at: None,
                updated_at: None,
                closed_at: None,
            },
            GitlabIssueItem {
                iid: Some(2),
                title: Some("Good".to_string()),
                web_url: Some("https://gitlab.com/test/repo/-/issues/2".to_string()),
                description: None,
                state: Some("opened".to_string()),
                labels: vec![],
                created_at: None,
                updated_at: None,
                closed_at: None,
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "#2 Good");
    }

    #[test]
    fn test_convert_skips_missing_title() {
        let items = vec![GitlabIssueItem {
            iid: Some(1),
            title: None,
            web_url: Some("https://gitlab.com/test/repo/-/issues/1".to_string()),
            description: None,
            state: Some("opened".to_string()),
            labels: vec![],
            created_at: None,
            updated_at: None,
            closed_at: None,
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let items = vec![GitlabIssueItem {
            iid: Some(1),
            title: Some("Title".to_string()),
            web_url: Some("https://gitlab.com/test/repo/-/issues/1".to_string()),
            description: Some(String::new()),
            state: Some("opened".to_string()),
            labels: vec![],
            created_at: None,
            updated_at: None,
            closed_at: None,
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_convert_extracts_labels() {
        let items = vec![GitlabIssueItem {
            iid: Some(1),
            title: Some("Title".to_string()),
            web_url: Some("https://gitlab.com/test/repo/-/issues/1".to_string()),
            description: None,
            state: Some("opened".to_string()),
            labels: vec!["bug".to_string(), "urgent".to_string()],
            created_at: None,
            updated_at: None,
            closed_at: None,
        }];
        let out = convert(items, 10);
        match &out[0].metadata {
            ResultMetadata::Issue(m) => {
                assert_eq!(m.labels, vec!["bug", "urgent"]);
            }
            other => panic!("expected Issue metadata, got: {other:?}"),
        }
    }

    #[test]
    fn test_max_results_zero_returns_empty() {
        let out = convert(vec![], 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_parse_json_array() {
        let body = r#"[
            {
                "iid": 1,
                "title": "Bug report",
                "web_url": "https://gitlab.com/test/repo/-/issues/1",
                "description": "Something is broken",
                "state": "opened",
                "labels": ["bug"],
                "created_at": "2024-01-15T10:30:00Z",
                "updated_at": "2024-01-20T14:22:00Z",
                "closed_at": null
            }
        ]"#;
        let parsed: Vec<GitlabIssueItem> = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].iid, Some(1));
        assert_eq!(
            parsed[0].web_url.as_deref(),
            Some("https://gitlab.com/test/repo/-/issues/1")
        );
    }

    #[test]
    fn test_parse_json_array_empty() {
        let body = r#"[]"#;
        let parsed: Vec<GitlabIssueItem> = serde_json::from_str(body).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_truncate_body_short() {
        assert_eq!(truncate_body("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_body_at_word_boundary() {
        assert_eq!(truncate_body("hello world foo bar", 11), "hello");
    }

    #[test]
    fn test_truncate_body_no_spaces() {
        assert_eq!(truncate_body("abcdefghij", 5), "abcde");
    }

    #[test]
    fn test_truncate_body_handles_multibyte_utf8() {
        let body = "abc \u{1f980} rust \u{1f9ea} unicode";
        let out = truncate_body(body, 7);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.len() <= body.len());
        assert!(out.chars().count() <= 7);
    }

    #[test]
    fn test_truncate_body_zero_max_returns_empty() {
        let out = truncate_body("anything", 0);
        assert_eq!(out, "");
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
                .path("/api/v4/search")
                .header("PRIVATE-TOKEN", "test-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"[
                        {
                            "iid": 42,
                            "title": "Fix parser",
                            "web_url": "https://gitlab.com/tokio-rs/axum/-/issues/42",
                            "description": "Parser bug description",
                            "state": "opened",
                            "labels": ["bug"],
                            "created_at": "2024-01-15T10:30:00Z",
                            "updated_at": "2024-01-20T14:22:00Z",
                            "closed_at": null
                        },
                        {
                            "iid": 43,
                            "title": "Add feature",
                            "web_url": "https://gitlab.com/tokio-rs/axum/-/issues/43",
                            "description": "Feature request",
                            "state": "closed",
                            "labels": ["enhancement"],
                            "created_at": "2024-02-01T08:00:00Z",
                            "updated_at": "2024-02-05T12:00:00Z",
                            "closed_at": "2024-02-05T12:00:00Z"
                        }
                    ]"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "rust",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "#42 Fix parser");
        assert_eq!(
            results[0].url,
            "https://gitlab.com/tokio-rs/axum/-/issues/42"
        );
        assert_eq!(
            results[0].snippet.as_deref(),
            Some("Parser bug description")
        );
        assert_eq!(results[0].source_engine, "gitlab_issues");
    }

    #[tokio::test]
    async fn test_empty_result_set() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v4/search");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"[]"#);
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
            when.method(GET).path("/api/v4/search");
            then.status(401).body("401 Unauthorized");
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
                assert_eq!(engine, "gitlab_issues");
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
            when.method(GET).path("/api/v4/search");
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
                assert_eq!(engine, "gitlab_issues");
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
            when.method(GET).path("/api/v4/search");
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
                assert_eq!(engine, "gitlab_issues");
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
            when.method(GET).path("/api/v4/search");
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
                assert_eq!(engine, "gitlab_issues");
                assert!(reason.contains("invalid JSON"), "reason: {reason}");
            }
            other => panic!("expected ParseFailed, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_respects_max_results() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/api/v4/search")
                .query_param("per_page", "2");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"[
                        {"iid": 1, "title": "A", "web_url": "https://gitlab.com/test/repo/-/issues/1", "state": "opened"},
                        {"iid": 2, "title": "B", "web_url": "https://gitlab.com/test/repo/-/issues/2", "state": "opened"},
                        {"iid": 3, "title": "C", "web_url": "https://gitlab.com/test/repo/-/issues/3", "state": "opened"}
                    ]"#,
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
        assert_eq!(results[0].title, "#1 A");
        assert_eq!(results[1].title, "#2 B");
    }

    #[tokio::test]
    async fn test_api_key_sent_in_header() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/api/v4/search")
                .header("PRIVATE-TOKEN", "my-secret-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"[]"#);
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
            when.method(GET).path("/api/v4/search");
            then.status(200)
                .header("content-type", "application/json")
                .delay(std::time::Duration::from_secs(10))
                .body(r#"[]"#);
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
                assert_eq!(engine, "gitlab_issues");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_project_scoped_endpoint() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/api/v4/projects/12345/issues")
                .query_param("search", "bug")
                .header("PRIVATE-TOKEN", "test-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"[{"iid": 1, "title": "Bug", "web_url": "https://gitlab.com/12345/-/issues/1", "state": "opened"}]"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("12345"),
            "bug",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "#1 Bug");
    }

    #[tokio::test]
    async fn test_project_scoped_url_encoding() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/api/v4/projects/group%2Fsubgroup%2Frepo/issues");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"[]"#);
        });

        let client = reqwest::Client::new();
        search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("group/subgroup/repo"),
            "test",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");
    }
}
