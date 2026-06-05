//! Provider registry: holds enabled providers and orchestrates concurrent
//! search across them.

use std::sync::Arc;

use eggsearch_core::{
    error::CoreResult,
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::SearchQuery,
};
use futures::future::join_all;
use tracing::warn;

use crate::providers::{
    crates_io::CratesIoProvider, docs_rs::DocsRsProvider, duckduckgo_html::DuckDuckGoHtmlProvider,
    mock::MockProvider, wikipedia::WikipediaProvider,
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
            .map(|p| p.clone())
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
