//! eggsearch: a lightweight MCP (Model Context Protocol) metasearch
//! server for AI agents.
//!
//! This crate is a single binary. Its submodules are:
//!
//! - [`core`]:    source card model, config, error, query types.
//! - [`meta`]:    metasearch adapter with vendored search engines.
//! - [`mcp`]:     MCP server (rmcp) exposing `web_search`,
//!   `web_fetch`, and `provider_status` tools.
//!
//! The `mock` feature exposes the test-only mock engine harness used by
//! the integration tests.
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

