//! GitLab Releases API provider.
//!
//! Uses the GitLab REST API `/api/v4/projects/:id/releases` with a
//! personal access token passed via the `PRIVATE-TOKEN` header.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::code_metadata::CodeHost;
use crate::core::source_card::ReleaseMetadata;

const ENGINE: &str = "gitlab_releases";
const DEFAULT_BASE_URL: &str = "https://gitlab.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

/// Parsed GitLab release asset link.
#[derive(Debug, Deserialize)]
struct GitlabReleaseLink {
    name: Option<String>,
    url: Option<String>,
}

/// Parsed GitLab release asset container.
#[derive(Debug, Deserialize)]
struct GitlabReleaseAssets {
    #[serde(default)]
    links: Vec<GitlabReleaseLink>,
}

/// Parsed GitLab release item.
#[derive(Debug, Deserialize)]
struct GitlabReleaseItem {
    tag_name: Option<String>,
    name: Option<String>,
    web_url: Option<String>,
    assets: Option<GitlabReleaseAssets>,
    released_at: Option<String>,
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

/// Search for releases, optionally scoped to a specific project.
///
/// When `project_id` is `Some`, uses:
/// `GET {base}/api/v4/projects/{encoded_id}/releases?per_page={per_page}`
///
/// When `project_id` is `None`, returns empty results (releases
/// require a known project).
pub async fn search_with_project(
    client: &Client,
    api_key: &str,
    base_url: Option<&str>,
    project_id: Option<&str>,
    _query: &str,
    max_results: usize,
    timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    if max_results == 0 {
        return Ok(Vec::new());
    }

    let Some(pid) = project_id else {
        return Ok(Vec::new());
    };

    let base = base_url.unwrap_or(DEFAULT_BASE_URL);
    let per_page = max_results.clamp(1, 100);
    let encoded = urlencoding::encode(pid);
    let url = format!("{base}/api/v4/projects/{encoded}/releases");

    let response = tokio::time::timeout(
        timeout,
        client
            .get(&url)
            .query(&[("per_page", &per_page.to_string())])
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

    let parsed: Vec<GitlabReleaseItem> =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed, max_results))
}

/// Build a snippet from asset links: concatenates link names up to
/// `SNIPPET_MAX_CHARS`.
fn build_asset_snippet(assets: &GitlabReleaseAssets) -> Option<String> {
    let names: Vec<&str> = assets
        .links
        .iter()
        .filter_map(|l| l.name.as_deref())
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        return None;
    }
    let combined = names.join(", ");
    let truncated = truncate_body(&combined, SNIPPET_MAX_CHARS);
    let trimmed = truncated.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Pick the best URL for a release: prefer `web_url`, fall back to
/// the first asset link URL.
fn release_url(item: &GitlabReleaseItem) -> Option<String> {
    if let Some(url) = &item.web_url {
        if !url.is_empty() && url.starts_with("http") {
            return Some(url.clone());
        }
    }
    item.assets
        .as_ref()
        .and_then(|a| a.links.first())
        .and_then(|l| l.url.clone())
        .filter(|u| !u.is_empty() && u.starts_with("http"))
}

/// Extract owner and repo from a GitLab web_url.
///
/// GitLab release URLs follow the pattern:
/// `https://gitlab.com/{owner}/{repo}/-/releases/{tag}`
/// Nested namespaces are preserved in `owner` (e.g. `group/subgroup`).
fn parse_owner_repo_from_url(web_url: &str) -> (Option<String>, Option<String>) {
    let after_scheme = web_url
        .find("://")
        .map(|i| &web_url[i + 3..])
        .unwrap_or(web_url);
    let after_host = after_scheme
        .find('/')
        .map(|i| &after_scheme[i..])
        .unwrap_or("");
    let path = after_host.strip_prefix('/').unwrap_or(after_host);
    if let Some(pos) = path.find("/-/") {
        let namespace = &path[..pos];
        let mut parts = namespace.rsplitn(2, '/');
        let repo = parts.next().map(|s| s.to_string());
        let owner = parts.next().map(|s| s.to_string());
        return (owner, repo);
    }
    (None, None)
}

