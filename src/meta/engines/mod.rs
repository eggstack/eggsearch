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

use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client,
};

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

// Mimic a real browser as closely as possible to avoid bot-detection rejections.
pub fn build_http_client(user_agent: Option<&str>) -> anyhow::Result<Client> {
    let mut headers = HeaderMap::new();

    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/124.0.0.0 Safari/537.36",
        ),
    );

    let builder = Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .gzip(true)
        .brotli(true)
        .timeout(Duration::from_secs(20));

    let builder = match user_agent {
        Some(ua) => builder.user_agent(ua),
        None => builder,
    };

    let client = builder.build()?;

    Ok(client)
}
