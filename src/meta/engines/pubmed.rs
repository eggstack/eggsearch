//! PubMed provider built on the NCBI E-utilities.
//!
//! Retrieval is two bounded steps: one `esearch` call for PMIDs followed
//! by a single `esummary` batch for citation metadata. Requests identify
//! themselves truthfully as eggsearch (`tool=eggsearch` plus a real
//! User-Agent) and may carry an operator-supplied contact identity and
//! NCBI API key; neither is required for keyless routing. If the summary
//! phase fails the whole call fails — metadata is never fabricated.

use std::collections::BTreeMap;
use std::time::Duration;

use eggfetch_core::Client;
use serde::Deserialize;

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::request::EngineSearchRequest;

const ENGINE: &str = "pubmed";
const DEFAULT_BASE_URL: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils";
const ARTICLE_BASE_URL: &str = "https://pubmed.ncbi.nlm.nih.gov/";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_RETMAX: usize = 25;
const MAX_AUTHORS_IN_SNIPPET: usize = 3;
const SNIPPET_MAX_CHARS: usize = 500;
const TOOL_NAME: &str = "eggsearch";
const USER_AGENT: &str = concat!(
    "eggsearch/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/eggstack/eggsearch)"
);

#[derive(Debug, Deserialize)]
struct ESearchResponse {
    #[serde(default)]
    esearchresult: Option<ESearchResult>,
}

#[derive(Debug, Deserialize)]
struct ESearchResult {
    #[serde(default)]
    idlist: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ESummaryResponse {
    #[serde(default)]
    result: Option<BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct ESummaryAuthor {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ESummaryItem {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    volume: Option<String>,
    #[serde(default)]
    issue: Option<String>,
    #[serde(default)]
    pages: Option<String>,
    #[serde(default)]
    pubdate: Option<String>,
    #[serde(default)]
    epubdate: Option<String>,
    #[serde(default)]
    authors: Vec<ESummaryAuthor>,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(url) if !url.trim().is_empty() => url.trim().trim_end_matches('/').to_string(),
        _ => DEFAULT_BASE_URL.to_string(),
    }
}

fn nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// Identity parameters shared by both E-utilities calls.
///
/// `api_key` and `email` are optional; their absence never blocks
/// routing, it only forgoes the higher rate limit and the documented
/// contact identity.
fn identity_params(api_key: Option<&str>, email: Option<&str>, out: &mut Vec<(String, String)>) {
    out.push(("tool".to_string(), TOOL_NAME.to_string()));
    if let Some(key) = api_key.map(str::trim).filter(|k| !k.is_empty()) {
        out.push(("api_key".to_string(), key.to_string()));
    }
    if let Some(contact) = email.map(str::trim).filter(|e| !e.is_empty()) {
        out.push(("email".to_string(), contact.to_string()));
    }
}

fn esearch_params(
    query: &str,
    max_results: usize,
    api_key: Option<&str>,
    email: Option<&str>,
) -> Vec<(String, String)> {
    let mut params = vec![
        ("db".to_string(), "pubmed".to_string()),
        ("term".to_string(), query.to_string()),
        ("retmode".to_string(), "json".to_string()),
        (
            "retmax".to_string(),
            max_results.clamp(1, MAX_RETMAX).to_string(),
        ),
    ];
    identity_params(api_key, email, &mut params);
    params
}

fn esummary_params(
    ids: &[String],
    api_key: Option<&str>,
    email: Option<&str>,
) -> Vec<(String, String)> {
    let mut params = vec![
        ("db".to_string(), "pubmed".to_string()),
        ("id".to_string(), ids.join(",")),
        ("retmode".to_string(), "json".to_string()),
    ];
    identity_params(api_key, email, &mut params);
    params
}

async fn get_json<T: for<'de> Deserialize<'de>>(
    client: &Client,
    url: &str,
    params: &[(String, String)],
    timeout: Duration,
) -> Result<T, EngineError> {
    let mut req = client.get(url).map_err(|e| EngineError::Http {
        engine: ENGINE,
        source: e,
    })?;
    for (key, value) in params {
        req = req.query(key.as_str(), value.as_str());
    }
    let resp = req
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .timeout(super::engine_timeout(timeout))
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
    serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
        engine: ENGINE,
        reason: format!("invalid JSON: {e}"),
    })
}