fn convert(items: Vec<GitlabReleaseItem>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(items.len()));
    for item in items {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = release_url(&item) else {
            continue;
        };

        let tag = item.tag_name.as_deref().unwrap_or("");
        let name = item.name.as_deref().unwrap_or("");

        let title = if name.is_empty() {
            tag.to_string()
        } else {
            format!("{tag} {name}")
        };

        let snippet = item.assets.as_ref().and_then(build_asset_snippet);

        let (owner, repo) = parse_owner_repo_from_url(&url);

        let metadata = ResultMetadata::Release(ReleaseMetadata {
            host: Some(CodeHost::Gitlab),
            owner,
            repo,
            tag: item.tag_name.clone(),
            name: item.name.clone(),
            draft: None,
            prerelease: None,
            created_at: None,
            published_at: item.released_at.clone(),
        });

        out.push(SearchResult {
            title,
            url,
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
            GitlabReleaseItem {
                tag_name: Some("v0.7.0".to_string()),
                name: Some("Release v0.7.0".to_string()),
                web_url: Some(
                    "https://gitlab.com/tokio-rs/axum/-/releases/v0.7.0".to_string(),
                ),
                assets: Some(GitlabReleaseAssets {
                    links: vec![
                        GitlabReleaseLink {
                            name: Some("Source code (tar.gz)".to_string()),
                            url: Some("https://gitlab.com/tokio-rs/axum/-/archive/v0.7.0/axum-v0.7.0.tar.gz".to_string()),
                        },
                        GitlabReleaseLink {
                            name: Some("Source code (zip)".to_string()),
                            url: Some("https://gitlab.com/tokio-rs/axum/-/archive/v0.7.0/axum-v0.7.0.zip".to_string()),
                        },
                    ],
                }),
                released_at: Some("2024-01-16T12:00:00Z".to_string()),
            },
            GitlabReleaseItem {
                tag_name: Some("v0.6.0".to_string()),
                name: None,
                web_url: Some(
                    "https://gitlab.com/tokio-rs/axum/-/releases/v0.6.0".to_string(),
                ),
                assets: None,
                released_at: Some("2023-12-02T10:00:00Z".to_string()),
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "v0.7.0 Release v0.7.0");
        assert_eq!(
            out[0].url,
            "https://gitlab.com/tokio-rs/axum/-/releases/v0.7.0"
        );
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("Source code (tar.gz), Source code (zip)")
        );
        assert_eq!(out[0].source_engine, "gitlab_releases");
        match &out[0].metadata {
            ResultMetadata::Release(m) => {
                assert_eq!(m.host, Some(CodeHost::Gitlab));
                assert_eq!(m.owner.as_deref(), Some("tokio-rs"));
                assert_eq!(m.repo.as_deref(), Some("axum"));
                assert_eq!(m.tag.as_deref(), Some("v0.7.0"));
                assert_eq!(m.name.as_deref(), Some("Release v0.7.0"));
                assert_eq!(m.published_at.as_deref(), Some("2024-01-16T12:00:00Z"));
            }
            other => panic!("expected Release metadata, got: {other:?}"),
        }
        assert_eq!(out[1].title, "v0.6.0");
        match &out[1].metadata {
            ResultMetadata::Release(m) => {
                assert!(m.name.is_none());
            }
            other => panic!("expected Release metadata, got: {other:?}"),
        }
    }

    #[test]
    fn test_convert_respects_max_results() {
        let items: Vec<GitlabReleaseItem> = (0..5)
            .map(|i| GitlabReleaseItem {
                tag_name: Some(format!("v{i}.0.0")),
                name: Some(format!("Release {i}")),
                web_url: Some(format!("https://gitlab.com/test/repo/-/releases/v{i}.0.0")),
                assets: None,
                released_at: None,
            })
            .collect();
        let out = convert(items, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_url() {
        let items = vec![GitlabReleaseItem {
            tag_name: Some("v1.0.0".to_string()),
            name: Some("Release".to_string()),
            web_url: None,
            assets: None,
            released_at: None,
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_url() {
        let items = vec![GitlabReleaseItem {
            tag_name: Some("v1.0.0".to_string()),
            name: Some("Release".to_string()),
            web_url: Some(String::new()),
            assets: None,
            released_at: None,
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let items = vec![
            GitlabReleaseItem {
                tag_name: Some("v1.0.0".to_string()),
                name: Some("Release".to_string()),
                web_url: Some("ftp://example.com/releases/1".to_string()),
                assets: None,
                released_at: None,
            },
            GitlabReleaseItem {
                tag_name: Some("v2.0.0".to_string()),
                name: Some("Release".to_string()),
                web_url: Some("https://gitlab.com/test/repo/-/releases/v2.0.0".to_string()),
                assets: None,
                released_at: None,
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "v2.0.0 Release");
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let items = vec![GitlabReleaseItem {
            tag_name: Some("v1.0.0".to_string()),
            name: Some("Release".to_string()),
            web_url: Some("https://gitlab.com/test/repo/-/releases/v1.0.0".to_string()),
            assets: Some(GitlabReleaseAssets { links: vec![] }),
            released_at: None,
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
    fn test_build_asset_snippet_empty() {
        let assets = GitlabReleaseAssets { links: vec![] };
        assert!(build_asset_snippet(&assets).is_none());
    }

    #[test]
    fn test_build_asset_snippet_with_names() {
        let assets = GitlabReleaseAssets {
            links: vec![
                GitlabReleaseLink {
                    name: Some("tar.gz".to_string()),
                    url: None,
                },
                GitlabReleaseLink {
                    name: Some("zip".to_string()),
                    url: None,
                },
            ],
        };
        assert_eq!(build_asset_snippet(&assets).as_deref(), Some("tar.gz, zip"));
    }

    #[test]
    fn test_release_url_prefers_web_url() {
        let item = GitlabReleaseItem {
            tag_name: Some("v1".to_string()),
            name: None,
            web_url: Some("https://gitlab.com/test/repo/-/releases/v1".to_string()),
            assets: Some(GitlabReleaseAssets {
                links: vec![GitlabReleaseLink {
                    name: None,
                    url: Some("https://example.com/fallback".to_string()),
                }],
            }),
            released_at: None,
        };
        assert_eq!(
            release_url(&item).as_deref(),
            Some("https://gitlab.com/test/repo/-/releases/v1")
        );
    }

    #[test]
    fn test_release_url_falls_back_to_asset_link() {
        let item = GitlabReleaseItem {
            tag_name: Some("v1".to_string()),
            name: None,
            web_url: None,
            assets: Some(GitlabReleaseAssets {
                links: vec![GitlabReleaseLink {
                    name: None,
                    url: Some("https://example.com/download".to_string()),
                }],
            }),
            released_at: None,
        };
        assert_eq!(
            release_url(&item).as_deref(),
            Some("https://example.com/download")
        );
    }

    #[test]
    fn test_parse_json_array() {
        let body = r#"[
            {
                "tag_name": "v0.7.0",
                "name": "Release v0.7.0",
                "web_url": "https://gitlab.com/test/repo/-/releases/v0.7.0",
                "assets": {"links": [{"name": "tar.gz", "url": "https://example.com/tar.gz"}]},
                "released_at": "2024-01-16T12:00:00Z"
            }
        ]"#;
        let parsed: Vec<GitlabReleaseItem> = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].tag_name.as_deref(), Some("v0.7.0"));
    }

    #[test]
    fn test_parse_json_array_empty() {
        let body = r#"[]"#;
        let parsed: Vec<GitlabReleaseItem> = serde_json::from_str(body).unwrap();
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

    #[test]
    fn test_parse_owner_repo_from_url() {
        let (owner, repo) =
            parse_owner_repo_from_url("https://gitlab.com/tokio-rs/axum/-/releases/v1.0");
        assert_eq!(owner.as_deref(), Some("tokio-rs"));
        assert_eq!(repo.as_deref(), Some("axum"));

        let (owner, repo) =
            parse_owner_repo_from_url("https://gitlab.com/group/subgroup/project/-/releases/v1.0");
        assert_eq!(owner.as_deref(), Some("group/subgroup"));
        assert_eq!(repo.as_deref(), Some("project"));
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
                .path("/api/v4/projects/12345/releases")
                .header("PRIVATE-TOKEN", "test-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"[
                        {
                            "tag_name": "v0.7.0",
                            "name": "Release v0.7.0",
                            "web_url": "https://gitlab.com/12345/-/releases/v0.7.0",
                            "assets": {"links": [{"name": "tar.gz", "url": "https://example.com/tar.gz"}]},
                            "released_at": "2024-01-16T12:00:00Z"
                        },
                        {
                            "tag_name": "v0.6.0",
                            "name": null,
                            "web_url": "https://gitlab.com/12345/-/releases/v0.6.0",
                            "released_at": "2023-12-02T10:00:00Z"
                        }
                    ]"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("12345"),
            "anything",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "v0.7.0 Release v0.7.0");
        assert_eq!(results[0].source_engine, "gitlab_releases");
    }

    #[tokio::test]
    async fn test_empty_result_set() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v4/projects/99999/releases");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"[]"#);
        });

        let client = reqwest::Client::new();
        let results = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("99999"),
            "anything",
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
            when.method(GET).path("/api/v4/projects/1/releases");
            then.status(401).body("401 Unauthorized");
        });

        let client = reqwest::Client::new();
        let err = search_with_project(
            &client,
            "bad-token",
            Some(&server.url("")),
            Some("1"),
            "anything",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 401");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "gitlab_releases");
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
            when.method(GET).path("/api/v4/projects/1/releases");
            then.status(403).body("rate limit exceeded");
        });

        let client = reqwest::Client::new();
        let err = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("1"),
            "anything",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 403");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "gitlab_releases");
                assert_eq!(status, 403);
            }
            other => panic!("expected BadStatus(403), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_not_found_404() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v4/projects/999/releases");
            then.status(404).body("Not Found");
        });

        let client = reqwest::Client::new();
        let err = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("999"),
            "anything",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 404");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "gitlab_releases");
                assert_eq!(status, 404);
            }
            other => panic!("expected BadStatus(404), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_server_error_500() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v4/projects/1/releases");
            then.status(500).body("Internal Server Error");
        });

        let client = reqwest::Client::new();
        let err = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("1"),
            "anything",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 500");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "gitlab_releases");
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
            when.method(GET).path("/api/v4/projects/1/releases");
            then.status(200)
                .header("content-type", "application/json")
                .body("this is not json");
        });

        let client = reqwest::Client::new();
        let err = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("1"),
            "anything",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with parse error");

        match err {
            EngineError::ParseFailed { engine, reason } => {
                assert_eq!(engine, "gitlab_releases");
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
                .path("/api/v4/projects/1/releases")
                .query_param("per_page", "2");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"[
                        {"tag_name": "v3.0.0", "web_url": "https://gitlab.com/1/-/releases/v3.0.0"},
                        {"tag_name": "v2.0.0", "web_url": "https://gitlab.com/1/-/releases/v2.0.0"},
                        {"tag_name": "v1.0.0", "web_url": "https://gitlab.com/1/-/releases/v1.0.0"}
                    ]"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("1"),
            "anything",
            2,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "v3.0.0");
        assert_eq!(results[1].title, "v2.0.0");
    }

    #[tokio::test]
    async fn test_api_key_sent_in_header() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/api/v4/projects/1/releases")
                .header("PRIVATE-TOKEN", "my-secret-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"[]"#);
        });

        let client = reqwest::Client::new();
        search_with_project(
            &client,
            "my-secret-token",
            Some(&server.url("")),
            Some("1"),
            "anything",
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
            when.method(GET).path("/api/v4/projects/1/releases");
            then.status(200)
                .header("content-type", "application/json")
                .delay(std::time::Duration::from_secs(10))
                .body(r#"[]"#);
        });

        let client = reqwest::Client::new();
        let err = search_with_project(
            &client,
            "test-token",
            Some(&server.url("")),
            Some("1"),
            "anything",
            10,
            Duration::from_millis(50),
        )
        .await
        .expect_err("should fail with timeout");

        match err {
            EngineError::Timeout { engine } => {
                assert_eq!(engine, "gitlab_releases");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_no_project_returns_empty() {
        let client = reqwest::Client::new();
        let results = search_with_project(
            &client,
            "test-token",
            None,
            None,
            "anything",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_project_scoped_url_encoding() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/api/v4/projects/group%2Fsubgroup%2Frepo/releases");
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
            "anything",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");
    }

    #[test]
    fn test_provider_descriptor_for_gitlab_releases() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("gitlab_releases", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "gitlab_releases");
        assert_eq!(desc.display_name, "GitLab Releases");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_release_search);
        assert!(desc.capabilities.supports_repo_filter);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_issue_search);
        assert!(!desc.capabilities.supports_org_filter);
        assert!(!desc.capabilities.supports_path_filter);
    }

    #[test]
    fn test_provider_descriptor_gitlab_releases_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("gitlab_releases", false, false, true, false, None, None)
                .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
