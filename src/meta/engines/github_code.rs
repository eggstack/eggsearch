//! GitHub Code Search API provider.
//!
//! Uses the GitHub REST API `/search/code` endpoint with a personal
//! access token passed via the `Authorization: Bearer` header.

use std::time::Duration;

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{CodeSearchMetadata, ResultMetadata, SearchResult};

const ENGINE: &str = "github_code";
const DEFAULT_BASE_URL: &str = "https://api.github.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Parsed GitHub code search API response.
#[derive(Debug, Deserialize)]
struct GithubCodeResponse {
    #[serde(default)]
    items: Vec<GithubCodeItem>,
}

#[derive(Debug, Deserialize)]
struct GithubCodeItem {
    #[allow(dead_code)]
    name: Option<String>,
    path: Option<String>,
    html_url: Option<String>,
    repository: Option<GithubRepo>,
    #[allow(dead_code)]
    score: Option<f64>,
    #[serde(default)]
    text_matches: Vec<TextMatch>,
}

/// A text-match fragment returned by the GitHub Code Search API
/// when using the `application/vnd.github.text-match+json` media type.
#[derive(Debug, Deserialize)]
struct TextMatch {
    fragment: Option<String>,
    #[serde(default)]
    matches: Vec<TextMatchItem>,
}

#[derive(Debug, Deserialize)]
struct TextMatchItem {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRepo {
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

    let base = base_url.unwrap_or(DEFAULT_BASE_URL);
    let url = format!("{base}/search/code");

    let per_page = max_results.clamp(1, 100);

    let per_page = per_page.to_string();
    let authorization = format!("Bearer {api_key}");
    let query_params = [("q", query), ("per_page", per_page.as_str())];
    let headers = [
        ("Accept", "application/vnd.github.text-match+json"),
        ("Authorization", authorization.as_str()),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ];
    let parsed: GithubCodeResponse = super::get_json_bounded(
        client,
        ENGINE,
        url.as_str(),
        &query_params,
        &headers,
        MAX_BODY_BYTES,
        timeout,
    )
    .await?;

    Ok(convert(parsed.items, max_results))
}

fn convert(items: Vec<GithubCodeItem>, max_results: usize) -> Vec<SearchResult> {
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
        let title = build_title(item.path.as_deref(), item.repository.as_ref());
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

        // Extract matched symbol and fragment from text_matches.
        let (matched_symbol, text_fragment) = extract_text_match(&item.text_matches);

        let metadata = if matched_symbol.is_some() || text_fragment.is_some() {
            ResultMetadata::CodeSearch(CodeSearchMetadata {
                matched_symbol,
                text_fragment,
            })
        } else {
            ResultMetadata::None
        };

        out.push(SearchResult {
            title,
            url: html_url.clone(),
            snippet,
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at: None,
            metadata,
        });
    }
    out
}

/// Extract the first matched symbol and text fragment from text matches.
fn extract_text_match(text_matches: &[TextMatch]) -> (Option<String>, Option<String>) {
    for tm in text_matches {
        let matched_text = tm
            .matches
            .iter()
            .find_map(|m| m.text.clone())
            .or_else(|| tm.fragment.clone());
        if matched_text.is_some() {
            let fragment = tm.fragment.clone();
            return (matched_text, fragment);
        }
    }
    (None, None)
}

