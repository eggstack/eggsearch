use super::common::*;
use crate::mcp::policy::{live_allowed, live_search_denied_message, Policy};
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    /// Research workflow type for structured scaffolding. "general":
    /// default broad research. "architecture_decision": evaluates options
    /// for a design choice. "api_evaluation": assesses an API for
    /// adoption. "library_comparison": compares libraries side-by-side
    /// (use with compare_targets). "migration_planning": plans version
    /// or framework migrations. "security_review": security-focused
    /// evidence gathering. "performance_investigation": performance
    /// benchmarking and profiling context. "ecosystem_survey": maps a
    /// technology ecosystem. Workflow sets deterministic source-type
    /// and domain dimensions — the agent decides which suggested
    /// fetches to act on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// Research depth controlling subquery count. "quick": ~4 subqueries
    /// for fast reconnaissance. "standard": ~8 subqueries for balanced
    /// coverage. "deep": ~12 subqueries for thorough multi-source
    /// discovery. Default "standard" when omitted. Deeper settings
    /// produce more source diversity but take longer.
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

/// Run the `research_search` tool.
pub async fn run_research_search(
    state: Arc<ServerState>,
    args: ResearchSearchArgs,
) -> Result<serde_json::Value, ToolError> {
    use crate::core::research::{
        ResearchDepth, ResearchDomain, ResearchSearchRequest, ResearchSourceType,
    };

    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(ToolError::Validation(live_search_denied_message(
            "research_search",
        )));
    }

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

    let workflow = parse_strict_enum_arg(
        "workflow",
        args.workflow.as_deref(),
        crate::core::research::ResearchWorkflow::parse,
        &[
            "general",
            "api_evaluation",
            "library_comparison",
            "migration_planning",
            "security_review",
            "performance_investigation",
            "ecosystem_survey",
            "architecture_decision",
            "(aliases: api, comparison, migration, security, performance, architecture)",
        ],
    )?;

    let depth = parse_strict_enum_arg(
        "depth",
        args.depth.as_deref(),
        ResearchDepth::parse,
        &["quick", "standard", "deep"],
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

    let mut response = state
        .adapter
        .research_search(&req, effective_max, state.config.search.max_results_cap)
        .await;

    merge_selection_stage_attempts(&routing_decision, &mut response.retrieval_summary);

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

    let value = serde_json::to_value(&response)
        .map_err(|e| ToolError::internal(format!("serialization error: {e}")))?;

    Ok(value)
}
