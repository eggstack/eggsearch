//! Wikipedia provider built on the MediaWiki Action API.
//!
//! Uses the documented `action=query&list=search` contract against the
//! English Wikipedia Action API endpoint and converts the response into
//! ordinary source cards. Search snippets arrive as small HTML fragments
//! and are reduced to text through eggsearch's own extraction path rather
//! than a bespoke stripper, then sanitized by the normal adapter pipeline.

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::request::EngineSearchRequest;

const ENGINE: &str = "wikipedia";
const DEFAULT_BASE_URL: &str = "https://en.wikipedia.org/w/api.php";
const ARTICLE_BASE_URL: &str = "https://en.wikipedia.org/wiki/";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_SEARCH_LIMIT: usize = 50;
const SNIPPET_MAX_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
struct WikipediaResponse {
    #[serde(default)]
    query: Option<WikipediaQuery>,
    #[serde(default)]
    error: Option<WikipediaError>,
}

#[derive(Debug, Deserialize)]
struct WikipediaQuery {
    #[serde(default)]
    search: Vec<WikipediaHit>,
}

#[derive(Debug, Deserialize)]
struct WikipediaError {
    #[serde(default)]
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WikipediaHit {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    snippet: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(url) if !url.trim().is_empty() => url.trim().to_string(),
        _ => DEFAULT_BASE_URL.to_string(),
    }
}

/// Request parameters for one `list=search` call.
///
/// `srprop` asks only for the fields eggsearch actually maps: a
/// highlight `snippet` and the page `timestamp` that becomes the card's
/// result timestamp.
fn search_params(query: &str, max_results: usize) -> Vec<(String, String)> {
    vec![
        ("action".to_string(), "query".to_string()),
        ("list".to_string(), "search".to_string()),
        ("srsearch".to_string(), query.to_string()),
        (
            "srlimit".to_string(),
            max_results.clamp(1, MAX_SEARCH_LIMIT).to_string(),
        ),
        ("srprop".to_string(), "snippet|timestamp".to_string()),
        ("format".to_string(), "json".to_string()),
        ("formatversion".to_string(), "2".to_string()),
    ]
}

/// Canonical article URL for a MediaWiki page title.
///
/// Subpage separators stay literal while each segment is percent-encoded;
/// spaces use the MediaWiki underscore form.
fn article_url(title: &str) -> Option<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return None;
    }
    let encoded: Vec<String> = trimmed
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| urlencoding::encode(segment).replace("%20", "_"))
        .collect();
    if encoded.is_empty() {
        return None;
    }
    Some(format!("{ARTICLE_BASE_URL}{}", encoded.join("/")))
}

/// Reduce an API-provided HTML snippet to bounded plain text.
fn snippet_text(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let (_, _, body, ..) =
        crate::fetch::extract::HtmlExtractor::new(raw.as_bytes(), ARTICLE_BASE_URL)
            .extract(SNIPPET_MAX_CHARS * 2, false);
    let normalized = crate::core::sanitize::normalize_whitespace(&body);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(crate::core::sanitize::truncate_at_word(
        trimmed,
        SNIPPET_MAX_CHARS,
    ))
}

