//! `MetadataSearchAdapter`: the metasearch-first boundary between
//! eggsearch and `metadata-search-engine-rs`. Callers receive Codegg-owned
//! types; upstream types do not leak past this module.

use std::time::Duration;

use eggsearch_core::SearchWarning;
use eggsearch_core::SourceCard;
use eggsearch_core::TrustLevel;
use eggsearch_core::WebSearchRequest;
use tracing::{debug, warn};

use crate::engine::build_default_engines;
use crate::response::{ProviderFailure, ProviderStatus, WebSearchResponse};

/// Coarse error class for provider failures. Exposed via `provider_status`
/// and the `web_search` tool's `providers_failed` field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorClass {
    Timeout,
    HttpStatus,
    ParseError,
    NetworkError,
    RateLimited,
    InvalidQuery,
    Unknown,
}

impl ErrorClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::HttpStatus => "http_status",
            Self::ParseError => "parse_error",
            Self::NetworkError => "network_error",
            Self::RateLimited => "rate_limited",
            Self::InvalidQuery => "invalid_query",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(feature = "metasearch")]
use metadata_search_engine_rs::aggregator::{aggregate, query_all_engines};
#[cfg(feature = "metasearch")]
use metadata_search_engine_rs::engines::SearchEngine;

#[cfg(feature = "metasearch")]
fn classify(err: &metadata_search_engine_rs::error::EngineError) -> ErrorClass {
    use metadata_search_engine_rs::error::EngineError::*;
    match err {
        Timeout { .. } => ErrorClass::Timeout,
        BadStatus { status, .. } if *status == 429 => ErrorClass::RateLimited,
        BadStatus { .. } => ErrorClass::HttpStatus,
        ParseFailed { .. } => ErrorClass::ParseError,
        Http { .. } => ErrorClass::NetworkError,
    }
}

#[cfg(feature = "metasearch")]
type Engines = Vec<std::sync::Arc<dyn SearchEngine>>;

#[cfg(not(feature = "metasearch"))]
type Engines = Vec<()>;

/// Constructed once at server startup. Holds the upstream `SearchEngine`
/// instances and the effective provider list.
pub struct MetadataSearchAdapter {
    engines: Engines,
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
    #[cfg(feature = "metasearch")]
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

    /// Stub for when the `metasearch` feature is off.
    #[cfg(not(feature = "metasearch"))]
    pub fn new(_enabled_providers: Vec<String>, _global_timeout: Duration) -> anyhow::Result<Self> {
        Err(anyhow::anyhow!(
            "metasearch feature is not enabled in this build; MetadataSearchAdapter is unavailable"
        ))
    }