/// Convert an E-utilities date (`2024 Feb 3`, `2024 Feb`, `2024`) into an
/// RFC 3339 timestamp. Unrecognized shapes yield `None` rather than a
/// guess.
fn parse_pubmed_date(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.split_whitespace();
    let year: i32 = parts.next()?.parse().ok()?;
    if !(1900..=2999).contains(&year) {
        return None;
    }
    let month_token = parts
        .next()
        .map(|m| m.trim_end_matches(|c: char| !c.is_alphabetic()));
    let month = match month_token {
        None => 1,
        Some(token) => {
            let needle = token.get(..3)?.to_ascii_lowercase();
            match needle.as_str() {
                "jan" => 1,
                "feb" => 2,
                "mar" => 3,
                "apr" => 4,
                "may" => 5,
                "jun" => 6,
                "jul" => 7,
                "aug" => 8,
                "sep" => 9,
                "oct" => 10,
                "nov" => 11,
                "dec" => 12,
                _ => return None,
            }
        }
    };
    let day: u32 = match parts.next() {
        Some(token) => {
            let digits: String = token.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().ok()?
        }
        None => 1,
    };
    let iso = format!("{year:04}-{month:02}-{day:02}");
    crate::core::source_card::parse_result_timestamp(&iso)
}

fn author_label(item: &ESummaryItem) -> String {
    let names: Vec<String> = item
        .authors
        .iter()
        .filter_map(|a| nonempty(a.name.as_deref()))
        .collect();
    match names.len() {
        0 => String::new(),
        n if n <= MAX_AUTHORS_IN_SNIPPET => names.join(", "),
        _ => format!("{} et al.", names[..MAX_AUTHORS_IN_SNIPPET].join(", ")),
    }
}

fn venue_label(item: &ESummaryItem) -> String {
    let mut label = nonempty(item.source.as_deref()).unwrap_or_default();
    let volume = nonempty(item.volume.as_deref());
    let issue = nonempty(item.issue.as_deref());
    match (volume, issue) {
        (Some(volume), Some(issue)) => label.push_str(&format!(" {volume}({issue})")),
        (Some(volume), None) => label.push_str(&format!(" {volume}")),
        (None, Some(issue)) => label.push_str(&format!(" ({issue})")),
        (None, None) => {}
    }
    if let Some(pages) = nonempty(item.pages.as_deref()) {
        label.push_str(&format!(": {pages}"));
    }
    label.trim().to_string()
}

/// Bounded citation line built from summary metadata; PubMed's ESummary
/// does not return abstracts.
fn citation_snippet(item: &ESummaryItem, uid: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let authors = author_label(item);
    if !authors.is_empty() {
        parts.push(authors);
    }
    let venue = venue_label(item);
    if !venue.is_empty() {
        parts.push(venue);
    }
    let date = nonempty(item.pubdate.as_deref())
        .or_else(|| nonempty(item.epubdate.as_deref()))
        .unwrap_or_default();
    if !date.is_empty() {
        parts.push(date);
    }
    parts.push(format!("PMID {uid}"));
    let joined = parts.join(". ");
    Some(crate::core::sanitize::truncate_at_word(
        &joined,
        SNIPPET_MAX_CHARS,
    ))
}

fn convert(
    ids: &[String],
    items: &BTreeMap<String, serde_json::Value>,
    max_results: usize,
) -> Result<Vec<SearchResult>, EngineError> {
    let mut out = Vec::with_capacity(ids.len().min(max_results));
    for uid in ids.iter().take(max_results) {
        let Some(raw) = items.get(uid) else {
            continue;
        };
        let Ok(item) = serde_json::from_value::<ESummaryItem>(raw.clone()) else {
            continue;
        };
        let title = item
            .title
            .as_deref()
            .map(crate::core::sanitize::normalize_whitespace)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let Some(title) = title else {
            continue;
        };
        let url = format!("{ARTICLE_BASE_URL}{}/", urlencoding::encode(uid));
        out.push(SearchResult {
            title,
            url,
            snippet: citation_snippet(&item, uid),
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at: item
                .epubdate
                .as_deref()
                .and_then(parse_pubmed_date)
                .or_else(|| item.pubdate.as_deref().and_then(parse_pubmed_date)),
            metadata: ResultMetadata::None,
        });
    }
    Ok(out)
}

