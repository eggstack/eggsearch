//! GitHub repository discovery built on the official repository search
//! endpoint.
//!
//! This is repository *discovery*: full name, canonical HTML URL,
//! description, and bounded metadata. It deliberately does not overlap
//! `github_code` (code/file search), `github_issues`, or
//! `github_releases`, and it never claims code search or repository
//! indexing. Keyless routing uses GitHub's unauthenticated limits; an
//! operator `GITHUB_TOKEN` may be attached to raise them and is never
//! required.

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::request::EngineSearchRequest;

const ENGINE: &str = "github_repositories";
const DEFAULT_BASE_URL: &str = "https://api.github.com";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_PER_PAGE: usize = 100;
const SNIPPET_MAX_CHARS: usize = 500;
const API_VERSION: &str = "2022-11-28";

#[derive(Debug, Deserialize)]
struct RepositorySearchResponse {
    #[serde(default)]
    items: Vec<RepositoryItem>,
}

#[derive(Clone, Debug, Deserialize)]
struct RepositoryItem {
    #[serde(default)]
    full_name: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    stargazers_count: Option<u64>,
    #[serde(default)]
    forks_count: Option<u64>,
    #[serde(default)]
    open_issues_count: Option<u64>,
    #[serde(default)]
    pushed_at: Option<String>,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(url) if !url.trim().is_empty() => url.trim().trim_end_matches('/').to_string(),
        _ => DEFAULT_BASE_URL.to_string(),
    }
}

fn search_path() -> &'static str {
    "/search/repositories"
}

fn search_params(query: &str, max_results: usize) -> Vec<(String, String)> {
    vec![
        ("q".to_string(), query.to_string()),
        (
            "per_page".to_string(),
            max_results.clamp(1, MAX_PER_PAGE).to_string(),
        ),
        ("sort".to_string(), "updated".to_string()),
        ("order".to_string(), "desc".to_string()),
    ]
}

fn metadata_snippet(item: &RepositoryItem) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(language) = item
        .language
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        parts.push(language.to_string());
    }
    if let Some(stars) = item.stargazers_count {
        parts.push(format!("{stars} stars"));
    }
    if let Some(forks) = item.forks_count {
        parts.push(format!("{forks} forks"));
    }
    if let Some(open) = item.open_issues_count {
        parts.push(format!("{open} open issues"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(crate::core::sanitize::truncate_at_word(
        &parts.join(" · "),
        SNIPPET_MAX_CHARS,
    ))
}

fn snippet_for(item: &RepositoryItem) -> Option<String> {
    item.description
        .as_deref()
        .map(crate::core::sanitize::normalize_whitespace)
        .map(|d| crate::core::sanitize::truncate_at_word(d.trim(), SNIPPET_MAX_CHARS))
        .filter(|d| !d.is_empty())
        .or_else(|| metadata_snippet(item))
}

fn convert(items: Vec<RepositoryItem>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(items.len()));
    for item in items {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = item
            .html_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty() && super::is_http_url(u))
        else {
            continue;
        };
        let url = url.to_string();
        let title = item
            .full_name
            .as_deref()
            .map(crate::core::sanitize::normalize_whitespace)
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .or_else(|| {
                url.rsplit('/')
                    .find(|segment| !segment.is_empty())
                    .map(str::to_string)
            });
        let Some(title) = title else {
            continue;
        };
        out.push(SearchResult {
            title,
            url,
            snippet: snippet_for(&item),
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at: item
                .pushed_at
                .as_deref()
                .and_then(crate::core::source_card::parse_result_timestamp),
            metadata: ResultMetadata::None,
        });
    }
    out
}