fn convert(hits: Vec<WikipediaHit>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(hits.len()));
    for hit in hits {
        if out.len() >= max_results {
            break;
        }
        let Some(title) = hit.title.as_deref() else {
            continue;
        };
        let Some(url) = article_url(title) else {
            continue;
        };
        let title = crate::core::sanitize::normalize_whitespace(title)
            .trim()
            .to_string();
        if title.is_empty() {
            continue;
        }
        out.push(SearchResult {
            title,
            url,
            snippet: hit.snippet.as_deref().and_then(snippet_text),
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at: hit
                .timestamp
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
    let mut req = client.get(url.as_str()).map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;
    for (key, value) in search_params(&request.query, request.max_results) {
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
    let parsed: WikipediaResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    if let Some(error) = parsed.error {
        return Err(EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!(
                "MediaWiki API error: {}",
                error.code.unwrap_or_else(|| "unknown".to_string())
            ),
        });
    }
    let hits = parsed.query.map(|q| q.search).unwrap_or_default();
    Ok(convert(hits, request.max_results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn req(query: &str, max_results: usize) -> EngineSearchRequest {
        EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
    }

    fn parse(payload: &str) -> WikipediaResponse {
        serde_json::from_str(payload).expect("fixture parses")
    }

    const SEARCH_FIXTURE: &str = r#"{
      "batchcomplete": "",
      "query": {
        "searchinfo": { "totalhits": 2 },
        "search": [
          {
            "ns": 0,
            "title": "Rust (programming language)",
            "pageid": 29477,
            "size": 94000,
            "snippet": "<span class=\"searchmatch\">Rust</span> is a &quot;statically typed&quot; language",
            "timestamp": "2024-05-01T12:00:00Z"
          },
          {
            "ns": 0,
            "title": "Rust (disambiguation)",
            "pageid": 100,
            "snippet": "<span class=\"searchmatch\">Rust</span> may refer to:",
            "timestamp": "2023-11-02T09:30:00Z"
          }
        ]
      }
    }"#;

    #[test]
    fn search_params_use_documented_list_search_contract() {
        let params = search_params("rust async", 5);
        let get = |key: &str| {
            params
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
                .unwrap_or_default()
        };
        assert_eq!(get("action"), "query");
        assert_eq!(get("list"), "search");
        assert_eq!(get("srsearch"), "rust async");
        assert_eq!(get("srlimit"), "5");
        assert_eq!(get("format"), "json");
        assert_eq!(get("formatversion"), "2");
    }

    #[test]
    fn search_params_clamp_result_budget() {
        let limit = |max: usize| {
            search_params("q", max)
                .into_iter()
                .find(|(k, _)| k == "srlimit")
                .map(|(_, v)| v)
                .unwrap_or_default()
        };
        assert_eq!(limit(0), "1");
        assert_eq!(limit(MAX_SEARCH_LIMIT + 25), MAX_SEARCH_LIMIT.to_string());
    }

    #[test]
    fn resolve_url_prefers_override() {
        assert_eq!(resolve_url(None), DEFAULT_BASE_URL);
        assert_eq!(resolve_url(Some("  ")), DEFAULT_BASE_URL);
        assert_eq!(
            resolve_url(Some("http://127.0.0.1:1/w/api.php")),
            "http://127.0.0.1:1/w/api.php"
        );
    }

    #[test]
    fn article_url_encodes_and_uses_underscores() {
        assert_eq!(
            article_url("Rust (programming language)").as_deref(),
            Some("https://en.wikipedia.org/wiki/Rust_%28programming_language%29")
        );
        assert_eq!(
            article_url("Talk:Rust/History").as_deref(),
            Some("https://en.wikipedia.org/wiki/Talk%3ARust/History")
        );
        assert!(article_url("   ").is_none());
        assert!(article_url("/").is_none());
    }

    #[test]
    fn snippet_strips_api_markup_without_custom_stripper() {
        let text = snippet_text(
            "<span class=\"searchmatch\">Rust</span> is a &quot;statically typed&quot; language",
        )
        .expect("snippet");
        assert_eq!(text, "Rust is a \"statically typed\" language");
        assert!(!text.contains("span"));
    }

    #[test]
    fn snippet_is_bounded_and_rejects_empty() {
        let long = format!("<span>{}</span>", "word ".repeat(400));
        let text = snippet_text(&long).expect("snippet");
        assert!(text.chars().count() <= SNIPPET_MAX_CHARS);
        assert!(snippet_text("   ").is_none());
        assert!(snippet_text("<span></span>").is_none());
    }

    #[test]
    fn convert_maps_fixture_to_source_cards() {
        let parsed = parse(SEARCH_FIXTURE);
        let out = convert(parsed.query.expect("query").search, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Rust (programming language)");
        assert_eq!(
            out[0].url,
            "https://en.wikipedia.org/wiki/Rust_%28programming_language%29"
        );
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("Rust is a \"statically typed\" language")
        );
        assert_eq!(
            out[0].published_at.as_deref(),
            Some("2024-05-01T12:00:00+00:00")
        );
        assert_eq!(out[0].source_engine, "wikipedia");
        assert!(matches!(out[0].metadata, ResultMetadata::None));
    }

    #[test]
    fn convert_respects_result_budget() {
        let parsed = parse(SEARCH_FIXTURE);
        let out = convert(parsed.query.expect("query").search, 1);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn convert_skips_entries_without_usable_title() {
        let out = convert(
            vec![
                WikipediaHit {
                    title: Some("   ".to_string()),
                    snippet: Some("<span>x</span>".to_string()),
                    timestamp: None,
                },
                WikipediaHit {
                    title: None,
                    snippet: None,
                    timestamp: None,
                },
            ],
            10,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn empty_and_malformed_payloads_are_bounded() {
        let empty = parse(r#"{"batchcomplete":""}"#);
        assert!(empty.query.is_none());
        assert!(empty.error.is_none());
        assert!(convert(empty.query.map(|q| q.search).unwrap_or_default(), 10).is_empty());
        assert!(serde_json::from_str::<WikipediaResponse>("not json").is_err());
    }

    #[test]
    fn api_error_body_is_provider_scoped_failure() {
        let failed = parse(r#"{"error":{"code":"ratelimited","info":"slow down"}}"#);
        let code = failed.error.expect("error").code;
        assert_eq!(code.as_deref(), Some("ratelimited"));
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
    fn descriptor_claims_only_timestamps() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "wikipedia",
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
        assert!(!desc.capabilities.supports_freshness);
        assert!(!desc.capabilities.supports_domain_filters);
        assert!(!desc.capabilities.supports_language);
    }
}
