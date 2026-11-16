use super::common::*;
use crate::mcp::policy::{live_allowed, live_search_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSearchArgs {
    /// Research query.
    pub query: String,
    /// Research domain hint.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "domain")]
    pub research_domain: Option<String>,
    /// Source types to include.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        alias = "sources",
        alias = "source_types"
    )]
    pub desired_source_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_counterpoints: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_primary_sources: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_recent_discussion: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub include_security_considerations: Option<bool>,
    /// Compact include set: counterpoints, primary_sources, recent_discussion, security_considerations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    /// Max total results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
    /// Max result groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_groups: Option<usize>,
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
    /// Task goal: understand, architecture, debug, migration, security, dependency, performance, compare, pre_change, post_change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub workflow: Option<String>,
    /// Depth: quick, standard, deep.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<String>,
    /// Compare targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compare_targets: Vec<String>,
    /// Constraints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    /// Known context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_context: Option<String>,
    /// Zero-based index of the first non-empty group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    /// Max groups from `offset`; clamped to the server maximum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Response detail: brief, compact, standard, or diagnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_detail: Option<crate::mcp::projection::ResponseDetail>,
}

/// Run the `research_search` tool.
pub async fn run_research_search(
    state: Arc<ServerState>,
    args: ResearchSearchArgs,
) -> Result<serde_json::Value, ToolError> {
    let mut progress = crate::mcp::progress::ProgressSink::disabled();
    run_research_search_with_progress(state, args, &mut progress).await
}

pub const RESEARCH_PAGE_GROUPS: usize = 14;

pub(crate) fn research_search_plan_key(args: &ResearchSearchArgs) -> String {
    research_search_plan_key_over(args, args.max_results, args.max_per_group)
}

fn research_search_plan_key_over(
    args: &ResearchSearchArgs,
    max_results: Option<usize>,
    max_per_group: Option<usize>,
) -> String {
    let mut key =
        crate::mcp::plan_key::PlanKey::new(crate::mcp::plan_key::PlanTool::ResearchSearch)
            .str(&args.query)
            .opt_str(args.research_domain.as_deref())
            .opt_str(args.goal.as_deref())
            .opt_str(args.workflow.as_deref())
            .opt_str(args.depth.as_deref())
            .opt_usize(max_results)
            .opt_usize(max_per_group)
            .opt_str(args.freshness.as_deref())
            .opt_str(args.known_context.as_deref());
    for source_type in &args.desired_source_types {
        key = key.str(source_type);
    }
    for provider in &args.providers {
        key = key.str(provider);
    }
    for target in &args.compare_targets {
        key = key.str(target);
    }
    key.finish()
}

