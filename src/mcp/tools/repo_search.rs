use super::common::*;
use crate::mcp::policy::{live_allowed, live_search_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepoSearchArgs {
    /// Query. May contain repo hints (repo:owner/name).
    #[serde(default)]
    pub query: String,
    /// Code host (github, gitlab, codeberg, gitea, forgejo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Repository owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Repository name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Organization filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
    /// Path hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// File hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Language filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Symbol hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Task goal: understand, architecture, debug, migration, security, dependency, performance, compare, pre_change, post_change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// Source set override. Omit for goal defaults.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_docs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_registry: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_issues: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_releases: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_examples: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_pull_requests: Option<bool>,
    /// Max total results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    /// Max results per group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per_group: Option<usize>,
    /// Freshness hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub providers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub profile: Option<String>,
    /// Package ecosystem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    /// Package name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// Package version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Version requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_requirement: Option<String>,
    /// Package namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_namespace: Option<String>,
    /// Compare version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_security_context: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_changelog: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_migration_guides: Option<bool>,
    /// Include local workspace results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_local: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
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

    let semantics = super::canonical::resolve_repo_semantics(
        args.goal.as_deref(),
        args.profile.as_deref(),
        args.mode.as_deref(),
        args.workflow.as_deref(),
    )?;
    let profile = semantics.profile;
    let mode = semantics.mode;
    let workflow = semantics.workflow;

    let resolved_sources = super::canonical::resolve_repo_sources(
        &args.sources,
        args.include_docs,
        args.include_registry,
        args.include_issues,
        args.include_releases,
        args.include_examples,
        args.include_pull_requests,
        args.include_changelog,
        args.include_migration_guides,
        args.include_security_context,
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
        include_docs: resolved_sources.include_docs,
        include_registry: resolved_sources.include_registry,
        include_issues: resolved_sources.include_issues,
        include_releases: resolved_sources.include_releases,
        include_examples: resolved_sources.include_examples,
        include_pull_requests: resolved_sources.include_pull_requests,
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
        include_security_context: resolved_sources.include_security_context,
        include_changelog: resolved_sources.include_changelog,
        include_migration_guides: resolved_sources.include_migration_guides,
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
