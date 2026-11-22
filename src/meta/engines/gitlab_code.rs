//! GitLab Code Search API provider.
//!
//! Uses the GitLab REST API `/api/v4/search?scope=blobs` (global) or
//! `/api/v4/projects/:id/search?scope=blobs` (project-scoped) with a
//! personal access token passed via the `PRIVATE-TOKEN` header.

use std::time::Duration;

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{CodeSearchMetadata, ResultMetadata, SearchResult};
use super::truncate_body;

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

    let per_page = per_page.to_string();
    let query_params = [("search", query), ("per_page", per_page.as_str())];
    let headers = [("PRIVATE-TOKEN", api_key)];
    let parsed: Vec<GitlabCodeBlob> = super::get_json_bounded(
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
            excerpts: Vec::new(),
            published_at: None,
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

        let client = crate::meta::engines::build_http_client(None).expect("test client");
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

        let client = crate::meta::engines::build_http_client(None).expect("test client");
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
