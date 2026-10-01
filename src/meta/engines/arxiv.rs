//! arXiv provider built on the public arXiv metadata API.
//!
//! The upstream contract is an Atom 1.0 feed, parsed with the repository's
//! existing `quick-xml` reader rather than regex matching or a second XML
//! stack. arXiv's API terms ask for at most one request every three
//! seconds on a single connection, so every call passes through one
//! process-wide [`RequestGate`] that both spaces and serializes requests:
//! concurrent calls queue behind the gate instead of bypassing it.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use eggfetch_core::Client;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use tokio::sync::{Mutex, MutexGuard};

use super::error::EngineError;
use super::models::{ResultMetadata, SearchResult};
use super::request::EngineSearchRequest;

const ENGINE: &str = "arxiv";
const DEFAULT_BASE_URL: &str = "https://export.arxiv.org/api/query";
const ARTICLE_BASE_URL: &str = "https://arxiv.org/abs/";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_QUERY_RESULTS: usize = 50;
const SNIPPET_MAX_CHARS: usize = 500;

/// Minimum spacing between two arXiv API requests, as required by the
/// arXiv API terms of use.
const MIN_REQUEST_INTERVAL: Duration = Duration::from_secs(3);

/// Process-wide pacing gate shared by every arXiv engine instance.
pub fn shared_gate() -> Arc<RequestGate> {
    static GATE: OnceLock<Arc<RequestGate>> = OnceLock::new();
    Arc::clone(GATE.get_or_init(|| Arc::new(RequestGate::new(MIN_REQUEST_INTERVAL))))
}

/// Serializes arXiv requests and enforces the minimum interval between
/// them.
///
/// Holding the returned [`GateTurn`] keeps the lock, so only one arXiv
/// request is ever in flight; acquiring also sleeps until at least
/// `interval` has passed since the previous request started.
pub struct RequestGate {
    interval: Duration,
    last_started: Mutex<Option<Instant>>,
}

/// Exclusive access to the shared arXiv request channel.
pub struct GateTurn<'a> {
    started: Instant,
    _guard: MutexGuard<'a, Option<Instant>>,
}

impl GateTurn<'_> {
    /// Instant at which this request was admitted.
    pub fn started(&self) -> Instant {
        self.started
    }
}

impl RequestGate {
    /// Create a gate with an explicit minimum interval.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_started: Mutex::new(None),
        }
    }

    /// Minimum spacing enforced between requests.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Wait for the next arXiv turn.
    pub async fn wait_turn(&self) -> GateTurn<'_> {
        let mut guard = self.last_started.lock().await;
        if let Some(previous) = *guard {
            if let Some(remaining) = self.interval.checked_sub(previous.elapsed()) {
                tokio::time::sleep(remaining).await;
            }
        }
        let started = Instant::now();
        *guard = Some(started);
        GateTurn {
            started,
            _guard: guard,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct ArxivEntry {
    id: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    published: Option<String>,
    updated: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Id,
    Title,
    Summary,
    Published,
    Updated,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(url) if !url.trim().is_empty() => url.trim().to_string(),
        _ => DEFAULT_BASE_URL.to_string(),
    }
}

fn search_params(query: &str, max_results: usize) -> Vec<(String, String)> {
    vec![
        ("search_query".to_string(), format!("all:{query}")),
        ("start".to_string(), "0".to_string()),
        (
            "max_results".to_string(),
            max_results.clamp(1, MAX_QUERY_RESULTS).to_string(),
        ),
    ]
}

fn local_name(name: &quick_xml::name::QName<'_>) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).into_owned()
}

fn field_for(name: &str) -> Option<Field> {
    match name {
        "id" => Some(Field::Id),
        "title" => Some(Field::Title),
        "summary" => Some(Field::Summary),
        "published" => Some(Field::Published),
        "updated" => Some(Field::Updated),
        _ => None,
    }
}

