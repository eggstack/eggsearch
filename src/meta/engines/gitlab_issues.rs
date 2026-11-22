//! GitLab Issues API provider.
//!
//! Uses the GitLab REST API `/api/v4/projects/:id/issues` (project-scoped)
//! or `/api/v4/search?scope=issues` (global) with a personal access
//! token passed via the `PRIVATE-TOKEN` header.

use std::time::Duration;

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::truncate_body;
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

    let per_page = per_page.to_string();
    let query_params = [("search", query), ("per_page", per_page.as_str())];
    let headers = [("PRIVATE-TOKEN", api_key)];
    let parsed: Vec<GitlabIssueItem> = super::get_json_bounded(
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

/// Extract owner and repo from a GitLab web_url.
///
/// GitLab issue URLs follow the pattern:
/// `https://gitlab.com/{owner}/{repo}/-/issues/{number}`
/// Nested namespaces are preserved in `owner` (e.g. `group/subgroup`).
fn parse_owner_repo_from_url(web_url: &str) -> (Option<String>, Option<String>) {
    // Find "://..." then skip host, look for "/{namespace}/-/" pattern
    let after_scheme = web_url
        .find("://")
        .map(|i| &web_url[i + 3..])
        .unwrap_or(web_url);
    let after_host = after_scheme
        .find('/')
        .map(|i| &after_scheme[i..])
        .unwrap_or("");
    // after_host starts with "/{namespace}/-/..."
    let path = after_host.strip_prefix('/').unwrap_or(after_host);
    // Find "/-/" separator
    if let Some(pos) = path.find("/-/") {
        let namespace = &path[..pos];
        // namespace is "owner/repo" or "group/subgroup/repo"
        // For issues: repo is the last segment, owner is everything before
        let mut parts = namespace.rsplitn(2, '/');
        let repo = parts.next().map(|s| s.to_string());
        let owner = parts.next().map(|s| s.to_string());
        return (owner, repo);
    }
    (None, None)
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
        if !super::is_http_url(web_url) {
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
            .map(|s| crate::core::sanitize::normalize_whitespace(&s))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let (owner, repo) = parse_owner_repo_from_url(web_url);

        let metadata = ResultMetadata::Issue(IssueMetadata {
            host: Some(CodeHost::Gitlab),
            owner,
            repo,
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
                assert_eq!(m.owner.as_deref(), Some("tokio-rs"));
                assert_eq!(m.repo.as_deref(), Some("axum"));
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
    fn test_parse_owner_repo_from_url() {
        let (owner, repo) =
            parse_owner_repo_from_url("https://gitlab.com/tokio-rs/axum/-/issues/123");
        assert_eq!(owner.as_deref(), Some("tokio-rs"));
        assert_eq!(repo.as_deref(), Some("axum"));

        let (owner, repo) =
            parse_owner_repo_from_url("https://gitlab.com/group/subgroup/project/-/issues/1");
        assert_eq!(owner.as_deref(), Some("group/subgroup"));
        assert_eq!(repo.as_deref(), Some("project"));

        let (owner, repo) = parse_owner_repo_from_url("https://gitlab.com/some/path");
        assert!(owner.is_none());
        assert!(repo.is_none());
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

        let client = crate::meta::engines::build_http_client(None).expect("test client");
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
    fn test_provider_descriptor_for_gitlab_issues() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("gitlab_issues", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "gitlab_issues");
        assert_eq!(desc.display_name, "GitLab Issues Search");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_issue_search);
        assert!(desc.capabilities.supports_repo_filter);
        assert!(desc.capabilities.supports_org_filter);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_release_search);
        assert!(!desc.capabilities.supports_path_filter);
    }

    #[test]
    fn test_provider_descriptor_gitlab_issues_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("gitlab_issues", false, false, true, false, None, None)
                .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
