use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::error::EngineError;
use super::models::SearchResult;
use super::request::EngineSearchRequest;
use crate::core::query::{Freshness, SearchDateRange};

const ENGINE: &str = "exa";
const DEFAULT_URL: &str = "https://api.exa.ai/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const EXA_MAX_RESULTS: usize = 100;

#[derive(Debug, Serialize)]
struct ExaContents {
    highlights: bool,
}

#[derive(Debug, Serialize)]
struct ExaSearchRequest {
    query: String,
    #[serde(rename = "numResults")]
    num_results: usize,
    #[serde(rename = "type")]
    search_type: String,
    #[serde(rename = "includeDomains", skip_serializing_if = "Option::is_none")]
    include_domains: Option<Vec<String>>,
    #[serde(rename = "excludeDomains", skip_serializing_if = "Option::is_none")]
    exclude_domains: Option<Vec<String>>,
    #[serde(rename = "startPublishedDate", skip_serializing_if = "Option::is_none")]
    start_published_date: Option<String>,
    #[serde(rename = "endPublishedDate", skip_serializing_if = "Option::is_none")]
    end_published_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contents: Option<ExaContents>,
}

#[derive(Debug, Deserialize)]
struct ExaSearchResponse {
    #[serde(default)]
    results: Vec<ExaResult>,
}

#[derive(Debug, Deserialize)]
struct ExaResult {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "publishedDate")]
    published_date: Option<String>,
    #[serde(default)]
    highlights: Option<Vec<String>>,
    #[serde(default, rename = "highlightScores")]
    highlight_scores: Option<Vec<f64>>,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => DEFAULT_URL.to_string(),
    }
}

fn clamp_num_results(max_results: usize) -> usize {
    max_results.clamp(1, EXA_MAX_RESULTS)
}

fn format_day_start(date: &str) -> String {
    format!("{}T00:00:00.000Z", date.trim())
}

fn format_day_end(date: &str) -> String {
    format!("{}T23:59:59.999Z", date.trim())
}

