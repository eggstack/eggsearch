//! Mock provider used for tests and offline operation.

use async_trait::async_trait;
use eggsearch_core::{
    error::CoreResult,
    provider::{SearchContext, SearchProvider, SearchProviderResponse},
    query::SearchQuery,
    result::{SearchResult, SourceKind, TrustLevel},
};
use url::Url;

/// A provider that returns a fixed set of results, used in tests and as
/// a fallback when live search is disabled but the caller still wants a
/// structured response.
#[derive(Clone, Debug, Default)]
pub struct MockProvider {
    results: Vec<SearchResult>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_results(results: Vec<SearchResult>) -> Self {
        Self { results }
    }

    /// A mock provider pre-populated with a few demo results.
    pub fn demo() -> Self {
        let mk = |url: &str, title: &str, snippet: &str| SearchResult {
            title: title.to_string(),
            url: Url::parse(url).unwrap(),
            snippet: Some(snippet.to_string()),
            published_at: None,
            rank: 0,
            score: None,
            provider_id: "mock".to_string(),
            source_kind: SourceKind::Web,
            trust_level: TrustLevel::ExternalUntrusted,
        };
        Self {
            results: vec![
                mk(
                    "https://example.com/article-1",
                    "Example article about the topic",
                    "A mock result demonstrating structured search response shape.",
                ),
                mk(
                    "https://www.rust-lang.org/",
                    "Rust Programming Language",
                    "Homepage of the Rust programming language.",
                ),
                mk(
                    "https://docs.rs/tokio/latest/tokio/",
                    "Tokio - An asynchronous runtime for Rust",
                    "Async runtime documentation, useful for many Rust projects.",
                ),
            ],
        }
    }

    pub fn default_for(query: &str) -> Self {
        let q = query.trim();
        let mk = |url: &str, title: &str, snippet: &str| SearchResult {
            title: title.to_string(),
            url: Url::parse(url).unwrap(),
            snippet: Some(snippet.to_string()),
            published_at: None,
            rank: 0,
            score: None,
            provider_id: "mock".to_string(),
            source_kind: SourceKind::Web,
            trust_level: TrustLevel::ExternalUntrusted,
        };
        Self {
            results: vec![
                mk(
                    "https://example.com/article-1",
                    "Example article about the topic",
                    "A mock result demonstrating structured search response shape.",
                ),
                mk(
                    "https://www.rust-lang.org/",
                    "Rust Programming Language",
                    &format!("Homepage related to '{q}'."),
                ),
                mk(
                    "https://docs.rs/tokio/latest/tokio/",
                    "Tokio - An asynchronous runtime for Rust",
                    "Async runtime documentation, relevant to your search.",
                ),
            ],
        }
    }
}

#[async_trait]
impl SearchProvider for MockProvider {
    fn id(&self) -> &'static str {
        "mock"
    }

    async fn search(
        &self,
        query: SearchQuery,
        _ctx: SearchContext,
    ) -> CoreResult<SearchProviderResponse> {
        let mut resp = SearchProviderResponse::empty(self.id(), query.clone());
        let max = query.max_results.max(1);
        for (i, mut r) in self.results.iter().cloned().enumerate() {
            if i >= max {
                break;
            }
            r.rank = i;
            resp.results.push(r);
        }
        if resp.results.is_empty() {
            resp.warnings.push(eggsearch_core::result::SearchWarning {
                provider_id: self.id().to_string(),
                message: "mock provider returned no results".to_string(),
            });
        }
        Ok(resp)
    }
}
