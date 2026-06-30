//! MCP tool implementations for the metasearch server.
//!
//! Eight tools are exposed:
//! - `web_search`        — live metasearch.
//! - `web_fetch`         — explicit URL fetch.
//! - `batch_fetch`       — bounded batch fetch over explicit URLs/locators.
//! - `provider_status`   — diagnostic report of configured providers.
//! - `repo_search`       — structured repository evidence discovery.
//! - `repo_fetch`        — structured repository file fetch by locator.
//! - `security_search`   — security vulnerability and advisory search.
//! - `research_search`   — research-oriented multi-source evidence discovery.

use std::sync::Arc;

use crate::core::config::Mode;
use crate::core::provider::ProviderDescriptor;
use crate::core::WebSearchRequest;
use serde::{Deserialize, Serialize};

use crate::fetch::FetchClient;
use crate::mcp::policy::{
    fetch_allowed, live_allowed, web_fetch_denied_message, web_search_denied_message, Policy,
};
use crate::mcp::state::ServerState;

/// Error from a tool call, tagged by whether it reflects bad client
/// input (`Validation`) or a server-side/runtime issue (`Internal`).
#[derive(Debug)]
pub enum ToolError {
    Validation(String),
    Internal(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(msg) | Self::Internal(msg) => write!(f, "{msg}"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebSearchArgs {
    /// Search query string. Must be non-empty.
    pub query: String,
    /// Maximum number of SourceCards to return. If the request exceeds
    /// the server's configured cap, the response includes a warning
    /// and the count is clamped.
    #[serde(default)]
    pub max_results: Option<usize>,
    /// Specific provider IDs to query; empty means "use the server's
    /// configured defaults".
    #[serde(default)]
    pub providers: Vec<String>,
    /// Optional safe-search mode. Reserved for future use; the
    /// current HTML providers do not enforce it. Supplying this
    /// field causes the server to emit an advisory warning on the
    /// response.
    #[serde(default)]
    pub safe_search: Option<crate::core::SafeSearch>,
    /// Optional per-request timeout override in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Search intent hint: `web`, `docs`, `code`, `issues`,
    /// `releases`, `security`, or `news`. Optional; defaults to
    /// `web`. Used as a retrieval and ranking hint only.
    #[serde(default)]
    pub intent: Option<crate::core::query::SearchIntent>,
    /// Freshness hint: `any`, `day`, `week`, `month`, or `year`.
    /// Optional; defaults to `any`. Best-effort; not all providers
    /// support date filtering.
    #[serde(default)]
    pub freshness: Option<crate::core::query::Freshness>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderStatusArgs {
    /// Reserved for future use. The `provider_status` tool currently
    /// reports configuration only; live network probes are not
    /// implemented.
    #[serde(default)]
    pub probe: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoSearchArgs {
    /// Free-text query. May contain repo hints (repo:owner/name, etc.).
    pub query: String,
    /// Optional. Code host to target (github, gitlab, codeberg).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Optional. Repository owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Optional. Repository name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Optional. Organization filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
    /// Optional. Path hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional. File hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Optional. Language filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Optional. Symbol hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Optional. Include official docs results (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_docs: Option<bool>,
    /// Optional. Include package registry results (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_registry: Option<bool>,
    /// Optional. Include issue results (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_issues: Option<bool>,
    /// Optional. Include release results (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_releases: Option<bool>,
    /// Optional. Include example results (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_examples: Option<bool>,
    /// Optional. Include pull request results (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_pull_requests: Option<bool>,
    /// Optional. Maximum total results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    /// Optional. Maximum results per group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per_group: Option<usize>,
    /// Optional. Freshness hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    /// Optional. Per-request timeout override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Optional. Explicit provider ID list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    /// Optional. Search profile for provider selection ("generic",
    /// "coding", "security", "research").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Optional. Package ecosystem ("crates.io", "pypi", "npm").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    /// Optional. Package name for package-aware search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// Optional. Specific package version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Optional. Version requirement for range queries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_requirement: Option<String>,
    /// Optional. Compare version for migration/changelog context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare_version: Option<String>,
    /// Optional. Include security advisory context (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_security_context: Option<bool>,
    /// Optional. Include changelog results (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_changelog: Option<bool>,
    /// Optional. Include migration guide results (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_migration_guides: Option<bool>,
    /// Optional. Include local workspace results when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_local: Option<bool>,
    /// Optional. Search mode. "normal" (default) uses standard repo-search
    /// subqueries. "exact_error" optimizes for compiler/runtime error messages
    /// with phrase-preserving subqueries, error-code extraction, and sensitive
    /// token redaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecuritySearchArgs {
    /// Free-text query. May contain CVE/GHSA/RustSec identifiers.
    pub query: Option<String>,
    /// Package ecosystem (e.g. "crates.io", "npm", "pypi").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    /// Package name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// Version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Explicit CVE ID (e.g. "CVE-2024-12345").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cve_id: Option<String>,
    /// Explicit GHSA ID (e.g. "GHSA-abcd-1234-efgh").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ghsa_id: Option<String>,
    /// Explicit OSV ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osv_id: Option<String>,
    /// Explicit RustSec ID (e.g. "RUSTSEC-2024-0001").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rustsec_id: Option<String>,
    /// Minimum severity level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_min: Option<String>,
    /// Include KEV (Known Exploited Vulnerabilities) data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_kev: Option<bool>,
    /// Include exploit context in results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_exploit_context: Option<bool>,
    /// Include defensive/mitigation guidance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_defensive_guidance: Option<bool>,
    /// Include vendor advisory links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_vendor_advisories: Option<bool>,
    /// Maximum total results to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    /// Maximum results per group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per_group: Option<usize>,
    /// Freshness hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    /// Per-request timeout override in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Explicit provider ID list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSearchArgs {
    /// Free-text research query. Must be non-empty.
    pub query: String,
    /// Optional. Research domain hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_domain: Option<String>,
    /// Optional. Source types to include.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub desired_source_types: Vec<String>,
    /// Optional. Include counterpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_counterpoints: Option<bool>,
    /// Optional. Prioritize primary sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_primary_sources: Option<bool>,
    /// Optional. Include recent discussion and news.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_recent_discussion: Option<bool>,
    /// Optional. Include security considerations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_security_considerations: Option<bool>,
    /// Optional. Maximum total results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    /// Optional. Maximum result groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_groups: Option<usize>,
    /// Optional. Maximum results per group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per_group: Option<usize>,
    /// Optional. Freshness hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    /// Optional. Per-request timeout override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Optional. Explicit provider ID list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    /// Optional. Research workflow type for structured scaffolding.
    /// Values: "general", "architecture_decision", "api_evaluation",
    /// "library_comparison", "migration_planning", "security_review",
    /// "performance_investigation", "ecosystem_survey".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// Optional. Research depth: "quick", "standard", or "deep".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<String>,
    /// Optional. Compare targets for library comparison workflows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compare_targets: Vec<String>,
    /// Optional. Constraints or requirements for the research.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    /// Optional. Known context the caller already has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebFetchArgs {
    /// The URL to fetch. Must be a valid HTTP(S) URL.
    pub url: String,
    /// Maximum characters to extract. Defaults to server config.
    #[serde(default)]
    pub max_chars: Option<usize>,
    /// Timeout in milliseconds. Defaults to server config.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Extraction mode: "text" (default), "markdown", or "metadata_only".
    #[serde(default)]
    pub extract_mode: Option<crate::core::fetch::ExtractMode>,
    /// Whether to include extracted links. Defaults to the server's
    /// `[fetch].include_links_default` config value when omitted.
    #[serde(default)]
    pub include_links: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BatchFetchArgs {
    /// Items to fetch. Must be non-empty.
    pub items: Vec<crate::core::batch_fetch::BatchFetchItem>,
    /// Maximum number of items to process. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// Per-item character extraction cap. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars_per_item: Option<usize>,
    /// Total character budget across all items. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_chars: Option<usize>,
    /// Timeout in milliseconds. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Whether to continue fetching remaining items after a failure.
    /// Defaults to `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_on_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoFetchArgs {
    /// Code host. Optional; accepted values: github, gitlab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Repository owner (or namespace for GitLab nested groups).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Branch, tag, or commit ref. Defaults to "main" when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    /// Full commit SHA for stable permalink construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// File path relative to repository root.
    pub path: String,
    /// First line to return (1-indexed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u32>,
    /// Last line to return (1-indexed, inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
    /// Extra lines of context before line_start. Defaults to 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_before: Option<u32>,
    /// Extra lines of context after line_end. Defaults to 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_after: Option<u32>,
    /// Maximum characters to return. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars: Option<usize>,
    /// Timeout in milliseconds. Defaults to server config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Override URL for the actual fetch (internal/test-only). When
    /// set, this URL is fetched instead of the internally-constructed
    /// raw URL. Hidden from the MCP tool schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub test_fetch_url: Option<String>,
}

/// Run the `web_search` tool against the shared adapter. The response
/// is serialized as JSON and returned to the MCP caller.
pub async fn run_web_search(
    state: Arc<ServerState>,
    args: WebSearchArgs,
) -> Result<serde_json::Value, ToolError> {
    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Internal(web_search_denied_message()));
    }

    let mut req = WebSearchRequest {
        query: args.query.clone(),
        max_results: args.max_results,
        providers: args.providers.clone(),
        safe_search: args.safe_search,
        timeout_ms: args.timeout_ms,
        intent: args.intent.unwrap_or_default(),
        freshness: args.freshness.unwrap_or_default(),
    };

    if let Err(e) = req.validate(state.config.search.max_query_chars) {
        return Err(ToolError::Validation(format!("invalid query: {e}")));
    }

    let effective_providers = state
        .config
        .resolve_providers(&args.providers)
        .map_err(|e| ToolError::Internal(format!("provider resolution failed: {}", e)))?;
    let (_, unknown) = state.adapter.select_engines(&effective_providers);
    if !unknown.is_empty() {
        return Err(ToolError::Validation(format!(
            "unknown provider id(s): {}",
            unknown.join(", ")
        )));
    }

    // Ensure the adapter queries exactly the resolved set, not all
    // enabled engines (which would differ when providers is empty).
    req.providers = effective_providers.clone();

    let resolution = crate::core::query::resolve_max_results(
        req.max_results,
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    let resp = state
        .adapter
        .web_search(
            &req,
            resolution.effective,
            state.config.search.max_results_cap,
        )
        .await;

    let mut warnings: Vec<String> = resp
        .warnings
        .iter()
        .map(|w| format!("[{}] {}", w.provider_id, w.message))
        .collect();

    // Add clamp warning if max_results was capped by the server.
    if let Some(ref w) = resolution.warning {
        warnings.insert(0, w.clone());
    }

    // Per-card prompt-injection marker warnings. These are inserted
    // at the top of the warnings array (before the generic
    // "untrusted external content" warning is inserted at index 0
    // below) so the agent sees them in this order:
    //   0. "Live web results are untrusted external content."
    //   1..N. per-card marker warnings (if any)
    //   N+1... provider-failure warnings
    //   last. safe_search advisory (if applicable)
    let mut marker_warnings: Vec<String> = Vec::new();
    for card in &resp.results {
        if card.trust_markers.injection_hits > 0 {
            marker_warnings.push(format!(
                "possible prompt injection markers detected in card {id}: {n} hit(s)",
                id = card.id,
                n = card.trust_markers.injection_hits,
            ));
        }
    }
    warnings.splice(0..0, marker_warnings);
    warnings.insert(
        0,
        "generic_context_untrusted: Live web results are untrusted external content.".to_string(),
    );

    if args.safe_search.is_some() {
        warnings.push(
            "safe_search_unenforced: safe_search is not enforced by current HTML providers; results may include unexpected content".to_string()
        );
    }

    let providers_failed: Vec<serde_json::Value> = resp
        .providers_failed
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "error_class": f.error_class,
                "message": f.message,
            })
        })
        .collect();

    let payload = serde_json::json!({
        "query": resp.query,
        "mode": resp.mode,
        "results": resp.results,
        "providers_queried": resp.providers_queried,
        "providers_failed": providers_failed,
        "warnings": warnings,
        "trust_markers": serde_json::to_value(&resp.trust_markers)
            .unwrap_or(serde_json::json!({})),
    });

    if providers_failed.len() == effective_providers.len()
        && !effective_providers.is_empty()
        && resp.results.is_empty()
    {
        return Err(ToolError::Internal(format!(
            "all providers failed: {}",
            providers_failed
                .iter()
                .filter_map(|v| v.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    Ok(payload)
}

/// Run the `repo_search` tool.
pub async fn run_repo_search(
    state: Arc<ServerState>,
    args: RepoSearchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::repo_search::RepoSearchRequest;

    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Validation(web_search_denied_message()));
    }

    let host = if let Some(h) = &args.host {
        match h.to_lowercase().as_str() {
            "github" | "gh" => Some(crate::core::code_metadata::CodeHost::Github),
            "gitlab" | "gl" => Some(crate::core::code_metadata::CodeHost::Gitlab),
            "codeberg" | "cb" => Some(crate::core::code_metadata::CodeHost::Codeberg),
            other => {
                return Err(ToolError::Validation(format!(
                    "unknown host '{other}'; accepted values: github (gh), gitlab (gl), codeberg (cb)"
                )));
            }
        }
    } else {
        None
    };

    let freshness = args
        .freshness
        .as_deref()
        .and_then(|f| serde_json::from_value(serde_json::Value::String(f.to_string())).ok())
        .unwrap_or_default();

    let profile = args
        .profile
        .as_deref()
        .and_then(crate::core::repo_search::SearchProfile::parse);

    let mode = args
        .mode
        .as_deref()
        .and_then(crate::core::repo_search::RepoSearchMode::parse);

    let req = RepoSearchRequest {
        query: args.query,
        host,
        owner: args.owner,
        repo: args.repo,
        org: args.org,
        path: args.path,
        file: args.file,
        language: args.language,
        symbol: args.symbol,
        include_docs: args.include_docs,
        include_registry: args.include_registry,
        include_issues: args.include_issues,
        include_releases: args.include_releases,
        include_examples: args.include_examples,
        include_pull_requests: args.include_pull_requests,
        max_results: args.max_results,
        max_per_group: args.max_per_group,
        freshness,
        timeout_ms: args.timeout_ms,
        providers: args.providers.clone(),
        profile,
        ecosystem: args
            .ecosystem
            .as_deref()
            .and_then(crate::core::package::PackageEcosystem::parse),
        package: args.package.clone(),
        version: args.version.clone(),
        version_requirement: args.version_requirement.clone(),
        compare_version: args.compare_version.clone(),
        include_security_context: args.include_security_context,
        include_changelog: args.include_changelog,
        include_migration_guides: args.include_migration_guides,
        include_local: args.include_local,
        mode,
        exact_error_config: Some(state.config.search.exact_error.clone()),
    };

    if let Err(e) = req.validate(state.config.search.max_query_chars) {
        return Err(ToolError::Validation(format!("invalid query: {e}")));
    }

    let (effective_providers, degraded, mut profile_warnings) = state
        .config
        .resolve_profile_providers(req.profile, &req.providers);

    // Explicit providers must be strict: unknown or disabled providers
    // are a hard error, not a degraded fallback.
    if !req.providers.is_empty() && degraded {
        let msg = profile_warnings
            .iter()
            .find(|w| w.message.starts_with("provider_resolution_failed:"))
            .map(|w| w.message.clone())
            .unwrap_or_else(|| "provider resolution failed".to_string());
        return Err(ToolError::Validation(msg));
    }

    // For profile requests (no explicit providers), filter through
    // actual adapter availability. Config-level resolution may list
    // providers that appear enabled but were not actually built
    // (e.g. missing API key env var).
    let mut skipped_provider_ids = Vec::new();
    let mut profile_degraded = false;
    let effective_providers = if let Some(profile) = req.profile {
        if req.providers.is_empty() {
            let built_ids: std::collections::BTreeSet<&str> = state
                .adapter
                .provider_ids()
                .iter()
                .map(|s| s.as_str())
                .collect();
            let mut filtered = Vec::new();
            let mut any_skipped = false;
            for id in &effective_providers {
                if built_ids.contains(id.as_str()) {
                    filtered.push(id.clone());
                } else {
                    skipped_provider_ids.push(id.clone());
                    profile_warnings.push(crate::core::result::SearchWarning::new(
                        "_system",
                        format!("profile_provider_not_built: {id} is in {profile:?} profile but no engine was constructed"),
                    ));
                    any_skipped = true;
                }
            }

            if filtered.is_empty() && !effective_providers.is_empty() {
                // All profile providers were not built — degrade to defaults
                profile_degraded = true;
                profile_warnings.push(crate::core::result::SearchWarning::new(
                    "_system",
                    format!("profile_degraded: {profile:?} profile fell back to default providers"),
                ));
                match state.config.resolve_providers(&[]) {
                    Ok(defaults) => defaults,
                    Err(e) => {
                        return Err(ToolError::Internal(format!(
                            "default provider resolution failed: {e}"
                        )));
                    }
                }
            } else if any_skipped {
                // Some providers were unavailable but others remain
                profile_warnings.push(crate::core::result::SearchWarning::new(
                    "_system",
                    format!("profile_partial: {profile:?} profile skipped unavailable providers"),
                ));
                filtered
            } else {
                filtered
            }
        } else {
            effective_providers
        }
    } else {
        effective_providers
    };

    let (_, unknown) = state.adapter.select_engines(&effective_providers);
    if !unknown.is_empty() {
        return Err(ToolError::Validation(format!(
            "unknown provider id(s): {}",
            unknown.join(", ")
        )));
    }

    let effective_max = req.effective_max_results(
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    let mut req = req;
    req.providers = effective_providers;

    let mut response = state
        .adapter
        .repo_search(
            &req,
            effective_max,
            state.config.search.max_results_cap,
            state.local_backend.as_deref(),
        )
        .await;

    // Merge profile warnings into response warnings
    response.warnings.extend(profile_warnings);

    // Populate telemetry provider selection.
    // `degraded` from config means explicit providers failed.
    // `profile_degraded` means all profile providers were unavailable
    // and execution fell back to default providers.
    // `has_partial_warning` means some providers were skipped but others remain.
    let is_degraded = degraded || profile_degraded;
    let has_partial_warning = response
        .warnings
        .iter()
        .any(|w| w.message.starts_with("profile_partial:"));
    response.telemetry.provider_selection = crate::core::repo_search::ProviderSelectionTelemetry {
        profile_requested: req.profile,
        profile_applied: req.profile,
        degraded: is_degraded,
        partial: has_partial_warning && !is_degraded,
        skipped_providers: skipped_provider_ids,
        reason: if is_degraded {
            Some("profile fell back to default providers".to_string())
        } else if has_partial_warning {
            Some(format!(
                "{:?} profile skipped unavailable providers",
                req.profile.unwrap_or_default()
            ))
        } else {
            req.profile
                .map(|p| format!("using {} profile providers", p.as_str()))
        },
    };

    // Propagate degraded/partial provider selection into uncertainty_summary
    if let Some(ref mut summary) = response.telemetry.uncertainty_summary {
        summary.degraded_provider_selection = is_degraded;
        summary.partial_provider_selection = has_partial_warning && !is_degraded;
    }

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::Internal(format!("serialization error: {e}")))?;

    Ok(value)
}

/// Run the `research_search` tool.
pub async fn run_research_search(
    state: Arc<ServerState>,
    args: ResearchSearchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::research::{
        ResearchDepth, ResearchDomain, ResearchSearchRequest, ResearchSourceType, ResearchWorkflow,
    };

    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Validation(web_search_denied_message()));
    }

    let research_domain =
        args.research_domain
            .as_deref()
            .and_then(|d| match d.to_lowercase().as_str() {
                "general" => Some(ResearchDomain::General),
                "software_architecture" | "architecture" => {
                    Some(ResearchDomain::SoftwareArchitecture)
                }
                "api_design" | "api" => Some(ResearchDomain::ApiDesign),
                "distributed_systems" | "distributed" => Some(ResearchDomain::DistributedSystems),
                "security" => Some(ResearchDomain::Security),
                "performance" => Some(ResearchDomain::Performance),
                "language_ecosystem" | "ecosystem" => Some(ResearchDomain::LanguageEcosystem),
                "machine_learning" | "ml" => Some(ResearchDomain::MachineLearning),
                "infrastructure" | "infra" => Some(ResearchDomain::Infrastructure),
                _ => None,
            });

    let workflow = args
        .workflow
        .as_deref()
        .and_then(|w| match w.to_lowercase().as_str() {
            "general" => Some(ResearchWorkflow::General),
            "architecture_decision" | "architecture" => {
                Some(ResearchWorkflow::ArchitectureDecision)
            }
            "api_evaluation" | "api" => Some(ResearchWorkflow::ApiEvaluation),
            "library_comparison" | "comparison" => Some(ResearchWorkflow::LibraryComparison),
            "migration_planning" | "migration" => Some(ResearchWorkflow::MigrationPlanning),
            "security_review" | "security" => Some(ResearchWorkflow::SecurityReview),
            "performance_investigation" | "performance" => {
                Some(ResearchWorkflow::PerformanceInvestigation)
            }
            "ecosystem_survey" | "ecosystem" => Some(ResearchWorkflow::EcosystemSurvey),
            _ => None,
        });

    let depth = args
        .depth
        .as_deref()
        .and_then(|d| match d.to_lowercase().as_str() {
            "quick" => Some(ResearchDepth::Quick),
            "standard" => Some(ResearchDepth::Standard),
            "deep" => Some(ResearchDepth::Deep),
            _ => None,
        });

    let desired_source_types: Vec<ResearchSourceType> = args
        .desired_source_types
        .iter()
        .filter_map(|s| match s.to_lowercase().as_str() {
            "primary_sources" | "primary" => Some(ResearchSourceType::PrimarySources),
            "official_docs" | "docs" => Some(ResearchSourceType::OfficialDocs),
            "specifications" | "specs" => Some(ResearchSourceType::Specifications),
            "reference_implementations" | "reference" | "implementations" => {
                Some(ResearchSourceType::ReferenceImplementations)
            }
            "design_discussions" | "design" => Some(ResearchSourceType::DesignDiscussions),
            "benchmarks" | "benchmark" => Some(ResearchSourceType::Benchmarks),
            "security_considerations" | "security" => {
                Some(ResearchSourceType::SecurityConsiderations)
            }
            "issue_threads" | "issues" => Some(ResearchSourceType::IssueThreads),
            "release_notes" | "releases" => Some(ResearchSourceType::ReleaseNotes),
            "academic_or_formal_sources" | "academic" | "formal" => {
                Some(ResearchSourceType::AcademicOrFormalSources)
            }
            "recent_news" | "news" => Some(ResearchSourceType::RecentNews),
            "community_discussion" | "community" => Some(ResearchSourceType::CommunityDiscussion),
            "counterpoints" | "counterpoint" => Some(ResearchSourceType::Counterpoints),
            _ => None,
        })
        .collect();

    let freshness = args
        .freshness
        .as_deref()
        .and_then(|f| serde_json::from_value(serde_json::Value::String(f.to_string())).ok())
        .unwrap_or_default();

    let req = ResearchSearchRequest {
        query: args.query,
        research_domain,
        desired_source_types,
        include_counterpoints: args.include_counterpoints,
        include_primary_sources: args.include_primary_sources,
        include_recent_discussion: args.include_recent_discussion,
        include_security_considerations: args.include_security_considerations,
        max_results: args.max_results,
        max_groups: args.max_groups,
        max_per_group: args.max_per_group,
        freshness,
        timeout_ms: args.timeout_ms,
        providers: args.providers,
        workflow,
        depth,
        compare_targets: args.compare_targets,
        constraints: args.constraints,
        known_context: args.known_context,
    };

    if let Err(e) = req.validate(state.config.search.max_query_chars) {
        return Err(ToolError::Validation(format!("invalid request: {e}")));
    }

    let effective_providers = state
        .config
        .resolve_providers(&req.providers)
        .map_err(|e| ToolError::Validation(format!("provider resolution failed: {e}")))?;
    let (_, unknown) = state.adapter.select_engines(&effective_providers);
    if !unknown.is_empty() {
        return Err(ToolError::Validation(format!(
            "unknown provider id(s): {}",
            unknown.join(", ")
        )));
    }

    let effective_max = req.effective_max_results(
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    let mut req = req;
    req.providers = effective_providers;

    let response = state
        .adapter
        .research_search(&req, effective_max, state.config.search.max_results_cap)
        .await;

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::Internal(format!("serialization error: {e}")))?;

    Ok(value)
}

/// Run the `provider_status` tool.
pub fn run_provider_status(
    state: Arc<ServerState>,
    _args: ProviderStatusArgs,
) -> Result<serde_json::Value, String> {
    let mut descriptors: Vec<ProviderDescriptor> = state.adapter.provider_status();

    // Update local_workspace descriptor to reflect actual backend state
    if let Some(desc) = descriptors.iter_mut().find(|d| d.id == "local_workspace") {
        let backend_enabled = state.local_backend.is_some();
        desc.enabled = backend_enabled;
        desc.configured = backend_enabled;
    }

    let local_enabled = state.local_backend.is_some();

    // Build code_hosts summary from provider descriptors
    let code_hosts = build_code_hosts_summary(&descriptors);

    let payload = serde_json::json!({
        "providers": descriptors,
        "code_hosts": code_hosts,
        "mode": mode_str(state.config.search.mode),
        "server_capabilities": {
            "generic_search": true,
            "explicit_fetch": true,
            "repo_search": true,
            "repo_fetch": true,
            "security_search": true,
            "research_search": true,
            "batch_fetch": true,
            "document_fetch": true,
            "pdf_fetch": cfg!(feature = "pdf"),
            "local_workspace": local_enabled,
        },
        "quality_metadata": {
            "enabled": true,
            "per_result": true,
            "group_summary": true,
            "uses_model_judging": false,
        },
        "tool_capabilities": {
            "repo_fetch": {
                "remote_hosts": ["github", "gitlab"],
                "workspace": local_enabled,
                "line_ranges": true,
                "context_lines": true,
                "max_chars_enforced": true,
            },
            "repo_search": {
                "profiles": ["generic", "coding", "security", "research"],
                "package_resolution": ["crates_io", "pypi", "npm"],
                "local_workspace": local_enabled,
                "subquery_telemetry": true,
            },
            "local_workspace": {
                "enabled": local_enabled,
                "symbol_enrichment": "regex_heuristic",
            },
            "batch_fetch": {
                "enabled": state.config.fetch.enabled,
                "max_items_cap": state.config.fetch.batch_max_items_cap,
                "max_total_chars_cap": state.config.fetch.batch_max_total_chars_cap,
                "supports_web": true,
                "supports_repo": true,
                "preserves_item_trust": true,
            },
        },
    });
    Ok(payload)
}

/// Build a `code_hosts` summary grouping providers by host kind.
///
/// Each host kind gets an entry with aggregated capability flags.
fn build_code_hosts_summary(descriptors: &[ProviderDescriptor]) -> Vec<serde_json::Value> {
    struct HostSummary {
        kind: String,
        id: String,
        enabled: bool,
        configured: bool,
        code_search: bool,
        issue_search: bool,
        release_search: bool,
    }

    let mut hosts: std::collections::BTreeMap<String, HostSummary> =
        std::collections::BTreeMap::new();

    for desc in descriptors {
        let kind = match desc.id.as_str() {
            "github_code" | "github_issues" | "github_releases" => "github",
            "gitlab_code" | "gitlab_issues" | "gitlab_releases" => "gitlab",
            "gitea_code" | "gitea_issues" | "gitea_releases" => "gitea",
            _ => continue,
        };

        let host_kind = kind.to_string();
        let entry = hosts
            .entry(host_kind.clone())
            .or_insert_with(|| HostSummary {
                kind: host_kind,
                id: kind.to_string(),
                enabled: false,
                configured: false,
                code_search: false,
                issue_search: false,
                release_search: false,
            });

        entry.enabled = entry.enabled || desc.enabled;
        entry.configured = entry.configured || desc.configured;
        entry.code_search = entry.code_search || desc.capabilities.supports_code_search;
        entry.issue_search = entry.issue_search || desc.capabilities.supports_issue_search;
        entry.release_search = entry.release_search || desc.capabilities.supports_release_search;
    }

    hosts
        .into_values()
        .map(|h| {
            serde_json::json!({
                "kind": h.kind,
                "id": h.id,
                "enabled": h.enabled,
                "configured": h.configured,
                "capabilities": {
                    "code_search": h.code_search,
                    "issue_search": h.issue_search,
                    "release_search": h.release_search,
                },
            })
        })
        .collect()
}

/// Run the `web_fetch` tool.
pub async fn run_web_fetch(
    state: Arc<ServerState>,
    args: WebFetchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::fetch::ExtractMode;

    if matches!(fetch_allowed(state.config.fetch.enabled), Policy::Deny) {
        return Err(ToolError::Internal(web_fetch_denied_message()));
    }

    if args.url.trim().is_empty() {
        return Err(ToolError::Validation("url must not be empty".into()));
    }

    if let Some(0) = args.max_chars {
        return Err(ToolError::Validation("max_chars must be > 0".to_string()));
    }

    let extract_mode = args.extract_mode.unwrap_or(ExtractMode::Text);

    let client: Arc<FetchClient> = state.fetch_client().ok_or_else(|| {
        ToolError::Internal("fetch client unavailable; is [fetch].enabled = true?".to_string())
    })?;

    let include_links = args
        .include_links
        .unwrap_or(state.config.fetch.include_links_default);

    let response = client
        .fetch(&args.url, args.max_chars, extract_mode, include_links)
        .await;

    match response {
        Ok(resp) => {
            // `resp.warnings` already contains, in order: extractor
            // warnings, per-field prompt-injection marker warnings
            // (when sanitize_output is enabled and Tier 3 fires), and
            // the standard "untrusted" warning. Pass them through
            // unchanged; the marker warnings sit visibly between the
            // extractor warnings and the untrusted advisory.
            let payload = serde_json::json!({
                "url": resp.url,
                "final_url": resp.final_url,
                "title": resp.title,
                "description": resp.description,
                "content_type": resp.content_type,
                "status": resp.status,
                "fetched": resp.fetched,
                "truncated": resp.truncated,
                "trust": "external_untrusted",
                "text": resp.text,
                "links": resp.links,
                "links_seen": resp.links_seen,
                "links_truncated": resp.links_truncated,
                "warnings": resp.warnings,
                "trust_markers": serde_json::to_value(&resp.trust_markers)
                    .unwrap_or(serde_json::json!({})),
                "document": resp.document,
                "fetch_transform": resp.fetch_transform,
            });
            Ok(payload)
        }
        Err(e) => Err(ToolError::Internal(format!("{}: {}", e.error_code(), e))),
    }
}

/// Run the `repo_fetch` tool.
pub async fn run_repo_fetch(
    state: Arc<ServerState>,
    args: RepoFetchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::code_evidence::infer_source_role;
    use crate::core::code_metadata::CodeHost;
    use crate::core::fetch::ExtractMode;
    use crate::core::repo_fetch::{
        apply_line_range, github_browser_url, github_permalink_url, github_raw_permalink_url,
        github_raw_url, gitlab_browser_url, gitlab_raw_url, FetchTrust, RepoFetchRequest,
        RepoFetchResponse, RepoLocator,
    };

    // --- workspace:// local file fetch (bypasses fetch policy) ---
    if let Some(ref h) = args.host {
        if h.to_lowercase() == "workspace" {
            return run_workspace_fetch(state, args).await;
        }
    }

    if matches!(fetch_allowed(state.config.fetch.enabled), Policy::Deny) {
        return Err(ToolError::Internal(web_fetch_denied_message()));
    }

    // Parse host.
    let host = if let Some(h) = &args.host {
        match h.to_lowercase().as_str() {
            "github" | "gh" => Some(CodeHost::Github),
            "gitlab" | "gl" => Some(CodeHost::Gitlab),
            other => {
                return Err(ToolError::Validation(format!(
                    "unknown host '{other}'; accepted values: github (gh), gitlab (gl)"
                )));
            }
        }
    } else {
        None
    };

    // Determine effective host: infer from owner/repo if not explicit.
    // For now we require an explicit host or default to GitHub.
    let effective_host = host.unwrap_or(CodeHost::Github);

    let ref_name = args.ref_name.unwrap_or_else(|| "main".to_string());

    // Build and validate the request.
    let req = RepoFetchRequest {
        host: Some(effective_host),
        owner: args.owner.clone(),
        repo: args.repo.clone(),
        ref_name: Some(ref_name.clone()),
        commit_sha: args.commit_sha.clone(),
        path: args.path.clone(),
        line_start: args.line_start,
        line_end: args.line_end,
        context_before: args.context_before,
        context_after: args.context_after,
        max_chars: args.max_chars,
        timeout_ms: args.timeout_ms,
    };

    req.validate(state.config.fetch.max_chars_cap)
        .map_err(ToolError::Validation)?;

    let owner = &req.owner;
    let repo = &req.repo;
    let path = &req.path;
    let rn = req.ref_name.as_deref().unwrap_or("main");

    // Build URLs based on host.
    let (browser_url, raw_url) = match effective_host {
        CodeHost::Github => {
            let browser = github_browser_url(owner, repo, rn, path);
            let raw = github_raw_url(owner, repo, rn, path);
            (browser, raw)
        }
        CodeHost::Gitlab => {
            let browser = gitlab_browser_url(owner, repo, rn, path);
            let raw = gitlab_raw_url(owner, repo, rn, path);
            (browser, raw)
        }
        _ => {
            return Err(ToolError::Validation(format!(
                "host '{effective_host:?}' is not supported for repo_fetch"
            )));
        }
    };

    let permalink_url = req.commit_sha.as_ref().map(|sha| {
        match effective_host {
            CodeHost::Github => github_permalink_url(owner, repo, sha, path),
            CodeHost::Gitlab => {
                // GitLab permalink uses the browser URL pattern with SHA.
                gitlab_browser_url(owner, repo, sha, path)
            }
            _ => raw_url.clone(),
        }
    });

    let raw_permalink_url = req.commit_sha.as_ref().map(|sha| {
        match effective_host {
            CodeHost::Github => github_raw_permalink_url(owner, repo, sha, path),
            CodeHost::Gitlab => {
                // GitLab raw permalink uses the raw URL pattern with SHA.
                gitlab_raw_url(owner, repo, sha, path)
            }
            _ => raw_url.clone(),
        }
    });

    let locator = RepoLocator {
        kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
        host: Some(effective_host),
        owner: Some(owner.to_string()),
        repo: Some(repo.to_string()),
        ref_name: Some(rn.to_string()),
        commit_sha: req.commit_sha.clone(),
        path: path.to_string(),
        workspace_root: None,
    };

    let language = crate::core::code_metadata::language_from_extension(path).map(String::from);
    let source_role = infer_source_role(path);

    let max_chars = req.max_chars;
    let base_client: Arc<FetchClient> = state.fetch_client().ok_or_else(|| {
        ToolError::Internal("fetch client unavailable; is [fetch].enabled = true?".to_string())
    })?;

    // Use per-request timeout override when provided.
    let client: Arc<FetchClient> =
        if let Some(ms) = req.timeout_ms {
            Arc::new(base_client.with_timeout_ms(ms).map_err(|e| {
                ToolError::Internal(format!("failed to create timeout override: {e}"))
            })?)
        } else {
            base_client
        };

    // When commit_sha is provided, prefer the stable raw permalink URL
    // for exact evidence retrieval. Test override always wins.
    let cloned_permalink = raw_permalink_url.clone();
    let canonical_fetch_url = cloned_permalink.as_deref().unwrap_or(&raw_url);
    let fetch_url = args
        .test_fetch_url
        .as_deref()
        .unwrap_or(canonical_fetch_url);

    let response = client
        .fetch(fetch_url, max_chars, ExtractMode::Text, false)
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status;
            let content_type = resp.content_type.clone();
            let truncated = resp.truncated;
            let warnings = resp.warnings.clone();
            let trust_markers = resp.trust_markers.clone();
            let text = resp.text.clone();

            // Parse lines from text for line slicing.
            let all_lines: Vec<String> = text
                .as_deref()
                .unwrap_or("")
                .lines()
                .map(String::from)
                .collect();
            let total_lines = if all_lines.is_empty() {
                None
            } else {
                Some(all_lines.len() as u32)
            };

            // Apply line range.
            let (sliced_lines, returned_start, returned_end, _line_truncated, line_warning) =
                apply_line_range(
                    &all_lines,
                    req.line_start,
                    req.line_end,
                    req.context_before.unwrap_or(0),
                    req.context_after.unwrap_or(0),
                );

            // Build text from sliced lines.
            let sliced_text = if sliced_lines.is_empty() {
                None
            } else {
                let t: String = sliced_lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(t)
            };

            let mut warnings = warnings;
            if let Some(w) = line_warning {
                warnings.push(w);
            }

            let fetch_response = RepoFetchResponse {
                locator,
                fetched: resp.fetched,
                status: Some(status),
                content_type,
                language,
                source_role: Some(source_role),
                browser_url,
                raw_url: raw_url.clone(),
                permalink_url,
                raw_permalink_url,
                fetched_url: Some(fetch_url.to_string()),
                ref_resolved: Some(rn.to_string()),
                line_start: req.line_start,
                line_end: req.line_end,
                returned_line_start: returned_start,
                returned_line_end: returned_end,
                total_lines,
                text: sliced_text,
                lines: sliced_lines,
                document: resp.document,
                truncated,
                warnings,
                trust: FetchTrust::ExternalUntrusted,
                trust_markers,
            };

            let value = serde_json::to_value(&fetch_response)
                .map_err(|e| ToolError::Internal(format!("serialization error: {e}")))?;
            Ok(value)
        }
        Err(e) => Err(ToolError::Internal(format!("{}: {}", e.error_code(), e))),
    }
}

