//! SerpAPI Google Search engine.
//!
//! Implements the current documented Google Search API contract:
//! `GET https://serpapi.com/search?engine=google&q=...`. The engine is set
//! explicitly instead of relying on the upstream default, and only
//! `organic_results` are read. SERP verticals that SerpAPI also sells
//! (`tbm=nws`, shopping, images, scholar) are never requested, and the
//! experimental `async`, `no_cache`, `zero_trace`, and `json_restrictor`
//! parameters are not used.
//!
//! The credential is a query parameter by upstream contract. To keep it out
//! of every error path, the base URL is handed to the transport first and
//! all parameters — including `api_key` — are added through the parameter
//! builder, which cannot fail with a URL-bearing error.
//!
//! `tbs` is documented only generically upstream ("advanced search
//! parameters ... dates") without value syntax, so freshness and explicit
//! date ranges are left to local approximation and are not advertised as
//! native capabilities.

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::request::EngineSearchRequest;
use crate::core::query::SafeSearch;

const ENGINE: &str = "serpapi";
const DEFAULT_URL: &str = "https://serpapi.com/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const SNIPPET_MAX_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
struct SerpApiResponse {
    #[serde(default)]
    organic_results: Vec<SerpApiOrganicResult>,
}

#[derive(Clone, Debug, Deserialize)]
struct SerpApiOrganicResult {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    snippet: Option<String>,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(url) if !url.trim().is_empty() => url.trim().to_string(),
        _ => DEFAULT_URL.to_string(),
    }
}

/// Google `safe` is a two-state filter, so the three-state provider-neutral
/// model collapses the two enforcing levels onto `active`.
fn map_safe_search(value: Option<SafeSearch>) -> Option<&'static str> {
    match value {
        None => None,
        Some(SafeSearch::Off) => Some("off"),
        Some(SafeSearch::Moderate) | Some(SafeSearch::Strict) => Some("active"),
    }
}

/// Map a language or locale onto Google's `hl` form
/// (`en`, `en-gb`); anything that is not representable is dropped so the
/// constraint degrades to local approximation instead of a wrong value.
fn map_language(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    let normalized = raw.replace('_', "-").to_ascii_lowercase();
    let parts: Vec<&str> = normalized.split('-').collect();
    match parts.as_slice() {
        [primary]
            if (2..=3).contains(&primary.len())
                && primary.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            Some(primary.to_string())
        }
        [primary, region]
            if (2..=3).contains(&primary.len())
                && region.len() == 2
                && primary.chars().all(|c| c.is_ascii_alphabetic())
                && region.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            Some(format!("{primary}-{region}"))
        }
        _ => None,
    }
}

/// Map a region onto Google's `gl` two-letter country code.
fn map_region(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.len() != 2 || !raw.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(raw.to_ascii_lowercase())
}

/// Every request parameter except the credential. `engine` is always sent
/// explicitly so a future upstream default change cannot silently alter the
/// engine that backs this provider id.
fn search_params(request: &EngineSearchRequest) -> Vec<(String, String)> {
    let mut params = vec![
        ("engine".to_string(), "google".to_string()),
        ("q".to_string(), request.query.clone()),
    ];
    if let Some(safe) = map_safe_search(request.safe_search) {
        params.push(("safe".to_string(), safe.to_string()));
    }
    if let Some(language) = map_language(request.language.as_deref()) {
        params.push(("hl".to_string(), language));
    }
    if let Some(region) = map_region(request.region.as_deref()) {
        params.push(("gl".to_string(), region));
    }
    params
}

fn snippet_for(result: &SerpApiOrganicResult) -> Option<String> {
    result
        .snippet
        .as_deref()
        .map(crate::core::sanitize::normalize_whitespace)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .map(|text| crate::core::sanitize::truncate_at_word(&text, SNIPPET_MAX_CHARS))
}

fn title_for(result: &SerpApiOrganicResult, url: &str) -> Option<String> {
    result
        .title
        .as_deref()
        .map(crate::core::sanitize::normalize_whitespace)
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .or_else(|| {
            url.rsplit('/')
                .find(|segment| !segment.is_empty())
                .map(str::to_string)
        })
}

fn convert(raw: Vec<SerpApiOrganicResult>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(raw.len()));
    for result in raw {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = result
            .link
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty() && super::is_http_url(url))
        else {
            continue;
        };
        let url = url.to_string();
        let Some(title) = title_for(&result, &url) else {
            continue;
        };
        out.push(SearchResult {
            title,
            url,
            snippet: snippet_for(&result),
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at: None,
            metadata: ResultMetadata::None,
        });
    }
    out
}