/// Parse an arXiv Atom feed into raw entries.
///
/// Returns a parse error for malformed or truncated documents so the
/// failure stays provider-scoped instead of degrading into empty results.
fn parse_feed(xml: &str) -> Result<Vec<ArxivEntry>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_end_names = true;
    let mut buf = Vec::new();
    let mut entries: Vec<ArxivEntry> = Vec::new();
    let mut current: Option<ArxivEntry> = None;
    let mut field: Option<Field> = None;
    let mut text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(element)) => match local_name(&element.name()).as_str() {
                "entry" => {
                    if current.is_some() {
                        return Err("nested <entry> element".to_string());
                    }
                    current = Some(ArxivEntry::default());
                    field = None;
                }
                other => {
                    field = current.as_ref().and_then(|_| field_for(other));
                    text.clear();
                }
            },
            Ok(Event::Text(value)) => {
                if field.is_some() {
                    if let Ok(decoded) = value.decode() {
                        text.push_str(&decoded);
                    }
                }
            }
            Ok(Event::CData(value)) => {
                if field.is_some() {
                    text.push_str(&String::from_utf8_lossy(value.as_ref()));
                }
            }
            Ok(Event::End(element)) => match local_name(&element.name()).as_str() {
                "entry" => match current.take() {
                    Some(entry) => entries.push(entry),
                    None => return Err("unbalanced </entry> element".to_string()),
                },
                _ => {
                    if let (Some(entry), Some(active)) = (current.as_mut(), field.take()) {
                        let value = crate::core::sanitize::normalize_whitespace(text.trim())
                            .trim()
                            .to_string();
                        if !value.is_empty() {
                            match active {
                                Field::Id => entry.id.get_or_insert(value),
                                Field::Title => entry.title.get_or_insert(value),
                                Field::Summary => entry.summary.get_or_insert(value),
                                Field::Published => entry.published.get_or_insert(value),
                                Field::Updated => entry.updated.get_or_insert(value),
                            };
                        }
                        text.clear();
                    }
                }
            },
            Ok(_) => {}
            Err(e) => return Err(format!("invalid Atom document: {e}")),
        }
        buf.clear();
    }

    if current.is_some() {
        return Err("truncated Atom document".to_string());
    }
    Ok(entries)
}

/// Canonical abstract-page URL for an arXiv identifier.
///
/// Version suffixes are stripped so every revision of one paper resolves
/// to the same stable identity.
/// Canonical abstract-page URL for an arXiv identifier.
///
/// Version suffixes are stripped so every revision of one paper resolves
/// to the same stable identity; pre-2007 archive-qualified identifiers
/// (`cs/9901001`) keep their archive segment.
fn article_url(id: &str) -> Option<String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tail = match trimmed.split_once("/abs/") {
        Some((_, rest)) => rest.trim_matches('/'),
        None => trimmed,
    };
    if tail.is_empty() {
        return None;
    }
    let (archive, last) = match tail.rsplit_once('/') {
        Some((archive, last)) => (Some(archive), last),
        None => (None, tail),
    };
    let unversioned = match last.split_once('v') {
        Some((base, version))
            if !base.is_empty()
                && !version.is_empty()
                && version.chars().all(|c| c.is_ascii_digit()) =>
        {
            base
        }
        _ => last,
    };
    if unversioned.is_empty() {
        return None;
    }
    let mut segments: Vec<String> = Vec::new();
    if let Some(archive) = archive.filter(|a| !a.is_empty()) {
        segments.push(urlencoding::encode(archive).into_owned());
    }
    segments.push(urlencoding::encode(unversioned).into_owned());
    Some(format!("{ARTICLE_BASE_URL}{}", segments.join("/")))
}

fn convert(entries: Vec<ArxivEntry>, max_results: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(entries.len()));
    for entry in entries {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = entry.id.as_deref().and_then(article_url) else {
            continue;
        };
        let title = entry
            .title
            .as_deref()
            .map(crate::core::sanitize::normalize_whitespace)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let Some(title) = title else {
            continue;
        };
        let snippet = entry
            .summary
            .as_deref()
            .map(crate::core::sanitize::normalize_whitespace)
            .map(|s| crate::core::sanitize::truncate_at_word(s.trim(), SNIPPET_MAX_CHARS))
            .filter(|s| !s.is_empty());
        let published_at = entry
            .published
            .as_deref()
            .or(entry.updated.as_deref())
            .and_then(crate::core::source_card::parse_result_timestamp);
        out.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            excerpts: Vec::new(),
            published_at,
            metadata: ResultMetadata::None,
        });
    }
    out
}

