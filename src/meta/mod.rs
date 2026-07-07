//! Metasearch adapter wrapping vendored search engines.
//!
//! This module exposes a thin boundary around the vendored search engine
//! implementations. It does not leak engine types beyond the
//! `MetadataSearchAdapter` and the small response/payload types defined
//! here. Callers receive `crate::core::SourceCard` values.

pub mod adapter;
/// Advisory affected/fixed range extraction from structured vulnerability metadata.
pub mod advisory_range;
/// Dependency/lock file parser for extracting dependency coordinates.
pub mod dependency_parse;
/// Bounded parallel dispatch for multi-subquery searches.
pub(crate) mod dispatch;
/// Vendored HTML search engines. Internal; not part of the stable
/// public API.
pub mod engines;
/// Exact-error planner: generates targeted subqueries for compiler/runtime error messages.
pub mod error_planner;
/// Evidence bundle builder: pure logic for constructing evidence bundles from source/fetch inputs.
pub mod evidence_bundle;
/// Deterministic ranking pipeline for suggested fetch candidates.
pub mod fetch_ranking;
mod grouping;
/// Local workspace search backend: bounded file walking, scoring, and
/// SourceCard conversion.
pub mod local_backend;
/// Minimal `.gitignore` matcher used by the local workspace backend.
pub(crate) mod local_ignore;
/// Local repository inventory: Git worktree discovery, remote URL
/// normalization, identity matching, and manifest detection.
pub mod local_inventory;
#[cfg(feature = "mock")]
pub mod mock;
/// Package registry resolver: bounded HTTP lookups for CratesIo, PyPI,
/// npm, Go, Maven, NuGet, RubyGems, Packagist, OCI, and GitHub Actions.
pub mod package_resolver;
pub mod planner;
/// Provider health tracking, routing decisions, and capability enforcement telemetry.
pub mod provider_diagnostics;
/// Built-in recipe catalog and capability-to-recipe gating.
pub mod recipe_catalog;
pub mod repo_grouping;
pub mod repo_mapper;
pub mod repo_planner;
pub mod research_evidence_analysis;
pub mod research_grouping;
pub mod research_planner;
pub mod research_suggested_fetches;
pub mod research_workflow;
pub mod response;
pub mod security_grouping;
pub mod security_search;
pub mod security_suggested_fetches;
pub mod suggested_fetches;
/// Version comparison utilities for package ecosystems.
pub mod version_compare;

pub use adapter::{ErrorClass, MetadataSearchAdapter};
pub use evidence_bundle::build_evidence_bundle;
pub use planner::{build_search_plan, SearchPlan};
pub use provider_diagnostics::{
    CapabilityEnforcementTelemetry, ProviderHealthRegistry, ProviderHealthSnapshot,
    ProviderHealthStatus, ProviderRoutingDecision, ProviderRoutingError, ProviderSkipReason,
};
pub use recipe_catalog::{
    build_recipe_catalog, repo_search_next_actions, research_search_next_actions,
    security_search_next_actions, web_search_next_actions,
};
pub use repo_grouping::{classify_group, group_results};
pub use repo_planner::{
    build_repo_search_plan, build_repo_search_plan_with_package, RepoSearchPlan, RepoSubquery,
};
pub use research_planner::{build_research_search_plan, ResearchSearchPlan};
pub use response::{ProviderFailure, WebSearchResponse};