pub async fn search(
    client: &Client,
    api_key: &str,
    base_url: Option<&str>,
    request: &EngineSearchRequest,
) -> Result<Vec<SearchResult>, EngineError> {
    if request.max_results == 0 {
        return Ok(Vec::new());
    }
    let url = resolve_url(base_url);
    let mut req = client.get(url.as_str()).map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;
    for (key, value) in search_params(request) {
        req = req.query(key.as_str(), value.as_str());
    }
    req = req.header("Accept", "application/json");
    // The credential is appended last and only after the transport accepted
    // the endpoint, so no request-build failure can carry it.
    req = req.query("api_key", api_key);
    let resp = req
        .timeout(super::engine_timeout(request.timeout))
        .send()
        .await
        .map_err(|e| super::map_request_error(ENGINE, e))?;
    let status = resp.status();
    if !status.is_success() {
        // Provider-scoped failure. A 429 is classified as rate-limited by the
        // adapter (health/cooldown); it is never a global search failure and
        // the error body is deliberately not read or echoed.
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: status.as_u16(),
        });
    }
    let bytes = super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await?;
    let parsed: SerpApiResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(convert(parsed.organic_results, request.max_results))
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
      "search_metadata": {
        "id": "5f1b0d",
        "status": "Success",
        "created_at": "2026-09-01T10:00:00.000Z"
      },
      "search_parameters": {"q": "rust axum", "engine": "google"},
      "search_information": {
        "total_results": 1280000,
        "time_taken_displayed": 0.41
      },
      "organic_results": [
        {
          "position": 1,
          "title": "Axum  -  GitHub",
          "link": "https://github.com/tokio-rs/axum",
          "redirect_link": "https://www.google.com/url?sa=t&url=https://github.com/tokio-rs/axum",
          "displayed_link": "github.com › tokio-rs › axum",
          "snippet": "Web framework   built on Tokio and Hyper   with a\nconsistent API.",
          "source": "GitHub"
        },
        {
          "position": 2,
          "title": "Docs.rs  -  axum",
          "link": "https://docs.rs/axum/latest/axum/",
          "snippet": "Documentation for the axum crate."
        },
        {
          "position": 3,
          "title": "javascript link",
          "link": "javascript:alert(1)",
          "snippet": "not a source"
        }
      ],
      "related_searches": [{"query": "axum middleware"}],
      "pagination": {"current": 1}
    }"#;

    #[test]
    fn engine_is_always_pinned_to_google() {
        let params = search_params(&req("rust axum", 10));
        assert_eq!(param(&params, "engine"), Some("google"));
        assert_eq!(param(&params, "q"), Some("rust axum"));
    }

    #[test]
    fn no_unrequested_serp_verticals_or_experimental_modes() {
        let params = search_params(&req("rust", 10));
        for forbidden in ["tbm", "async", "no_cache", "zero_trace", "json_restrictor"] {
            assert!(
                param(&params, forbidden).is_none(),
                "{forbidden} must never be sent; eggsearch requests no extra vertical or mode"
            );
        }
    }

    #[test]
    fn result_count_is_enforced_locally_because_upstream_has_no_num_parameter() {
        let parsed: SerpApiResponse = serde_json::from_str(FIXTURE).expect("fixture");
        assert_eq!(
            convert(parsed.organic_results.clone(), 10).len(),
            2,
            "one organic entry is not an http(s) source"
        );
        assert_eq!(convert(parsed.organic_results, 1).len(), 1);
        assert!(convert(Vec::new(), 0).is_empty());
    }

    #[test]
    fn safe_search_maps_to_the_two_state_google_filter() {
        assert_eq!(map_safe_search(None), None);
        assert_eq!(map_safe_search(Some(SafeSearch::Off)), Some("off"));
        assert_eq!(map_safe_search(Some(SafeSearch::Moderate)), Some("active"));
        assert_eq!(map_safe_search(Some(SafeSearch::Strict)), Some("active"));
    }

    #[test]
    fn safe_search_is_only_sent_when_requested() {
        let mut request = req("rust", 5);
        assert!(param(&search_params(&request), "safe").is_none());
        request.safe_search = Some(SafeSearch::Strict);
        assert_eq!(param(&search_params(&request), "safe"), Some("active"));
    }

    #[test]
    fn language_maps_only_when_google_can_represent_it() {
        assert_eq!(map_language(Some("en")).as_deref(), Some("en"));
        assert_eq!(map_language(Some("en-US")).as_deref(), Some("en-us"));
        assert_eq!(map_language(Some("en_GB")).as_deref(), Some("en-gb"));
        assert_eq!(map_language(Some("not-a-locale!!!")), None);
        assert_eq!(map_language(Some("  ")), None);
        assert_eq!(map_language(None), None);
    }

    #[test]
    fn region_maps_only_two_letter_country_codes() {
        assert_eq!(map_region(Some("US")).as_deref(), Some("us"));
        assert_eq!(map_region(Some("fr")).as_deref(), Some("fr"));
        assert_eq!(map_region(Some("USA")), None);
        assert_eq!(map_region(Some("u1")), None);
        assert_eq!(map_region(None), None);
    }

    #[test]
    fn freshness_and_domain_filters_are_never_sent() {
        let mut request = req("rust", 5);
        request.freshness = crate::core::query::Freshness::Week;
        request.date_range = Some(crate::core::query::SearchDateRange::new(
            "2026-01-01",
            "2026-01-31",
        ));
        request.include_domains = vec!["docs.rs".to_string()];
        request.exclude_domains = vec!["example.com".to_string()];
        let params = search_params(&request);
        assert!(
            param(&params, "tbs").is_none(),
            "tbs value syntax is undocumented upstream; eggsearch must not guess it"
        );
        assert!(param(&params, "as_qdr").is_none());
        assert_eq!(param(&params, "q"), Some("rust"));
    }

    #[test]
    fn convert_maps_organic_results_without_timestamps_or_excerpts() {
        let parsed: SerpApiResponse = serde_json::from_str(FIXTURE).expect("fixture");
        let out = convert(parsed.organic_results, 10);
        assert_eq!(out[0].title, "Axum - GitHub");
        assert_eq!(out[0].url, "https://github.com/tokio-rs/axum");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("Web framework built on Tokio and Hyper with a consistent API.")
        );
        assert_eq!(out[0].source_engine, "serpapi");
        assert!(out[0].published_at.is_none());
        assert!(out[0].excerpts.is_empty());
    }

    #[test]
    fn convert_falls_back_to_url_segment_title() {
        let out = convert(
            vec![SerpApiOrganicResult {
                title: None,
                link: Some("https://example.com/docs/axum/".to_string()),
                snippet: None,
            }],
            10,
        );
        assert_eq!(out[0].title, "axum");
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn convert_drops_entries_without_usable_link() {
        let out = convert(
            vec![
                SerpApiOrganicResult {
                    title: Some("T".to_string()),
                    link: None,
                    snippet: Some("c".to_string()),
                },
                SerpApiOrganicResult {
                    title: Some("T".to_string()),
                    link: Some("   ".to_string()),
                    snippet: Some("c".to_string()),
                },
                SerpApiOrganicResult {
                    title: Some("T".to_string()),
                    link: Some("data:text/html,x".to_string()),
                    snippet: Some("c".to_string()),
                },
            ],
            10,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn empty_and_malformed_payloads_are_bounded() {
        let empty: SerpApiResponse = serde_json::from_str("{}").expect("empty");
        assert!(convert(empty.organic_results, 10).is_empty());
        let no_organic: SerpApiResponse =
            serde_json::from_str(r#"{"error": "Your account is out of credits."}"#)
                .expect("error envelope deserializes");
        assert!(
            no_organic.organic_results.is_empty(),
            "a 200 error envelope must yield no source cards"
        );
        assert!(serde_json::from_str::<SerpApiResponse>(r#"{"organic_results": 5}"#).is_err());
        assert!(serde_json::from_str::<SerpApiResponse>("{").is_err());
    }

    #[test]
    fn resolve_url_defaults_and_trims() {
        assert_eq!(resolve_url(None), DEFAULT_URL);
        assert_eq!(resolve_url(Some("  ")), DEFAULT_URL);
        assert_eq!(
            resolve_url(Some("  http://127.0.0.1:1/search  ")),
            "http://127.0.0.1:1/search"
        );
    }

    #[tokio::test]
    async fn zero_budget_short_circuits_without_request() {
        let client = Client::new();
        assert!(search(&client, "key", None, &req("rust", 0))
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn descriptor_is_opt_in_and_conservative() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "serpapi", true, false, true, true, None, None,
        )
        .expect("descriptor");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.capabilities.supports_safe_search);
        assert!(desc.capabilities.supports_language);
        assert!(desc.capabilities.supports_region);
        assert!(
            !desc.capabilities.supports_freshness,
            "tbs value syntax is undocumented; claiming freshness would overstate enforcement"
        );
        assert!(!desc.capabilities.supports_domain_filters);
        assert!(!desc.capabilities.supports_news);
        assert!(!desc.capabilities.supports_result_timestamps);
    }
}
