//! Vendored HTML search engines for the metasearch adapter. These are
//! internal implementation details; the public types are re-exported
//! from [`crate::meta`].

#![allow(missing_docs)]

pub mod brave;
pub mod duckduckgo;
pub mod error;
pub mod models;
pub mod normalizer;
pub mod startpage;
pub mod yahoo;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;

use self::error::EngineError;
use self::models::SearchResult;

// A heap-allocated future that is Send — required for dyn trait + tokio multi-thread.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>>;
}

pub struct DuckDuckGoEngine {
    pub client: Arc<Client>,
}

pub struct BraveEngine {
    pub client: Arc<Client>,
}

pub struct StartpageEngine {
    pub client: Arc<Client>,
}

pub struct YahooEngine {
    pub client: Arc<Client>,
}

impl SearchEngine for DuckDuckGoEngine {
    fn name(&self) -> &'static str {
        "duckduckgo"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(duckduckgo::search(&self.client, query, max_results))
    }
}

impl SearchEngine for BraveEngine {
    fn name(&self) -> &'static str {
        "brave"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(brave::search(&self.client, query, max_results))
    }
}

impl SearchEngine for StartpageEngine {
    fn name(&self) -> &'static str {
        "startpage"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(startpage::search(&self.client, query, max_results))
    }
}

impl SearchEngine for YahooEngine {
    fn name(&self) -> &'static str {
        "yahoo"
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(yahoo::search(&self.client, query, max_results))
    }
}

// Browser-like UA used as the fallback when no operator-supplied UA is provided.
// Mimic a real browser as closely as possible to avoid bot-detection rejections
// from HTML providers — but only when the operator has not configured their own.
const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/124.0.0.0 Safari/537.36";

pub fn build_http_client(user_agent: Option<&str>) -> anyhow::Result<Client> {
    let ua = resolve_user_agent(user_agent);

    let builder = Client::builder()
        .user_agent(ua)
        .cookie_store(true)
        .gzip(true)
        .brotli(true)
        .timeout(Duration::from_secs(20));

    let client = builder.build()?;

    Ok(client)
}

// Pick the UA the client will actually send: the operator's configured value
// if present, otherwise the browser-like fallback.
fn resolve_user_agent(user_agent: Option<&str>) -> &str {
    user_agent.unwrap_or(DEFAULT_USER_AGENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_user_agent_uses_configured_value() {
        assert_eq!(
            resolve_user_agent(Some("eggsearch/test-ua")),
            "eggsearch/test-ua"
        );
    }

    #[test]
    fn resolve_user_agent_uses_default_when_none() {
        let ua = resolve_user_agent(None);
        assert!(
            ua.contains("Mozilla"),
            "default UA should be Mozilla-like, got: {ua}"
        );
    }

    #[test]
    fn build_http_client_succeeds_with_configured_ua() {
        let client = build_http_client(Some("eggsearch/test-ua")).expect("build");
        drop(client);
    }

    #[test]
    fn build_http_client_succeeds_with_default_ua() {
        let client = build_http_client(None).expect("build");
        drop(client);
    }
}
