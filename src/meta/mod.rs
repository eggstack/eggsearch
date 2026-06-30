//! Metasearch adapter wrapping vendored search engines.
//!
//! This module exposes a thin boundary around the vendored search engine
//! implementations. It does not leak engine types beyond the
//! `MetadataSearchAdapter` and the small response/payload types defined
//! here. Callers receive `crate::core::SourceCard` values.

pub mod adapter;
/// Bounded parallel dispatch for multi-subquery searches.
pub(crate) mod dispatch;
/// Deterministic ranking pipeline for suggested fetch candidates.
pub mod fetch_ranking;
/// Vendored HTML search engines. Internal; not part of the stable
/// public API.
pub mod engines;
/// Exact-error planner: generates targeted subqueries for compiler/runtime error messages.
pub mod error_planner;
mod grouping;
/// Local workspace search backend: bounded file walking, scoring, and
/// SourceCard conversion.
pub mod local_backend;
/// Local repository inventory: Git worktree discovery, remote URL
/// normalization, identity matching, and manifest detection.
pub mod local_inventory;
#[cfg(feature = "mock")]
pub mod mock;
/// Package registry resolver: bounded HTTP lookups for crates.io, PyPI, npm.
pub mod package_resolver;
pub mod planner;
pub mod repo_grouping;
pub mod repo_mapper;
pub mod repo_planner;
pub mod research_grouping;
pub mod research_planner;
pub mod research_suggested_fetches;
pub mod research_workflow;
pub mod response;
pub mod security_grouping;
pub mod security_search;
pub mod security_suggested_fetches;
pub mod suggested_fetches;

pub use adapter::{ErrorClass, MetadataSearchAdapter};
pub use planner::{build_search_plan, SearchPlan};
pub use repo_grouping::{classify_group, group_results};
pub use repo_planner::{
    build_repo_search_plan, build_repo_search_plan_with_package, RepoSearchPlan, RepoSubquery,
};
pub use research_planner::{build_research_search_plan, ResearchSearchPlan};
pub use response::{ProviderFailure, WebSearchResponse};
