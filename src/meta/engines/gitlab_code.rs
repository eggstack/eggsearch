//! GitLab Code Search API provider.
//!
//! Uses the GitLab REST API `/api/v4/search?scope=blobs` (global) or
//! `/api/v4/projects/:id/search?scope=blobs` (project-scoped) with a
//! personal access token passed via the `PRIVATE-TOKEN` header.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{CodeSearchMetadata, ResultMetadata, SearchResult};

const ENGINE: &str = "gitlab_code";
const DEFAULT_BASE_URL: &str = "https://gitlab.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

/// Parsed GitLab code search API response.
///
/// The GitLab search API returns a JSON array of blobs directly (no
/// wrapper object).
#[derive(Debug, Deserialize)]
struct GitlabCodeBlob {
    path: Option<String>,
    #[allow(dead_code)]
    filename: Option<String>,
    data: Option<String>,
    #[allow(dead_code)]
    r#ref: Option<String>,
    url: Option<String>,
    project_id: Option<u64>,
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

/// Search for code, optionally scoped to a specific project.
///
/// When `project_id` is `Some`, uses the project-scoped endpoint:
/// `GET {base}/api/v4/projects/{encoded_id}/search?scope=blobs&search={query}`
///
/// When `project_id` is `None`, uses the global endpoint:
/// `GET {base}/api/v4/search?scope=blobs&search={query}`
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
            format!("{base}/api/v4/projects/{encoded}/search?scope=blobs")
        }
        None => format!("{base}/api/v4/search?scope=blobs"),
    };

    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(&url)
            .query(&[("search", query), ("per_page", &per_page.to_string())])
            .header("PRIVATE-TOKEN", api_key)
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

    let parsed: Vec<GitlabCodeBlob> =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed, max_results))
}

#[allow(dead_code)]
fn build_blob_url(
    base: &str,
    project_id: Option<u64>,
    ref_name: Option<&str>,
    path: Option<&str>,
) -> Option<String> {
    let path = path?;
    let pid = project_id?;
    let r = ref_name.unwrap_or("main");
    Some(format!("{base}/{pid}/-/blob/{r}/{path}"))
}

