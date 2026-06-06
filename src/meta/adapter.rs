//! `MetadataSearchAdapter`: the metasearch-first boundary.
//! Callers receive eggsearch-owned types; engine types do not leak past
//! this module.

use std::sync::Arc;
use std::time::Duration;

use crate::core::SearchWarning;
use crate::core::SourceCard;
use crate::core::TrustLevel;
use crate::core::WebSearchRequest;
use tracing::{debug, warn};

use crate::meta::engines::error::EngineError;
use crate::meta::engines::models::{AggregatedResult, SearchResult};
use crate::meta::engines::{build_http_client, SearchEngine};
use crate::meta::response::{ProviderFailure, ProviderStatus, WebSearchResponse};

/// Coarse error class for provider failures. Exposed via `provider_status`
/// and the `web_search` tool's `providers_failed` field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorClass {
    /// The engine did not respond within the per-engine timeout.
    Timeout,
    /// The engine responded with a non-2xx HTTP status.
    HttpStatus,
    /// The engine responded but the HTML could not be parsed.
    ParseError,
    /// A network-level error (DNS, TLS, connection reset, etc.).
    NetworkError,
    /// The engine returned HTTP 429 (rate-limited).
    RateLimited,
    /// Unclassified failure.
    Unknown,
}

impl ErrorClass {
    /// Stable snake-case string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::HttpStatus => "http_status",
            Self::ParseError => "parse_error",
            Self::NetworkError => "network_error",
            Self::RateLimited => "rate_limited",
            Self::Unknown => "unknown",
        }
    }
}

fn classify(err: &EngineError) -> ErrorClass {
    use EngineError::*;
    match err {
        Timeout { .. } => ErrorClass::Timeout,
        BadStatus { status, .. } if *status == 429 => ErrorClass::RateLimited,
        BadStatus { .. } => ErrorClass::HttpStatus,
        ParseFailed { .. } => ErrorClass::ParseError,
        Http { .. } | NetworkError { .. } => ErrorClass::NetworkError,
    }
}

type EngineList = Vec<Arc<dyn SearchEngine>>;

/// Constructed once at server startup. Holds the `SearchEngine`
/// instances and the effective provider list.
pub struct MetadataSearchAdapter {
    engines: EngineList,
    provider_ids: Vec<String>,
    /// Hard timeout for the whole `web_search` call, including fan-out.
    global_timeout: Duration,
}

impl std::fmt::Debug for MetadataSearchAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetadataSearchAdapter")
            .field("providers", &self.provider_ids)
            .field("global_timeout_ms", &self.global_timeout.as_millis())
            .finish()
    }
}

impl MetadataSearchAdapter {
    /// Build an adapter for the given enabled provider ids.
    pub fn new(enabled_providers: Vec<String>, global_timeout: Duration) -> anyhow::Result<Self> {
        let (engines, skipped) = build_default_engines(&enabled_providers)?;
        if !skipped.is_empty() {
            warn!(?skipped, "skipped unknown provider ids in config");
        }
        if engines.is_empty() {
            return Err(anyhow::anyhow!(
                "no engines could be built; check the [search].providers config"
            ));
        }
        let provider_ids = engines.iter().map(|e| e.name().to_string()).collect();
        Ok(Self {
            engines,
            provider_ids,
            global_timeout,
        })
    }

    /// Build an adapter from an explicit list of `SearchEngine` trait
    /// objects. Used by tests to inject mock engines.
    pub fn from_engines(
        engines: Vec<Arc<dyn SearchEngine>>,
        global_timeout: Duration,
    ) -> Self {
        let provider_ids = engines.iter().map(|e| e.name().to_string()).collect();
        Self {
            engines,
            provider_ids,
            global_timeout,
        }
    }

    /// Subset of `engines` whose `name()` matches one of the given
    /// provider ids. Unknown ids are returned in the second tuple slot
    /// so callers can return a structured error.
    pub fn select_engines(
        &self,
        provider_ids: &[String],
    ) -> (Vec<Arc<dyn SearchEngine>>, Vec<String>) {
        if provider_ids.is_empty() {
            return (self.engines.clone(), Vec::new());
        }
        let mut out = Vec::new();
        let mut unknown = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in provider_ids {
            if !seen.insert(id.clone()) {
                continue;
            }
            match self.engines.iter().find(|e| e.name() == id.as_str()) {
                Some(e) => out.push(e.clone()),
                None => unknown.push(id.clone()),
            }
        }
        (out, unknown)
    }

    /// List the provider ids that this adapter will query.
    pub fn provider_ids(&self) -> &[String] {
        &self.provider_ids
    }

