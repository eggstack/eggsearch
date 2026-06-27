use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::code_metadata::CodeHost;
use crate::core::source_card::ReleaseMetadata;

const ENGINE: &str = "github_releases";
const DEFAULT_BASE_URL: &str = "https://api.github.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
struct GithubReleasesResponse {
    #[serde(default)]
    items: Vec<GithubReleaseItem>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseItem {
    tag_name: Option<String>,
    name: Option<String>,
    html_url: Option<String>,
    body: Option<String>,
    draft: Option<bool>,
    prerelease: Option<bool>,
    created_at: Option<String>,
    published_at: Option<String>,
}

fn parse_repo_from_query(query: &str) -> Option<(String, String)> {
    for part in query.split_whitespace() {
        if let Some(rest) = part.strip_prefix("repo:") {
            let rest = rest.trim();
            if let Some((owner, repo)) = rest.split_once('/') {
                if !owner.is_empty() && !repo.is_empty() {
                    return Some((owner.to_string(), repo.to_string()));
                }
            }
        }
    }
    None
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

    let Some((owner, repo)) = parse_repo_from_query(query) else {
        return Ok(Vec::new());
    };

    let base = base_url.unwrap_or(DEFAULT_BASE_URL);
    let url = format!("{base}/repos/{owner}/{repo}/releases");

    let per_page = max_results.clamp(1, 100);

    let response = tokio::time::timeout(
        timeout,
        client
            .get(&url)
            .query(&[("per_page", &per_page.to_string())])
            .header("Accept", "application/vnd.github+json")
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

    let parsed: GithubReleasesResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed.items, max_results, &owner, &repo))
}

/// UTF-8-safe snippet truncation that preserves the historical
/// word-boundary-trim semantics without ever slicing by byte offset
/// inside a multi-byte code point.
///
/// Counts Unicode scalar values (chars), not bytes. The returned
/// `pos` from `rfind(char::is_whitespace)` is a valid UTF-8 boundary
/// because it indexes inside the already-valid truncated string.
/// UTF-8-safe snippet truncation. See `github_issues::truncate_body`
/// for the full contract — the impl is mirrored here so each engine
/// stands alone.
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