/// Run the `batch_fetch` tool.
pub async fn run_batch_fetch(
    state: Arc<ServerState>,
    args: BatchFetchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::batch_fetch::{
        BatchFetchItem, BatchFetchItemType, BatchFetchResponse, BatchFetchResult,
    };

    // Policy check
    if matches!(fetch_allowed(state.config.fetch.enabled), Policy::Deny) {
        return Err(ToolError::Internal(web_fetch_denied_message()));
    }

    // Validate items non-empty
    if args.items.is_empty() {
        return Err(ToolError::Validation("items must not be empty".to_string()));
    }

    // Resolve effective limits
    let batch_max_items = args
        .max_items
        .unwrap_or(state.config.fetch.batch_max_items)
        .min(state.config.fetch.batch_max_items_cap);
    let per_item_cap = args
        .max_chars_per_item
        .unwrap_or(state.config.fetch.batch_max_chars_per_item);
    let total_cap = args
        .max_total_chars
        .unwrap_or(state.config.fetch.batch_max_total_chars)
        .min(state.config.fetch.batch_max_total_chars_cap);
    let continue_on_error = args.continue_on_error.unwrap_or(true);

    // Validate item count
    if args.items.len() > state.config.fetch.batch_max_items_cap {
        return Err(ToolError::Validation(format!(
            "items count ({}) exceeds batch_max_items_cap ({})",
            args.items.len(),
            state.config.fetch.batch_max_items_cap
        )));
    }

    // Clamp to effective max_items
    let effective_items: Vec<&BatchFetchItem> = args.items.iter().take(batch_max_items).collect();
    let mut warnings = Vec::new();
    if effective_items.len() < args.items.len() {
        warnings.push(format!(
            "batch_item_count_truncated: requested {} items, processing {} (batch_max_items={})",
            args.items.len(),
            effective_items.len(),
            batch_max_items
        ));
    }

    // Pre-validate all items before launching any fetches
    for (i, item) in effective_items.iter().enumerate() {
        match item {
            BatchFetchItem::Web { url, max_chars, .. } => {
                if url.trim().is_empty() {
                    return Err(ToolError::Validation(format!(
                        "item {i}: url must not be empty"
                    )));
                }
                // Validate URL scheme early (http/https only)
                let trimmed = url.trim();
                if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                    return Err(ToolError::Validation(format!(
                        "item {i}: url scheme must be http or https, got: {}",
                        &trimmed[..trimmed.len().min(20)]
                    )));
                }
                if let Some(mc) = max_chars {
                    if *mc == 0 {
                        return Err(ToolError::Validation(format!(
                            "item {i}: max_chars must be > 0"
                        )));
                    }
                }
            }
            BatchFetchItem::Repo {
                owner,
                repo,
                path,
                host,
                max_chars,
                ..
            } => {
                if owner.trim().is_empty() {
                    return Err(ToolError::Validation(format!(
                        "item {i}: owner must not be empty"
                    )));
                }
                if repo.trim().is_empty() {
                    return Err(ToolError::Validation(format!(
                        "item {i}: repo must not be empty"
                    )));
                }
                if path.trim().is_empty() {
                    return Err(ToolError::Validation(format!(
                        "item {i}: path must not be empty"
                    )));
                }
                if path.contains("..") {
                    return Err(ToolError::Validation(format!(
                        "item {i}: path must not contain '..'"
                    )));
                }
                if path.starts_with('/') {
                    return Err(ToolError::Validation(format!(
                        "item {i}: path must not be absolute (starts with '/')"
                    )));
                }
                if let Some(h) = host {
                    match h.to_lowercase().as_str() {
                        "github" | "gh" | "gitlab" | "gl" | "workspace" => {}
                        other => {
                            return Err(ToolError::Validation(format!(
                                "item {i}: unknown host '{other}'; accepted: github (gh), gitlab (gl), workspace"
                            )));
                        }
                    }
                }
                if let Some(mc) = max_chars {
                    if *mc == 0 {
                        return Err(ToolError::Validation(format!(
                            "item {i}: max_chars must be > 0"
                        )));
                    }
                }
            }
        }
    }

    let client: Arc<FetchClient> = state.fetch_client().ok_or_else(|| {
        ToolError::Internal("fetch client unavailable; is [fetch].enabled = true?".to_string())
    })?;

    let concurrency = state.config.fetch.batch_concurrency;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

    // Execute fetches in ordered bounded waves, preserving input order.
    //
    // When continue_on_error=true (default): each wave spawns up to
    // `concurrency` tasks concurrently via JoinSet. Budget tracking
    // and abort checks happen between waves.
    //
    // When continue_on_error=false: effective_concurrency is set to 1
    // so items are fetched one at a time, preserving strict
    // abort-on-first-failure semantics.
    let effective_concurrency = if continue_on_error { concurrency } else { 1 };
    let mut results: Vec<BatchFetchResult> = Vec::with_capacity(effective_items.len());
    let mut total_chars: usize = 0;
    let mut budget_exhausted = false;
    let mut aborted = false;

    for wave_start in (0..effective_items.len()).step_by(effective_concurrency) {
        // Pre-wave checks: skip remaining items if aborted or budget exhausted
        if aborted || budget_exhausted {
            for (i, item) in effective_items.iter().enumerate().skip(wave_start) {
                let msg = if budget_exhausted {
                    "total character budget exhausted".to_string()
                } else {
                    "batch aborted due to previous failure".to_string()
                };
                results.push(BatchFetchResult {
                    index: i,
                    item_type: match item {
                        BatchFetchItem::Web { .. } => BatchFetchItemType::Web,
                        BatchFetchItem::Repo { .. } => BatchFetchItemType::Repo,
                    },
                    label: item.label(),
                    ok: false,
                    response: None,
                    error: Some(msg),
                    chars_returned: 0,
                    truncated: false,
                });
            }
            break;
        }

        let wave_end = (wave_start + effective_concurrency).min(effective_items.len());
        let remaining_budget = total_cap.saturating_sub(total_chars);

        // Divide remaining budget across wave items to prevent concurrent
        // overshoot. Each item gets at most per_wave_item_budget chars.
        let wave_len = wave_end - wave_start;
        let per_wave_item_budget = remaining_budget
            .checked_div(wave_len)
            .unwrap_or(remaining_budget);
        let item_budget_cap = per_item_cap.max(1).min(per_wave_item_budget.max(1));

        let mut join_set = tokio::task::JoinSet::new();
        let mut wave_indices = Vec::new();

        for (i, item) in effective_items
            .iter()
            .enumerate()
            .take(wave_end)
            .skip(wave_start)
        {
            if budget_exhausted {
                results.push(BatchFetchResult {
                    index: i,
                    item_type: match item {
                        BatchFetchItem::Web { .. } => BatchFetchItemType::Web,
                        BatchFetchItem::Repo { .. } => BatchFetchItemType::Repo,
                    },
                    label: item.label(),
                    ok: false,
                    response: None,
                    error: Some("total character budget exhausted".to_string()),
                    chars_returned: 0,
                    truncated: false,
                });
                continue;
            }

            wave_indices.push(i);

            let fetch_future = make_batch_fetch_future(
                i,
                item,
                item_budget_cap,
                state.clone(),
                client.clone(),
                semaphore.clone(),
                item.label(),
                args.timeout_ms,
                state.config.fetch.include_links_default,
            );

            join_set.spawn(fetch_future);
        }

        // Collect all wave results keyed by their returned index.
        // JoinSet::join_next() returns whichever task completes first,
        // so we must not associate results by iteration order.
        let mut wave_results: std::collections::BTreeMap<usize, BatchFetchResult> =
            std::collections::BTreeMap::new();

        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok(Ok(batch_result)) => {
                    wave_results.insert(batch_result.index, batch_result);
                }
                Ok(Err(tool_err)) => {
                    // Tool error without an index — cannot know which item.
                    // This should be rare; make_batch_fetch_future returns
                    // BatchFetchResult for known failures. Record as a
                    // special internal error that will be attached to a
                    // synthesized failure after collection.
                    tracing::warn!("batch_fetch tool error without index: {tool_err}");
                }
                Err(join_err) => {
                    // Task panic/cancellation — index is lost. Will be
                    // synthesized as a failure for missing indices below.
                    tracing::warn!("batch_fetch task panicked: {join_err}");
                }
            }
        }

        // Push results in input order, synthesizing failures for any
        // indices that are missing (panic, cancellation, or tool error).
        for idx in &wave_indices {
            match wave_results.remove(idx) {
                Some(batch_result) => {
                    if !batch_result.ok {
                        aborted = true;
                    }
                    total_chars += batch_result.chars_returned;
                    // The result's index is already correct from the future.
                    // No mutation needed.
                    results.push(batch_result);
                }
                None => {
                    // Index was not returned — task panicked or tool error.
                    aborted = true;
                    let item_type = match &effective_items[*idx] {
                        BatchFetchItem::Web { .. } => BatchFetchItemType::Web,
                        BatchFetchItem::Repo { .. } => BatchFetchItemType::Repo,
                    };
                    results.push(BatchFetchResult {
                        index: *idx,
                        item_type,
                        label: effective_items[*idx].label(),
                        ok: false,
                        response: None,
                        error: Some("task failed or panicked".to_string()),
                        chars_returned: 0,
                        truncated: false,
                    });
                }
            }
        }

        // Check budget after wave completes
        if total_chars >= total_cap {
            budget_exhausted = true;
        }
    }

    if budget_exhausted {
        warnings.push(format!(
            "batch_total_budget_exhausted: total character budget of {total_cap} was reached; remaining items skipped"
        ));
    }

    let fetched = results.iter().filter(|r| r.ok).count();
    let failed = results.iter().filter(|r| !r.ok).count();
    let truncated = results.iter().any(|r| r.truncated);

    let response = BatchFetchResponse {
        fetched,
        failed,
        truncated,
        total_chars_returned: total_chars,
        results,
        warnings,
    };

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::Internal(format!("serialization error: {e}")))?;
    Ok(value)
}

