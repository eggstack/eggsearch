use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use crate::core::code_metadata::CodeHost;
use crate::core::source_card::IssueMetadata;

const ENGINE: &str = "github_issues";
const DEFAULT_BASE_URL: &str = "https://api.github.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
struct GithubIssuesResponse {
    #[serde(default)]
    items: Vec<GithubIssueItem>,
}

#[derive(Debug, Deserialize)]
struct GithubIssueItem {
    number: Option<u64>,
    title: Option<String>,
    html_url: Option<String>,
    body: Option<String>,
    state: Option<String>,
    labels: Option<Vec<GithubLabel>>,
    created_at: Option<String>,
    updated_at: Option<String>,
    closed_at: Option<String>,
    pull_request: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GithubLabel {
    name: Option<String>,
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

    let base = base_url.unwrap_or(DEFAULT_BASE_URL);
    let url = format!("{base}/search/issues");

    let per_page = max_results.clamp(1, 100);

    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .get(&url)
            .query(&[("q", query), ("per_page", &per_page.to_string())])
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {api_key}"))
            .header("X-GitHub-Api-Version", "2022-11-28")
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

    let parsed: GithubIssuesResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;

    Ok(convert(parsed.items, max_results))
}

fn truncate_body(body: &str, max_chars: usize) -> String {
    crate::core::sanitize::truncate_at_word(body, max_chars)
}

fn parse_owner_repo(html_url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(html_url).ok()?;
    let segments: Vec<&str> = parsed
        .path()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .collect();
    if segments.len() >= 2 {
        Some((segments[0].to_string(), segments[1].to_string()))
    } else {
        None
    }
}

fn is_pull_request(item: &GithubIssueItem) -> bool {
    if item.pull_request.is_some() {
        return true;
    }
    if let Some(url) = &item.html_url {
        return url.contains("/pull/");
    }
    false
}

fn convert(items: Vec<GithubIssueItem>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(items.len()));
    for item in items {
        if out.len() >= max_results {
            break;
        }
        let Some(html_url) = &item.html_url else {
            continue;
        };
        if !super::is_http_url(html_url) {
            continue;
        }
        let Some(title) = &item.title else {
            continue;
        };
        if title.is_empty() {
            continue;
        }
        let (owner, repo) = match parse_owner_repo(html_url) {
            Some(v) => v,
            None => continue,
        };
        let number_str = item.number.map(|n| n.to_string()).unwrap_or_default();
        let title = format!("#{number_str} {title} - {owner}/{repo}");

        let snippet = item
            .body
            .as_deref()
            .map(|b| truncate_body(b, SNIPPET_MAX_CHARS))
            .map(|s| crate::core::sanitize::normalize_whitespace(&s))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let labels: Vec<String> = item
            .labels
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|l| l.name.clone())
            .collect();

        let pr = is_pull_request(&item);