fn relative_start_bound(freshness: Freshness, now: DateTime<Utc>) -> Option<String> {
    let days: i64 = match freshness {
        Freshness::Any => return None,
        Freshness::Day => 1,
        Freshness::Week => 7,
        Freshness::Month => 30,
        Freshness::Year => 365,
    };
    let lower = now - chrono::Duration::days(days);
    Some(lower.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

fn published_date_bounds(
    freshness: Freshness,
    date_range: Option<&SearchDateRange>,
    now: DateTime<Utc>,
) -> (Option<String>, Option<String>) {
    if let Some(range) = date_range {
        let start = range.start.trim();
        let end = range.end.trim();
        if start.is_empty() || end.is_empty() {
            return (None, None);
        }
        return (Some(format_day_start(start)), Some(format_day_end(end)));
    }
    match relative_start_bound(freshness, now) {
        Some(lower) => (Some(lower), None),
        None => (None, None),
    }
}

fn build_request_body(request: &EngineSearchRequest, now: DateTime<Utc>) -> ExaSearchRequest {
    let num_results = clamp_num_results(request.max_results);
    let include_domains = if request.include_domains.is_empty() {
        None
    } else {
        Some(request.include_domains.clone())
    };
    let exclude_domains = if request.exclude_domains.is_empty() {
        None
    } else {
        Some(request.exclude_domains.clone())
    };
    let (start_published_date, end_published_date) =
        published_date_bounds(request.freshness, request.date_range.as_ref(), now);
    let contents = if request.wants_excerpts() {
        Some(ExaContents { highlights: true })
    } else {
        None
    };
    ExaSearchRequest {
        query: request.query.clone(),
        num_results,
        search_type: "auto".to_string(),
        include_domains,
        exclude_domains,
        start_published_date,
        end_published_date,
        contents,
    }
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
    let now = Utc::now();
    let body = build_request_body(request, now);
    let timeout = request.timeout;
    let max_results = request.max_results;
    let excerpt_count = request
        .excerpt_count
        .min(crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT);
    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .post(url)
            .json(&body)
            .header("Accept", "application/json")
            .header("x-api-key", api_key)
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

    let parsed: ExaSearchResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(convert(parsed.results, max_results, excerpt_count))
}

fn convert(raw: Vec<ExaResult>, max_results: usize, excerpt_count: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(raw.len()));
    for r in raw {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = r.url else { continue };
        if !super::is_http_url(&url) {
            continue;
        }
        let title = r
            .title
            .map(|t| crate::core::sanitize::normalize_whitespace(&t))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let Some(title) = title else { continue };
        let published_at = r
            .published_date
            .as_deref()
            .and_then(crate::core::source_card::parse_result_timestamp);
        let mut excerpts = Vec::new();
        if excerpt_count > 0 {
            if let Some(highlights) = r.highlights {
                let scores = r.highlight_scores.unwrap_or_default();
                for (idx, text) in highlights.into_iter().enumerate() {
                    if excerpts.len() >= excerpt_count {
                        break;
                    }
                    let text = crate::core::sanitize::normalize_whitespace(&text)
                        .trim()
                        .to_string();
                    if text.is_empty() {
                        continue;
                    }
                    let score = scores.get(idx).copied();
                    excerpts.push(crate::core::source_card::SourceExcerpt {
                        text,
                        score,
                        provenance: crate::core::source_card::ExcerptProvenance::ProviderHighlight,
                    });
                }
            }
        }
        out.push(SearchResult {
            title,
            url,
            snippet: None,
            source_engine: ENGINE.to_string(),
            excerpts,
            published_at,
            metadata: Default::default(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use chrono::TimeZone;

    fn simple_req(query: &str, max_results: usize) -> EngineSearchRequest {
        EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0)
            .single()
            .expect("valid datetime")
    }

    #[test]
    fn clamp_num_results_bounds() {
        assert_eq!(clamp_num_results(0), 1);
        assert_eq!(clamp_num_results(5), 5);
        assert_eq!(clamp_num_results(500), EXA_MAX_RESULTS);
    }

    #[test]
    fn resolve_url_defaults_and_trims() {
        assert_eq!(resolve_url(None), DEFAULT_URL);
        assert_eq!(resolve_url(Some("")), DEFAULT_URL);
        assert_eq!(resolve_url(Some("  ")), DEFAULT_URL);
        assert_eq!(
            resolve_url(Some("  https://proxy.example/search  ")),
            "https://proxy.example/search"
        );
    }

    #[test]
    fn exact_range_maps_to_publication_bounds() {
        let range = SearchDateRange::new("2024-01-01", "2024-01-31");
        let (start, end) = published_date_bounds(Freshness::Any, Some(&range), fixed_now());
        assert_eq!(start.as_deref(), Some("2024-01-01T00:00:00.000Z"));
        assert_eq!(end.as_deref(), Some("2024-01-31T23:59:59.999Z"));
    }

    #[test]
    fn exact_range_takes_precedence_over_freshness() {
        let mut req = simple_req("q", 5);
        req.freshness = Freshness::Week;
        req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-01-31"));
        let body = build_request_body(&req, fixed_now());
        assert_eq!(
            body.start_published_date.as_deref(),
            Some("2024-01-01T00:00:00.000Z")
        );
        assert_eq!(
            body.end_published_date.as_deref(),
            Some("2024-01-31T23:59:59.999Z")
        );
    }

    #[test]
    fn relative_freshness_uses_utc_lower_bound_and_omits_end() {
        for (freshness, days) in [
            (Freshness::Day, 1),
            (Freshness::Week, 7),
            (Freshness::Month, 30),
            (Freshness::Year, 365),
        ] {
            let (start, end) = published_date_bounds(freshness, None, fixed_now());
            let start = start.expect("relative freshness sets start");
            assert!(end.is_none(), "relative end bound is omitted");
            let parsed = chrono::DateTime::parse_from_rfc3339(&start)
                .expect("start is RFC 3339")
                .with_timezone(&Utc);
            let expected = fixed_now() - chrono::Duration::days(days);
            assert_eq!(parsed, expected);
        }
        let (start, end) = published_date_bounds(Freshness::Any, None, fixed_now());
        assert!(start.is_none());
        assert!(end.is_none());
    }

    #[test]
    fn default_request_contains_query_count_type_only() {
        let req = simple_req("rust async", 10);
        let body = build_request_body(&req, fixed_now());
        let value = serde_json::to_value(&body).expect("serializable");
        assert_eq!(value["query"], "rust async");
        assert_eq!(value["numResults"], 10);
        assert_eq!(value["type"], "auto");
        assert!(value.get("includeDomains").is_none());
        assert!(value.get("excludeDomains").is_none());
        assert!(value.get("startPublishedDate").is_none());
        assert!(value.get("endPublishedDate").is_none());
        assert!(value.get("contents").is_none());
        let text = serde_json::to_string(&value).expect("stringify");
        for forbidden in [
            "summary",
            "context",
            "text",
            "subpages",
            "systemPrompt",
            "outputSchema",
            "additionalQueries",
            "livecrawl",
            "maxAgeHours",
        ] {
            assert!(
                !text.contains(forbidden),
                "default request must not contain {forbidden}: {text}"
            );
        }
    }

    #[test]
    fn domains_map_to_native_fields() {
        let mut req = simple_req("q", 5);
        req.include_domains = vec!["example.com".to_string()];
        req.exclude_domains = vec!["spam.example".to_string()];
        let body = build_request_body(&req, fixed_now());
        let value = serde_json::to_value(&body).expect("serializable");
        assert_eq!(value["includeDomains"], serde_json::json!(["example.com"]));
        assert_eq!(value["excludeDomains"], serde_json::json!(["spam.example"]));
    }

    #[test]
    fn highlights_requested_only_with_excerpt_demand() {
        let plain = simple_req("q", 5);
        let plain_body = build_request_body(&plain, fixed_now());
        assert!(plain_body.contents.is_none());

        let mut with = simple_req("q", 5);
        with.excerpt_count = 2;
        let with_body = build_request_body(&with, fixed_now());
        assert!(with_body.contents.as_ref().is_some_and(|c| c.highlights));
        let value = serde_json::to_value(&with_body).expect("serializable");
        assert_eq!(value["contents"]["highlights"], true);
    }

    #[test]
    fn convert_maps_timestamp_and_highlights_with_scores() {
        let raw = vec![ExaResult {
            title: Some("Example".to_string()),
            url: Some("https://example.com/a".to_string()),
            published_date: Some("2023-11-16T01:36:32.547Z".to_string()),
            highlights: Some(vec!["first highlight".to_string(), "second".to_string()]),
            highlight_scores: Some(vec![0.9, 0.4]),
        }];
        let out = convert(raw, 10, 2);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source_engine, "exa");
        assert!(out[0].published_at.is_some());
        assert_eq!(out[0].excerpts.len(), 2);
        assert_eq!(out[0].excerpts[0].text, "first highlight");
        assert_eq!(out[0].excerpts[0].score, Some(0.9));
        assert_eq!(out[0].excerpts[1].score, Some(0.4));
        assert!(matches!(
            out[0].excerpts[0].provenance,
            crate::core::source_card::ExcerptProvenance::ProviderHighlight
        ));
    }

    #[test]
    fn convert_ignores_highlights_without_demand() {
        let raw = vec![ExaResult {
            title: Some("T".to_string()),
            url: Some("https://example.com".to_string()),
            published_date: None,
            highlights: Some(vec!["h1".to_string()]),
            highlight_scores: None,
        }];
        let out = convert(raw, 10, 0);
        assert_eq!(out.len(), 1);
        assert!(out[0].excerpts.is_empty());
    }

    #[test]
    fn convert_ignores_invalid_timestamp_but_keeps_result() {
        let raw = vec![ExaResult {
            title: Some("T".to_string()),
            url: Some("https://example.com".to_string()),
            published_date: Some("not-a-date".to_string()),
            highlights: None,
            highlight_scores: None,
        }];
        let out = convert(raw, 10, 0);
        assert_eq!(out.len(), 1);
        assert!(out[0].published_at.is_none());
    }

    #[test]
    fn convert_bounds_highlights_and_skips_empty() {
        let raw = vec![ExaResult {
            title: Some("T".to_string()),
            url: Some("https://example.com".to_string()),
            published_date: None,
            highlights: Some(vec![
                "a".to_string(),
                String::new(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
            ]),
            highlight_scores: Some(vec![0.1]),
        }];
        let out = convert(raw, 10, 2);
        assert_eq!(out[0].excerpts.len(), 2);
        assert_eq!(out[0].excerpts[0].score, Some(0.1));
        assert_eq!(out[0].excerpts[1].score, None);
    }

    #[test]
    fn convert_skips_missing_title_and_non_http() {
        let raw = vec![
            ExaResult {
                title: None,
                url: Some("https://example.com/a".to_string()),
                published_date: None,
                highlights: None,
                highlight_scores: None,
            },
            ExaResult {
                title: Some("T".to_string()),
                url: Some("/relative".to_string()),
                published_date: None,
                highlights: None,
                highlight_scores: None,
            },
            ExaResult {
                title: Some("Valid".to_string()),
                url: Some("https://valid.example".to_string()),
                published_date: None,
                highlights: None,
                highlight_scores: None,
            },
        ];
        let out = convert(raw, 10, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "https://valid.example");
    }

    #[test]
    fn descriptor_flags_are_conservative() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "exa", true, false, true, false, None, None,
        )
        .expect("descriptor");
        assert_eq!(desc.id, "exa");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.capabilities.supports_freshness);
        assert!(desc.capabilities.supports_domain_filters);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_safe_search);
        assert!(!desc.capabilities.supports_language);
        assert!(!desc.capabilities.supports_region);
        assert!(!desc.capabilities.supports_news);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_issue_search);
        assert!(!desc.capabilities.supports_release_search);
    }
}