/// Build a boxed future that fetches a single batch item.
///
/// The future acquires a semaphore permit, executes the fetch, and
/// returns a `BatchFetchResult`. Extracted so both concurrent-wave
/// and sequential-mode paths can share the same fetch logic.
#[allow(clippy::too_many_arguments)]
fn make_batch_fetch_future(
    i: usize,
    item: &crate::core::batch_fetch::BatchFetchItem,
    item_max_chars: usize,
    state: Arc<ServerState>,
    client: Arc<FetchClient>,
    semaphore: Arc<tokio::sync::Semaphore>,
    label: String,
    timeout_ms: Option<u64>,
    include_links_default: bool,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<crate::core::batch_fetch::BatchFetchResult, ToolError>,
            > + Send,
    >,
> {
    use crate::core::batch_fetch::{BatchFetchItem, BatchFetchItemType, BatchFetchResult};

    match item {
        BatchFetchItem::Web {
            url,
            extract_mode,
            include_links,
            max_chars,
        } => {
            let effective_max = max_chars.unwrap_or(item_max_chars).min(item_max_chars);
            let em = effective_max.max(1);
            let mode = extract_mode.unwrap_or(crate::core::fetch::ExtractMode::Text);
            let il = include_links.unwrap_or(include_links_default);
            let url = url.clone();
            Box::pin(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|e| ToolError::Internal(format!("semaphore closed: {e}")))?;
                let response = client.fetch(&url, Some(em), mode, il).await;
                match response {
                    Ok(resp) => {
                        let text_len = resp
                            .document
                            .as_ref()
                            .map(|d| d.text_chars_returned)
                            .unwrap_or_else(|| resp.text.as_ref().map(|t| t.len()).unwrap_or(0));
                        let truncated = resp.truncated;
                        let payload = serde_json::json!({
                            "url": resp.url,
                            "final_url": resp.final_url,
                            "title": resp.title,
                            "description": resp.description,
                            "content_type": resp.content_type,
                            "status": resp.status,
                            "fetched": resp.fetched,
                            "truncated": resp.truncated,
                            "trust": "external_untrusted",
                            "text": resp.text,
                            "links": resp.links,
                            "links_seen": resp.links_seen,
                            "links_truncated": resp.links_truncated,
                            "warnings": resp.warnings,
                            "trust_markers": serde_json::to_value(&resp.trust_markers)
                                .unwrap_or(serde_json::json!({})),
                            "document": resp.document,
                            "fetch_transform": resp.fetch_transform,
                        });
                        Ok(BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Web,
                            label,
                            ok: true,
                            response: Some(payload),
                            error: None,
                            chars_returned: text_len,
                            truncated,
                        })
                    }
                    Err(e) => Ok(BatchFetchResult {
                        index: i,
                        item_type: BatchFetchItemType::Web,
                        label,
                        ok: false,
                        response: None,
                        error: Some(format!("{}: {}", e.error_code(), e)),
                        chars_returned: 0,
                        truncated: false,
                    }),
                }
            })
        }
        BatchFetchItem::Repo {
            host,
            owner,
            repo,
            ref_name,
            commit_sha,
            path,
            line_start,
            line_end,
            context_before,
            context_after,
            max_chars,
        } => {
            let effective_max = max_chars.unwrap_or(item_max_chars).min(item_max_chars);
            let repo_args = RepoFetchArgs {
                host: host.clone(),
                owner: owner.clone(),
                repo: repo.clone(),
                ref_name: ref_name.clone(),
                commit_sha: commit_sha.clone(),
                path: path.clone(),
                line_start: *line_start,
                line_end: *line_end,
                context_before: *context_before,
                context_after: *context_after,
                max_chars: Some(effective_max),
                timeout_ms,
                test_fetch_url: None,
            };
            Box::pin(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|e| ToolError::Internal(format!("semaphore closed: {e}")))?;
                match run_repo_fetch(state, repo_args).await {
                    Ok(payload) => {
                        let text_len = payload
                            .get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| s.len())
                            .unwrap_or(0);
                        let truncated = payload
                            .get("truncated")
                            .and_then(|t| t.as_bool())
                            .unwrap_or(false);
                        Ok(BatchFetchResult {
                            index: i,
                            item_type: BatchFetchItemType::Repo,
                            label,
                            ok: true,
                            response: Some(payload),
                            error: None,
                            chars_returned: text_len,
                            truncated,
                        })
                    }
                    Err(e) => Ok(BatchFetchResult {
                        index: i,
                        item_type: BatchFetchItemType::Repo,
                        label,
                        ok: false,
                        response: None,
                        error: Some(e.to_string()),
                        chars_returned: 0,
                        truncated: false,
                    }),
                }
            })
        }
    }
}

