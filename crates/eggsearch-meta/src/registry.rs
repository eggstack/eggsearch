//! Provider registry: holds enabled providers and orchestrates concurrent
//! search across them.

use std::sync::Arc;

use eggsearch_core::{
    config::{AppConfig, ProviderConfig},
    error::CoreResult,
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::SearchQuery,
};
use futures::future::join_all;
use serde::Serialize;
use tracing::warn;

use crate::providers::{
    brave::BraveProvider, crates_io::CratesIoProvider, docs_rs::DocsRsProvider,
    duckduckgo_html::DuckDuckGoHtmlProvider, exa::ExaProvider, mock::MockProvider,
    searxng::SearxngProvider, tavily::TavilyProvider, wikipedia::WikipediaProvider,
};

/// Holds the providers configured for the current server instance.
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn SearchProvider>>,
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field(
                "providers",
                &self.providers.iter().map(|p| p.id()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_providers() -> Self {
        let mut r = Self::new();
        r.register(Arc::new(DuckDuckGoHtmlProvider::new()));
        r.register(Arc::new(WikipediaProvider::new()));
        r.register(Arc::new(CratesIoProvider::new()));
        r.register(Arc::new(DocsRsProvider::new()));
        // Mock provider ships a small set of demo results so callers can
        // exercise the search pipeline without live network access.
        r.register(Arc::new(MockProvider::demo()));
        r
    }

    pub fn register(&mut self, provider: Arc<dyn SearchProvider>) {
        if !self.providers.iter().any(|p| p.id() == provider.id()) {
            self.providers.push(provider);
        }
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.providers.iter().map(|p| p.id()).collect()
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn SearchProvider>> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .cloned()
    }

    /// Build a registry from a config block. The bool controls whether the
    /// built-in mock provider is also registered (used by tests and the
    /// default CLI behavior).
    ///
    /// Returns the registry along with a per-provider diagnostic report
    /// that the caller can surface via `eggsearch doctor` / `providers`.
    /// Misconfigured optional providers are skipped, not fatal: the rest
    /// of the registry still loads.
    pub fn from_config(config: &AppConfig, include_mock: bool) -> (Self, RegistryDiagnostics) {
        let mut r = Self::new();
        let mut diags: Vec<ProviderDiagnostic> = Vec::new();

        for (id, cfg) in &config.search.providers {
            if !cfg.enabled {
                diags.push(ProviderDiagnostic {
                    id: id.clone(),
                    enabled: false,
                    status: DiagnosticStatus::Disabled,
                    message: None,
                });
                continue;
            }
            match build_provider(id, cfg) {
                Ok(provider) => {
                    r.register(provider);
                    diags.push(ProviderDiagnostic {
                        id: id.clone(),
                        enabled: true,
                        status: DiagnosticStatus::Loaded,
                        message: None,
                    });
                }
                Err(e) => {
                    warn!(provider = %id, "provider misconfigured: {e}");
                    diags.push(ProviderDiagnostic {
                        id: id.clone(),
                        enabled: true,
                        status: DiagnosticStatus::Misconfigured,
                        message: Some(e.to_string()),
                    });
                }
            }
        }

        if include_mock {
            r.register(Arc::new(MockProvider::demo()));
        }

        let loaded = r.ids().into_iter().map(|s| s.to_string()).collect();
        (
            r,
            RegistryDiagnostics {
                diagnostics: diags,
                loaded,
            },
        )
    }

    /// Run the query against each enabled provider in parallel, returning
    /// per-provider responses (errors converted to warnings).
    pub async fn search_all(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> CoreResult<Vec<SearchProviderResponse>> {
        let selected: Vec<Arc<dyn SearchProvider>> = if query.providers.is_empty() {
            self.providers.clone()
        } else {
            query
                .providers
                .iter()
                .filter_map(|id| self.get(id))
                .collect()
        };

        let futs = selected.into_iter().map(|p| {
            let q = query.clone();
            let c = ctx.clone();
            let q_for_err = query.clone();
            async move {
                match p.search(q, c).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!(provider = p.id(), "search failed: {e}");
                        let mut resp = SearchProviderResponse::empty(p.id(), q_for_err);
                        resp.warnings.push(eggsearch_core::result::SearchWarning {
                            provider_id: p.id().to_string(),
                            message: format!("{e}"),
                        });
                        resp
                    }
                }
            }
        });
        let out = join_all(futs).await;
        Ok(out)
    }
}

/// Build a single provider by id, returning a structured error if the
/// config is missing required fields (e.g. SearXNG base_url, API key env).
fn build_provider(
    id: &str,
    cfg: &ProviderConfig,
) -> eggsearch_core::error::CoreResult<Arc<dyn SearchProvider>> {
    match id {
        "duckduckgo_html" => Ok(Arc::new(DuckDuckGoHtmlProvider::new())),
        "wikipedia" => Ok(Arc::new(WikipediaProvider::new())),
        "crates_io" => Ok(Arc::new(CratesIoProvider::new())),
        "docs_rs" => Ok(Arc::new(DocsRsProvider::new())),
        "searxng" => Ok(Arc::new(SearxngProvider::from_config(cfg)?)),
        "brave" => Ok(Arc::new(BraveProvider::from_config(cfg)?)),
        "tavily" => Ok(Arc::new(TavilyProvider::from_config(cfg)?)),
        "exa" => Ok(Arc::new(ExaProvider::from_config(cfg)?)),
        // The mock provider is handled separately by `include_mock`.
        "mock" => Err(eggsearch_core::error::CoreError::Config(
            "mock provider is not configurable; it is added by the CLI when --include-mock is set"
                .to_string(),
        )),
        other => Err(eggsearch_core::error::CoreError::Config(format!(
            "unknown provider id '{other}'"
        ))),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    /// Provider is enabled and successfully registered.
    Loaded,
    /// Provider is enabled but could not be constructed (missing key, etc.).
    Misconfigured,
    /// Provider is explicitly disabled in the config.
    Disabled,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProviderDiagnostic {
    pub id: String,
    pub enabled: bool,
    pub status: DiagnosticStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct RegistryDiagnostics {
    pub loaded: Vec<String>,
    pub diagnostics: Vec<ProviderDiagnostic>,
}

impl RegistryDiagnostics {
    pub fn healthy(&self) -> bool {
        self.diagnostics
            .iter()
            .all(|d| d.status != DiagnosticStatus::Misconfigured)
    }

    pub fn misconfigured(&self) -> impl Iterator<Item = &ProviderDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.status == DiagnosticStatus::Misconfigured)
    }
}
