//! MCP tool implementations, split by behavior.
//!
//! Each tool handler lives in its own module. Shared validation,
//! error, browser, and selection helpers live in `common`.
//! The stable `crate::mcp::tools::X` paths are preserved via re-exports.

mod batch_fetch;
mod batch_fetch_budget;
mod batch_fetch_item;
pub mod canonical;
mod common;
mod evidence_bundle;
mod fetch;
mod knowledge_brief;
mod provider_status;
mod repo_fetch;
mod repo_map;
mod repo_search;
mod research_search;
mod security_search;
mod web_fetch;
#[cfg(feature = "browser")]
mod web_fetch_escalation;
mod web_fetch_options;
mod web_fetch_window;
mod web_search;

pub(crate) use batch_fetch::run_batch_fetch_with_host_scope_and_progress;
pub use batch_fetch::{run_batch_fetch, run_batch_fetch_with_progress, BatchFetchArgs};
pub(crate) use common::{map_tool_outcome, ToolOutcome};
pub use common::{map_tool_result, RepairHint, ToolError, ToolErrorCode, ToolErrorExecution};
pub use evidence_bundle::{run_build_evidence_bundle, EvidenceBundleArgs};
pub(crate) use fetch::{run_fetch, run_fetch_with_host_scope_and_progress, FetchArgs};
pub use knowledge_brief::{run_knowledge_brief, KnowledgeBriefArgs};
pub use provider_status::{run_provider_status, run_provider_status_async, ProviderStatusArgs};
pub use repo_fetch::{run_repo_fetch, run_repo_fetch_with_host_scope, RepoFetchArgs};
pub use repo_map::{run_repo_map, run_repo_map_with_host_scope, RepoMapArgs};
pub use repo_search::{run_repo_search, run_repo_search_with_host_scope, RepoSearchArgs};
pub use research_search::{
    run_research_search, run_research_search_with_progress, ResearchSearchArgs,
};
pub use security_search::{run_security_search, SecuritySearchArgs};
pub(crate) use web_fetch::run_web_fetch_outcome_with_progress;
pub use web_fetch::{run_web_fetch, run_web_fetch_with_progress, WebFetchArgs};
pub use web_search::{run_web_search, WebSearchArgs};

/// Compact fields retained in an advertised tool schema.
pub fn compact_fields(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "web_search" => web_search::COMPACT_FIELDS,
        "provider_status" => provider_status::COMPACT_FIELDS,
        "repo_search" => repo_search::COMPACT_FIELDS,
        "fetch" => fetch::COMPACT_FIELDS,
        "security_search" => security_search::COMPACT_FIELDS,
        "research_search" => research_search::COMPACT_FIELDS,
        "repo_map" => repo_map::COMPACT_FIELDS,
        "build_evidence_bundle" => evidence_bundle::COMPACT_FIELDS,
        "knowledge_brief" => knowledge_brief::COMPACT_FIELDS,
        _ => return None,
    })
}

#[cfg(all(test, feature = "mock", feature = "browser"))]
mod browser_availability_tests;

#[cfg(test)]
mod tests;
