use super::common::*;
use crate::mcp::policy::{live_allowed, live_search_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

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
    /// When true, compare advisory affected/fixed version ranges against
    /// the provided version (or versions parsed from dependency_files)
    /// and return per-package applicability assessments. This is
    /// metadata comparison only — it does NOT determine runtime
    /// exploitability or reachability. Assessments have status
    /// (affected/not_affected/unknown) and confidence (high/medium/
    /// low). Always treat results as advisory metadata, not safety
    /// guarantees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assess_applicability: Option<bool>,
    /// Local dependency file paths to parse for applicability assessment.
    /// Supported: Cargo.lock, Cargo.toml, package-lock.json,
    /// npm-shrinkwrap.json, go.mod, requirements.txt, requirements.in,
    /// Gemfile.lock, composer.lock, pom.xml, .csproj (PackageReference),
    /// .github/workflows/*.yml (uses: entries), Dockerfile,
    /// docker-compose.yml (FROM/image:). Parsed entries feed into
    /// version-range comparison when assess_applicability is true.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_files: Vec<String>,
    /// Workflow type for coverage model selection. Overrides the
    /// default security_review model when set. Accepted values:
    /// api_comprehension, repository_architecture, error_investigation,
    /// version_migration, security_review, dependency_evaluation,
    /// performance_investigation, comparative_research,
    /// pre_change_evidence, post_change_review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
}

/// Run the `security_search` tool.
pub async fn run_security_search(
    state: Arc<ServerState>,
    args: SecuritySearchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::SecuritySearchRequest;

    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Validation(live_search_denied_message(
            "security_search",
        )));
    }

    let query_present = args
        .query
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let identifier_present = args.cve_id.is_some()
        || args.ghsa_id.is_some()
        || args.osv_id.is_some()
        || args.rustsec_id.is_some()
        || args
            .package
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
    if !query_present && !identifier_present {
        return Err(ToolError::Validation(
            "security_search requires a non-empty query, package, \
             cve_id, ghsa_id, osv_id, or rustsec_id"
                .into(),
        ));
    }

    let query = args.query.unwrap_or_default();

    let severity_min = match args.severity_min.as_deref() {
        Some(s) => {
            let parsed = crate::core::SeverityLevel::from_str_loose(s);
            if parsed == crate::core::SeverityLevel::Unknown {
                return Err(ToolError::Validation(format!(
                    "invalid severity_min '{s}'; accepted values: critical, high, medium, low \
                     (aliases: crit, important, moderate, med, minor)"
                )));
            }
            Some(parsed)
        }
        None => None,
    };

    let freshness = parse_strict_freshness(args.freshness.as_deref())?.unwrap_or_default();

    let workflow = parse_strict_enum_arg(
        "workflow",
        args.workflow.as_deref(),
        crate::core::workflow_coverage::WorkflowKind::parse,
        &[
            "api_comprehension",
            "repository_architecture",
            "error_investigation",
            "version_migration",
            "security_review",
            "dependency_evaluation",
            "performance_investigation",
            "comparative_research",
            "pre_change_evidence",
            "post_change_review",
            "(aliases: api, architecture, error, migration, security, dependency, performance, research/comparative, pre_change, post_change)",
        ],
    )?;

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
        assess_applicability: args.assess_applicability,
        dependency_files: args.dependency_files.clone(),
        workflow,
    };

    if let Err(e) = req.validate(state.config.search.max_query_chars) {
        return Err(ToolError::Validation(format!("invalid request: {e}")));
    }

    // Validate dependency_files against the configured local workspace
    // roots. Without this, an MCP caller could supply arbitrary
    // server-side paths to be read by the applicability pipeline,
    // bypassing the documented local workspace safety model.
    let mut dependency_file_roots: Vec<std::path::PathBuf> = Vec::new();
    if !args.dependency_files.is_empty() {
        let backend = state.local_backend.clone().ok_or_else(|| {
            ToolError::Validation(
                "dependency_files requires local workspace to be enabled".to_string(),
            )
        })?;
        if !backend.is_enabled() {
            return Err(ToolError::Validation(
                "dependency_files requires local workspace to be enabled".to_string(),
            ));
        }
        let dependency_files = args.dependency_files.clone();
        let roots = tokio::task::spawn_blocking(move || -> Result<Vec<std::path::PathBuf>, ToolError> {
            let max_file_bytes = backend.config().max_file_bytes;
            let root_canonicals: Vec<std::path::PathBuf> = backend
                .roots()
                .iter()
                .filter_map(|(_, p)| std::fs::canonicalize(p).ok())
                .collect();
            if root_canonicals.is_empty() {
                return Err(ToolError::Validation(
                    "dependency_files requires at least one configured local workspace root"
                        .to_string(),
                ));
            }
            for file_path in &dependency_files {
                let path = Path::new(file_path);
                if path.as_os_str().is_empty() {
                    return Err(ToolError::Validation(
                        "dependency_files path must not be empty".to_string(),
                    ));
                }
                let canonical_input = std::fs::canonicalize(path).map_err(|e| {
                    ToolError::Validation(format!(
                        "dependency_files path '{file_path}' cannot be resolved: {e}"
                    ))
                })?;
                if !canonical_input.is_file() {
                    return Err(ToolError::Validation(format!(
                        "dependency_files path '{file_path}' is not a regular file"
                    )));
                }
                if let Ok(meta) = std::fs::metadata(&canonical_input) {
                    if meta.len() > max_file_bytes as u64 {
                        return Err(ToolError::Validation(format!(
                            "dependency_files path '{file_path}' exceeds max_file_bytes ({max_file_bytes})",
                        )));
                    }
                }
                let mut inside_root = false;
                for root_canon in &root_canonicals {
                    if canonical_input.starts_with(root_canon) {
                        inside_root = true;
                        break;
                    }
                }
                if !inside_root {
                    return Err(ToolError::Validation(format!(
                        "dependency_files path '{file_path}' is not within any configured local workspace root"
                    )));
                }
            }
            Ok(root_canonicals)
        })
        .await
        .map_err(|e| ToolError::internal(format!("dependency_files validation failed: {e}")))??;
        dependency_file_roots = roots;
    }

    let routing_decision = crate::meta::provider_diagnostics::resolve_provider_routing(
        &req.providers,
        None,
        state.adapter.provider_ids(),
        &state.config,
        state.adapter.health(),
        true,
    )
    .map_err(|e| ToolError::Validation(e.to_string()))?;

    let effective_max = req.effective_max_results(
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    let mut req = req;
    req.providers = routing_decision.selected_providers.clone();

    let mut response = crate::meta::security_search::run_security_search_plan(
        &state.adapter,
        &state.kev_client,
        &req,
        effective_max,
        state.config.search.max_results_cap,
        dependency_file_roots,
    )
    .await;

    merge_selection_stage_attempts(&routing_decision, &mut response.retrieval_summary);

    response.routing_decision = Some(routing_decision);

    // Add next-action hints
    let source_ids: Vec<String> = response
        .groups
        .iter()
        .flat_map(|g| &g.results)
        .filter_map(|r| r.stable_id.clone())
        .collect();
    let has_suggested_fetches = !response.suggested_fetches.is_empty();
    response.next_actions =
        crate::meta::security_search_next_actions(&source_ids, has_suggested_fetches);

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;

    Ok(value)
}