fn convert(items: Vec<GitlabCodeBlob>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(items.len()));
    for item in items {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = &item.url else {
            continue;
        };
        if !super::is_http_url(url) {
            continue;
        }
        let path = match item.path.as_deref() {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };

        let project_id_str = item
            .project_id
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let title = format!("{path} - {project_id_str}");

        let snippet = item
            .data
            .as_deref()
            .map(|b| truncate_body(b, SNIPPET_MAX_CHARS))
            .map(|s| crate::core::sanitize::normalize_whitespace(&s))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let text_fragment = snippet.clone().filter(|s| !s.is_empty());

        let metadata = ResultMetadata::CodeSearch(CodeSearchMetadata {
            matched_symbol: None,
            text_fragment,
        });

        out.push(SearchResult {
            title,
            url: url.clone(),
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
            GitlabCodeBlob {
                path: Some("src/lib.rs".to_string()),
                filename: Some("lib.rs".to_string()),
                data: Some("fn main() {}".to_string()),
                r#ref: Some("main".to_string()),
                url: Some("https://gitlab.com/12345/-/blob/main/src/lib.rs".to_string()),
                project_id: Some(12345),
            },
            GitlabCodeBlob {
                path: Some("src/main.rs".to_string()),
                filename: Some("main.rs".to_string()),
                data: None,
                r#ref: Some("main".to_string()),
                url: Some("https://gitlab.com/12345/-/blob/main/src/main.rs".to_string()),
                project_id: Some(12345),
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "src/lib.rs - 12345");
        assert_eq!(
            out[0].url,
            "https://gitlab.com/12345/-/blob/main/src/lib.rs"
        );
        assert_eq!(out[0].snippet.as_deref(), Some("fn main() {}"));
        assert_eq!(out[0].source_engine, "gitlab_code");
        match &out[0].metadata {
            ResultMetadata::CodeSearch(m) => {
                assert_eq!(m.text_fragment.as_deref(), Some("fn main() {}"));
            }
            other => panic!("expected CodeSearch metadata, got: {other:?}"),
        }
        assert_eq!(out[1].title, "src/main.rs - 12345");
        assert!(out[1].snippet.is_none());
    }

    #[test]
    fn test_convert_respects_max_results() {
        let items: Vec<GitlabCodeBlob> = (0..5)
            .map(|i| GitlabCodeBlob {
                path: Some(format!("src/f{i}.rs")),
                filename: Some(format!("f{i}.rs")),
                data: None,
                r#ref: Some("main".to_string()),
                url: Some(format!("https://gitlab.com/1/-/blob/main/src/f{i}.rs")),
                project_id: Some(1),
            })
            .collect();
        let out = convert(items, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_url() {
        let items = vec![GitlabCodeBlob {
            path: Some("src/lib.rs".to_string()),
            filename: Some("lib.rs".to_string()),
            data: None,
            r#ref: Some("main".to_string()),
            url: None,
            project_id: Some(1),
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_url() {
        let items = vec![GitlabCodeBlob {
            path: Some("src/lib.rs".to_string()),
            filename: Some("lib.rs".to_string()),
            data: None,
            r#ref: Some("main".to_string()),
            url: Some(String::new()),
            project_id: Some(1),
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let items = vec![
            GitlabCodeBlob {
                path: Some("a.rs".to_string()),
                filename: Some("a.rs".to_string()),
                data: None,
                r#ref: Some("main".to_string()),
                url: Some("ftp://example.com/a.rs".to_string()),
                project_id: Some(1),
            },
            GitlabCodeBlob {
                path: Some("b.rs".to_string()),
                filename: Some("b.rs".to_string()),
                data: None,
                r#ref: Some("main".to_string()),
                url: Some("https://gitlab.com/1/-/blob/main/b.rs".to_string()),
                project_id: Some(1),
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "b.rs - 1");
    }

    #[test]
    fn test_convert_skips_missing_path() {
        let items = vec![GitlabCodeBlob {
            path: None,
            filename: Some("lib.rs".to_string()),
            data: None,
            r#ref: Some("main".to_string()),
            url: Some("https://gitlab.com/1/-/blob/main/lib.rs".to_string()),
            project_id: Some(1),
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let items = vec![GitlabCodeBlob {
            path: Some("src/lib.rs".to_string()),
            filename: Some("lib.rs".to_string()),
            data: Some(String::new()),
            r#ref: Some("main".to_string()),
            url: Some("https://gitlab.com/1/-/blob/main/src/lib.rs".to_string()),
            project_id: Some(1),
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_max_results_zero_returns_empty() {
        let out = convert(vec![], 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_build_blob_url() {
        assert_eq!(
            build_blob_url(
                "https://gitlab.com",
                Some(12345),
                Some("main"),
                Some("src/lib.rs")
            ),
            Some("https://gitlab.com/12345/-/blob/main/src/lib.rs".to_string())
        );
    }

    #[test]
    fn test_build_blob_url_no_path() {
        assert!(build_blob_url("https://gitlab.com", Some(1), Some("main"), None).is_none());
    }

    #[test]
    fn test_build_blob_url_no_project_id() {
        assert!(build_blob_url("https://gitlab.com", None, Some("main"), Some("a.rs")).is_none());
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

    #[test]
    fn test_parse_json_array() {
        let body = r#"[
            {"path": "src/lib.rs", "filename": "lib.rs", "data": "fn main() {}", "ref": "main", "url": "https://gitlab.com/1/-/blob/main/src/lib.rs", "project_id": 1},
            {"path": "src/main.rs", "filename": "main.rs", "data": null, "ref": "main", "url": "https://gitlab.com/1/-/blob/main/src/main.rs", "project_id": 1}
        ]"#;
        let parsed: Vec<GitlabCodeBlob> = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].path.as_deref(), Some("src/lib.rs"));
    }

    #[test]
    fn test_parse_json_array_empty() {
        let body = r#"[]"#;
        let parsed: Vec<GitlabCodeBlob> = serde_json::from_str(body).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_convert_populates_code_search_metadata() {
        let items = vec![GitlabCodeBlob {
            path: Some("src/lib.rs".to_string()),
            filename: Some("lib.rs".to_string()),
            data: Some("fn router() {}".to_string()),
            r#ref: Some("main".to_string()),
            url: Some("https://gitlab.com/12345/-/blob/main/src/lib.rs".to_string()),
            project_id: Some(12345),
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        match &out[0].metadata {
            ResultMetadata::CodeSearch(m) => {
                assert_eq!(m.text_fragment.as_deref(), Some("fn router() {}"));
            }
            other => panic!("expected CodeSearch metadata, got: {other:?}"),
        }
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
                        {"path": "src/lib.rs", "filename": "lib.rs", "data": "pub fn router() -> Router {}", "ref": "main", "url": "https://gitlab.com/12345/-/blob/main/src/lib.rs", "project_id": 12345},
                        {"path": "src/main.rs", "filename": "main.rs", "data": "fn main() {}", "ref": "main", "url": "https://gitlab.com/12345/-/blob/main/src/main.rs", "project_id": 12345},
                        {"path": "src/handler.rs", "filename": "handler.rs", "data": "pub async fn handler() {}", "ref": "main", "url": "https://gitlab.com/12345/-/blob/main/src/handler.rs", "project_id": 12345}
                    ]"#,
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
        assert_eq!(results[0].title, "src/lib.rs - 12345");
        assert_eq!(
            results[0].url,
            "https://gitlab.com/12345/-/blob/main/src/lib.rs"
        );
        assert_eq!(
            results[0].snippet.as_deref(),
            Some("pub fn router() -> Router {}")
        );
        assert_eq!(results[0].source_engine, "gitlab_code");
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
                assert_eq!(engine, "gitlab_code");
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
                assert_eq!(engine, "gitlab_code");
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
                assert_eq!(engine, "gitlab_code");
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
                assert_eq!(engine, "gitlab_code");
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
                        {"path": "src/a.rs", "filename": "a.rs", "data": "a", "ref": "main", "url": "https://gitlab.com/1/-/blob/main/src/a.rs", "project_id": 1},
                        {"path": "src/b.rs", "filename": "b.rs", "data": "b", "ref": "main", "url": "https://gitlab.com/1/-/blob/main/src/b.rs", "project_id": 1},
                        {"path": "src/c.rs", "filename": "c.rs", "data": "c", "ref": "main", "url": "https://gitlab.com/1/-/blob/main/src/c.rs", "project_id": 1}
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
        assert_eq!(results[0].title, "src/a.rs - 1");
        assert_eq!(results[1].title, "src/b.rs - 1");
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
                assert_eq!(engine, "gitlab_code");
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
                .path("/api/v4/projects/12345/search")
                .query_param("scope", "blobs")
                .query_param("search", "Router")
                .header("PRIVATE-TOKEN", "test-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"[{"path": "src/lib.rs", "data": "fn router()", "ref": "main", "url": "https://gitlab.com/12345/-/blob/main/src/lib.rs", "project_id": 12345}]"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("12345"),
            "Router",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "src/lib.rs - 12345");
    }

    #[tokio::test]
    async fn test_project_scoped_url_encoding() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/api/v4/projects/group%2Fsubgroup%2Frepo/search")
                .query_param("scope", "blobs");
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

    #[test]
    fn test_provider_descriptor_for_gitlab_code() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("gitlab_code", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "gitlab_code");
        assert_eq!(desc.display_name, "GitLab Code Search");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_code_search);
        assert!(desc.capabilities.supports_repo_filter);
        assert!(desc.capabilities.supports_org_filter);
        assert!(desc.capabilities.supports_path_filter);
        assert!(!desc.capabilities.supports_language_filter);
        assert!(!desc.capabilities.supports_symbol_hint);
        assert!(!desc.capabilities.supports_issue_search);
        assert!(!desc.capabilities.supports_release_search);
        assert!(!desc.capabilities.supports_result_timestamps);
    }

    #[test]
    fn test_provider_descriptor_gitlab_code_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("gitlab_code", false, false, true, false, None, None)
                .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
