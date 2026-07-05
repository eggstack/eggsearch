//! Mock upstream engines for tests.
//!
//! Gated behind the `mock` feature so that downstream binaries don't
//! pull in this code by default.

#![allow(missing_docs)]

use std::future::pending;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    timeout_sink: Option<Arc<Mutex<Option<Duration>>>>,
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
                metadata: Default::default(),
            })
            .collect();
        Self {
            name,
            results: rs,
            failure: None,
            hang: false,
            timeout_sink: None,
        }
    }

    pub fn failure(name: &'static str, failure: MockFailure) -> Self {
        Self {
            name,
            results: Vec::new(),
            failure: Some(failure),
            hang: false,
            timeout_sink: None,
        }
    }

    pub fn hang(name: &'static str) -> Self {
        Self {
            name,
            results: Vec::new(),
            failure: None,
            hang: true,
            timeout_sink: None,
        }
    }

    pub fn record_timeout(name: &'static str, sink: Arc<Mutex<Option<Duration>>>) -> Self {
        Self {
            name,
            results: Vec::new(),
            failure: None,
            hang: false,
            timeout_sink: Some(sink),
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
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        if let Some(sink) = &self.timeout_sink {
            if let Ok(mut g) = sink.lock() {
                *g = Some(timeout);
            }
        }
        let limit = max_results;
        Box::pin(async move {
            if self.hang {
                pending::<()>().await;
                unreachable!("pending future resolved")
            }
            match self.failure {
                Some(f) => Err(f.to_engine_error(self.name)),
                None => {
                    // Respect the per-call `max_results` argument so
                    // mock behavior mirrors real providers. This is
                    // what makes the candidate-pool regression test
                    // able to catch the original bug where production
                    // providers were called with `final_max_results`
                    // instead of `candidate_limit`.
                    let mut results = self.results.clone();
                    results.truncate(limit);
                    Ok(results)
                }
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

/// A mock engine that records the `max_results` argument it was last
/// called with. The recorded value is exposed via the shared `sink`
/// for assertions in tests. Used to verify that adapter fan-out passes
/// the candidate-pool limit to providers rather than the final return
/// count.
pub struct RecordingMockEngine {
    name: &'static str,
    results: Vec<SearchResult>,
    seen_limit: Arc<Mutex<Option<usize>>>,
}

impl RecordingMockEngine {
    pub fn new(
        name: &'static str,
        results: Vec<MockResult>,
        sink: Arc<Mutex<Option<usize>>>,
    ) -> Self {
        let rs = results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.snippet,
                source_engine: r.source_engine,
                metadata: Default::default(),
            })
            .collect();
        Self {
            name,
            results: rs,
            seen_limit: sink,
        }
    }

    /// Wrap this engine into an `Arc<dyn SearchEngine>` for use with
    /// `MetadataSearchAdapter`.
    pub fn into_engine(self) -> Arc<dyn SearchEngine> {
        Arc::new(self)
    }
}

impl SearchEngine for RecordingMockEngine {
    fn name(&self) -> &'static str {
        self.name
    }

    fn search<'a>(
        &'a self,
        _query: &'a str,
        max_results: usize,
        _timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        if let Ok(mut g) = self.seen_limit.lock() {
            *g = Some(max_results);
        }
        let results = self.results.clone();
        let limit = max_results;
        Box::pin(async move {
            let mut out = results;
            out.truncate(limit);
            Ok(out)
        })
    }
}
