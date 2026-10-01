//! Hacker News provider built on the public HN Search API (Algolia).
//!
//! Constrains retrieval to story results for parity with ordinary Hacker
//! News search and maps eggsearch freshness/date-range constraints onto
//! the documented `numericFilters=created_at_i>…` contract before claiming
//! native freshness support.

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::request::EngineSearchRequest;
use crate::core::query::Freshness;

const ENGINE: &str = "hn_algolia";
const DEFAULT_BASE_URL: &str = "https://hn.algolia.com/api/v1/search";
const ITEM_BASE_URL: &str = "https://news.ycombinator.com/item?id=";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_HITS_PER_PAGE: usize = 50;
const SNIPPET_MAX_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
struct HnResponse {
    #[serde(default)]
    hits: Vec<HnHit>,
}

#[derive(Debug, Deserialize)]
struct HnHit {
    #[serde(default, rename = "objectID")]
    object_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    story_text: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    points: Option<i64>,
    #[serde(default)]
    num_comments: Option<i64>,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(url) if !url.trim().is_empty() => url.trim().to_string(),
        _ => DEFAULT_BASE_URL.to_string(),
    }
}

fn fresh_window(freshness: Freshness) -> Option<i64> {
    match freshness {
        Freshness::Any => None,
        Freshness::Day => Some(24 * 60 * 60),
        Freshness::Week => Some(7 * 24 * 60 * 60),
        Freshness::Month => Some(30 * 24 * 60 * 60),
        Freshness::Year => Some(365 * 24 * 60 * 60),
    }
}

fn epoch_day(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let date = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()?;
    Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp())
}

/// Map freshness or an exact date range onto the API's `numericFilters`.
///
/// An exact `date_range` wins over a relative `freshness` window, matching
/// the precedence used by the other constraint-mapping engines.
fn numeric_filters(request: &EngineSearchRequest, now_epoch: i64) -> Option<String> {
    if let Some(range) = &request.date_range {
        let start = epoch_day(&range.start);
        let end = epoch_day(&range.end);
        return match (start, end) {
            (Some(start), Some(end)) => Some(format!("created_at_i>={start},created_at_i<={end}")),
            (Some(start), None) => Some(format!("created_at_i>={start}")),
            (None, Some(end)) => Some(format!("created_at_i<={end}")),
            (None, None) => None,
        };
    }
    let window = fresh_window(request.freshness)?;
    Some(format!("created_at_i>{}", now_epoch - window))
}

fn search_params(request: &EngineSearchRequest, now_epoch: i64) -> Vec<(String, String)> {
    let mut params = vec![
        ("query".to_string(), request.query.clone()),
        ("tags".to_string(), "story".to_string()),
        (
            "hitsPerPage".to_string(),
            request.max_results.clamp(1, MAX_HITS_PER_PAGE).to_string(),
        ),
        ("page".to_string(), "0".to_string()),
    ];
    if let Some(filters) = numeric_filters(request, now_epoch) {
        params.push(("numericFilters".to_string(), filters));
    }
    params
}

fn item_url(object_id: &str) -> String {
    format!("{ITEM_BASE_URL}{}", urlencoding::encode(object_id.trim()))
}

fn html_to_text(raw: &str, max_chars: usize) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let (_, _, body, ..) = crate::fetch::extract::HtmlExtractor::new(raw.as_bytes(), ITEM_BASE_URL)
        .extract(max_chars * 2, false);
    let normalized = crate::core::sanitize::normalize_whitespace(&body);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(crate::core::sanitize::truncate_at_word(trimmed, max_chars))
}

fn discussion_snippet(hit: &HnHit) -> Option<String> {
    if let Some(text) = hit
        .story_text
        .as_deref()
        .and_then(|raw| html_to_text(raw, SNIPPET_MAX_CHARS))
    {
        return Some(text);
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(points) = hit.points {
        parts.push(format!("{points} points"));
    }
    if let Some(comments) = hit.num_comments {
        parts.push(format!("{comments} comments"));
    }
    if let Some(author) = hit
        .author
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty())
    {
        parts.push(format!("by {author}"));
    }
    if parts.is_empty() {
        return None;
    }
    let joined = parts.join(" · ");
    Some(crate::core::sanitize::truncate_at_word(
        &joined,
        SNIPPET_MAX_CHARS,
    ))
}

