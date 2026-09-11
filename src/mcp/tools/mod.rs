//! MCP tool implementations, split by behavior.
//!
//! Each tool handler lives in its own module. Shared validation,
//! error, browser, and selection helpers live in `common`.
//! The stable `crate::mcp::tools::X` paths are preserved via re-exports.

mod batch_fetch;
pub mod canonical;
mod common;
mod evidence_bundle;
mod provider_status;
mod repo_fetch;
mod repo_map;
mod repo_search;
mod research_search;
mod security_search;
mod web_fetch;
mod web_search;

pub use batch_fetch::{run_batch_fetch, BatchFetchArgs};
pub use common::ToolError;
pub use evidence_bundle::{run_build_evidence_bundle, EvidenceBundleArgs};
pub use provider_status::{run_provider_status, run_provider_status_async, ProviderStatusArgs};
pub use repo_fetch::{run_repo_fetch, RepoFetchArgs};
pub use repo_map::{run_repo_map, RepoMapArgs};
pub use repo_search::{run_repo_search, RepoSearchArgs};
pub use research_search::{run_research_search, ResearchSearchArgs};
pub use security_search::{run_security_search, SecuritySearchArgs};
pub use web_fetch::{run_web_fetch, WebFetchArgs};
pub use web_search::{run_web_search, WebSearchArgs};

#[cfg(test)]
mod tests;
