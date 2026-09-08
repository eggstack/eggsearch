//! Metasearch adapter, split by behavior.
//!
//! Central orchestrator is `MetadataSearchAdapter`; provider invocation,
//! response adaptation, error classification, and result normalization live in
//! focused submodules. The stable `crate::meta::adapter::X` paths are preserved
//! via re-exports.

use crate::core::config::ApiProviderConfig;
use crate::meta::engines::SearchEngine;
use crate::meta::provider_diagnostics::ProviderHealthRegistry;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

/// A provider that was skipped during engine construction, with a
/// human-readable reason explaining why it could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedProvider {
    /// The provider id that was skipped.
    pub id: String,
    /// Human-readable reason the provider was skipped.
    pub reason: String,
}

pub(crate) type EngineList = Vec<Arc<dyn SearchEngine>>;

#[derive(Debug)]
struct PlannedSubquery {
    label: String,
    query: String,
    /// Lower number = higher priority. Ties broken by order.
    priority: i32,
    /// Pre-computed intended evidence roles for this subquery.
    /// When non-empty, `dispatch_subqueries` uses these directly
    /// instead of calling `map_provider_to_intended_roles()`.
    intended_roles: Vec<crate::core::evidence_role::EvidenceRole>,
    /// Provider-neutral repository scope for native repo filtering.
    repo_scope: Option<crate::meta::engines::request::RepoScope>,
    /// Bounded excerpt demand for this subquery.
    excerpt_count: usize,
}

/// Constructed once at server startup. Holds the `SearchEngine`
/// instances and the effective provider list.
pub struct MetadataSearchAdapter {
    engines: EngineList,
    provider_ids: Vec<String>,
    /// Hard timeout for the whole `web_search` call, including fan-out.
    global_timeout: Duration,
    /// Whether to wrap untrusted search-result text in
    /// `<<<EXTERNAL_UNTRUSTED ...>>>` framing and emit per-card
    /// prompt-injection warnings. Tier 1 (control-char stripping +
    /// length bounding) is always on; this flag gates Tier 2
    /// (framing) and Tier 3 (marker scan).
    sanitize_output: bool,
    /// Provider ids listed as defaults in the config. Used by
    /// `provider_status()` to populate the `default` flag on each
    /// descriptor.
    default_providers: Vec<String>,
    /// Whether the SearXNG provider is fully configured (has a
    /// non-empty `base_url`). Used by `provider_status()` to set
    /// the `configured` flag on the SearXNG descriptor.
    searxng_configured: bool,
    /// Which API providers are configured (have a valid api_key_env
    /// that resolves at runtime). Used by `provider_status()` to set
    /// the `configured` flag on API provider descriptors.
    api_configured: std::collections::BTreeMap<String, bool>,
    /// Maximum total in-flight (subquery, provider) jobs during
    /// parallel dispatch.
    multiquery_concurrency: usize,
    /// Maximum concurrent jobs for any single provider during parallel
    /// dispatch.
    multiquery_provider_concurrency: usize,
    /// Process-local provider health registry for cooldown and diagnostics.
    health: Arc<ProviderHealthRegistry>,
}

impl std::fmt::Debug for MetadataSearchAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetadataSearchAdapter")
            .field("providers", &self.provider_ids)
            .field("global_timeout_ms", &self.global_timeout.as_millis())
            .field("sanitize_output", &self.sanitize_output)
            .field("health", &self.health)
            .finish()
    }
}