    /// Build an adapter from an explicit list of `SearchEngine` trait
    /// objects. Used by tests to inject mock engines.
    #[cfg(feature = "metasearch")]
    pub fn from_engines(
        engines: Vec<std::sync::Arc<dyn SearchEngine>>,
        global_timeout: Duration,
    ) -> Self {
        let provider_ids = engines.iter().map(|e| e.name().to_string()).collect();
        Self {
            engines,
            provider_ids,
            global_timeout,
        }
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
        crate::engine::KNOWN_PROVIDERS
            .iter()
            .map(|id| {
                let (kind, requires_api_key) = crate::engine::provider_kind(id);
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
    /// the MCP `web_search` tool.
    #[cfg(feature = "metasearch")]
    pub async fn web_search(
        &self,
        req: &WebSearchRequest,
        max_results_cap: usize,
    ) -> WebSearchResponse {
        let max_results = req.effective_max_results(10, max_results_cap);
        debug!(
            query = %req.query,
            providers = ?self.provider_ids,
            max_results,
            "dispatching metasearch"
        );

        let (raw_results, raw_failures) = match tokio::time::timeout(
            self.global_timeout,
            query_all_engines(&self.engines, &req.query, max_results),
        )
        .await
        {
            Ok(t) => t,
            Err(_) => {
                warn!("metasearch global timeout exceeded");
                (
                    Vec::new(),
                    self.provider_ids
                        .iter()
                        .map(|id| {
                            (
                                id.clone(),
                                metadata_search_engine_rs::error::EngineError::Timeout {
                                    engine: "global",
                                },
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            }
        };

        let aggregated = aggregate(raw_results.clone(), max_results);
        let results: Vec<SourceCard> = aggregated
            .into_iter()
            .filter_map(convert_aggregated)
            .collect();

        let providers_queried: Vec<String> = raw_results
            .iter()
            .map(|(id, _)| id.clone())
            .chain(raw_failures.iter().map(|(id, _)| id.clone()))
            .collect();

        let mut providers_failed: Vec<ProviderFailure> = raw_failures
            .into_iter()
            .map(|(id, err)| ProviderFailure {
                id,
                error_class: classify(&err).as_str().to_string(),
                message: err.to_string(),
            })
            .collect();

        if providers_failed.is_empty() && results.is_empty() {
            for id in &self.provider_ids {
                providers_failed.push(ProviderFailure {
                    id: id.clone(),
                    error_class: ErrorClass::Timeout.as_str().to_string(),
                    message: "global timeout".to_string(),
                });
            }
        }

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

    #[cfg(not(feature = "metasearch"))]
    pub async fn web_search(
        &self,
        req: &WebSearchRequest,
        _max_results_cap: usize,
    ) -> WebSearchResponse {
        WebSearchResponse {
            query: req.query.clone(),
            mode: "off",
            results: Vec::new(),
            providers_queried: Vec::new(),
            providers_failed: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[cfg(feature = "metasearch")]
pub(crate) fn convert_aggregated(
    a: metadata_search_engine_rs::models::AggregatedResult,
) -> Option<SourceCard> {
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
        assert_eq!(ErrorClass::InvalidQuery.as_str(), "invalid_query");
        assert_eq!(ErrorClass::Unknown.as_str(), "unknown");
    }

    #[cfg(feature = "metasearch")]
    #[test]
    fn convert_aggregated_maps_fields() {
        let a = metadata_search_engine_rs::models::AggregatedResult {
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
        assert_eq!(c.providers, vec!["duckduckgo".to_string(), "brave".to_string()]);
        assert_eq!(c.score, Some(0.0327));
        assert_eq!(c.trust, TrustLevel::ExternalUntrusted);
        assert!(!c.fetched);
    }

    #[cfg(feature = "metasearch")]
    #[test]
    fn convert_aggregated_drops_empty_url() {
        let a = metadata_search_engine_rs::models::AggregatedResult {
            title: "t".to_string(),
            url: String::new(),
            snippet: None,
            engines: vec!["duckduckgo".to_string()],
            score: 0.1,
        };
        assert!(convert_aggregated(a).is_none());
    }

    #[cfg(feature = "metasearch")]
    #[test]
    fn convert_aggregated_drops_invalid_url() {
        let a = metadata_search_engine_rs::models::AggregatedResult {
            title: "t".to_string(),
            url: "not a url".to_string(),
            snippet: None,
            engines: vec!["duckduckgo".to_string()],
            score: 0.1,
        };
        assert!(convert_aggregated(a).is_none());
    }

    #[cfg(feature = "metasearch")]
    #[test]
    fn convert_aggregated_omits_empty_snippet() {
        let a = metadata_search_engine_rs::models::AggregatedResult {
            title: "t".to_string(),
            url: "https://example.com".to_string(),
            snippet: Some(String::new()),
            engines: vec!["duckduckgo".to_string()],
            score: 0.1,
        };
        let c = convert_aggregated(a).expect("expected card");
        assert!(c.snippet.is_none());
    }

    /// Mock SearchEngine that returns a fixed set of results. Used to
    /// exercise the full `web_search` -> `aggregate` -> SourceCard path
    /// without hitting the network.
    #[cfg(feature = "metasearch")]
    struct MockEngine {
        name: &'static str,
        results: Vec<metadata_search_engine_rs::models::SearchResult>,
    }

    #[cfg(feature = "metasearch")]
    impl SearchEngine for MockEngine {
        fn name(&self) -> &'static str {
            self.name
        }
        fn search<'a>(
            &'a self,
            _query: &'a str,
            _max_results: usize,
        ) -> metadata_search_engine_rs::engines::BoxFuture<
            'a,
            Result<Vec<metadata_search_engine_rs::models::SearchResult>, metadata_search_engine_rs::error::EngineError>,
        > {
            let results = self.results.clone();
            Box::pin(async move { Ok(results) })
        }
    }

    #[cfg(feature = "metasearch")]
    fn mk_result(title: &str, url: &str, engine: &str) -> metadata_search_engine_rs::models::SearchResult {
        metadata_search_engine_rs::models::SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: Some(format!("Snippet for {title}")),
            source_engine: engine.to_string(),
        }
    }

    #[cfg(feature = "metasearch")]
    #[tokio::test]
    async fn web_search_with_mock_engines_returns_source_cards() {
        use std::sync::Arc;
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
        let adapter = MetadataSearchAdapter::from_engines(engines, Duration::from_secs(5));
        let req = WebSearchRequest::new("rust axum");
        let resp = adapter.web_search(&req, 10).await;
        assert_eq!(resp.query, "rust axum");
        assert_eq!(resp.mode, "live_metasearch");
        assert_eq!(resp.providers_queried.len(), 2);
        assert!(resp.providers_failed.is_empty());
        // A1 appears in both engines; deduplication via aggregate
        // collapses the two results into one card with both providers.
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
