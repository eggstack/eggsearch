//! eggsearch: a lightweight MCP (Model Context Protocol) metasearch
//! server for AI agents.
//!
//! This crate is a single binary. Its submodules are:
//!
//! - [`core`]:    source card model, config, error, query types.
//! - [`meta`]:    metasearch adapter with vendored search engines.
//! - [`mcp`]:     MCP server (rmcp) exposing ten stable tools:
//!   `web_search`, `web_fetch`, `batch_fetch`, `provider_status`,
//!   `repo_search`, `repo_fetch`, `repo_map`, `security_search`,
//!   `research_search`, and `build_evidence_bundle`.
//!
//! The `mock` feature exposes the test-only mock engine harness used by
//! the integration tests.
//!
//! The `browser` feature enables optional headless Chrome/Chromium
//! rendering for JavaScript-heavy pages via the Chrome DevTools Protocol.
//!
//! # Example
//!
//! Construct and validate a [`core::WebSearchRequest`] the same way the
//! MCP `web_search` tool does:
//!
//! ```
//! use eggsearch::core::WebSearchRequest;
//!
//! let mut req = WebSearchRequest::new("rust axum middleware");
//! req.max_results = Some(5);
//! req.providers = vec!["duckduckgo".to_string(), "brave".to_string()];
//! req.validate(512).expect("request is valid");
//! ```

#![warn(missing_docs)]

pub mod core;
pub mod fetch;
pub mod mcp;
pub mod meta;

#[cfg(feature = "mock")]
pub use meta::local_inventory_cache::test_harness as bounded_command_test;