impl MetadataSearchAdapter {
    /// Build an adapter for the given enabled provider ids.
    ///
    /// `searxng_base_url` is the operator-supplied base URL of a
    /// self-hosted SearXNG instance. When `None` (or empty), the
    /// `searxng` provider id (if enabled) is silently skipped; the
    /// caller decides whether that should be a hard error or a warning.
    ///
    /// `api_providers` contains the API-key backed provider
    /// configurations. Each enabled entry with a resolvable API key
    /// env var produces a live engine instance.
    ///
    /// `sanitize_output` enables Tier 2 (framing) and Tier 3
    /// (prompt-injection marker scanning) on top of the always-on
    /// Tier 1 (control-char stripping + length bounding).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        enabled_providers: Vec<String>,
        global_timeout: Duration,
        user_agent: Option<String>,
        searxng_base_url: Option<String>,
        sanitize_output: bool,
        default_providers: Vec<String>,
        api_providers: &std::collections::BTreeMap<String, ApiProviderConfig>,
        multiquery_concurrency: usize,
        multiquery_provider_concurrency: usize,
    ) -> anyhow::Result<Self> {
        let searxng_configured = searxng_base_url.as_deref().is_some_and(|s| !s.is_empty());
        let (engines, skipped) = build_default_engines(
            &enabled_providers,
            user_agent,
            searxng_base_url,
            api_providers,
        )?;
        if !skipped.is_empty() {
            let skipped_ids: Vec<String> = skipped.iter().map(|s| s.id.clone()).collect();
            warn!(?skipped_ids, "skipped provider ids in config");
        }
        if engines.is_empty() {
            return Err(anyhow::anyhow!(
                "no engines could be built; check the [search].providers config"
            ));
        }

        // Compute which API providers are configured (have env var set)
        let mut api_configured = std::collections::BTreeMap::new();
        for (id, cfg) in api_providers {
            let configured = cfg.enabled
                && cfg
                    .api_key_env
                    .as_deref()
                    .is_some_and(|env| std::env::var(env).is_ok());
            api_configured.insert(id.clone(), configured);
        }

        let provider_ids = engines.iter().map(|e| e.name().to_string()).collect();
        Ok(Self {
            engines,
            provider_ids,
            global_timeout,
            sanitize_output,
            default_providers,
            searxng_configured,
            api_configured,
            multiquery_concurrency,
            multiquery_provider_concurrency,
            health: Arc::new(ProviderHealthRegistry::new()),
        })
    }

    /// Build an adapter from an explicit list of `SearchEngine` trait
    /// objects. Used by tests to inject mock engines. The
    /// `sanitize_output` flag defaults to `false` to preserve
    /// pre-sanitization integration-test expectations (titles,
    /// snippets, and the `TrustMarkers` aggregates are returned in
    /// their raw, unframed form). Production code uses
    /// [`MetadataSearchAdapter::new`] which takes the operator's
    /// configured value (default `true`).
    pub fn from_engines(engines: Vec<Arc<dyn SearchEngine>>, global_timeout: Duration) -> Self {
        let provider_ids = engines.iter().map(|e| e.name().to_string()).collect();
        Self {
            engines,
            provider_ids,
            global_timeout,
            sanitize_output: false,
            default_providers: Vec::new(),
            searxng_configured: false,
            api_configured: std::collections::BTreeMap::new(),
            multiquery_concurrency: 8,
            multiquery_provider_concurrency: 2,
            health: Arc::new(ProviderHealthRegistry::new()),
        }
    }

    /// Like [`Self::from_engines`] but with an explicit
    /// `sanitize_output` flag. Used by integration tests that need
    /// to exercise the Tier 2 (framing) and Tier 3 (marker scan)
    /// behavior on a mock adapter. Only available with the `mock`
    /// feature so that downstream binaries don't see this
    /// test-only constructor.
    #[cfg(feature = "mock")]
    pub fn from_engines_with_sanitize(
        engines: Vec<Arc<dyn SearchEngine>>,
        global_timeout: Duration,
        sanitize_output: bool,
    ) -> Self {
        let provider_ids = engines.iter().map(|e| e.name().to_string()).collect();
        Self {
            engines,
            provider_ids,
            global_timeout,
            sanitize_output,
            default_providers: Vec::new(),
            searxng_configured: false,
            api_configured: std::collections::BTreeMap::new(),
            multiquery_concurrency: 8,
            multiquery_provider_concurrency: 2,
            health: Arc::new(ProviderHealthRegistry::new()),
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
            return (Vec::new(), Vec::new());
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

    /// Access the process-local provider health registry.
    pub fn health(&self) -> &Arc<ProviderHealthRegistry> {
        &self.health
    }

    /// Whether the SearXNG provider has a valid base URL.
    pub fn searxng_configured(&self) -> bool {
        self.searxng_configured
    }

    /// Runtime configured state for API-key providers.
    pub fn api_configured(&self) -> &std::collections::BTreeMap<String, bool> {
        &self.api_configured
    }

    pub(crate) fn effective_timeout(&self, timeout_ms: Option<u64>) -> Duration {
        match timeout_ms {
            Some(ms) => Duration::from_millis(ms).min(self.global_timeout),
            None => self.global_timeout,
        }
    }

    fn selected_engines(&self, provider_ids: &[String]) -> (EngineList, Vec<String>) {
        if provider_ids.is_empty() {
            return (self.engines.clone(), self.provider_ids.clone());
        }

        let (subset, unknown) = self.select_engines(provider_ids);
        if !unknown.is_empty() {
            warn!(
                ?unknown,
                "select_engines returned unknown ids; caller should have rejected these"
            );
        }
        let ids = subset.iter().map(|e| e.name().to_string()).collect();
        (subset, ids)
    }
}
mod advisory;
mod builders;
mod error;
mod execution;
mod normalization;
mod repo;
mod research;
mod security;
mod status;
mod web;

pub use advisory::{NativeAdvisoryOperation, ProviderAdvisoryOutcome, ProviderAdvisoryStatus};
pub use builders::build_default_engines;
pub use error::ErrorClass;
pub(crate) use normalization::build_retrieval_failures;

#[cfg(test)]
mod tests;