pub async fn run_research_search_with_progress(
    state: Arc<ServerState>,
    args: ResearchSearchArgs,
    progress: &mut crate::mcp::progress::ProgressSink,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::research::{
        ResearchDepth, ResearchDomain, ResearchSearchRequest, ResearchSourceType,
    };

    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Validation(live_search_denied_message(
            "research_search",
        )));
    }

    if let Some(0) = args.limit {
        return Err(ToolError::execution_with_repair(
            ToolErrorCode::InvalidSemanticValue,
            "limit must be > 0; an addressed window is a half-open group range".to_string(),
            RepairHint::new(Some("limit"), &[], None),
        ));
    }
    let addressed = args.offset.is_some() || args.limit.is_some();
    let plan_key = research_search_plan_key(&args);
    if addressed {
        let served_results = args
            .max_results
            .map(|value| value.min(state.config.search.max_results_cap));
        let served_per_group = args.max_per_group.map(|value| value.max(1));
        let served = research_search_plan_key_over(&args, served_results, served_per_group);
        if served != plan_key {
            let (field, supplied, effective) = if served_results != args.max_results {
                (
                    "max_results",
                    args.max_results.unwrap_or_default(),
                    served_results.unwrap_or_default(),
                )
            } else {
                (
                    "max_per_group",
                    args.max_per_group.unwrap_or_default(),
                    served_per_group.unwrap_or_default(),
                )
            };
            return Err(addressing_rejected(AddressError::PlanMismatch {
                field,
                supplied,
                effective,
            }));
        }
    }
    if args.max_groups == Some(0) {
        return Err(ToolError::Validation(
            "invalid request: max_groups must be > 0".to_string(),
        ));
    }
    let page_groups = args
        .max_groups
        .unwrap_or(RESEARCH_PAGE_GROUPS)
        .min(RESEARCH_PAGE_GROUPS);
    let requested_limit = args.limit.unwrap_or(page_groups);
    let limit = requested_limit.clamp(1, RESEARCH_PAGE_GROUPS);
    let limit_clamped = limit < requested_limit;

    let research_domain = parse_strict_enum_arg(
        "research_domain",
        args.research_domain.as_deref(),
        ResearchDomain::parse,
        &[
            "general",
            "software_architecture",
            "api_design",
            "distributed_systems",
            "security",
            "performance",
            "language_ecosystem",
            "machine_learning",
            "infrastructure",
            "(aliases: architecture, api, distributed, ml, infra)",
        ],
    )?;

    let workflow = super::canonical::resolve_research_workflow(
        args.goal.as_deref(),
        args.workflow.as_deref(),
    )?;

    let depth = parse_strict_enum_arg(
        "depth",
        args.depth.as_deref(),
        ResearchDepth::parse,
        &["quick", "standard", "deep"],
    )?;

    let resolved_includes = super::canonical::resolve_research_includes(
        &args.include,
        args.include_counterpoints,
        args.include_primary_sources,
        args.include_recent_discussion,
        args.include_security_considerations,
    )?;

    let mut desired_source_types: Vec<ResearchSourceType> = Vec::new();
    let mut invalid_source_types: Vec<String> = Vec::new();
    for s in &args.desired_source_types {
        match ResearchSourceType::parse(s) {
            Some(t) => desired_source_types.push(t),
            None => invalid_source_types.push(s.clone()),
        }
    }
    if !invalid_source_types.is_empty() {
        return Err(ToolError::Validation(format!(
            "invalid desired_source_types entries: {}; accepted values: primary_sources, official_docs, specifications, reference_implementations, design_discussions, benchmarks, security_considerations, issue_threads, release_notes, academic_or_formal_sources, recent_news, community_discussion, counterpoints",
            invalid_source_types.join(", ")
        )));
    }

    let freshness = parse_strict_freshness(args.freshness.as_deref())?.unwrap_or_default();

    let req = ResearchSearchRequest {
        query: args.query,
        research_domain,
        desired_source_types,
        include_counterpoints: resolved_includes.include_counterpoints,
        include_primary_sources: resolved_includes.include_primary_sources,
        include_recent_discussion: resolved_includes.include_recent_discussion,
        include_security_considerations: resolved_includes.include_security_considerations,
        max_results: args.max_results,
        max_groups: Some(page_groups.max(RESEARCH_PAGE_GROUPS)),
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

    let routing_decision = crate::meta::provider_diagnostics::resolve_provider_routing(
        &req.providers,
        None,
        state.adapter.provider_ids(),
        &state.config,
        state.adapter.health(),
        true,
        None,
    )
    .map_err(|e| ToolError::Validation(e.to_string()))?;

    let effective_max = req.effective_max_results(
        state.config.search.default_max_results,
        state.config.search.max_results_cap,
    );

    let mut req = req;
    req.providers = routing_decision.selected_providers.clone();

    progress.phase("plan");
    let mut response = state
        .adapter
        .research_search(&req, effective_max, state.config.search.max_results_cap)
        .await;

    merge_selection_stage_attempts(&routing_decision, &mut response.retrieval_summary);

    progress.phase("merge");
    let group_ids: Vec<String> = response
        .groups
        .iter()
        .filter(|group| !group.results.is_empty())
        .map(|group| {
            group
                .results
                .last()
                .and_then(|card| card.stable_id.clone().or_else(|| Some(card.id.clone())))
                .unwrap_or_default()
        })
        .collect();
    let non_empty: Vec<usize> = response
        .groups
        .iter()
        .enumerate()
        .filter(|(_, group)| !group.results.is_empty())
        .map(|(index, _)| index)
        .collect();
    let window = AddressWindow::resolve(group_ids.len(), args.offset, limit);
    if window.to < non_empty.len() || window.from > 0 {
        let selected: Vec<usize> = non_empty[window.from..window.to].to_vec();
        let mut paged = Vec::with_capacity(selected.len());
        for index in selected {
            paged.push(response.groups[index].clone());
        }
        response.groups = paged;
    }
    progress.phase("group");

    // Add routing decision and capability enforcement telemetry
    if let Some(ref mut telem) = response.telemetry {
        telem.routing_decision = Some(routing_decision);
        telem.capability_enforcement = Some(
            crate::meta::provider_diagnostics::CapabilityEnforcementTelemetry::for_research_search(
                &req,
                &req.providers,
            ),
        );
    }

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
            crate::meta::research_search_next_actions(&source_ids, has_suggested_fetches);
    }

    if limit_clamped {
        response
            .structured_warnings
            .push(crate::core::warning::AgentWarning::new(
                crate::core::warning::WarningCode::MaxResultsClamped,
                format!("limit was clamped to the server maximum of {RESEARCH_PAGE_GROUPS} groups"),
            ));
    }
    let mut value = serde_json::to_value(&response)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;
    if addressed {
        value = with_addressing_keys(value, GROUP_ADDRESS_KEYS, window);
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "aggregate_scope".to_string(),
                serde_json::Value::from("document"),
            );
            object.insert(
                "total_groups".to_string(),
                serde_json::Value::from(window.total),
            );
            object.insert(
                "returned_groups".to_string(),
                serde_json::Value::from(window.len()),
            );
        }
    }

    let detail = crate::mcp::projection::ResponseDetail::from_opt_for_profile(
        args.response_detail,
        state.config.mcp.schema_profile,
    );
    progress.phase("project");
    Ok(crate::mcp::projection::project_with_limits(
        "research_search",
        value,
        detail,
        crate::mcp::projection::ProjectionLimits {
            brief_max_chars: state.config.mcp.brief_max_chars,
            excerpt_budget_chars: state.config.mcp.excerpt_budget_chars,
        },
    ))
}
