//! Mock upstream engines for tests.
//!
//! Gated behind the `mock` feature so that downstream binaries don't
//! pull in this code by default.

#![cfg(feature = "mock")]
#![allow(missing_docs)]

use std::future::pending;
use std::sync::Arc;

use crate::meta::engines::error::EngineError;
use crate::meta::engines::models::SearchResult;
use crate::meta::engines::BoxFuture;
use crate::meta::engines::SearchEngine;

/// A canned upstream result. Construct with `MockResult::new(...)`
/// and optionally `.with_snippet(...)`.
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

/// Coarse failure kinds the mock engine can produce.
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
            MockFailure::Network => EngineError::NetworkError {
                engine,
                reason: "mock network failure".to_string(),
            },
        }
    }
}

/// A configurable mock engine.
pub struct MockEngine {
    name: &'static str,
    results: Vec<SearchResult>,
    failure: Option<MockFailure>,
    hang: bool,
}

impl MockEngine {
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

    pub fn failure(name: &'static str, failure: MockFailure) -> Self {
        Self {
            name,
            results: Vec::new(),
            failure: Some(failure),
            hang: false,
        }
    }

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

/// Convenience: wrap a list of mock engines into `Arc<dyn SearchEngine>`.
pub fn mock_engines(engines: Vec<MockEngine>) -> Vec<Arc<dyn SearchEngine>> {
    engines
        .into_iter()
        .map(|e| Arc::new(e) as Arc<dyn SearchEngine>)
        .collect()
}