pub async fn search(
    client: &Client,
    base_url: Option<&str>,
    api_key: Option<&str>,
    email: Option<&str>,
    request: &EngineSearchRequest,
) -> Result<Vec<SearchResult>, EngineError> {
    if request.max_results == 0 {
        return Ok(Vec::new());
    }
    let base = resolve_url(base_url);
    let esearch_url = format!("{base}/esearch.fcgi");
    let esummary_url = format!("{base}/esummary.fcgi");

    let found: ESearchResponse = get_json(
        client,
        &esearch_url,
        &esearch_params(&request.query, request.max_results, api_key, email),
        request.timeout,
    )
    .await?;
    let ids = found.esearchresult.map(|r| r.idlist).unwrap_or_default();
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let summary: ESummaryResponse = get_json(
        client,
        &esummary_url,
        &esummary_params(&ids, api_key, email),
        request.timeout,
    )
    .await?;
    let items = summary.result.ok_or_else(|| EngineError::ParseFailed {
        engine: ENGINE,
        reason: "esummary response carried no result section".to_string(),
    })?;
    convert(&ids, &items, request.max_results)
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

    const ESEARCH_FIXTURE: &str = r#"{
      "esearchresult": {
        "count": "2",
        "idlist": ["31357002", "31234567"],
        "retmax": "2"
      }
    }"#;

    const ESUMMARY_FIXTURE: &str = r#"{
      "header": { "type": "esummary" },
      "result": {
        "uids": ["31357002", "31234567"],
        "31357002": {
          "uid": "31357002",
          "pubdate": "2020 Feb 5",
          "epubdate": "2020 Feb 3",
          "source": "Journal of Testing",
          "authors": [
            { "name": "Smith J" },
            { "name": "Doe A" },
            { "name": "Roe B" },
            { "name": "Poe C" }
          ],
          "title": "A bounded retrieval study.",
          "volume": "12",
          "issue": "3",
          "pages": "100-110",
          "lang": ["eng"],
          "pubtype": ["journal-article"]
        },
        "31234567": {
          "uid": "31234567",
          "pubdate": "2019",
          "source": "Other Journal",
          "authors": [],
          "title": "Year only record."
        }
      }
    }"#;

    #[test]
    fn esearch_params_use_documented_contract_and_bound_retmax() {
        let params = esearch_params("crispr", 100, None, None);
        assert_eq!(param(&params, "db"), Some("pubmed"));
        assert_eq!(param(&params, "term"), Some("crispr"));
        assert_eq!(param(&params, "retmode"), Some("json"));
        assert_eq!(param(&params, "retmax"), Some("25"));
        assert_eq!(param(&params, "retmax"), Some("25"));
        let params = esearch_params("x", 0, None, None);
        assert_eq!(param(&params, "retmax"), Some("1"));
    }

    #[test]
    fn identity_is_truthful_and_never_fabricated() {
        let keyless = esearch_params("x", 5, None, None);
        assert_eq!(param(&keyless, "tool"), Some("eggsearch"));
        assert_eq!(param(&keyless, "api_key"), None);
        assert_eq!(param(&keyless, "email"), None);
        assert!(!USER_AGENT.contains("example.com"));
        assert!(USER_AGENT.starts_with("eggsearch/"));
    }

    #[test]
    fn optional_credential_and_contact_are_opt_in() {
        let keyed = esearch_params("x", 5, Some("  key  "), Some(" ops@example.org "));
        assert_eq!(param(&keyed, "api_key"), Some("key"));
        assert_eq!(param(&keyed, "email"), Some("ops@example.org"));
        let blank = esearch_params("x", 5, Some(""), Some("   "));
        assert_eq!(param(&blank, "api_key"), None);
        assert_eq!(param(&blank, "email"), None);
    }

    #[test]
    fn esummary_batches_ids_into_one_call() {
        let ids = vec!["1".to_string(), "2".to_string()];
        let params = esummary_params(&ids, None, None);
        assert_eq!(param(&params, "id"), Some("1,2"));
        assert_eq!(param(&params, "db"), Some("pubmed"));
        assert_eq!(param(&params, "tool"), Some("eggsearch"));
    }

    #[test]
    fn pubmed_dates_convert_to_rfc3339() {
        assert_eq!(
            parse_pubmed_date("2020 Feb 3").as_deref(),
            Some("2020-02-03T00:00:00+00:00")
        );
        assert_eq!(
            parse_pubmed_date("2020 Feb").as_deref(),
            Some("2020-02-01T00:00:00+00:00")
        );
        assert_eq!(
            parse_pubmed_date("2020").as_deref(),
            Some("2020-01-01T00:00:00+00:00")
        );
        assert_eq!(
            parse_pubmed_date("2020 Feb 3-10").as_deref(),
            Some("2020-02-03T00:00:00+00:00")
        );
    }

    #[test]
    fn pubmed_dates_reject_unrecognized_shapes() {
        assert!(parse_pubmed_date("").is_none());
        assert!(parse_pubmed_date("in press").is_none());
        assert!(parse_pubmed_date("Feb 2020").is_none());
        assert!(parse_pubmed_date("2020 Foo 3").is_none());
        assert!(parse_pubmed_date("1200").is_none());
    }

    #[test]
    fn convert_maps_summary_metadata() {
        let found: ESearchResponse = serde_json::from_str(ESEARCH_FIXTURE).expect("esearch");
        let summary: ESummaryResponse = serde_json::from_str(ESUMMARY_FIXTURE).expect("esummary");
        let ids = found.esearchresult.expect("result").idlist;
        let items = summary.result.expect("result");
        let out = convert(&ids, &items, 10).expect("convert");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "A bounded retrieval study.");
        assert_eq!(out[0].url, "https://pubmed.ncbi.nlm.nih.gov/31357002/");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("Smith J, Doe A, Roe B et al.. Journal of Testing 12(3): 100-110. 2020 Feb 5. PMID 31357002")
        );
        assert_eq!(
            out[0].published_at.as_deref(),
            Some("2020-02-03T00:00:00+00:00"),
            "epubdate wins over print pubdate"
        );
        assert_eq!(out[1].url, "https://pubmed.ncbi.nlm.nih.gov/31234567/");
        assert_eq!(
            out[1].published_at.as_deref(),
            Some("2019-01-01T00:00:00+00:00")
        );
        assert_eq!(
            out[1].snippet.as_deref(),
            Some("Other Journal. 2019. PMID 31234567")
        );
        assert_eq!(out[0].source_engine, "pubmed");
    }

    #[test]
    fn convert_skips_unknown_ids_and_respects_budget() {
        let summary: ESummaryResponse = serde_json::from_str(ESUMMARY_FIXTURE).expect("esummary");
        let items = summary.result.expect("result");
        let ids = vec!["31357002".to_string(), "99999999".to_string()];
        let out = convert(&ids, &items, 10).expect("convert");
        assert_eq!(out.len(), 1);
        let ids = vec!["31357002".to_string(), "31234567".to_string()];
        assert_eq!(convert(&ids, &items, 1).expect("convert").len(), 1);
    }

    #[test]
    fn empty_search_is_not_an_error() {
        let empty: ESearchResponse =
            serde_json::from_str(r#"{"esearchresult":{"count":"0"}}"#).expect("esearch");
        assert!(empty
            .esearchresult
            .map(|r| r.idlist.is_empty())
            .unwrap_or(true));
        let blank: ESearchResponse = serde_json::from_str("{}").expect("esearch");
        assert!(blank.esearchresult.is_none());
        assert!(
            serde_json::from_str::<ESearchResponse>(r#"{"esearchresult": {"idlist": 5}}"#).is_err()
        );
        assert!(serde_json::from_str::<ESearchResponse>("{").is_err());
    }

    #[test]
    fn missing_summary_section_is_provider_scoped_error() {
        let no_result: ESummaryResponse =
            serde_json::from_str(r#"{"header":{}}"#).expect("summary");
        assert!(no_result.result.is_none());
    }

    #[tokio::test]
    async fn zero_budget_short_circuits_without_request() {
        let client = Client::new();
        assert!(search(&client, None, None, None, &req("rust", 0))
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn descriptor_is_keyless_scholarly() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "pubmed", true, false, true, true, None, None,
        )
        .expect("descriptor");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::JsonApi);
        assert!(!desc.requires_api_key);
        assert!(desc.capabilities.supports_scholarly_search);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_doi_lookup);
    }
}