fn convert(hits: Vec<HnHit>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(hits.len()));
    for hit in hits {
        if out.len() >= max_results {
            break;
        }
        let external = hit
            .url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty() && super::is_http_url(u))
            .map(str::to_string);
        let discussion = hit
            .object_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(item_url);
        let url = external.or(discussion.clone());
        let Some(url) = url else {
            continue;
        };
        let title = hit
            .title
            .as_deref()
            .map(crate::core::sanitize::normalize_whitespace)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| discussion.clone().unwrap_or_else(|| url.clone()));
        out.push(SearchResult {
            title,
            url,
            snippet: discussion_snippet(&hit),
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at: hit
                .created_at
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
    request: &EngineSearchRequest,
) -> Result<Vec<SearchResult>, EngineError> {
    if request.max_results == 0 {
        return Ok(Vec::new());
    }
    let url = resolve_url(base_url);
    let now_epoch = chrono::Utc::now().timestamp();
    let mut req = client.get(url.as_str()).map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;
    for (key, value) in search_params(request, now_epoch) {
        req = req.query(key.as_str(), value.as_str());
    }
    let resp = req
        .header("Accept", "application/json")
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
    let parsed: HnResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(convert(parsed.hits, request.max_results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::query::SearchDateRange;
    use std::time::Duration;

    const NOW: i64 = 1_700_000_000;

    fn req(query: &str, max_results: usize) -> EngineSearchRequest {
        EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
    }

    fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    const SEARCH_FIXTURE: &str = r#"{
      "hits": [
        {
          "objectID": "38765432",
          "title": "Show HN: A tiny search engine",
          "url": "https://example.com/tiny-search",
          "story_text": "<p>I wrote a <b>small</b> search engine.</p>",
          "author": "alice",
          "points": 128,
          "num_comments": 42,
          "created_at": "2024-02-01T10:00:00.000Z"
        },
        {
          "objectID": "38765433",
          "title": "Ask HN: search backends",
          "story_text": "<p>Which do you prefer?</p>",
          "author": "bob",
          "points": 7,
          "num_comments": 3,
          "created_at": "2024-02-02T11:00:00.000Z"
        }
      ],
      "nbHits": 2,
      "page": 0
    }"#;

    #[test]
    fn search_params_constrain_to_stories() {
        let params = search_params(&req("rust", 5), NOW);
        assert_eq!(param(&params, "query"), Some("rust"));
        assert_eq!(param(&params, "tags"), Some("story"));
        assert_eq!(param(&params, "hitsPerPage"), Some("5"));
        assert_eq!(param(&params, "page"), Some("0"));
        assert_eq!(param(&params, "numericFilters"), None);
    }

    #[test]
    fn search_params_clamp_page_size() {
        let params = search_params(&req("q", 0), NOW);
        assert_eq!(param(&params, "hitsPerPage"), Some("1"));
        let params = search_params(&req("q", 500), NOW);
        assert_eq!(param(&params, "hitsPerPage"), Some("50"));
    }

    #[test]
    fn relative_freshness_maps_to_created_at_filter() {
        let mut request = req("rust", 5);
        request.freshness = Freshness::Week;
        let filters = numeric_filters(&request, NOW).expect("filters");
        assert_eq!(filters, format!("created_at_i>{}", NOW - 7 * 24 * 60 * 60));
        request.freshness = Freshness::Any;
        assert!(numeric_filters(&request, NOW).is_none());
    }

    #[test]
    fn exact_date_range_maps_to_both_bounds() {
        let mut request = req("rust", 5);
        request.date_range = Some(SearchDateRange::new("2024-01-01", "2024-02-01"));
        let filters = numeric_filters(&request, NOW).expect("filters");
        let start = epoch_day("2024-01-01").expect("start");
        let end = epoch_day("2024-02-01").expect("end");
        assert_eq!(
            filters,
            format!("created_at_i>={start},created_at_i<={end}")
        );
    }

    #[test]
    fn exact_range_wins_over_relative_freshness() {
        let mut request = req("rust", 5);
        request.freshness = Freshness::Day;
        request.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
        let filters = numeric_filters(&request, NOW).expect("filters");
        assert!(filters.starts_with("created_at_i>="));
        assert!(filters.contains(','));
    }

    #[test]
    fn invalid_range_bounds_are_omitted_not_guessed() {
        let mut request = req("rust", 5);
        request.date_range = Some(SearchDateRange::new("2024-13-45", "not-a-date"));
        assert!(numeric_filters(&request, NOW).is_none());
        assert!(epoch_day("").is_none());
    }

    #[test]
    fn convert_prefers_external_url_and_discussion_fallback() {
        let parsed: HnResponse = serde_json::from_str(SEARCH_FIXTURE).expect("fixture");
        let out = convert(parsed.hits, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].url, "https://example.com/tiny-search");
        assert_eq!(out[1].url, "https://news.ycombinator.com/item?id=38765433");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("I wrote a small search engine.")
        );
        assert_eq!(
            out[1].snippet.as_deref(),
            Some("Which do you prefer?"),
            "story text wins over synthesized discussion metadata"
        );
        assert_eq!(
            out[0].published_at.as_deref(),
            Some("2024-02-01T10:00:00+00:00")
        );
        assert_eq!(out[0].source_engine, "hn_algolia");
    }

    #[test]
    fn convert_falls_back_to_discussion_url_as_title() {
        let out = convert(
            vec![HnHit {
                object_id: Some("1".to_string()),
                title: None,
                url: None,
                story_text: None,
                author: Some("carol".to_string()),
                created_at: None,
                points: Some(5),
                num_comments: Some(1),
            }],
            10,
        );
        assert_eq!(out[0].title, "https://news.ycombinator.com/item?id=1");
        assert_eq!(out[0].url, "https://news.ycombinator.com/item?id=1");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("5 points · 1 comments · by carol"),
            "discussion metadata is synthesized when there is no story text"
        );
    }

    #[test]
    fn convert_omits_metadata_snippet_when_nothing_is_usable() {
        let out = convert(
            vec![HnHit {
                object_id: Some("2".to_string()),
                title: None,
                url: None,
                story_text: Some("   ".to_string()),
                author: None,
                created_at: None,
                points: None,
                num_comments: None,
            }],
            10,
        );
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn convert_drops_unaddressable_hits_and_respects_budget() {
        let out = convert(
            vec![HnHit {
                object_id: Some("  ".to_string()),
                title: Some("orphan".to_string()),
                url: Some("not-a-url".to_string()),
                story_text: None,
                author: None,
                created_at: None,
                points: None,
                num_comments: None,
            }],
            10,
        );
        assert!(out.is_empty());
        let parsed: HnResponse = serde_json::from_str(SEARCH_FIXTURE).expect("fixture");
        assert_eq!(convert(parsed.hits, 1).len(), 1);
    }

    #[test]
    fn empty_and_malformed_payloads_are_bounded() {
        let empty: HnResponse = serde_json::from_str("{}").expect("empty");
        assert!(convert(empty.hits, 10).is_empty());
        assert!(serde_json::from_str::<HnResponse>(r#"{"hits": 5}"#).is_err());
        assert!(serde_json::from_str::<HnResponse>("{").is_err());
    }

    #[tokio::test]
    async fn zero_budget_short_circuits_without_request() {
        let client = Client::new();
        assert!(search(&client, None, &req("rust", 0))
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn descriptor_claims_native_freshness_and_timestamps() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "hn_algolia",
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
        assert!(desc.capabilities.supports_freshness);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_language);
        assert!(!desc.capabilities.supports_domain_filters);
    }
}
