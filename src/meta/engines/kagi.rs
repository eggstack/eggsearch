//! Kagi Search API (v1) engine.
//!
//! Implements the current documented v1 contract: `POST
//! https://kagi.com/api/v1/search` with a JSON body and
//! `Authorization: Bearer <key>`. The historical v0 `GET /api/search`
//! endpoint used by CodeGG is never called, and no new dependency is added
//! for the call: the shared Eggfetch transport posts the JSON body.
//!
//! Every request pins `workflow: "search"` and only `data.search[]` is read.
//! Kagi returns several other collections (`data.news`, `data.code`,
//! `data.interesting_finds`, `data.related_search`, `data.infobox`, ...) in
//! the same payload; they are ignored rather than coerced into source cards.
//! Billed extras (`extract`) and account personalization
//! (`personalizations`) are never requested.
//!
//! Terms compliance (Kagi API Terms, effective 2026-09-22) constrains this
//! engine: results are not cached or persisted by the engine, and quota
//! responses are terminal provider-scoped failures with no retry.

use eggfetch_core::Client;
use serde::{Deserialize, Serialize};

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::request::EngineSearchRequest;
use crate::core::query::{Freshness, SafeSearch};

const ENGINE: &str = "kagi";
const DEFAULT_BASE_URL: &str = "https://kagi.com/api/v1";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_LIMIT: usize = 1024;
const SNIPPET_MAX_CHARS: usize = 500;
const DATE_FORMAT: &str = "%Y-%m-%d";

#[derive(Debug, Serialize)]
struct KagiSearchRequest {
    query: String,
    workflow: &'static str,
    limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    safe_search: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filters: Option<KagiFilters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lens: Option<KagiLens>,
}

#[derive(Debug, Serialize)]
struct KagiFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<String>,
}

#[derive(Debug, Serialize)]
struct KagiLens {
    #[serde(skip_serializing_if = "Option::is_none")]
    sites_included: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sites_excluded: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct KagiSearchResponse {
    #[serde(default)]
    data: Option<KagiData>,
}

#[derive(Debug, Deserialize)]
struct KagiData {
    #[serde(default)]
    search: Vec<KagiResult>,
}

#[derive(Clone, Debug, Deserialize)]
struct KagiResult {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    snippet: Option<String>,
    #[serde(default)]
    time: Option<String>,
}

fn resolve_base_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(url) if !url.trim().is_empty() => url.trim().trim_end_matches('/').to_string(),
        _ => DEFAULT_BASE_URL.to_string(),
    }
}

/// Kagi's `safe_search` is a single boolean, so the three-state
/// provider-neutral model collapses the two enforcing levels onto `true`.
fn map_safe_search(value: Option<SafeSearch>) -> Option<bool> {
    match value {
        None => None,
        Some(SafeSearch::Off) => Some(false),
        Some(SafeSearch::Moderate) | Some(SafeSearch::Strict) => Some(true),
    }
}

/// Map a region onto an ISO 3166-1 alpha-2 country code. Names and
/// malformed values are dropped so the constraint degrades to local
/// approximation instead of sending an unusable code.
fn map_region(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.len() != 2 || !raw.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(raw.to_ascii_uppercase())
}

fn parse_date(value: &str) -> Option<chrono::NaiveDate> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    chrono::NaiveDate::parse_from_str(trimmed, DATE_FORMAT).ok()
}

/// Calendar-day cutoff for a relative freshness window. `Year` is expressed
/// as twelve months so every freshness level has a faithful absolute
/// representation upstream.
fn freshness_cutoff(freshness: Freshness, today: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
    match freshness {
        Freshness::Any => None,
        Freshness::Day => today.checked_sub_days(chrono::Days::new(1)),
        Freshness::Week => today.checked_sub_days(chrono::Days::new(7)),
        Freshness::Month => today.checked_sub_months(chrono::Months::new(1)),
        Freshness::Year => today.checked_sub_months(chrono::Months::new(12)),
    }
}

/// Map freshness or an exact date range onto Kagi's absolute date filters.
/// An exact `date_range` wins over a relative `freshness` window, matching
/// the precedence used by the other constraint-mapping engines. Whether
/// upstream treats the bounds as inclusive is Kagi-defined; eggsearch passes
/// the caller's own bounds through unchanged.
fn date_filters(
    request: &EngineSearchRequest,
    today: chrono::NaiveDate,
) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
    if let Some(range) = &request.date_range {
        return (parse_date(&range.start), parse_date(&range.end));
    }
    (freshness_cutoff(request.freshness, today), None)
}