pub async fn search(
    client: &Client,
    base_url: Option<&str>,
    gate: &RequestGate,
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
    let _turn = gate.wait_turn().await;
    let resp = req
        .header("Accept", "application/atom+xml")
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
    let feed = String::from_utf8(bytes)
        .map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("Atom payload is not valid UTF-8: {e}"),
        })?
        .to_string();
    let entries = parse_feed(&feed).map_err(|reason| EngineError::ParseFailed {
        engine: ENGINE,
        reason,
    })?;
    Ok(convert(entries, request.max_results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <link href="http://arxiv.org/api/query" rel="self" type="application/atom+xml"/>
  <title type="html">ArXiv Query</title>
  <entry>
    <id>http://arxiv.org/abs/2401.00001v1</id>
    <updated>2024-01-03T00:00:00Z</updated>
    <published>2024-01-02T00:00:00Z</published>
    <title>Bounded
      Retrieval</title>
    <summary>  We study bounded retrieval.  </summary>
    <author><name>A. Researcher</name></author>
    <category term="cs.IR" scheme="http://arxiv.org/schemas/atom"/>
  </entry>
  <entry>
    <id>http://arxiv.org/abs/2401.00002v3</id>
    <updated>2024-02-02T00:00:00Z</updated>
    <title>Second Paper</title>
    <summary>Another abstract.</summary>
  </entry>
  <entry>
    <id>http://arxiv.org/abs/2401.00003</id>
    <updated>2024-03-03T00:00:00Z</updated>
    <title>Unversioned Identifier</title>
  </entry>
</feed>
"#;

    fn req(query: &str, max_results: usize) -> EngineSearchRequest {
        EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
    }

    fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn search_params_use_documented_query_contract() {
        let params = search_params("sparse retrieval", 7);
        assert_eq!(param(&params, "search_query"), Some("all:sparse retrieval"));
        assert_eq!(param(&params, "start"), Some("0"));
        assert_eq!(param(&params, "max_results"), Some("7"));
        assert_eq!(param(&params, "search_query"), Some("all:sparse retrieval"));
        assert_eq!(param(&params, "max_results"), Some("7"));
        assert_eq!(param(&params, "start"), Some("0"));
    }

    #[test]
    fn search_params_clamp_result_budget() {
        assert_eq!(param(&search_params("q", 0), "max_results"), Some("1"));
        assert_eq!(param(&search_params("q", 5000), "max_results"), Some("50"));
    }

    #[test]
    fn parse_feed_reads_atom_entries() {
        let entries = parse_feed(FEED).expect("feed parses");
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0].title.as_deref(),
            Some("Bounded Retrieval"),
            "Atom titles must collapse embedded newlines"
        );
        assert_eq!(
            entries[0].summary.as_deref(),
            Some("We study bounded retrieval.")
        );
        assert_eq!(
            entries[0].published.as_deref(),
            Some("2024-01-02T00:00:00Z")
        );
        assert_eq!(entries[0].updated.as_deref(), Some("2024-01-03T00:00:00Z"));
    }

    #[test]
    fn parse_feed_ignores_feed_level_title() {
        let entries = parse_feed(FEED).expect("feed parses");
        assert!(
            entries
                .iter()
                .all(|e| e.title.as_deref() != Some("ArXiv Query")),
            "feed-level metadata must not leak into entries"
        );
    }

    #[test]
    fn parse_feed_rejects_malformed_and_truncated_documents() {
        assert!(parse_feed("<feed><entry><title>x</title>").is_err());
        assert!(parse_feed(
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><title>a</title></feed>"#
        )
        .is_err());
        assert!(
            parse_feed(
                r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><id>a</id></wrong></entry></feed>"#
            )
            .is_err(),
            "end-name mismatches must fail closed instead of returning partial entries"
        );
        assert!(parse_feed("<feed><entry><id>a</id></entry>").is_ok());
    }

    #[test]
    fn parse_feed_accepts_cdata_summaries() {
        let feed = r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry>
          <id>http://arxiv.org/abs/2401.9v1</id><title>T</title>
          <summary><![CDATA[cdata abstract]]></summary></entry></feed>"#;
        let entries = parse_feed(feed).expect("cdata parses");
        assert_eq!(entries[0].summary.as_deref(), Some("cdata abstract"));
    }

    #[test]
    fn empty_feed_yields_no_entries() {
        let entries = parse_feed(r#"<feed xmlns="http://www.w3.org/2005/Atom"></feed>"#)
            .expect("empty feed parses");
        assert!(entries.is_empty());
    }

    #[test]
    fn article_url_strips_version_suffix() {
        assert_eq!(
            article_url("http://arxiv.org/abs/2401.00001v1").as_deref(),
            Some("https://arxiv.org/abs/2401.00001")
        );
        assert_eq!(
            article_url("2401.00002v12").as_deref(),
            Some("https://arxiv.org/abs/2401.00002")
        );
        assert_eq!(
            article_url("https://arxiv.org/abs/cs/9901001v1").as_deref(),
            Some("https://arxiv.org/abs/cs/9901001")
        );
        assert_eq!(
            article_url("2401.00003").as_deref(),
            Some("https://arxiv.org/abs/2401.00003")
        );
        assert_eq!(
            article_url("2401.00004vbeta").as_deref(),
            Some("https://arxiv.org/abs/2401.00004vbeta")
        );
        assert!(article_url("  ").is_none());
    }

    #[test]
    fn convert_maps_entries_to_source_cards() {
        let out = convert(parse_feed(FEED).expect("feed parses"), 10);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].title, "Bounded Retrieval");
        assert_eq!(out[0].url, "https://arxiv.org/abs/2401.00001");
        assert_eq!(
            out[0].snippet.as_deref(),
            Some("We study bounded retrieval.")
        );
        assert_eq!(
            out[0].published_at.as_deref(),
            Some("2024-01-02T00:00:00+00:00")
        );
        assert_eq!(out[0].source_engine, "arxiv");
        assert_eq!(
            out[2].published_at.as_deref(),
            Some("2024-03-03T00:00:00+00:00"),
            "updated is the fallback when published is absent"
        );
    }

    #[test]
    fn convert_bounds_snippets_and_result_budget() {
        let long = "word ".repeat(400);
        let entries = vec![ArxivEntry {
            id: Some("http://arxiv.org/abs/1v1".to_string()),
            title: Some("Long".to_string()),
            summary: Some(long),
            published: None,
            updated: None,
        }];
        let out = convert(entries.clone(), 10);
        assert!(out[0].snippet.as_deref().unwrap().chars().count() <= SNIPPET_MAX_CHARS);
        assert_eq!(convert(entries, 0).len(), 0);
    }

    #[test]
    fn convert_skips_entries_without_identity_or_title() {
        let out = convert(
            vec![
                ArxivEntry {
                    id: None,
                    title: Some("orphan".to_string()),
                    ..ArxivEntry::default()
                },
                ArxivEntry {
                    id: Some("http://arxiv.org/abs/2v1".to_string()),
                    title: None,
                    ..ArxivEntry::default()
                },
            ],
            10,
        );
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn gate_serializes_and_spaces_concurrent_turns() {
        let interval = Duration::from_millis(40);
        let gate = Arc::new(RequestGate::new(interval));
        assert_eq!(gate.interval(), interval);
        let in_flight = Arc::new(AtomicUsize::new(0));
        let overlaps = Arc::new(AtomicUsize::new(0));
        let granted: Arc<Mutex<Vec<Instant>>> = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..4 {
            let gate = Arc::clone(&gate);
            let in_flight = Arc::clone(&in_flight);
            let overlaps = Arc::clone(&overlaps);
            let granted = Arc::clone(&granted);
            tasks.spawn(async move {
                let turn = gate.wait_turn().await;
                let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                if current > 1 {
                    overlaps.fetch_add(1, Ordering::SeqCst);
                }
                granted.lock().await.push(turn.started());
                tokio::time::sleep(Duration::from_millis(15)).await;
                in_flight.fetch_sub(1, Ordering::SeqCst);
                drop(turn);
            });
        }
        while tasks.join_next().await.is_some() {}

        assert_eq!(
            overlaps.load(Ordering::SeqCst),
            0,
            "concurrent arXiv calls must never overlap on the wire"
        );
        let mut starts = granted.lock().await.clone();
        starts.sort_unstable();
        assert_eq!(starts.len(), 4);
        for pair in starts.windows(2) {
            assert!(
                pair[1].duration_since(pair[0]) >= interval,
                "arXiv terms require at least {interval:?} between requests"
            );
        }
    }

    #[tokio::test]
    async fn shared_gate_is_process_wide() {
        let first = Arc::as_ptr(&shared_gate());
        let second = Arc::as_ptr(&shared_gate());
        assert_eq!(first, second);
        assert_eq!(shared_gate().interval(), MIN_REQUEST_INTERVAL);
    }

    #[tokio::test]
    async fn zero_budget_short_circuits_without_request() {
        let client = Client::new();
        let gate = RequestGate::new(Duration::from_millis(1));
        assert!(search(&client, None, &gate, &req("rust", 0))
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn descriptor_uses_structured_api_kind() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "arxiv", true, false, true, true, None, None,
        )
        .expect("descriptor");
        assert_eq!(
            desc.kind,
            crate::core::provider::ProviderKind::StructuredApi
        );
        assert!(!desc.requires_api_key);
        assert!(desc.capabilities.supports_scholarly_search);
        assert!(desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_doi_lookup);
    }
}