/// Handle `repo_fetch` for `workspace://` local files.
///
/// When `host = "workspace"`, `owner` is the root name and `repo` is
/// the root-relative file path. The file is read directly from the
/// local filesystem via the workspace backend.
async fn run_workspace_fetch(
    state: Arc<ServerState>,
    args: RepoFetchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::code_evidence::infer_source_role;
    use crate::core::repo_fetch::{
        apply_line_range, clamp_lines_to_max_chars, FetchTrust, RepoFetchResponse,
    };
    use crate::core::sanitize::TrustMarkers;

    let backend = state.local_backend.as_ref().ok_or_else(|| {
        ToolError::Validation("local workspace search is not enabled".to_string())
    })?;

    if !backend.is_enabled() {
        return Err(ToolError::Validation(
            "local workspace search is not enabled".to_string(),
        ));
    }

    let root_name = args.owner.clone();
    let relative_path = args.repo.clone();

    if relative_path.trim().is_empty() {
        return Err(ToolError::Validation(
            "repo (file path) must not be empty".to_string(),
        ));
    }
    if relative_path.contains("..") {
        return Err(ToolError::Validation(
            "path must not contain '..' (path traversal)".to_string(),
        ));
    }

    // Find the root by name
    let roots = backend.roots();
    let root_entry = roots.iter().find(|(_, p)| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == root_name)
            .unwrap_or(false)
    });

    let (_, root_path) = root_entry.ok_or_else(|| {
        ToolError::Validation(format!(
            "unknown workspace root '{root_name}'; available roots: {}",
            roots
                .iter()
                .filter_map(|(_, p)| p.file_name().and_then(|n| n.to_str()))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    let file_path = root_path.join(&relative_path);
    if !file_path.is_file() {
        return Err(ToolError::Validation(format!(
            "file not found: {relative_path}"
        )));
    }

    // Validate path is still under the root (defense in depth)
    let canonical = file_path
        .canonicalize()
        .map_err(|e| ToolError::Internal(format!("failed to canonicalize path: {e}")))?;
    if !canonical.starts_with(root_path) {
        return Err(ToolError::Validation(
            "path escapes workspace root".to_string(),
        ));
    }

    // Validate line range
    if let (Some(start), Some(end)) = (args.line_start, args.line_end) {
        if start > end {
            return Err(ToolError::Validation(format!(
                "line_start ({start}) must be <= line_end ({end})"
            )));
        }
    }

    // Read file content
    let content = std::fs::read_to_string(&canonical)
        .map_err(|e| ToolError::Internal(format!("failed to read file: {e}")))?;

    let all_lines: Vec<String> = content.lines().map(String::from).collect();
    let total_lines = if all_lines.is_empty() {
        None
    } else {
        Some(all_lines.len() as u32)
    };

    // Apply line range
    let (sliced_lines, returned_start, returned_end, _line_truncated, line_warning) =
        apply_line_range(
            &all_lines,
            args.line_start,
            args.line_end,
            args.context_before.unwrap_or(0),
            args.context_after.unwrap_or(0),
        );

    // Build text from sliced lines, enforcing max_chars budget
    let (mut clamped_lines, _initial_text, char_truncated) =
        clamp_lines_to_max_chars(&sliced_lines, args.max_chars);

    let mut warnings: Vec<String> = Vec::new();
    if let Some(w) = line_warning {
        warnings.push(w);
    }
    if char_truncated {
        warnings.push("workspace_fetch_truncated_by_max_chars".to_string());
    }

    let language =
        crate::core::code_metadata::language_from_extension(&relative_path).map(String::from);
    let source_role = infer_source_role(&relative_path);

    let pseudo_url = format!("workspace://{root_name}/{relative_path}");

    let locator = crate::core::repo_fetch::RepoLocator {
        kind: crate::core::repo_fetch::RepoLocatorKind::Workspace,
        host: None,
        owner: None,
        repo: None,
        ref_name: None,
        commit_sha: None,
        path: relative_path.clone(),
        workspace_root: Some(root_name.clone()),
    };

    let truncated = char_truncated;

    // Apply local trust-marker scanning: strip control chars and
    // scan for injection markers. Do NOT frame local source code
    // (no <<<EXTERNAL_UNTRUSTED>>> wrappers) — source lines must
    // remain intact for agent copy-paste.
    let mut trust_markers = TrustMarkers::default();
    // Strip control chars from individual lines
    let mut total_control_removed = 0usize;
    for line in &mut clamped_lines {
        let (cleaned, removed) = crate::core::sanitize::strip_control_chars(&line.text);
        total_control_removed += removed;
        line.text = cleaned;
    }
    trust_markers.control_chars_removed = total_control_removed;
    if total_control_removed > 0 {
        trust_markers.text_sanitized = true;
    }
    // Rebuild text from cleaned lines
    let sliced_text = if clamped_lines.is_empty() {
        None
    } else {
        Some(
            clamped_lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    };
    // Scan for injection markers in the full text
    if state.config.fetch.sanitize_output {
        if let Some(ref text) = sliced_text {
            let hits = crate::core::sanitize::scan_injection_markers(text);
            trust_markers.injection_hits = hits.len();
            if !hits.is_empty() {
                warnings.push(format!(
                    "local_content_marker_warning: possible prompt injection \
                     markers detected in local workspace content ({} hits)",
                    hits.len()
                ));
            }
        }
    }

    let fetch_response = RepoFetchResponse {
        locator,
        fetched: true,
        status: None,
        content_type: None,
        language,
        source_role: Some(source_role),
        browser_url: pseudo_url.clone(),
        raw_url: pseudo_url.clone(),
        permalink_url: None,
        raw_permalink_url: None,
        fetched_url: None,
        ref_resolved: None,
        line_start: args.line_start,
        line_end: args.line_end,
        returned_line_start: returned_start,
        returned_line_end: returned_end,
        total_lines,
        text: sliced_text,
        lines: clamped_lines,
        document: None,
        truncated,
        warnings,
        trust: FetchTrust::LocalTrusted,
        trust_markers,
    };

    let value = serde_json::to_value(&fetch_response)
        .map_err(|e| ToolError::Internal(format!("serialization error: {e}")))?;
    Ok(value)
}

/// Run the `security_search` tool.
pub async fn run_security_search(
    state: Arc<ServerState>,
    args: SecuritySearchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::SecuritySearchRequest;

    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Internal(web_search_denied_message()));
    }

    let query = args.query.unwrap_or_default();

    let severity_min = args
        .severity_min
        .as_deref()
        .map(crate::core::SeverityLevel::from_str_loose);

    let freshness = args
        .freshness
        .as_deref()
        .and_then(|f| serde_json::from_value(serde_json::Value::String(f.to_string())).ok())
        .unwrap_or_default();

    let req = SecuritySearchRequest {
        query: query.clone(),
        ecosystem: args.ecosystem.clone(),
        package: args.package.clone(),
        version: args.version.clone(),
        cve_id: args.cve_id.clone(),
        ghsa_id: args.ghsa_id.clone(),
        osv_id: args.osv_id.clone(),
        rustsec_id: args.rustsec_id.clone(),
        severity_min,
        include_kev: args.include_kev,
        include_exploit_context: args.include_exploit_context,
        include_defensive_guidance: args.include_defensive_guidance,
        include_vendor_advisories: args.include_vendor_advisories,
        max_results: args.max_results,
        max_per_group: args.max_per_group,
        freshness,
        timeout_ms: args.timeout_ms,
        providers: args.providers.clone(),
    };

    if let Err(e) = req.validate(state.config.search.max_query_chars) {
        return Err(ToolError::Validation(format!("invalid request: {e}")));
    }

    let effective_providers = state
        .config
        .resolve_providers(&req.providers)
        .map_err(|e| ToolError::Internal(format!("provider resolution failed: {}", e)))?;
    let (_, unknown) = state.adapter.select_engines(&effective_providers);
    if !unknown.is_empty() {
        return Err(ToolError::Validation(format!(
            "unknown provider id(s): {}",
            unknown.join(", ")
        )));
    }

    let effective_max = req.effective_max_results(
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    let response = crate::meta::security_search::run_security_search_plan(
        &state.adapter,
        &state.kev_client,
        &req,
        effective_max,
        state.config.search.max_results_cap,
    )
    .await;

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::Internal(format!("serialization error: {e}")))?;

    Ok(value)
}

fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "off",
        Mode::Live => "live",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::AppConfig;
    use crate::core::sanitize::TrustMarkers;
    use crate::mcp::state::ServerState;
    use std::sync::Arc;

    #[tokio::test]
    async fn safe_search_warning_emitted_when_requested() {
        let cfg = AppConfig::default();
        let state = Arc::new(ServerState::build(cfg).unwrap());

        let args = WebSearchArgs {
            query: "test query".to_string(),
            max_results: Some(5),
            providers: vec![],
            safe_search: Some(crate::core::SafeSearch::Strict),
            timeout_ms: None,
            intent: None,
            freshness: None,
        };

        let result = run_web_search(state, args).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        let warnings = value.get("warnings").unwrap().as_array().unwrap();
        assert!(warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("safe_search_unenforced")));
    }

    #[tokio::test]
    async fn web_search_payload_includes_top_level_trust_markers() {
        let cfg = AppConfig::default();
        let state = Arc::new(ServerState::build(cfg).unwrap());

        let args = WebSearchArgs {
            query: "test".to_string(),
            max_results: Some(3),
            providers: vec![],
            safe_search: None,
            timeout_ms: None,
            intent: None,
            freshness: None,
        };

        let result = run_web_search(state, args).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        // The payload must include a top-level `trust_markers` object.
        let markers = value
            .get("trust_markers")
            .expect("trust_markers should be on payload");
        // It must deserialize back to TrustMarkers (or at least
        // expose the documented boolean/numeric fields).
        assert!(markers.get("text_sanitized").is_some());
        assert!(markers.get("text_truncated").is_some());
        assert!(markers.get("text_framed").is_some());
        assert!(markers.get("control_chars_removed").is_some());
        assert!(markers.get("injection_hits").is_some());
    }

    #[test]
    fn trust_markers_payload_shape_matches_struct() {
        // Sanity: the JSON we emit for `trust_markers` is the same
        // shape as the TrustMarkers struct, so a host agent can
        // deserialize it.
        let m = TrustMarkers {
            text_sanitized: true,
            text_truncated: false,
            text_framed: true,
            control_chars_removed: 3,
            injection_hits: 2,
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["text_sanitized"], serde_json::json!(true));
        assert_eq!(v["text_truncated"], serde_json::json!(false));
        assert_eq!(v["text_framed"], serde_json::json!(true));
        assert_eq!(v["control_chars_removed"], serde_json::json!(3));
        assert_eq!(v["injection_hits"], serde_json::json!(2));
    }

    #[tokio::test]
    async fn repo_search_host_github_accepted() {
        let cfg = AppConfig::default();
        let state = Arc::new(ServerState::build(cfg).unwrap());

        let args = RepoSearchArgs {
            query: "repo:tokio-rs/axum".to_string(),
            host: Some("github".to_string()),
            ..Default::default()
        };

        let result = run_repo_search(state, args).await;
        // Should not fail with a validation error about the host.
        // It may fail for other reasons (e.g. no providers), but
        // the host itself is valid.
        match &result {
            Err(ToolError::Validation(msg)) if msg.contains("unknown host") => {
                panic!("github host should be accepted, got: {msg}");
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn repo_search_host_gh_alias_accepted() {
        let cfg = AppConfig::default();
        let state = Arc::new(ServerState::build(cfg).unwrap());

        let args = RepoSearchArgs {
            query: "repo:tokio-rs/axum".to_string(),
            host: Some("gh".to_string()),
            ..Default::default()
        };

        let result = run_repo_search(state, args).await;
        match &result {
            Err(ToolError::Validation(msg)) if msg.contains("unknown host") => {
                panic!("gh alias should be accepted, got: {msg}");
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn repo_search_host_unknown_rejected() {
        let cfg = AppConfig::default();
        let state = Arc::new(ServerState::build(cfg).unwrap());

        let args = RepoSearchArgs {
            query: "some query".to_string(),
            host: Some("unknownhost".to_string()),
            ..Default::default()
        };

        let result = run_repo_search(state, args).await;
        match result {
            Err(ToolError::Validation(msg)) => {
                assert!(
                    msg.contains("unknown host 'unknownhost'"),
                    "unexpected validation message: {msg}"
                );
            }
            other => panic!("expected validation error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn repo_search_host_none_accepted() {
        let cfg = AppConfig::default();
        let state = Arc::new(ServerState::build(cfg).unwrap());

        let args = RepoSearchArgs {
            query: "some query".to_string(),
            host: None,
            ..Default::default()
        };

        let result = run_repo_search(state, args).await;
        // host=None should not produce a validation error about host.
        match &result {
            Err(ToolError::Validation(msg)) if msg.contains("unknown host") => {
                panic!("None host should be accepted, got: {msg}");
            }
            _ => {}
        }
    }
}