fn format_date(date: Option<chrono::NaiveDate>) -> Option<String> {
    date.map(|value| value.format(DATE_FORMAT).to_string())
}

fn filters_for(request: &EngineSearchRequest, today: chrono::NaiveDate) -> Option<KagiFilters> {
    let (after, before) = date_filters(request, today);
    let region = map_region(request.region.as_deref());
    if after.is_none() && before.is_none() && region.is_none() {
        return None;
    }
    Some(KagiFilters {
        region,
        after: format_date(after),
        before: format_date(before),
    })
}

/// Native domain filtering is expressed through Kagi's inline lens, whose
/// documented `sites_included`/`sites_excluded` fields are restrictions
/// ("search only these domains" / "exclude these domains"), not boosts.
fn lens_for(request: &EngineSearchRequest) -> Option<KagiLens> {
    let included = normalized_domains(&request.include_domains);
    let excluded = normalized_domains(&request.exclude_domains);
    if included.is_empty() && excluded.is_empty() {
        return None;
    }
    Some(KagiLens {
        sites_included: (!included.is_empty()).then_some(included),
        sites_excluded: (!excluded.is_empty()).then_some(excluded),
    })
}

fn normalized_domains(domains: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for domain in domains {
        let Ok(normalized) = crate::core::query::normalize_domain(domain) else {
            continue;
        };
        if normalized.is_empty() || out.contains(&normalized) {
            continue;
        }
        out.push(normalized);
    }
    out
}

fn build_request_body(
    request: &EngineSearchRequest,
    today: chrono::NaiveDate,
) -> KagiSearchRequest {
    KagiSearchRequest {
        query: request.query.clone(),
        workflow: "search",
        limit: request.max_results.clamp(1, MAX_LIMIT),
        safe_search: map_safe_search(request.safe_search),
        filters: filters_for(request, today),
        lens: lens_for(request),
    }
}

fn snippet_for(result: &KagiResult) -> Option<String> {
    result
        .snippet
        .as_deref()
        .map(crate::core::sanitize::normalize_whitespace)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .map(|text| crate::core::sanitize::truncate_at_word(&text, SNIPPET_MAX_CHARS))
}