fn convert(
    items: Vec<GithubReleaseItem>,
    max_results: usize,
    owner: &str,
    repo: &str,
) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(items.len()));
    for item in items {
        if out.len() >= max_results {
            break;
        }
        if item.draft == Some(true) {
            continue;
        }
        let Some(html_url) = &item.html_url else {
            continue;
        };
        if html_url.is_empty() || !html_url.starts_with("http") {
            continue;
        }
        let tag = item.tag_name.as_deref().unwrap_or("");
        let name = item.name.as_deref().unwrap_or("");

        let title = if name.is_empty() {
            format!("{tag} - {owner}/{repo}")
        } else {
            format!("{tag} {name} - {owner}/{repo}")
        };

        let snippet = item
            .body
            .as_deref()
            .map(|b| truncate_body(b, SNIPPET_MAX_CHARS))
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let metadata = ResultMetadata::Release(ReleaseMetadata {
            host: Some(CodeHost::Github),
            owner: Some(owner.to_string()),
            repo: Some(repo.to_string()),
            tag: item.tag_name.clone(),
            name: item.name.clone(),
            draft: item.draft,
            prerelease: item.prerelease,
            created_at: item.created_at.clone(),
            published_at: item.published_at.clone(),
        });

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo_from_query() {
        assert_eq!(
            parse_repo_from_query("repo:tokio-rs/axum"),
            Some(("tokio-rs".to_string(), "axum".to_string()))
        );
        assert_eq!(
            parse_repo_from_query("some query repo:owner/name other"),
            Some(("owner".to_string(), "name".to_string()))
        );
    }

    #[test]
    fn test_parse_repo_from_query_missing() {
        assert!(parse_repo_from_query("just a query").is_none());
        assert!(parse_repo_from_query("repo:").is_none());
        assert!(parse_repo_from_query("repo:owner/").is_none());
        assert!(parse_repo_from_query("repo:/repo").is_none());
    }

    #[test]
    fn test_convert_extracts_results() {
        let items = vec![
            GithubReleaseItem {
                tag_name: Some("v0.7.0".to_string()),
                name: Some("Release v0.7.0".to_string()),
                html_url: Some("https://github.com/tokio-rs/axum/releases/tag/v0.7.0".to_string()),
                body: Some("Bug fixes and improvements".to_string()),
                draft: Some(false),
                prerelease: Some(false),
                created_at: Some("2024-01-15T10:30:00Z".to_string()),
                published_at: Some("2024-01-16T12:00:00Z".to_string()),
            },
            GithubReleaseItem {
                tag_name: Some("v0.6.0".to_string()),
                name: None,
                html_url: Some("https://github.com/tokio-rs/axum/releases/tag/v0.6.0".to_string()),
                body: Some("Previous release".to_string()),
                draft: Some(false),
                prerelease: Some(false),
                created_at: Some("2023-12-01T10:00:00Z".to_string()),
                published_at: Some("2023-12-02T10:00:00Z".to_string()),
            },
        ];
        let out = convert(items, 10, "tokio-rs", "axum");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "v0.7.0 Release v0.7.0 - tokio-rs/axum");
        assert_eq!(
            out[0].url,
            "https://github.com/tokio-rs/axum/releases/tag/v0.7.0"
        );
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("Bug fixes and improvements")
        );
        assert_eq!(out[0].source_engine, "github_releases");
        match &out[0].metadata {
            ResultMetadata::Release(m) => {
                assert_eq!(m.host, Some(CodeHost::Github));
                assert_eq!(m.owner.as_deref(), Some("tokio-rs"));
                assert_eq!(m.repo.as_deref(), Some("axum"));
                assert_eq!(m.tag.as_deref(), Some("v0.7.0"));
                assert_eq!(m.name.as_deref(), Some("Release v0.7.0"));
                assert_eq!(m.draft, Some(false));
                assert_eq!(m.prerelease, Some(false));
            }
            other => panic!("expected Release metadata, got: {other:?}"),
        }
        assert_eq!(out[1].title, "v0.6.0 - tokio-rs/axum");
        match &out[1].metadata {
            ResultMetadata::Release(m) => {
                assert!(m.name.is_none());
            }
            other => panic!("expected Release metadata, got: {other:?}"),
        }
    }

    #[test]
    fn test_convert_skips_draft_releases() {
        let items = vec![
            GithubReleaseItem {
                tag_name: Some("v1.0.0".to_string()),
                name: Some("Release".to_string()),
                html_url: Some("https://github.com/test/repo/releases/tag/v1.0.0".to_string()),
                body: None,
                draft: Some(false),
                prerelease: Some(false),
                created_at: None,
                published_at: None,
            },
            GithubReleaseItem {
                tag_name: Some("v1.1.0-draft".to_string()),
                name: Some("Draft".to_string()),
                html_url: Some(
                    "https://github.com/test/repo/releases/tag/v1.1.0-draft".to_string(),
                ),
                body: None,
                draft: Some(true),
                prerelease: Some(false),
                created_at: None,
                published_at: None,
            },
        ];
        let out = convert(items, 10, "test", "repo");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "v1.0.0 Release - test/repo");
    }

    #[test]
    fn test_convert_respects_max_results() {
        let items: Vec<GithubReleaseItem> = (0..5)
            .map(|i| GithubReleaseItem {
                tag_name: Some(format!("v{i}.0.0")),
                name: Some(format!("Release {i}")),
                html_url: Some(format!(
                    "https://github.com/test/repo/releases/tag/v{i}.0.0"
                )),
                body: None,
                draft: Some(false),
                prerelease: Some(false),
                created_at: None,
                published_at: None,
            })
            .collect();
        let out = convert(items, 2, "test", "repo");
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_html_url() {
        let items = vec![GithubReleaseItem {
            tag_name: Some("v1.0.0".to_string()),
            name: Some("Release".to_string()),
            html_url: None,
            body: None,
            draft: Some(false),
            prerelease: Some(false),
            created_at: None,
            published_at: None,
        }];
        let out = convert(items, 10, "test", "repo");
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_html_url() {
        let items = vec![GithubReleaseItem {
            tag_name: Some("v1.0.0".to_string()),
            name: Some("Release".to_string()),
            html_url: Some(String::new()),
            body: None,
            draft: Some(false),
            prerelease: Some(false),
            created_at: None,
            published_at: None,
        }];
        let out = convert(items, 10, "test", "repo");
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let items = vec![
            GithubReleaseItem {
                tag_name: Some("v1.0.0".to_string()),
                name: Some("Release".to_string()),
                html_url: Some("ftp://example.com/releases/1".to_string()),
                body: None,
                draft: Some(false),
                prerelease: Some(false),
                created_at: None,
                published_at: None,
            },
            GithubReleaseItem {
                tag_name: Some("v2.0.0".to_string()),
                name: Some("Release".to_string()),
                html_url: Some("https://github.com/test/repo/releases/tag/v2.0.0".to_string()),
                body: None,
                draft: Some(false),
                prerelease: Some(false),
                created_at: None,
                published_at: None,
            },
        ];
        let out = convert(items, 10, "test", "repo");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "v2.0.0 Release - test/repo");
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let items = vec![GithubReleaseItem {
            tag_name: Some("v1.0.0".to_string()),
            name: Some("Release".to_string()),
            html_url: Some("https://github.com/test/repo/releases/tag/v1.0.0".to_string()),
            body: Some(String::new()),
            draft: Some(false),
            prerelease: Some(false),
            created_at: None,
            published_at: None,
        }];
        let out = convert(items, 10, "test", "repo");
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
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
        // "🦀" is 4 bytes but 1 char. The legacy byte-slicing
        // implementation would panic when the byte slice landed
        // inside the crab emoji.
        let body = "abc 🦀 rust 🧪 unicode";
        let out = truncate_body(body, 7);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.len() <= body.len());
        assert!(out.chars().count() <= 7);
    }

    #[test]
    fn test_truncate_body_handles_cjk_text() {
        // CJK characters are 3 bytes each. The legacy implementation
        // would panic when max_chars fell inside a multi-byte
        // character.
        let body = "修正修正修正修正";
        let out = truncate_body(body, 5);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.chars().count() <= 5);
    }

    #[test]
    fn test_truncate_body_handles_emoji_only_text() {
        let body = "🦀🦀🦀🦀🦀";
        let out = truncate_body(body, 3);
        assert!(out.is_char_boundary(out.len()));
        assert_eq!(out.chars().count(), 3);
        assert_eq!(out, "🦀🦀🦀");
    }

    #[test]
    fn test_truncate_body_zero_max_returns_empty() {
        let out = truncate_body("anything", 0);
        assert_eq!(out, "");
    }

    #[test]
    fn test_max_results_zero_returns_empty() {
        let out = convert(vec![], 0, "test", "repo");
        assert!(out.is_empty());
    }

    #[test]
    fn test_parse_json_response_full() {
        let body = r#"{
            "items": [
                {
                    "tag_name": "v0.7.0",
                    "name": "Release v0.7.0",
                    "html_url": "https://github.com/test/repo/releases/tag/v0.7.0",
                    "body": "Release notes here",
                    "draft": false,
                    "prerelease": false,
                    "created_at": "2024-01-15T10:30:00Z",
                    "published_at": "2024-01-16T12:00:00Z"
                }
            ]
        }"#;
        let parsed: GithubReleasesResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].tag_name.as_deref(), Some("v0.7.0"));
        assert_eq!(parsed.items[0].draft, Some(false));
    }

    #[test]
    fn test_parse_json_response_empty_items() {
        let body = r#"{"items": []}"#;
        let parsed: GithubReleasesResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.items.is_empty());
    }

    #[test]
    fn test_search_returns_empty_without_repo_hint() {
        // This tests the early-return in the search function.
        // We can't easily test it without a mock server, so we test
        // the parse_repo_from_query helper instead.
        assert!(parse_repo_from_query("no repo hint here").is_none());
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
                .path("/repos/tokio-rs/axum/releases")
                .header("Authorization", "Bearer test-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "items": [
                        {
                            "tag_name": "v0.7.0",
                            "name": "Release v0.7.0",
                            "html_url": "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
                            "body": "Bug fixes",
                            "draft": false,
                            "prerelease": false,
                            "created_at": "2024-01-15T10:30:00Z",
                            "published_at": "2024-01-16T12:00:00Z"
                        },
                        {
                            "tag_name": "v0.6.0",
                            "name": null,
                            "html_url": "https://github.com/tokio-rs/axum/releases/tag/v0.6.0",
                            "body": "Previous release",
                            "draft": false,
                            "prerelease": false,
                            "created_at": "2023-12-01T10:00:00Z",
                            "published_at": "2023-12-02T10:00:00Z"
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
            "repo:tokio-rs/axum",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "v0.7.0 Release v0.7.0 - tokio-rs/axum");
        assert_eq!(results[0].source_engine, "github_releases");
    }

    #[tokio::test]
    async fn test_empty_result_set() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/repos/test/empty/releases");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"items": []}"#);
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "repo:test/empty",
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
            when.method(GET).path("/repos/test/repo/releases");
            then.status(401).body("Bad credentials");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "bad-token",
            Some(&server.url("")),
            "repo:test/repo",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 401");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_releases");
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
            when.method(GET).path("/repos/test/repo/releases");
            then.status(403).body("rate limit exceeded");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "repo:test/repo",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 403");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_releases");
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
            when.method(GET).path("/repos/test/nonexistent/releases");
            then.status(404).body("Not Found");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "repo:test/nonexistent",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 404");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_releases");
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
            when.method(GET).path("/repos/test/repo/releases");
            then.status(500).body("Internal Server Error");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "repo:test/repo",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 500");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_releases");
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
            when.method(GET).path("/repos/test/repo/releases");
            then.status(200)
                .header("content-type", "application/json")
                .body("this is not json");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "repo:test/repo",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with parse error");

        match err {
            EngineError::ParseFailed { engine, reason } => {
                assert_eq!(engine, "github_releases");
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
                .path("/repos/test/repo/releases")
                .query_param("per_page", "2");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "items": [
                        {"tag_name": "v3.0.0", "html_url": "https://github.com/test/repo/releases/tag/v3.0.0", "draft": false},
                        {"tag_name": "v2.0.0", "html_url": "https://github.com/test/repo/releases/tag/v2.0.0", "draft": false},
                        {"tag_name": "v1.0.0", "html_url": "https://github.com/test/repo/releases/tag/v1.0.0", "draft": false}
                    ]
                }"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "repo:test/repo",
            2,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "v3.0.0 - test/repo");
        assert_eq!(results[1].title, "v2.0.0 - test/repo");
    }

    #[tokio::test]
    async fn test_api_key_sent_in_header() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/repos/test/repo/releases")
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
            "repo:test/repo",
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
            when.method(GET).path("/repos/test/repo/releases");
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
            "repo:test/repo",
            10,
            Duration::from_millis(50),
        )
        .await
        .expect_err("should fail with timeout");

        match err {
            EngineError::Timeout { engine } => {
                assert_eq!(engine, "github_releases");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_skips_draft_releases() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/repos/test/repo/releases");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "items": [
                        {"tag_name": "v1.0.0", "name": "Stable", "html_url": "https://github.com/test/repo/releases/tag/v1.0.0", "draft": false, "prerelease": false},
                        {"tag_name": "v1.1.0", "name": "Draft", "html_url": "https://github.com/test/repo/releases/tag/v1.1.0", "draft": true, "prerelease": false},
                        {"tag_name": "v0.9.0", "name": "Pre", "html_url": "https://github.com/test/repo/releases/tag/v0.9.0", "draft": false, "prerelease": true}
                    ]
                }"#,
                );
        });

        let client = reqwest::Client::new();
        let results = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "repo:test/repo",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "v1.0.0 Stable - test/repo");
        assert_eq!(results[1].title, "v0.9.0 Pre - test/repo");
    }

    #[test]
    fn test_provider_descriptor_for_github_releases() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("github_releases", true, false, true).unwrap();
        assert_eq!(desc.id, "github_releases");
        assert_eq!(desc.display_name, "GitHub Releases");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_release_search);
        assert!(desc.capabilities.supports_repo_filter);
        assert!(desc.capabilities.supports_org_filter);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_issue_search);
    }

    #[test]
    fn test_provider_descriptor_github_releases_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc = built_in_provider_descriptor("github_releases", false, false, true).unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