pub async fn search(
    client: &Client,
    base_url: Option<&str>,
    api_key: Option<&str>,
    request: &EngineSearchRequest,
) -> Result<Vec<SearchResult>, EngineError> {
    if request.max_results == 0 {
        return Ok(Vec::new());
    }
    let url = format!("{}{}", resolve_url(base_url), search_path());
    let mut req = client.get(url.as_str()).map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;
    for (key, value) in search_params(&request.query, request.max_results) {
        req = req.query(key.as_str(), value.as_str());
    }
    req = req
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION);
    if let Some(key) = api_key.map(str::trim).filter(|k| !k.is_empty()) {
        req = req.header("Authorization", format!("Bearer {key}").as_str());
    }
    let resp = req
        .timeout(super::engine_timeout(request.timeout))
        .send()
        .await
        .map_err(|e| super::map_request_error(ENGINE, e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: status.as_u16(),
        });
    }
    let bytes = super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await?;
    let parsed: RepositorySearchResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(convert(parsed.items, request.max_results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn req(query: &str, max_results: usize) -> EngineSearchRequest {
        EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
    }

    fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    const FIXTURE: &str = r#"{
      "total_count": 2,
      "incomplete_results": false,
      "items": [
        {
          "full_name": "tokio-rs/axum",
          "html_url": "https://github.com/tokio-rs/axum",
          "description": "Web framework built on Tokio and Hyper",
          "language": "Rust",
          "stargazers_count": 21000,
          "forks_count": 1900,
          "open_issues_count": 120,
          "updated_at": "2024-06-01T10:00:00Z",
          "pushed_at": "2024-05-30T08:00:00Z",
          "archived": false,
          "fork": false
        },
        {
          "full_name": "example/quiet",
          "html_url": "https://github.com/example/quiet",
          "language": "Go",
          "stargazers_count": 3,
          "pushed_at": "not-a-timestamp"
        }
      ]
    }"#;

    #[test]
    fn search_params_use_repository_discovery_contract() {
        let params = search_params("rust web framework", 10);
        assert_eq!(param(&params, "q"), Some("rust web framework"));
        assert_eq!(param(&params, "per_page"), Some("10"));
        assert_eq!(param(&params, "sort"), Some("updated"));
        assert_eq!(param(&params, "order"), Some("desc"));
        assert_eq!(search_path(), "/search/repositories");
    }

    #[test]
    fn search_params_clamp_page_size() {
        assert_eq!(param(&search_params("q", 0), "per_page"), Some("1"));
        assert_eq!(param(&search_params("q", 1000), "per_page"), Some("100"));
    }

    #[test]
    fn resolve_url_trims_trailing_slash() {
        assert_eq!(resolve_url(None), DEFAULT_BASE_URL);
        assert_eq!(resolve_url(Some("  ")), DEFAULT_BASE_URL);
        assert_eq!(
            resolve_url(Some("http://127.0.0.1:1/")),
            "http://127.0.0.1:1"
        );
    }

    #[test]
    fn convert_maps_discovery_metadata() {
        let parsed: RepositorySearchResponse = serde_json::from_str(FIXTURE).expect("fixture");
        let out = convert(parsed.items, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "tokio-rs/axum");
        assert_eq!(out[0].url, "https://github.com/tokio-rs/axum");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("Web framework built on Tokio and Hyper")
        );
        assert_eq!(
            out[0].published_at.as_deref(),
            Some("2024-05-30T08:00:00+00:00")
        );
        assert_eq!(out[0].source_engine, "github_repositories");
        assert_eq!(
            out[1].snippet.as_deref(),
            Some("Go · 3 stars"),
            "missing description falls back to bounded metadata"
        );
        assert!(out[1].published_at.is_none());
    }

    #[test]
    fn convert_drops_entries_without_usable_url() {
        let out = convert(
            vec![RepositoryItem {
                full_name: Some("owner/repo".to_string()),
                html_url: Some("javascript:alert(1)".to_string()),
                ..RepositoryItem {
                    full_name: Some(String::new()),
                    html_url: None,
                    description: None,
                    language: None,
                    stargazers_count: None,
                    forks_count: None,
                    open_issues_count: None,
                    pushed_at: None,
                }
            }],
            10,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn convert_falls_back_to_repository_slug_title() {
        let out = convert(
            vec![RepositoryItem {
                full_name: None,
                html_url: Some("https://github.com/example/slug-repo".to_string()),
                description: None,
                language: None,
                stargazers_count: None,
                forks_count: None,
                open_issues_count: None,
                pushed_at: None,
            }],
            10,
        );
        assert_eq!(out[0].title, "slug-repo");
    }

    #[test]
    fn convert_respects_result_budget() {
        let parsed: RepositorySearchResponse = serde_json::from_str(FIXTURE).expect("fixture");
        let items = parsed.items;
        assert_eq!(convert(items.clone(), 1).len(), 1);
        assert_eq!(convert(items, 0).len(), 0);
    }

    #[test]
    fn empty_and_malformed_payloads_are_bounded() {
        let empty: RepositorySearchResponse = serde_json::from_str("{}").expect("empty");
        assert!(convert(empty.items, 10).is_empty());
        assert!(serde_json::from_str::<RepositorySearchResponse>(r#"{"items": 5}"#).is_err());
        assert!(serde_json::from_str::<RepositorySearchResponse>("{").is_err());
    }

    #[tokio::test]
    async fn zero_budget_short_circuits_without_request() {
        let client = Client::new();
        assert!(search(&client, None, None, &req("rust", 0))
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn descriptor_is_discovery_not_code_search() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "github_repositories",
            true,
            false,
            true,
            true,
            None,
            None,
        )
        .expect("descriptor");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::JsonApi);
        assert!(!desc.requires_api_key);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_repo_indexing);
        assert!(!desc.capabilities.supports_issue_search);
        assert!(!desc.capabilities.supports_release_search);
    }
}
