//! Metasearch adapter wrapping vendored search engines.
//!
//! This module exposes a thin boundary around the vendored search engine
//! implementations. It does not leak engine types beyond the
//! `MetadataSearchAdapter` and the small response/payload types defined
//! here. Callers receive `crate::core::SourceCard` values.

pub mod adapter;
/// Vendored HTML search engines. Internal; not part of the stable
/// public API.
pub mod engines;
#[cfg(feature = "mock")]
pub mod mock;
pub mod planner;
pub mod repo_grouping;
pub mod repo_planner;
pub mod research_grouping;
pub mod research_planner;
pub mod research_suggested_fetches;
pub mod response;
pub mod security_grouping;
pub mod security_suggested_fetches;
pub mod suggested_fetches;

pub use adapter::{ErrorClass, MetadataSearchAdapter};
pub use planner::{build_search_plan, SearchPlan};
pub use repo_grouping::{classify_group, group_results};
pub use repo_planner::{build_repo_search_plan, RepoSearchPlan, RepoSubquery};
pub use research_planner::{build_research_search_plan, ResearchSearchPlan};
pub use response::{ProviderFailure, WebSearchResponse};
