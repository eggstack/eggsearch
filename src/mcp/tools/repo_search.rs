use super::common::*;
use crate::mcp::policy::{live_allowed, live_search_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoSearchArgs {
    /// Free-text query. May contain repo hints (repo:owner/name, etc.).
    #[serde(default)]
    pub query: String,
    /// Optional. Code host to target (github, gitlab, codeberg, gitea, forgejo).
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
    /// Search profile for provider selection. "generic" (default): uses
    /// configured default providers. "coding": prefers native code/issues/
    /// releases providers (GitHub, GitLab, Gitea), falls back to generic
    /// web if unavailable. "security": prefers OSV and security-capable
    /// providers. "research": prefers diverse source discovery and broad
    /// web/API providers. Profiles are advisory — unavailable providers
    /// are skipped with warnings rather than failing. Use "coding" for
    /// codebase-specific queries, "security" for vulnerability lookups,
    /// "research" for multi-source evidence gathering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Optional. Package ecosystem ("crates.io", "pypi", "npm", "go",
    /// "maven", "nuget", "rubygems", "packagist", "oci",
    /// "github_actions").
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
    /// Optional. Package namespace (e.g. Maven group_id, OCI registry namespace).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_namespace: Option<String>,
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
    /// Include local workspace results when available. When true and
    /// the server operator has configured `local` roots, the search
    /// includes source files from local Git checkouts matching the
    /// requested repo. Local results carry trust=local_trusted and
    /// may have symbol-enriched metadata. Default true when local
    /// backend is enabled. Set to false to exclude local files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_local: Option<bool>,
    /// Search mode. "default" (or omitted) uses standard repo-search
    /// subqueries for general codebase discovery. "exact_error" optimizes
    /// for compiler/runtime/toolchain error messages: it preserves exact
    /// error phrases, extracts error codes (Rust E0xxx, TSxxxx, Python
    /// exceptions), targets docs/issues/changelogs, and redacts sensitive
    /// tokens (local paths, API keys, UUIDs, memory addresses). Use
    /// "exact_error" when the query is a literal error message you want
    /// diagnosed; use "default" for everything else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Workflow type for coverage model selection. Overrides profile-based
    /// and mode-based defaults when set. Accepted values: api_comprehension,
    /// repository_architecture, error_investigation, version_migration,
    /// security_review, dependency_evaluation, performance_investigation,
    /// comparative_research, pre_change_evidence, post_change_review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
}

/// Run the `repo_search` tool.
pub async fn run_repo_search(
    state: Arc<ServerState>,
    args: RepoSearchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::repo_search::RepoSearchRequest;

    // Permit a local-only path when the local backend is enabled and the
    // caller has not explicitly opted out of local results. The remote
    // provider dispatch is then suppressed below by routing with an empty
    // provider list.
    let local_only_path = matches!(live_allowed(state.config.search.mode), Policy::Deny)
        && state.local_backend.is_some()
        && args.include_local != Some(false);

    if matches!(live_allowed(state.config.search.mode), Policy::Deny) && !local_only_path {
        return Err(ToolError::Validation(live_search_denied_message(
            "repo_search",
        )));
    }

    let host = parse_code_host_arg(args.host.as_deref())?;

    let freshness = parse_strict_freshness(args.freshness.as_deref())?.unwrap_or_default();

    let profile = parse_strict_enum_arg(
        "profile",
        args.profile.as_deref(),
        crate::core::repo_search::SearchProfile::parse,
        &[
            "generic",
            "coding",
            "security",
            "research",
            "(aliases: default/web, code/repo, vuln/advisory, deep/thorough)",
        ],
    )?;

    let mode = parse_strict_enum_arg(
        "mode",
        args.mode.as_deref(),
        crate::core::repo_search::RepoSearchMode::parse,
        &["normal", "exact_error", "(aliases: default, error)"],
    )?;

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

    let (owner, repo) = if let Some(r) = &args.repo {
        if let Some((o, rest)) = r.split_once('/') {
            if args.owner.is_none() {
                if o.is_empty() || rest.is_empty() {
                    return Err(ToolError::Validation(format!(
                        "invalid repo '{r}': must be owner/name with non-empty parts"
                    )));
                }
                (Some(o.to_string()), Some(rest.to_string()))
            } else {
                (args.owner.clone(), args.repo.clone())
            }
        } else {
            (args.owner.clone(), args.repo.clone())
        }
    } else {
        (args.owner.clone(), args.repo.clone())
    };

    let req = RepoSearchRequest {
        query: args.query,
        host,
        owner,
        repo,
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
        providers: if local_only_path {
            Vec::new()
        } else {
            args.providers.clone()
        },
        profile,
        ecosystem: parse_strict_enum_arg(
            "ecosystem",
            args.ecosystem.as_deref(),
            crate::core::package::PackageEcosystem::parse,
            &[
                "crates_io",
                "pypi",
                "npm",
                "go",
                "maven",
                "nuget",
                "rubygems",
                "packagist",
                "oci",
                "github_actions",
                "(aliases: cargo, python, node, gradle, ruby, etc.)",
            ],
        )?,
        package: args.package.clone(),
        version: args.version.clone(),
        version_requirement: args.version_requirement.clone(),
        package_namespace: args.package_namespace.clone(),
        compare_version: args.compare_version.clone(),
        include_security_context: args.include_security_context,
        include_changelog: args.include_changelog,
        include_migration_guides: args.include_migration_guides,
        include_local: args.include_local,
        mode,
        workflow,
        exact_error_config: Some(state.config.search.exact_error.clone()),
    };

    if let Err(e) = req.validate(state.config.search.max_query_chars) {
        return Err(ToolError::Validation(format!("invalid query: {e}")));
    }

    let routing_decision = if local_only_path {
        crate::meta::provider_diagnostics::ProviderRoutingDecision {
            requested_profile: None,
            requested_providers: Vec::new(),
            selected_providers: Vec::new(),
            skipped_providers: Vec::new(),
            degraded: false,
            partial: false,
            reason: Some("local-only path: remote provider dispatch suppressed".to_string()),
        }
    } else {
        crate::meta::provider_diagnostics::resolve_provider_routing(
            &req.providers,
            req.profile,
            state.adapter.provider_ids(),
            &state.config,
            state.adapter.health(),
            true,
        )
        .map_err(|e| match e {
            crate::meta::provider_diagnostics::ProviderRoutingError::UnknownProvider(id) => {
                ToolError::Validation(format!("unknown provider id: {id}"))
            }
            crate::meta::provider_diagnostics::ProviderRoutingError::DisabledProvider(id) => {
                ToolError::Validation(format!("provider is disabled: {id}"))
            }
            crate::meta::provider_diagnostics::ProviderRoutingError::NoDefaultProviders(msg) => {
                ToolError::internal(format!("no default providers: {msg}"))
            }
        })?
    };

    // Convert routing decision skipped_providers into SearchWarnings
    let mut profile_warnings: Vec<crate::core::result::SearchWarning> = Vec::new();
    for skip in &routing_decision.skipped_providers {
        if skip.skip_code == Some(crate::core::provider::ProviderSkipCode::NotBuilt) {
            profile_warnings.push(crate::core::result::SearchWarning::new(
                "_system",
                format!(
                    "profile_provider_not_built: {} is in {:?} profile but no engine was constructed",
                    skip.provider_id, req.profile
                ),
            ));
        } else if skip.skip_code == Some(crate::core::provider::ProviderSkipCode::CooldownActive) {
            profile_warnings.push(crate::core::result::SearchWarning::new(
                "_system",
                format!(
                    "provider_cooldown: {} skipped due to {}",
                    skip.provider_id, skip.reason
                ),
            ));
        }
    }
    if routing_decision.degraded {
        profile_warnings.push(crate::core::result::SearchWarning::new(
            "_system",
            format!(
                "profile_degraded: {:?} profile fell back to default providers",
                req.profile
            ),
        ));
    } else if routing_decision.partial {
        profile_warnings.push(crate::core::result::SearchWarning::new(
            "_system",
            format!(
                "profile_partial: {:?} profile skipped unavailable providers",
                req.profile
            ),
        ));
    }

    let effective_providers = routing_decision.selected_providers.clone();
    let skipped_provider_ids: Vec<String> = routing_decision
        .skipped_providers
        .iter()
        .map(|s| s.provider_id.clone())
        .collect();

    let effective_max = req.effective_max_results(
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    let mut req = req;
    req.providers = effective_providers;

    let local_inventory = state.local_inventory().await;
    let mut response = state
        .adapter
        .repo_search(
            &req,
            effective_max,
            state.config.search.max_results_cap,
            state.local_backend.as_deref(),
            Some(&local_inventory),
        )
        .await;

    // Merge profile warnings into response warnings
    response.warnings.extend(profile_warnings);

    // Merge profile warnings into structured warnings
    for skip in &routing_decision.skipped_providers {
        if skip.skip_code == Some(crate::core::provider::ProviderSkipCode::NotBuilt) {
            response.structured_warnings.push(
                crate::core::warning::AgentWarning::new(
                    crate::core::warning::WarningCode::ProfileProviderNotBuilt,
                    format!(
                        "{} is in {:?} profile but no engine was constructed",
                        skip.provider_id, req.profile
                    ),
                )
                .with_provider_ids(vec![skip.provider_id.clone()])
                .with_severity(crate::core::warning::WarningSeverity::Warning),
            );
        } else if skip.skip_code == Some(crate::core::provider::ProviderSkipCode::CooldownActive) {
            response.structured_warnings.push(
                crate::core::warning::AgentWarning::new(
                    crate::core::warning::WarningCode::ProviderCooldown,
                    format!("{} skipped due to {}", skip.provider_id, skip.reason),
                )
                .with_provider_ids(vec![skip.provider_id.clone()])
                .with_severity(crate::core::warning::WarningSeverity::Warning),
            );
        }
    }
    if routing_decision.degraded {
        response.structured_warnings.push(
            crate::core::warning::AgentWarning::new(
                crate::core::warning::WarningCode::ProfileDegraded,
                format!("{:?} profile fell back to default providers", req.profile),
            )
            .with_severity(crate::core::warning::WarningSeverity::Warning)
            .with_recommended_action("Configure the required native providers for this profile."),
        );
    } else if routing_decision.partial {
        response.structured_warnings.push(
            crate::core::warning::AgentWarning::new(
                crate::core::warning::WarningCode::ProfilePartial,
                format!("{:?} profile skipped unavailable providers", req.profile),
            )
            .with_severity(crate::core::warning::WarningSeverity::Notice),
        );
    }

    // Populate telemetry provider selection from routing decision.
    // Use original req.profile for profile_requested/applied since
    // resolve_provider_routing clears it for explicit provider lists.
    let is_degraded = routing_decision.degraded;
    let has_partial = routing_decision.partial;
    response.telemetry.provider_selection = crate::core::repo_search::ProviderSelectionTelemetry {
        profile_requested: req.profile,
        profile_applied: req.profile,
        degraded: is_degraded,
        partial: has_partial && !is_degraded,
        skipped_providers: skipped_provider_ids,
        reason: routing_decision.reason.clone(),
    };

    // Propagate degraded/partial provider selection into uncertainty_summary
    if let Some(ref mut summary) = response.telemetry.uncertainty_summary {
        summary.degraded_provider_selection = is_degraded;
        summary.partial_provider_selection = has_partial && !is_degraded;
    }

    // Add capability enforcement telemetry
    response.telemetry.capability_enforcement = Some(
        crate::meta::provider_diagnostics::CapabilityEnforcementTelemetry::for_repo_search(
            &req,
            &req.providers,
        ),
    );

    // Add routing decision telemetry
    merge_selection_stage_attempts(&routing_decision, &mut response.retrieval_summary);
    response.telemetry.routing_decision = Some(routing_decision);

    // Supplement gap-driven next actions with recipe-based hints when
    // the adapter did not produce gap-driven actions.
    if response.next_actions.is_empty() {
        let source_ids: Vec<String> = response
            .groups
            .iter()
            .flat_map(|g| &g.results)
            .filter_map(|r| r.stable_id.clone())
            .collect();
        let has_suggested_fetches = !response.suggested_fetches.is_empty();
        response.next_actions =
            crate::meta::repo_search_next_actions(&source_ids, has_suggested_fetches);
    }

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;

    Ok(value)
}
