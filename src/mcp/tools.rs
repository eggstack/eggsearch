//! MCP tool implementations for the metasearch server.
//!
//! Six tools are exposed:
//! - `web_search`        — live metasearch.
//! - `web_fetch`         — explicit URL fetch.
//! - `provider_status`   — diagnostic report of configured providers.
//! - `repo_search`       — structured repository evidence discovery.
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
    /// Whether to include full file metadata. Defaults to true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_full_file_metadata: Option<bool>,
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
        providers: args.providers,
    };

    if let Err(e) = req.validate(state.config.search.max_query_chars) {
        return Err(ToolError::Validation(format!("invalid query: {e}")));
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
        .repo_search(&req, effective_max, state.config.search.max_results_cap)
        .await;

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::Internal(format!("serialization error: {e}")))?;

    Ok(value)
}

/// Run the `research_search` tool.
pub async fn run_research_search(
    state: Arc<ServerState>,
    args: ResearchSearchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::research::{ResearchDomain, ResearchSearchRequest, ResearchSourceType};

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
    let descriptors: Vec<ProviderDescriptor> = state.adapter.provider_status();
    let payload = serde_json::json!({
        "providers": descriptors,
        "mode": mode_str(state.config.search.mode),
        "server_capabilities": {
            "generic_search": true,
            "explicit_fetch": true,
            "repo_search": true,
            "repo_fetch": true,
            "security_search": true,
            "research_search": true,
            "document_fetch": true,
            "pdf_fetch": cfg!(feature = "pdf"),
        },
    });
    Ok(payload)
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
        apply_line_range, github_browser_url, github_permalink_url, github_raw_url,
        gitlab_browser_url, gitlab_raw_url, FetchTrust, RepoFetchResponse, RepoFetchRequest,
        RepoLocator,
    };

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
        include_full_file_metadata: args.include_full_file_metadata,
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
                // GitLab permalink uses the same raw URL pattern with SHA.
                gitlab_raw_url(owner, repo, sha, path)
            }
            _ => raw_url.clone(),
        }
    });

    let locator = RepoLocator {
        host: effective_host,
        owner: owner.to_string(),
        repo: repo.to_string(),
        ref_name: rn.to_string(),
        commit_sha: req.commit_sha.clone(),
        path: path.to_string(),
    };

    let language = crate::core::code_metadata::language_from_extension(path)
        .map(String::from);
    let source_role = infer_source_role(path);

    let max_chars = req.max_chars;
    let client: Arc<FetchClient> = state.fetch_client().ok_or_else(|| {
        ToolError::Internal("fetch client unavailable; is [fetch].enabled = true?".to_string())
    })?;

    // Fetch the raw URL.
    let response = client
        .fetch(&raw_url, max_chars, ExtractMode::Text, false)
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
            let (sliced_lines, returned_start, returned_end, _line_truncated) = apply_line_range(
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
                content_sha256: None,
                ref_resolved: Some(rn.to_string()),
                line_start: req.line_start,
                line_end: req.line_end,
                returned_line_start: returned_start,
                returned_line_end: returned_end,
                total_lines,
                text: sliced_text,
                lines: sliced_lines,
                document: None,
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