fn title_for(result: &KagiResult, url: &str) -> Option<String> {
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

fn convert(raw: Vec<KagiResult>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(raw.len()));
    for result in raw {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = result
            .url
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
            published_at: result
                .time
                .as_deref()
                .and_then(crate::core::source_card::parse_result_timestamp),
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
    let url = format!("{}/search", resolve_base_url(base_url));
    let body = build_request_body(request, chrono::Utc::now().date_naive());
    let resp = client
        .post(url.as_str())
        .map_err(|e| EngineError::Http {
            engine: ENGINE,
            source: e,
        })?
        .json(&body)
        .map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("serialize: {e}"),
        })?
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {api_key}").as_str())
        .timeout(super::engine_timeout(request.timeout))
        .send()
        .await
        .map_err(|e| super::map_request_error(ENGINE, e))?;
    let status = resp.status();
    if !status.is_success() {
        // Kagi's terms forbid circumventing rate limits, so a quota response
        // is terminal: one attempt, provider-scoped rate-limited outcome, and
        // health cooldown. The error body is deliberately not read or echoed.
        return Err(EngineError::BadStatus {
            engine: ENGINE,
            status: status.as_u16(),
        });
    }
    let bytes = super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await?;
    let parsed: KagiSearchResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    let raw = parsed.data.map(|data| data.search).unwrap_or_default();
    Ok(convert(raw, request.max_results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::query::SearchDateRange;
    use std::time::Duration;

    fn day(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(y, m, d).expect("date")
    }

    fn today() -> chrono::NaiveDate {
        day(2026, 10, 2)
    }

    fn req(query: &str, max_results: usize) -> EngineSearchRequest {
        EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
    }

    const FIXTURE: &str = r#"{
      "meta": {"node": "kagi-1", "ms": 210, "trace": "abc123"},
      "data": {
        "search": [
          {
            "url": "https://kagi.com/blog/small-web",
            "title": "The small web",
            "snippet": "A  note\nabout independent  sites.  ",
            "time": "2024-11-29T03:54:26Z"
          },
          {
            "url": "https://example.org/quiet",
            "title": "Quiet page",
            "snippet": "No timestamp here.",
            "time": "not-a-timestamp"
          }
        ],
        "news": [{"url": "https://news.example/story", "title": "Story", "snippet": "s"}],
        "code": [{"url": "https://github.com/example/repo", "title": "repo", "snippet": "c"}],
        "related_search": [{"props": {"query": "other"}}],
        "interesting_finds": [{"url": "https://finds.example/x", "title": "x"}]
      }
    }"#;

    #[test]
    fn request_pins_the_search_workflow_and_never_extras() {
        let body = build_request_body(&req("rust axum", 10), today());
        assert_eq!(body.workflow, "search");
        assert_eq!(body.query, "rust axum");
        assert_eq!(body.limit, 10);
        let json = serde_json::to_string(&body).expect("serializable");
        assert!(
            !json.contains("\"extract\""),
            "extract is billed separately and must never be requested"
        );
        assert!(
            !json.contains("\"personalizations\""),
            "account personalization must never be sent"
        );
        assert!(!json.contains("\"format\""));
    }

    #[test]
    fn limit_is_clamped_to_the_documented_range() {
        assert_eq!(build_request_body(&req("q", 1), today()).limit, 1);
        assert_eq!(
            build_request_body(&req("q", 5000), today()).limit,
            MAX_LIMIT
        );
    }

    #[test]
    fn optional_fields_are_omitted_when_unconstrained() {
        let body = build_request_body(&req("q", 5), today());
        assert!(body.safe_search.is_none());
        assert!(body.filters.is_none());
        assert!(body.lens.is_none());
        let json = serde_json::to_string(&body).expect("serializable");
        assert_eq!(
            json, r#"{"query":"q","workflow":"search","limit":5}"#,
            "an unconstrained request must send only the required fields"
        );
    }

    #[test]
    fn safe_search_collapses_moderate_and_strict_to_true() {
        assert_eq!(map_safe_search(None), None);
        assert_eq!(map_safe_search(Some(SafeSearch::Off)), Some(false));
        assert_eq!(map_safe_search(Some(SafeSearch::Moderate)), Some(true));
        assert_eq!(map_safe_search(Some(SafeSearch::Strict)), Some(true));
    }

    #[test]
    fn region_maps_only_iso_alpha_2_codes() {
        assert_eq!(map_region(Some("us")).as_deref(), Some("US"));
        assert_eq!(map_region(Some("GB")).as_deref(), Some("GB"));
        assert_eq!(map_region(Some("france")), None);
        assert_eq!(map_region(Some("USA")), None);
        assert_eq!(map_region(None), None);
    }

    #[test]
    fn language_is_never_claimed_or_sent() {
        let mut request = req("q", 5);
        request.language = Some("en".to_string());
        let json =
            serde_json::to_string(&build_request_body(&request, today())).expect("serializable");
        assert!(
            !json.contains("\"en\""),
            "Kagi exposes no language field; a language must not be invented"
        );
    }

    #[test]
    fn relative_freshness_becomes_an_absolute_cutoff_date() {
        for (freshness, expected) in [
            (Freshness::Day, "2026-10-01"),
            (Freshness::Week, "2026-09-25"),
            (Freshness::Month, "2026-09-02"),
            (Freshness::Year, "2025-10-02"),
        ] {
            let mut request = req("q", 5);
            request.freshness = freshness;
            let body = build_request_body(&request, today());
            let filters = body.filters.expect("freshness is native");
            assert_eq!(filters.after.as_deref(), Some(expected));
            assert!(filters.before.is_none());
        }
        let plain = build_request_body(&req("q", 5), today());
        assert!(plain.filters.is_none(), "Freshness::Any sends no filter");
    }

    #[test]
    fn exact_date_range_wins_over_relative_freshness() {
        let mut request = req("q", 5);
        request.freshness = Freshness::Day;
        request.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
        let body = build_request_body(&request, today());
        let filters = body.filters.expect("range is native");
        assert_eq!(filters.after.as_deref(), Some("2024-01-01"));
        assert_eq!(filters.before.as_deref(), Some("2024-01-31"));
    }

    #[test]
    fn unparsable_date_range_is_dropped_rather_than_guessed() {
        let mut request = req("q", 5);
        request.date_range = Some(SearchDateRange::new("yesterday", "tomorrow"));
        assert!(build_request_body(&request, today()).filters.is_none());
    }

    #[test]
    fn domain_filters_map_onto_the_inline_lens() {
        let mut request = req("q", 5);
        request.include_domains = vec!["Docs.RS".to_string(), "docs.rs".to_string()];
        request.exclude_domains = vec!["example.com".to_string()];
        let body = build_request_body(&request, today());
        let lens = body.lens.expect("domain filters are native");
        assert_eq!(
            lens.sites_included.as_deref(),
            Some(["docs.rs".to_string()].as_slice()),
            "domains are normalized and deduplicated"
        );
        assert_eq!(
            lens.sites_excluded.as_deref(),
            Some(["example.com".to_string()].as_slice())
        );
        let json = serde_json::to_string(&lens).expect("serializable");
        assert!(json.contains("\"sites_included\""));
        assert!(json.contains("\"sites_excluded\""));
    }

    #[test]
    fn convert_reads_only_the_search_collection() {
        let parsed: KagiSearchResponse = serde_json::from_str(FIXTURE).expect("fixture");
        let out = convert(parsed.data.expect("data").search, 10);
        assert_eq!(out.len(), 2);
        assert!(
            out.iter().all(|r| r.url.starts_with("https://")),
            "news/code/finds/related collections must not become source cards"
        );
        assert_eq!(out[0].url, "https://kagi.com/blog/small-web");
        assert_eq!(out[0].title, "The small web");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("A note about independent sites.")
        );
        assert_eq!(
            out[0].published_at.as_deref(),
            Some("2024-11-29T03:54:26+00:00")
        );
        assert!(out[1].published_at.is_none());
    }

    #[test]
    fn convert_drops_entries_without_usable_url() {
        let out = convert(
            vec![
                KagiResult {
                    url: None,
                    title: Some("t".to_string()),
                    snippet: None,
                    time: None,
                },
                KagiResult {
                    url: Some("javascript:alert(1)".to_string()),
                    title: Some("t".to_string()),
                    snippet: None,
                    time: None,
                },
            ],
            10,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn convert_falls_back_to_url_segment_title() {
        let out = convert(
            vec![KagiResult {
                url: Some("https://example.com/guides/axum/".to_string()),
                title: None,
                snippet: None,
                time: None,
            }],
            10,
        );
        assert_eq!(out[0].title, "axum");
    }

    #[test]
    fn convert_respects_result_budget() {
        let parsed: KagiSearchResponse = serde_json::from_str(FIXTURE).expect("fixture");
        let raw = parsed.data.expect("data").search;
        assert_eq!(convert(raw.clone(), 1).len(), 1);
        assert!(convert(raw, 0).is_empty());
    }

    #[test]
    fn empty_null_data_and_malformed_payloads_are_bounded() {
        let empty: KagiSearchResponse = serde_json::from_str("{}").expect("empty");
        assert!(empty.data.is_none());
        let null_data: KagiSearchResponse =
            serde_json::from_str(r#"{"data": null}"#).expect("null");
        assert!(null_data.data.is_none());
        let error_body: KagiSearchResponse = serde_json::from_str(
            r#"{"meta":{},"data":null,"error":[{"code":"search.invalid","message":"bad query","url":"https://help.kagi.com/api/errors"}]}"#,
        )
        .expect("error envelope");
        assert!(error_body.data.is_none());
        assert!(serde_json::from_str::<KagiSearchResponse>(r#"{"data": {"search": 5}}"#).is_err());
        assert!(serde_json::from_str::<KagiSearchResponse>("{").is_err());
    }

    #[test]
    fn resolve_base_url_defaults_and_trims() {
        assert_eq!(resolve_base_url(None), DEFAULT_BASE_URL);
        assert_eq!(resolve_base_url(Some("   ")), DEFAULT_BASE_URL);
        assert_eq!(
            resolve_base_url(Some(" http://127.0.0.1:1/api/v1/ ")),
            "http://127.0.0.1:1/api/v1"
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
            "kagi", true, false, true, true, None, None,
        )
        .expect("descriptor");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.capabilities.supports_safe_search);
        assert!(desc.capabilities.supports_freshness);
        assert!(desc.capabilities.supports_region);
        assert!(desc.capabilities.supports_domain_filters);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(
            !desc.capabilities.supports_language,
            "Kagi exposes no language field"
        );
        assert!(!desc.capabilities.supports_news);
    }
}
