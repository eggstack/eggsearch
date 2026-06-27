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
pub mod response;
pub mod suggested_fetches;

pub use adapter::{ErrorClass, MetadataSearchAdapter};
pub use planner::{build_search_plan, SearchPlan};
pub use repo_grouping::{classify_group, group_results};
pub use repo_planner::{build_repo_search_plan, RepoSearchPlan, RepoSubquery};
pub use response::{ProviderFailure, WebSearchResponse};
