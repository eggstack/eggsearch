//! Mock upstream engines for tests.
//!
//! Gated behind the `mock` feature so that downstream binaries don't
//! pull in this code by default. Tests in `eggsearch-mcp` depend on
//! `eggsearch-meta/mock` to inject deterministic results.

#![cfg(feature = "mock")]

use std::future::pending;
use std::sync::Arc;

use metadata_search_engine_rs::engines::{BoxFuture, SearchEngine};
use metadata_search_engine_rs::error::EngineError;
use metadata_search_engine_rs::models::SearchResult;

/// A canned upstream result. Construct with `MockResult::new(...,
/// ..., ...)` and optionally `.with_snippet(...)`.
#[derive(Clone, Debug)]
pub struct MockResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub source_engine: String,
}

impl MockResult {
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        source_engine: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            snippet: None,
            source_engine: source_engine.into(),
        }
    }

    pub fn with_snippet(mut self, snippet: impl Into<String>) -> Self {
        self.snippet = Some(snippet.into());
        self
    }
}

/// Coarse failure kinds the mock engine can produce. Each maps to one
/// upstream `EngineError` variant. We do not store the upstream error
/// directly because it isn't `Clone`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MockFailure {
    Timeout,
    HttpStatus(u16),
    Parse,
    Network,
}

impl MockFailure {
    fn to_engine_error(self, engine: &'static str) -> EngineError {
        match self {
            MockFailure::Timeout => EngineError::Timeout { engine },
            MockFailure::HttpStatus(status) => EngineError::BadStatus { engine, status },
            MockFailure::Parse => EngineError::ParseFailed {
                engine,
                reason: "mock parse failure".to_string(),
            },
            MockFailure::Network => EngineError::ParseFailed {
                engine,
                reason: "mock network failure".to_string(),
            },
        }
    }
}

/// A configurable mock engine. Each search invocation either:
/// - returns the configured results,
/// - returns the configured failure, or
/// - never resolves (used to test the global-timeout path).
pub struct MockEngine {
    name: &'static str,
    results: Vec<SearchResult>,
    failure: Option<MockFailure>,
    hang: bool,
}

impl MockEngine {
    /// A mock that always returns the given results.
    pub fn success(name: &'static str, results: Vec<MockResult>) -> Self {
        let rs = results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.snippet,
                source_engine: r.source_engine,
            })
            .collect();
        Self {
            name,
            results: rs,
            failure: None,
            hang: false,
        }
    }

    /// A mock that always returns the given failure kind.
    pub fn failure(name: &'static str, failure: MockFailure) -> Self {
        Self {
            name,
            results: Vec::new(),
            failure: Some(failure),
            hang: false,
        }
    }

    /// A mock whose `search` future never resolves. Used to test the
    /// global-timeout path in the adapter.
    pub fn hang(name: &'static str) -> Self {
        Self {
            name,
            results: Vec::new(),
            failure: None,
            hang: true,
        }
    }
}

impl SearchEngine for MockEngine {
    fn name(&self) -> &'static str {
        self.name
    }

    fn search<'a>(
        &'a self,
        _query: &'a str,
        _max_results: usize,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            if self.hang {
                pending::<()>().await;
                unreachable!("pending future resolved")
            }
            match self.failure {
                Some(f) => Err(f.to_engine_error(self.name)),
                None => Ok(self.results.clone()),
            }
        })
    }
}

/// Convenience: wrap a list of mock engines into `Arc<dyn SearchEngine>`
/// so callers can pass it to `MetadataSearchAdapter::from_engines`.
pub fn mock_engines(engines: Vec<MockEngine>) -> Vec<Arc<dyn SearchEngine>> {
    engines
        .into_iter()
        .map(|e| Arc::new(e) as Arc<dyn SearchEngine>)
        .collect()
}