fn build_title(path: Option<&str>, repo: Option<&GithubRepo>) -> Option<String> {
    let path = path?;
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
        let items = vec![
            GithubCodeItem {
                name: Some("lib.rs".to_string()),
                path: Some("src/lib.rs".to_string()),
                html_url: Some("https://github.com/tokio-rs/axum/blob/main/src/lib.rs".to_string()),
                repository: Some(GithubRepo {
                    full_name: Some("tokio-rs/axum".to_string()),
                    description: Some("A web framework".to_string()),
                }),
                score: Some(1.0),
                text_matches: vec![],
            },
            GithubCodeItem {
                name: Some("main.rs".to_string()),
                path: Some("src/main.rs".to_string()),
                html_url: Some(
                    "https://github.com/tokio-rs/axum/blob/main/src/main.rs".to_string(),
                ),
                repository: Some(GithubRepo {
                    full_name: Some("tokio-rs/axum".to_string()),
                    description: None,
                }),
                score: Some(0.8),
                text_matches: vec![],
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "src/lib.rs - tokio-rs/axum");
        assert_eq!(
            out[0].url,
            "https://github.com/tokio-rs/axum/blob/main/src/lib.rs"
        );
        assert_eq!(out[0].snippet.as_deref(), Some("A web framework"));
        assert_eq!(out[0].source_engine, "github_code");
        assert_eq!(out[1].title, "src/main.rs - tokio-rs/axum");
        assert!(out[1].snippet.is_none());
    }

    #[test]
    fn test_convert_skips_missing_html_url() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: None,
            repository: None,
            score: None,
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_empty_html_url() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: Some(String::new()),
            repository: None,
            score: None,
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_skips_non_http_urls() {
        let items = vec![
            GithubCodeItem {
                name: Some("a.rs".to_string()),
                path: Some("a.rs".to_string()),
                html_url: Some("ftp://example.com/a.rs".to_string()),
                repository: None,
                score: None,
                text_matches: vec![],
            },
            GithubCodeItem {
                name: Some("b.rs".to_string()),
                path: Some("b.rs".to_string()),
                html_url: Some("https://github.com/test/repo/blob/main/b.rs".to_string()),
                repository: Some(GithubRepo {
                    full_name: Some("test/repo".to_string()),
                    description: None,
                }),
                score: None,
                text_matches: vec![],
            },
        ];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "b.rs - test/repo");
    }

    #[test]
    fn test_convert_skips_missing_path() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: None,
            html_url: Some("https://github.com/test/repo/blob/main/lib.rs".to_string()),
            repository: Some(GithubRepo {
                full_name: Some("test/repo".to_string()),
                description: None,
            }),
            score: None,
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn test_convert_drops_empty_snippet() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: Some("https://github.com/test/repo/blob/main/src/lib.rs".to_string()),
            repository: Some(GithubRepo {
                full_name: Some("test/repo".to_string()),
                description: Some(String::new()),
            }),
            score: None,
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn test_build_title_with_path_and_repo() {
        let repo = GithubRepo {
            full_name: Some("tokio-rs/axum".to_string()),
            description: None,
        };
        assert_eq!(
            build_title(Some("src/lib.rs"), Some(&repo)).unwrap(),
            "src/lib.rs - tokio-rs/axum"
        );
    }

    #[test]
    fn test_build_title_none_when_no_path() {
        assert!(build_title(None, None).is_none());
    }

    #[test]
    fn test_build_title_fallback_when_no_repo() {
        assert_eq!(
            build_title(Some("lib.rs"), None).unwrap(),
            "lib.rs - unknown"
        );
    }

    // --- Text-match extraction tests ---

    #[test]
    fn test_extract_text_match_with_matches() {
        let matches = vec![TextMatch {
            fragment: Some("pub fn router() -> Router {\n}".to_string()),
            matches: vec![TextMatchItem {
                text: Some("router".to_string()),
            }],
        }];
        let (symbol, fragment) = extract_text_match(&matches);
        assert_eq!(symbol.as_deref(), Some("router"));
        assert!(fragment.is_some());
    }

    #[test]
    fn test_extract_text_match_fragment_only() {
        let matches = vec![TextMatch {
            fragment: Some("fn main() {}".to_string()),
            matches: vec![],
        }];
        let (symbol, fragment) = extract_text_match(&matches);
        assert_eq!(symbol.as_deref(), Some("fn main() {}"));
        assert!(fragment.is_some());
    }

    #[test]
    fn test_extract_text_match_empty() {
        let (symbol, fragment) = extract_text_match(&[]);
        assert!(symbol.is_none());
        assert!(fragment.is_none());
    }

    #[test]
    fn test_convert_with_text_matches_produces_code_search_metadata() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: Some("https://github.com/test/repo/blob/main/src/lib.rs".to_string()),
            repository: Some(GithubRepo {
                full_name: Some("test/repo".to_string()),
                description: None,
            }),
            score: Some(1.0),
            text_matches: vec![TextMatch {
                fragment: Some("pub fn router() -> Router {\n}".to_string()),
                matches: vec![TextMatchItem {
                    text: Some("router".to_string()),
                }],
            }],
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        match &out[0].metadata {
            ResultMetadata::CodeSearch(m) => {
                assert_eq!(m.matched_symbol.as_deref(), Some("router"));
                assert!(m.text_fragment.is_some());
            }
            other => panic!("expected CodeSearch metadata, got {other:?}"),
        }
    }

    #[test]
    fn test_convert_without_text_matches_produces_none_metadata() {
        let items = vec![GithubCodeItem {
            name: Some("lib.rs".to_string()),
            path: Some("src/lib.rs".to_string()),
            html_url: Some("https://github.com/test/repo/blob/main/src/lib.rs".to_string()),
            repository: Some(GithubRepo {
                full_name: Some("test/repo".to_string()),
                description: None,
            }),
            score: Some(1.0),
            text_matches: vec![],
        }];
        let out = convert(items, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].metadata, ResultMetadata::None);
    }

    // -----------------------------------------------------------------------
    // HTTP-level tests using httpmock
    // -----------------------------------------------------------------------

    use crate::meta::engines::error::EngineError;

    #[tokio::test]
    async fn test_invalid_query_422() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/code");
            then.status(422).body("Validation Failed");
        });

        let client = crate::meta::engines::build_http_client(None).expect("test client");
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
                assert_eq!(engine, "github_code");
                assert_eq!(status, 422);
            }
            other => panic!("expected BadStatus(422), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_timeout() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/code");
            then.status(200)
                .header("content-type", "application/json")
                .delay(std::time::Duration::from_secs(10))
                .body(r#"{"items": []}"#);
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
                assert_eq!(engine, "github_code");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[test]
    fn test_provider_descriptor_for_github_code() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("github_code", true, false, true, false, None, None)
                .unwrap();
        assert_eq!(desc.id, "github_code");
        assert_eq!(desc.display_name, "GitHub Code Search");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.configured);
        assert!(desc.enabled);
        assert!(!desc.default);
        assert!(desc.capabilities.supports_code_search);
        assert!(desc.capabilities.supports_repo_filter);
        assert!(desc.capabilities.supports_org_filter);
        assert!(desc.capabilities.supports_path_filter);
        assert!(desc.capabilities.supports_language_filter);
        assert!(desc.capabilities.supports_symbol_hint);
    }

    #[test]
    fn test_provider_descriptor_github_code_unconfigured_when_disabled() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("github_code", false, false, true, false, None, None)
                .unwrap();
        assert!(!desc.configured);
        assert!(!desc.enabled);
    }
}