    /// Per-provider status report. Includes both enabled providers in
    /// this adapter and the full set of known provider ids, so callers
    /// can see what is available vs. what is enabled.
    pub fn provider_status(&self) -> Vec<ProviderStatus> {
        let enabled: std::collections::BTreeSet<&str> =
            self.provider_ids.iter().map(|s| s.as_str()).collect();
        KNOWN_PROVIDERS
            .iter()
            .map(|id| {
                let (kind, requires_api_key) = provider_kind(id);
                ProviderStatus {
                    id: (*id).to_string(),
                    enabled: enabled.contains(id),
                    kind: kind.to_string(),
                    requires_api_key,
                }
            })
            .collect()
    }

    /// Run a metasearch query. This is the primary entry point used by
    /// the MCP `web_search` tool. If `req.providers` is non-empty, only
    /// those engines are queried (and the caller is expected to have
    /// already rejected unknown ids via `select_engines`).
    ///
    /// Uses per-engine timeouts via `JoinSet` so that a global deadline
    /// preserves partial results from engines that responded in time.
    pub async fn web_search(
        &self,
        req: &WebSearchRequest,
        default_max_results: usize,
        max_results_cap: usize,
    ) -> WebSearchResponse {
        let max_results = req.effective_max_results(default_max_results, max_results_cap);
        let (engines, queried_ids) = if req.providers.is_empty() {
            (self.engines.clone(), self.provider_ids.clone())
        } else {
            let (subset, unknown) = self.select_engines(&req.providers);
            if !unknown.is_empty() {
                warn!(
                    ?unknown,
                    "select_engines returned unknown ids; caller should have rejected these"
                );
            }
            let ids = subset.iter().map(|e| e.name().to_string()).collect();
            (subset, ids)
        };

        // Per-request timeout override, bounded above by the global timeout.
        let effective_timeout = match req.timeout_ms {
            Some(ms) => {
                let req_timeout = Duration::from_millis(ms);
                if req_timeout < self.global_timeout {
                    req_timeout
                } else {
                    self.global_timeout
                }
            }
            None => self.global_timeout,
        };

        debug!(
            query = %req.query,
            providers = ?queried_ids,
            max_results,
            timeout_ms = effective_timeout.as_millis(),
            "dispatching metasearch"
        );

        // Fan out to engines with per-engine timeout, collecting results
        // incrementally. When the global deadline hits we keep whatever
        // arrived and cancel the rest.
        let mut join_set = tokio::task::JoinSet::new();
        for engine in &engines {
            let engine = Arc::clone(engine);
            let query = req.query.clone();
            join_set.spawn(async move {
                let result = engine.search(&query, max_results).await;
                (engine.name().to_string(), result)
            });
        }

        let deadline = tokio::time::Instant::now() + effective_timeout;
        let mut raw_results: Vec<(String, Vec<SearchResult>)> = Vec::new();
        let mut raw_failures: Vec<(String, EngineError)> = Vec::new();

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                warn!(
                    "metasearch global timeout exceeded with {} engines still pending",
                    join_set.len()
                );
                break;
            }
            match tokio::time::timeout(remaining, join_set.join_next()).await {
                Ok(Some(Ok((name, Ok(results))))) => {
                    raw_results.push((name, results));
                }
                Ok(Some(Ok((name, Err(err))))) => {
                    raw_failures.push((name, err));
                }
                Ok(Some(Err(join_err))) => {
                    warn!(?join_err, "engine task panicked");
                }
                Ok(None) => break,
                Err(_) => {
                    warn!(
                        "metasearch global timeout exceeded with {} engines still pending",
                        join_set.len()
                    );
                    break;
                }
            }
        }
        // JoinSet dropped here cancels any in-flight engine tasks.

        let aggregated = aggregate_rrf(raw_results.clone(), max_results);
        let results: Vec<SourceCard> = aggregated
            .into_iter()
            .filter_map(convert_aggregated)
            .collect();

        // Collect the set of provider ids that already completed (success
        // or individual failure) so we don't double-count.
        let mut accounted: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (id, _) in &raw_results {
            accounted.insert(id.clone());
        }
        for (id, _) in &raw_failures {
            accounted.insert(id.clone());
        }

        // Engines still in the join_set when the deadline hit are
        // considered timed-out. They were cancelled by the JoinSet drop.
        let mut providers_failed: Vec<ProviderFailure> = raw_failures
            .into_iter()
            .map(|(id, err)| ProviderFailure {
                id,
                error_class: classify(&err).as_str().to_string(),
                message: err.to_string(),
            })
            .collect();

        for id in &queried_ids {
            if !accounted.contains(id.as_str()) {
                providers_failed.push(ProviderFailure {
                    id: id.clone(),
                    error_class: ErrorClass::Timeout.as_str().to_string(),
                    message: "provider timed out".to_string(),
                });
            }
        }

        let providers_queried: Vec<String> = queried_ids;

        let warnings: Vec<SearchWarning> = providers_failed
            .iter()
            .map(|f| {
                SearchWarning::new(
                    f.id.clone(),
                    format!("[{}] {}", f.error_class, f.message),
                )
            })
            .collect();

        WebSearchResponse {
            query: req.query.clone(),
            mode: "live_metasearch",
            results,
            providers_queried,
            providers_failed,
            warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// Engine construction
// ---------------------------------------------------------------------------

/// The set of provider ids that ship with the vendored engine
/// implementations and that eggsearch can enable by default.
pub const KNOWN_PROVIDERS: &[&str] = &["duckduckgo", "brave", "startpage", "yahoo"];

/// Kind of an engine, for `provider_status` reporting.
pub fn provider_kind(id: &str) -> (&'static str, bool) {
    match id {
        "duckduckgo" | "startpage" | "yahoo" => ("html_scrape", false),
        "brave" => ("html_scrape", false),
        _other => ("unknown", false),
    }
}

/// Build the default engine set used by the server.
pub fn build_default_engines(
    enabled_providers: &[String],
) -> anyhow::Result<(EngineList, Vec<String>)> {
    use crate::meta::engines::{
        BraveEngine, DuckDuckGoEngine, StartpageEngine, YahooEngine,
    };

    let client = Arc::new(build_http_client()?);
    let mut engines: EngineList = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for id in enabled_providers {
        match id.as_str() {
            "duckduckgo" => engines.push(Arc::new(DuckDuckGoEngine {
                client: client.clone(),
            })),
            "brave" => engines.push(Arc::new(BraveEngine {
                client: client.clone(),
            })),
            "startpage" => engines.push(Arc::new(StartpageEngine {
                client: client.clone(),
            })),
            "yahoo" => engines.push(Arc::new(YahooEngine {
                client: client.clone(),
            })),
            other => skipped.push(other.to_string()),
        }
    }

    Ok((engines, skipped))
}

// ---------------------------------------------------------------------------
// RRF aggregation (vendored from metadata-search-engine-rs)
// ---------------------------------------------------------------------------

use std::collections::HashMap;

// Standard RRF constant (Cormack et al., 2009).
const RRF_K: f64 = 60.0;

fn aggregate_rrf(
    engine_results: Vec<(String, Vec<SearchResult>)>,
    max_results: usize,
) -> Vec<AggregatedResult> {
    let mut map: HashMap<String, AggregatedResult> = HashMap::new();

    for (engine_name, results) in engine_results {
        for (index, result) in results.into_iter().enumerate() {
            let rank = index + 1;
            let rrf_score = 1.0 / (RRF_K + rank as f64);

            let key = match crate::meta::engines::normalizer::normalize(&result.url) {
                Some(k) => k,
                None => {
                    debug!(url = %result.url, "skipping result with un-normalizable URL");
                    continue;
                }
            };

            match map.get_mut(&key) {
                Some(existing) => {
                    existing.score += rrf_score;
                    if !existing.engines.contains(&engine_name) {
                        existing.engines.push(engine_name.clone());
                    }
                    if existing.snippet.is_none() && result.snippet.is_some() {
                        existing.snippet = result.snippet;
                    }
                }
                None => {
                    map.insert(
                        key,
                        AggregatedResult {
                            title: result.title,
                            url: result.url,
                            snippet: result.snippet,
                            engines: vec![engine_name.clone()],
                            score: rrf_score,
                        },
                    );
                }
            }
        }
    }

    let mut ranked: Vec<AggregatedResult> = map.into_values().collect();

    ranked.sort_unstable_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
    });

    ranked.truncate(max_results);
    ranked
}

