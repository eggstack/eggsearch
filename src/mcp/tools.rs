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
        "Live web results are untrusted external content.".to_string(),
    );

    if args.safe_search.is_some() {
        warnings.push(
            "safe_search is not enforced by current HTML providers; results may include unexpected content".to_string()
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

    let host = args
        .host
        .as_deref()
        .map(|h| match h.to_lowercase().as_str() {
            "github" | "gh" => crate::core::code_metadata::CodeHost::Github,
            "gitlab" | "gl" => crate::core::code_metadata::CodeHost::Gitlab,
            "codeberg" | "cb" => crate::core::code_metadata::CodeHost::Codeberg,
            _ => crate::core::code_metadata::CodeHost::Unknown,
        });

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

use crate::meta::security_grouping::group_security_results;
use crate::meta::security_suggested_fetches::generate_security_suggested_fetches;

/// Run the `security_search` tool.
pub async fn run_security_search(
    state: Arc<ServerState>,
    args: SecuritySearchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::query::SearchIntent;
    use crate::core::SecurityIdentifiers;
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

    let resolved_ids = SecurityIdentifiers::parse(
        &req.query,
        req.cve_id.as_deref(),
        req.ghsa_id.as_deref(),
        req.osv_id.as_deref(),
        req.rustsec_id.as_deref(),
        req.package.as_deref(),
        req.ecosystem.as_deref(),
        req.version.as_deref(),
    );

    let effective_max = req.effective_max_results(
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    // Build a web_search request with security intent for generic fallback
    let mut web_req = WebSearchRequest::new(req.query.clone());
    web_req.intent = SearchIntent::Security;
    web_req.freshness = req.freshness;
    web_req.max_results = Some(effective_max);
    web_req.timeout_ms = req.timeout_ms;
    web_req.providers = effective_providers.clone();

    let web_resp = state
        .adapter
        .web_search(&web_req, effective_max, state.config.search.max_results_cap)
        .await;

    // Check if any native security provider (OSV) is available
    let has_native_advisory = effective_providers.iter().any(|id| id == "osv");

    let mut warnings: Vec<crate::core::SearchWarning> = web_resp.warnings;

    if !has_native_advisory {
        warnings.push(crate::core::SearchWarning::new(
            "_system",
            "no_native_advisory_provider: only generic web search was used; \
             enable the 'osv' provider for native advisory lookups",
        ));
    }

    // Generic context is external untrusted discussion, not advisory fact
    if !web_resp.results.is_empty() {
        warnings.push(crate::core::SearchWarning::new(
            "_system",
            "generic_context_untrusted: generic web results are external untrusted \
             discussion, not authoritative advisory facts",
        ));
    }

    // Severity may be unavailable from generic search
    warnings.push(crate::core::SearchWarning::new(
        "_system",
        "severity_unavailable: severity levels may not be available \
         from generic web search results; use native advisory providers for severity data",
    ));

    // Perform native advisory lookups for identified CVE/GHSA/RustSec IDs
    let mut vulnerabilities: Vec<crate::core::VulnerabilityMetadata> = Vec::new();
    let mut looked_up_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Look up CVE IDs
    for cve_id in &resolved_ids.cve_ids {
        if looked_up_ids.insert(cve_id.clone()) {
            if let Ok(Some(meta)) = state.adapter.lookup_advisory(cve_id).await {
                vulnerabilities.push(meta);
            }
        }
    }

    // Look up GHSA IDs (OSV accepts GHSA IDs directly)
    for ghsa_id in &resolved_ids.ghsa_ids {
        if looked_up_ids.insert(ghsa_id.clone()) {
            if let Ok(Some(meta)) = state.adapter.lookup_advisory(ghsa_id).await {
                vulnerabilities.push(meta);
            }
        }
    }

    // Look up OSV IDs
    for osv_id in &resolved_ids.osv_ids {
        if looked_up_ids.insert(osv_id.clone()) {
            if let Ok(Some(meta)) = state.adapter.lookup_advisory(osv_id).await {
                vulnerabilities.push(meta);
            }
        }
    }

    // Look up RustSec IDs (OSV accepts RustSec IDs)
    for rustsec_id in &resolved_ids.rustsec_ids {
        if looked_up_ids.insert(rustsec_id.clone()) {
            if let Ok(Some(meta)) = state.adapter.lookup_advisory(rustsec_id).await {
                vulnerabilities.push(meta);
            }
        }
    }

    // Enrich vulnerabilities with KEV data if requested
    if req.include_kev == Some(true) {
        let cve_ids_for_kev: Vec<String> = vulnerabilities
            .iter()
            .flat_map(|v| v.cve_ids.iter().cloned())
            .collect();

        if cve_ids_for_kev.is_empty() {
            // No CVE IDs available for KEV lookup
            warnings.push(crate::core::SearchWarning::new(
                "_system",
                "kev_lookup_skipped: KEV lookup requires CVE identifiers",
            ));
        } else {
            let mut kev_found_ids: Vec<String> = Vec::new();
            let mut kev_lookup_failed = false;

            for cve_id in &cve_ids_for_kev {
                match state.kev_client.lookup(cve_id).await {
                    Ok(Some(kev_meta)) => {
                        // Enrich the matching vulnerability
                        for vuln in &mut vulnerabilities {
                            if vuln.cve_ids.iter().any(|id| id == cve_id) {
                                vuln.kev = Some(kev_meta.clone());
                            }
                        }
                        kev_found_ids.push(cve_id.clone());
                    }
                    Ok(None) => {}
                    Err(_) => {
                        kev_lookup_failed = true;
                    }
                }
            }

            if kev_lookup_failed && kev_found_ids.is_empty() {
                warnings.push(crate::core::SearchWarning::new(
                    "_system",
                    "kev_lookup_failed: KEV catalog lookup failed; KEV status could not be determined",
                ));
            } else if !kev_found_ids.is_empty() {
                warnings.push(crate::core::SearchWarning::new(
                    "_system",
                    format!(
                        "kev_match: {} CVE(s) found in CISA KEV catalog",
                        kev_found_ids.len()
                    ),
                ));
            } else {
                warnings.push(crate::core::SearchWarning::new(
                    "_system",
                    "kev_absent_not_proof: no CVE(s) found in CISA KEV catalog; \
                     absence does not prove no exploitation",
                ));
            }
        }
    }

    // Warn about version matching limitations
    if req.version.is_some() {
        warnings.push(crate::core::SearchWarning::new(
            "_system",
            "version_match_unavailable: version-specific matching is not yet implemented; \
             affected version ranges are returned as-is from advisory databases",
        ));
    }

    // Convert web results to source cards grouped by security category
    let groups = group_security_results(&web_resp.results, req.max_per_group);
    let suggested_fetches = generate_security_suggested_fetches(
        &groups,
        &resolved_ids,
        req.ecosystem.as_deref(),
        req.package.as_deref(),
    );

    let response = crate::core::SecuritySearchResponse {
        query: req.query.clone(),
        mode: "security_metasearch".to_string(),
        resolved_identifiers: resolved_ids,
        vulnerabilities,
        groups,
        suggested_fetches,
        providers_queried: web_resp.providers_queried,
        providers_failed: web_resp.providers_failed,
        warnings,
        trust_markers: web_resp.trust_markers,
    };

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
            .any(|w| w.as_str().unwrap().contains("safe_search")));
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
}