        let metadata = ResultMetadata::Issue(IssueMetadata {
            host: Some(CodeHost::Github),
            owner: Some(owner),
            repo: Some(repo),
            number: item.number,
            state: item.state.clone(),
            is_pull_request: Some(pr),
            labels,
            created_at: item.created_at.clone(),
            updated_at: item.updated_at.clone(),
            closed_at: item.closed_at.clone(),
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
    fn test_convert_extracts_results() {
        let items = vec![
            GithubIssueItem {
                number: Some(123),
                title: Some("Bug in parser".to_string()),
                html_url: Some("https://github.com/tokio-rs/axum/issues/123".to_string()),
                body: Some("The parser crashes".to_string()),
                state: Some("open".to_string()),
                labels: Some(vec![
                    GithubLabel {
                        name: Some("bug".to_string()),
                    },
                    GithubLabel {
                        name: Some("p0".to_string()),
                    },
                ]),
                created_at: Some("2024-01-15T10:30:00Z".to_string()),
                updated_at: Some("2024-01-20T14:22:00Z".to_string()),
                closed_at: None,
                pull_request: None,
            },
            GithubIssueItem {
                number: Some(124),
                title: Some("Add feature X".to_string()),
                html_url: Some("https://github.com/tokio-rs/axum/issues/124".to_string()),
                body: Some("We need feature X".to_string()),
                state: Some("closed".to_string()),
                labels: Some(vec![GithubLabel {
                    name: Some("enhancement".to_string()),
                }]),
                created_at: Some("2024-02-01T08:00:00Z".to_string()),
                updated_at: Some("2024-02-05T12:00:00Z".to_string()),
                closed_at: Some("2024-02-05T12:00:00Z".to_string()),
                pull_request: None,
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "#123 Bug in parser - tokio-rs/axum");
        assert_eq!(out[0].url, "https://github.com/tokio-rs/axum/issues/123");
        assert_eq!(out[0].snippet.as_deref(), Some("The parser crashes"));
        assert_eq!(out[0].source_engine, "github_issues");
        match &out[0].metadata {
            ResultMetadata::Issue(m) => {
                assert_eq!(m.host, Some(CodeHost::Github));
                assert_eq!(m.owner.as_deref(), Some("tokio-rs"));
                assert_eq!(m.repo.as_deref(), Some("axum"));
                assert_eq!(m.number, Some(123));
                assert_eq!(m.state.as_deref(), Some("open"));
                assert_eq!(m.is_pull_request, Some(false));
                assert_eq!(m.labels, vec!["bug", "p0"]);
                assert!(m.closed_at.is_none());
            }
            other => panic!("expected Issue metadata, got: {other:?}"),
        }
        assert_eq!(out[1].title, "#124 Add feature X - tokio-rs/axum");
    }

    #[test]
    fn test_convert_respects_max_results() {
        let items: Vec<GithubIssueItem> = (0..5)
            .map(|i| GithubIssueItem {
                number: Some(i),
                title: Some(format!("Issue {i}")),
                html_url: Some(format!("https://github.com/test/repo/issues/{i}")),
                body: None,
                state: Some("open".to_string()),
                labels: None,
                created_at: None,
                updated_at: None,
                closed_at: None,
                pull_request: None,
            })
            .collect();
        let out = convert(items, 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_convert_skips_missing_html_url() {
        let items = vec![GithubIssueItem {
            number: Some(1),
            title: Some("Title".to_string()),
            html_url: None,
            body: None,
            state: Some("open".to_string()),
            labels: None,
            created_at: None,
            updated_at: None,
            closed_at: None,
            pull_request: None,
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_html_url() {
        let items = vec![GithubIssueItem {
            number: Some(1),
            title: Some("Title".to_string()),
            html_url: Some(String::new()),
            body: None,
            state: Some("open".to_string()),
            labels: None,
            created_at: None,
            updated_at: None,
            closed_at: None,
            pull_request: None,
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let items = vec![
            GithubIssueItem {
                number: Some(1),
                title: Some("Title".to_string()),
                html_url: Some("ftp://example.com/1".to_string()),
                body: None,
                state: Some("open".to_string()),
                labels: None,
                created_at: None,
                updated_at: None,
                closed_at: None,
                pull_request: None,
            },
            GithubIssueItem {
                number: Some(2),
                title: Some("Good".to_string()),
                html_url: Some("https://github.com/test/repo/issues/2".to_string()),
                body: None,
                state: Some("open".to_string()),
                labels: None,
                created_at: None,
                updated_at: None,
                closed_at: None,
                pull_request: None,
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "#2 Good - test/repo");
    }

    #[test]
    fn test_convert_skips_missing_title() {
        let items = vec![GithubIssueItem {
            number: Some(1),
            title: None,
            html_url: Some("https://github.com/test/repo/issues/1".to_string()),
            body: None,
            state: Some("open".to_string()),
            labels: None,
            created_at: None,
            updated_at: None,
            closed_at: None,
            pull_request: None,
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let items = vec![GithubIssueItem {
            number: Some(1),
            title: Some("Title".to_string()),
            html_url: Some("https://github.com/test/repo/issues/1".to_string()),
            body: Some(String::new()),
            state: Some("open".to_string()),
            labels: None,
            created_at: None,
            updated_at: None,
            closed_at: None,
            pull_request: None,
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_convert_is_pull_request_true_when_pr_field_present() {
        let items = vec![GithubIssueItem {
            number: Some(50),
            title: Some("Refactor handler".to_string()),
            html_url: Some("https://github.com/test/repo/pull/50".to_string()),
            body: None,
            state: Some("open".to_string()),
            labels: None,
            created_at: None,
            updated_at: None,
            closed_at: None,
            pull_request: Some(serde_json::json!({"url": "..."})),
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        match &out[0].metadata {
            ResultMetadata::Issue(m) => {
                assert_eq!(m.is_pull_request, Some(true));
            }
            other => panic!("expected Issue metadata, got: {other:?}"),
        }
    }

    #[test]
    fn test_convert_is_pull_request_true_when_url_contains_pull() {
        let items = vec![GithubIssueItem {
            number: Some(50),
            title: Some("Refactor handler".to_string()),
            html_url: Some("https://github.com/test/repo/pull/50".to_string()),
            body: None,
            state: Some("open".to_string()),
            labels: None,
            created_at: None,
            updated_at: None,
            closed_at: None,
            pull_request: None,
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        match &out[0].metadata {
            ResultMetadata::Issue(m) => {
                assert_eq!(m.is_pull_request, Some(true));
            }
            other => panic!("expected Issue metadata, got: {other:?}"),
        }
    }

    #[test]
    fn test_convert_is_pull_request_false_for_issue_url() {
        let items = vec![GithubIssueItem {
            number: Some(123),
            title: Some("Bug".to_string()),
            html_url: Some("https://github.com/test/repo/issues/123".to_string()),
            body: None,
            state: Some("open".to_string()),
            labels: None,
            created_at: None,
            updated_at: None,
            closed_at: None,
            pull_request: None,
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        match &out[0].metadata {
            ResultMetadata::Issue(m) => {
                assert_eq!(m.is_pull_request, Some(false));
            }
            other => panic!("expected Issue metadata, got: {other:?}"),
        }
    }

    #[test]
    fn test_convert_extracts_labels() {
        let items = vec![GithubIssueItem {
            number: Some(1),
            title: Some("Title".to_string()),
            html_url: Some("https://github.com/test/repo/issues/1".to_string()),
            body: None,
            state: Some("open".to_string()),
            labels: Some(vec![
                GithubLabel {
                    name: Some("bug".to_string()),
                },
                GithubLabel { name: None },
                GithubLabel {
                    name: Some("urgent".to_string()),
                },
            ]),
            created_at: None,
            updated_at: None,
            closed_at: None,
            pull_request: None,
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
    fn test_truncate_body_short() {
        assert_eq!(truncate_body("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_body_exact() {
        assert_eq!(truncate_body("hello", 5), "hello");
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
        // "🦀" is 4 bytes but 1 char. With the legacy byte-slicing
        // implementation, taking 7 bytes from "abc 🦀 rust 🧪 unicode"
        // would slice inside the crab emoji and panic.
        let body = "abc 🦀 rust 🧪 unicode";
        let out = truncate_body(body, 7);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.len() <= body.len());
        // Char count must be at most 7.
        assert!(out.chars().count() <= 7);
    }

    #[test]
    fn test_truncate_body_handles_cjk_text() {
        // "修正" is 6 bytes but 2 chars. The legacy implementation
        // would panic when max_chars fell in the middle of the second
        // CJK character.
        let body = "修正修正修正修正";
        let out = truncate_body(body, 5);
        assert!(out.is_char_boundary(out.len()));
        // Either 4 chars (truncated to 4 then word-trim) or fewer.
        assert!(out.chars().count() <= 5);
    }

    #[test]
    fn test_truncate_body_handles_emoji_only_text() {
        // 5 emojis = 5 chars / 20 bytes. The byte-slicing
        // implementation would panic immediately at max_chars = 3
        // because byte index 3 lands inside the second emoji.
        let body = "🦀🦀🦀🦀🦀";
        let out = truncate_body(body, 3);
        assert!(out.is_char_boundary(out.len()));
        assert_eq!(out.chars().count(), 3);
        assert_eq!(out, "🦀🦀🦀");
    }

    #[test]
    fn test_truncate_body_at_word_boundary_with_emoji() {
        // "hello 🦀 world" — when truncated to 7 chars the result is
        // "hello" (word boundary at index 5, before the emoji).
        let out = truncate_body("hello 🦀 world", 7);
        assert_eq!(out, "hello");
    }

    #[test]
    fn test_truncate_body_zero_max_returns_empty() {
        // 0 chars means nothing; no panic.
        let out = truncate_body("anything", 0);
        assert_eq!(out, "");
    }

    #[test]
    fn test_parse_owner_repo_valid() {
        let (owner, repo) =
            parse_owner_repo("https://github.com/tokio-rs/axum/issues/123").unwrap();
        assert_eq!(owner, "tokio-rs");
        assert_eq!(repo, "axum");
    }

    #[test]
    fn test_parse_owner_repo_invalid() {
        assert!(parse_owner_repo("https://github.com/single").is_none());
    }

    #[test]
    fn test_max_results_zero_returns_empty() {
        let out = convert(vec![], 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_parse_json_response_full() {
        let body = r#"{
            "items": [
                {
                    "number": 1,
                    "title": "Bug report",
                    "html_url": "https://github.com/test/repo/issues/1",
                    "body": "Something is broken",
                    "state": "open",
                    "labels": [{"name": "bug"}],
                    "created_at": "2024-01-15T10:30:00Z",
                    "updated_at": "2024-01-20T14:22:00Z",
                    "closed_at": null,
                    "pull_request": null
                }
            ]
        }"#;
        let parsed: GithubIssuesResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].number, Some(1));
        assert_eq!(
            parsed.items[0].html_url.as_deref(),
            Some("https://github.com/test/repo/issues/1")
        );
    }

    #[test]
    fn test_parse_json_response_empty_items() {
        let body = r#"{"items": []}"#;
        let parsed: GithubIssuesResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.items.is_empty());
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
                .path("/search/issues")
                .header("Authorization", "Bearer test-token");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "items": [
                        {
                            "number": 42,
                            "title": "Fix parser",
                            "html_url": "https://github.com/tokio-rs/axum/issues/42",
                            "body": "Parser bug description",
                            "state": "open",
                            "labels": [{"name": "bug"}],
                            "created_at": "2024-01-15T10:30:00Z",
                            "updated_at": "2024-01-20T14:22:00Z",
                            "closed_at": null,
                            "pull_request": null
                        },
                        {
                            "number": 43,
                            "title": "Add feature",
                            "html_url": "https://github.com/tokio-rs/axum/issues/43",
                            "body": "Feature request",
                            "state": "closed",
                            "labels": [{"name": "enhancement"}],
                            "created_at": "2024-02-01T08:00:00Z",
                            "updated_at": "2024-02-05T12:00:00Z",
                            "closed_at": "2024-02-05T12:00:00Z",
                            "pull_request": null
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
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "#42 Fix parser - tokio-rs/axum");
        assert_eq!(results[0].url, "https://github.com/tokio-rs/axum/issues/42");
        assert_eq!(
            results[0].snippet.as_deref(),
            Some("Parser bug description")
        );
        assert_eq!(results[0].source_engine, "github_issues");
    }

    #[tokio::test]
    async fn test_empty_result_set() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/issues");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"items": []}"#);
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
            when.method(GET).path("/search/issues");
            then.status(401).body("Bad credentials");
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
                assert_eq!(engine, "github_issues");
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
            when.method(GET).path("/search/issues");
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
                assert_eq!(engine, "github_issues");
                assert_eq!(status, 403);
            }
            other => panic!("expected BadStatus(403), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_invalid_query_422() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/issues");
            then.status(422).body("Validation Failed");
        });

        let client = reqwest::Client::new();
        let err = search(
            &client,
            "test-token",
            Some(&server.url("")),
            "bad:query:++",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect_err("should fail with 422");

        match err {
            EngineError::BadStatus { engine, status } => {
                assert_eq!(engine, "github_issues");
                assert_eq!(status, 422);
            }
            other => panic!("expected BadStatus(422), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_malformed_json() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/issues");
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
                assert_eq!(engine, "github_issues");
                assert!(reason.contains("invalid JSON"), "reason: {reason}");
            }
            other => panic!("expected ParseFailed, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_server_error_500() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/issues");
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
                assert_eq!(engine, "github_issues");
                assert_eq!(status, 500);
            }
            other => panic!("expected BadStatus(500), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_respects_max_results() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/search/issues")
                .query_param("per_page", "2");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "items": [
                        {"number": 1, "title": "A", "html_url": "https://github.com/test/repo/issues/1", "state": "open"},
                        {"number": 2, "title": "B", "html_url": "https://github.com/test/repo/issues/2", "state": "open"},
                        {"number": 3, "title": "C", "html_url": "https://github.com/test/repo/issues/3", "state": "open"}
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
        assert_eq!(results[0].title, "#1 A - test/repo");
        assert_eq!(results[1].title, "#2 B - test/repo");
    }

    #[tokio::test]
    async fn test_api_key_sent_in_header() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/search/issues")
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
            when.method(GET).path("/search/issues");
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
            "rust",
            10,
            Duration::from_millis(50),
        )
        .await
        .expect_err("should fail with timeout");

        match err {
            EngineError::Timeout { engine } => {
                assert_eq!(engine, "github_issues");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_pull_request_detected_via_pr_field() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/issues");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                    "items": [
                        {
                            "number": 50,
                            "title": "Refactor handler",
                            "html_url": "https://github.com/test/repo/pull/50",
                            "body": "Refactors the handler module",
                            "state": "open",
                            "labels": [],
                            "created_at": "2024-03-01T10:00:00Z",
                            "updated_at": "2024-03-01T10:00:00Z",
                            "closed_at": null,
                            "pull_request": {"url": "https://api.github.com/repos/test/repo/pulls/50"}
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
            "test",
            10,
            Duration::from_secs(5),
        )
        .await
        .expect("search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "#50 Refactor handler - test/repo");
        match &results[0].metadata {
            ResultMetadata::Issue(m) => {
                assert_eq!(m.is_pull_request, Some(true));
            }
            other => panic!("expected Issue metadata, got: {other:?}"),
        }
    }

    #[test]
    fn test_provider_descriptor_for_github_issues() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("github_issues", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "github_issues");
        assert_eq!(desc.display_name, "GitHub Issues Search");
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
    }

    #[test]
    fn test_provider_descriptor_github_issues_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("github_issues", false, false, true, false, None, None)
                .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