// ---------------------------------------------------------------------------
// Conversion to eggsearch types
// ---------------------------------------------------------------------------

fn convert_aggregated(a: AggregatedResult) -> Option<SourceCard> {
    if a.url.is_empty() {
        return None;
    }
    if url::Url::parse(&a.url).is_err() {
        return None;
    }
    let providers: Vec<String> = a.engines.into_iter().collect();
    let mut card = SourceCard::new(
        a.title,
        a.url,
        providers,
        Some(a.score),
        TrustLevel::ExternalUntrusted,
    );
    if let Some(s) = a.snippet {
        if !s.is_empty() {
            card = card.with_snippet(s);
        }
    }
    Some(card)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_class_strs_are_stable() {
        assert_eq!(ErrorClass::Timeout.as_str(), "timeout");
        assert_eq!(ErrorClass::HttpStatus.as_str(), "http_status");
        assert_eq!(ErrorClass::ParseError.as_str(), "parse_error");
        assert_eq!(ErrorClass::NetworkError.as_str(), "network_error");
        assert_eq!(ErrorClass::RateLimited.as_str(), "rate_limited");
        assert_eq!(ErrorClass::Unknown.as_str(), "unknown");
    }

    #[test]
    fn convert_aggregated_maps_fields() {
        let a = AggregatedResult {
            title: "Example".to_string(),
            url: "https://example.com/article".to_string(),
            snippet: Some("A short snippet.".to_string()),
            engines: vec!["duckduckgo".to_string(), "brave".to_string()],
            score: 0.0327,
        };
        let c = convert_aggregated(a).expect("expected card");
        assert_eq!(c.title, "Example");
        assert_eq!(c.url, "https://example.com/article");
        assert_eq!(c.snippet.as_deref(), Some("A short snippet."));
        assert_eq!(
            c.providers,
            vec!["duckduckgo".to_string(), "brave".to_string()]
        );
        assert_eq!(c.score, Some(0.0327));
        assert_eq!(c.trust, TrustLevel::ExternalUntrusted);
        assert!(!c.fetched);
    }

    #[test]
    fn convert_aggregated_drops_empty_url() {
        let a = AggregatedResult {
            title: "t".to_string(),
            url: String::new(),
            snippet: None,
            engines: vec!["duckduckgo".to_string()],
            score: 0.1,
        };
        assert!(convert_aggregated(a).is_none());
    }

    #[test]
    fn convert_aggregated_drops_invalid_url() {
        let a = AggregatedResult {
            title: "t".to_string(),
            url: "not a url".to_string(),
            snippet: None,
            engines: vec!["duckduckgo".to_string()],
            score: 0.1,
        };
        assert!(convert_aggregated(a).is_none());
    }

    #[test]
    fn convert_aggregated_omits_empty_snippet() {
        let a = AggregatedResult {
            title: "t".to_string(),
            url: "https://example.com".to_string(),
            snippet: Some(String::new()),
            engines: vec!["duckduckgo".to_string()],
            score: 0.1,
        };
        let c = convert_aggregated(a).expect("expected card");
        assert!(c.snippet.is_none());
    }

    struct MockEngine {
        name: &'static str,
        results: Vec<SearchResult>,
    }

    impl SearchEngine for MockEngine {
        fn name(&self) -> &'static str {
            self.name
        }
        fn search<'a>(
            &'a self,
            _query: &'a str,
            _max_results: usize,
        ) -> crate::meta::engines::BoxFuture<
            'a,
            Result<Vec<SearchResult>, EngineError>,
        > {
            let results = self.results.clone();
            Box::pin(async move { Ok(results) })
        }
    }

    fn mk_result(title: &str, url: &str, engine: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: Some(format!("Snippet for {title}")),
            source_engine: engine.to_string(),
        }
    }

    #[tokio::test]
    async fn web_search_with_mock_engines_returns_source_cards() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![
            Arc::new(MockEngine {
                name: "duckduckgo",
                results: vec![
                    mk_result("A1", "https://a.com/1", "duckduckgo"),
                    mk_result("A2", "https://a.com/2", "duckduckgo"),
                ],
            }),
            Arc::new(MockEngine {
                name: "brave",
                results: vec![mk_result("A1", "https://a.com/1", "brave")],
            }),
        ];
        let adapter =
            MetadataSearchAdapter::from_engines(engines, Duration::from_secs(5));
        let req = WebSearchRequest::new("rust axum");
        let resp = adapter.web_search(&req, 10, 10).await;
        assert_eq!(resp.query, "rust axum");
        assert_eq!(resp.mode, "live_metasearch");
        assert_eq!(resp.providers_queried.len(), 2);
        assert!(resp.providers_failed.is_empty());
        let a1 = resp
            .results
            .iter()
            .find(|c| c.title == "A1")
            .expect("A1 card");
        assert_eq!(a1.providers.len(), 2);
        assert!(a1.providers.contains(&"duckduckgo".to_string()));
        assert!(a1.providers.contains(&"brave".to_string()));
        assert_eq!(a1.trust, TrustLevel::ExternalUntrusted);
        assert!(!a1.fetched);
    }
}
