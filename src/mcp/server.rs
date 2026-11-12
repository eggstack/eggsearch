//! MCP server implementation using the `rmcp` crate.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResponse, CallToolResult, CancelTaskParams, GetTaskParams, GetTaskResult,
    Implementation, InitializeResult, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
    ServerConfig, UpdateTaskParams,
};
use rmcp::service::RequestContext;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler};

use crate::core::config::McpToolPreset;
use crate::mcp::advertisement;
use crate::mcp::progress::ProgressSink;
use crate::mcp::state::ServerState;
use crate::mcp::tasks::{
    complete_response, is_eligible, run_cancellable, task_response, TaskRuntime,
};
use crate::mcp::tools::{
    map_tool_result, run_batch_fetch_with_progress, run_build_evidence_bundle,
    run_provider_status_async, run_repo_fetch_with_host_scope, run_repo_map_with_host_scope,
    run_repo_search_with_host_scope, run_research_search_with_progress, run_security_search,
    run_web_fetch_with_progress, run_web_search, BatchFetchArgs, EvidenceBundleArgs,
    ProviderStatusArgs, RepoFetchArgs, RepoMapArgs, RepoSearchArgs, ResearchSearchArgs,
    SecuritySearchArgs, ToolError, WebFetchArgs, WebSearchArgs,
};

#[derive(Clone)]
pub struct EggsearchServer {
    state: Arc<ServerState>,
    tool_router: ToolRouter<Self>,
    advertised_tools: Arc<Vec<rmcp::model::Tool>>,
    advertised_fingerprint: Arc<str>,
    tool_preset: McpToolPreset,
    instructions: Arc<str>,
    tasks: Option<Arc<TaskRuntime>>,
}

impl std::fmt::Debug for EggsearchServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EggsearchServer").finish()
    }
}

impl EggsearchServer {
    pub fn new(state: Arc<ServerState>) -> Self {
        let profile = state.config.mcp.schema_profile;
        let preset = state.config.mcp.tool_preset;
        let tool_router = Self::tool_router();
        let mut advertised_tools = apply_contract_metadata(tool_router.list_all());
        if profile == crate::core::config::McpSchemaProfile::Compact {
            advertised_tools = apply_compact_profile(advertised_tools);
        }
        let advertised_tools = advertisement::filter_tools(preset, advertised_tools);
        let advertised_tools = Arc::new(advertised_tools);
        let advertised_fingerprint: Arc<str> = contract_fingerprint(&advertised_tools).into();
        let instructions: Arc<str> = advertisement::instructions(preset).into();
        tracing::info!(
            "mcp advertisement preset={} schema_profile={} advertised={} hidden={}",
            preset.as_str(),
            profile.as_str(),
            advertised_tools.len(),
            advertisement::hidden_tools(preset).join(",")
        );
        let tasks = TaskRuntime::from_config(&state.config.mcp.tasks).map(Arc::new);
        Self {
            state,
            tool_router,
            advertised_tools,
            advertised_fingerprint,
            tool_preset: preset,
            instructions,
            tasks,
        }
    }

    pub fn task_runtime(&self) -> Option<&TaskRuntime> {
        self.tasks.as_deref()
    }

    pub fn tool_definitions(&self) -> Vec<rmcp::model::Tool> {
        self.advertised_tools.as_ref().clone()
    }

    /// Active advertisement preset resolved from process configuration.
    pub fn tool_preset(&self) -> McpToolPreset {
        self.tool_preset
    }

    /// Tool names registered for dispatch, independent of advertisement.
    ///
    /// The router is never filtered by a preset: hidden tools still
    /// execute when a caller names them explicitly.
    pub fn routed_tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }

    /// Deterministic content fingerprint of the public tool contract.
    ///
    /// Covers canonical names, descriptions, annotations, and advertised
    /// input/output schemas. Suitable for client-side caching of
    /// `tools/list` by content rather than by count.
    pub fn tool_fingerprint(&self) -> String {
        self.advertised_fingerprint.to_string()
    }

    /// Single response seam: advertised pointers never name a hidden tool.
    fn respond(
        &self,
        result: Result<serde_json::Value, ToolError>,
    ) -> Result<CallToolResult, McpError> {
        map_tool_result(result.map(|value| advertisement::filter_payload(self.tool_preset, value)))
    }

    fn respond_response(
        &self,
        result: Result<serde_json::Value, ToolError>,
    ) -> Result<CallToolResponse, McpError> {
        let payload = result.map(|value| advertisement::filter_payload(self.tool_preset, value));
        complete_response(map_tool_result(payload)?)
    }

    fn spawn_task<F>(&self, eligible: bool, build: F) -> Option<rmcp::model::CreateTaskResult>
    where
        F: FnOnce(
                Arc<ServerState>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<serde_json::Value, ToolError>> + Send>,
            > + Send
            + 'static,
    {
        let runtime = self.tasks.as_ref()?;
        if !eligible || runtime.at_capacity() {
            return None;
        }
        let preset = self.tool_preset;
        let state = self.state.clone();
        let runtime = runtime.clone();
        Some(runtime.spawn("working", move |context| {
            Box::pin(async move {
                let work = async move {
                    match build(state).await {
                        Ok(value) => {
                            map_tool_result(Ok(advertisement::filter_payload(preset, value)))
                        }
                        Err(error) => map_tool_result(Err(error)),
                    }
                };
                run_cancellable(Some(context), work).await
            })
        }))
    }
}

fn apply_compact_profile(mut tools: Vec<rmcp::model::Tool>) -> Vec<rmcp::model::Tool> {
    for tool in &mut tools {
        if let Some(description) = compact_description(tool.name.as_ref()) {
            tool.description = Some(description.into());
        }
        let mut schema = serde_json::Value::Object(tool.input_schema.as_ref().clone());
        compact_schema(tool.name.as_ref(), &mut schema);
        if let Some(object) = schema.as_object() {
            tool.input_schema = Arc::new(object.clone());
        }
        tool.output_schema = None;
    }
    tools
}

fn compact_description(name: &str) -> Option<&'static str> {
    Some(match name {
        "web_search" => "Search the public web for sources; use web_fetch to inspect a result.",
        "provider_status" => "Check configured provider availability and diagnostics.",
        "repo_search" => "Search repository code, documentation, issues, and releases.",
        "web_fetch" => "Fetch one known HTTP(S) URL from search results or user input; return bounded extracted text. Use batch_fetch for several targets; do not search or crawl.",
        "repo_fetch" => "Fetch a known repository file or span by structured locator.",
        "security_search" => "Find vulnerability evidence and assess package applicability.",
        "research_search" => "Gather grouped evidence for complex research questions.",
        "batch_fetch" => "Fetch several known URLs or repository locators in one call.",
        "repo_map" => "Describe a repository layout before searching or fetching files.",
        "build_evidence_bundle" => {
            "Package already selected search and fetch evidence for handoff."
        }
        _ => return None,
    })
}

fn compact_schema(name: &str, schema: &mut serde_json::Value) {
    const COMMON: &[&str] = &["response_detail"];
    let allowed: &[&str] = match name {
        "web_search" => &[
            "query",
            "max_results",
            "safe_search",
            "intent",
            "freshness",
            "date_range",
            "include_domains",
            "exclude_domains",
            "language",
            "region",
            "excerpt_count",
            "response_detail",
        ],
        "provider_status" => &["probe"],
        "repo_search" => &[
            "query",
            "owner",
            "repo",
            "host",
            "profile",
            "goal",
            "sources",
            "include_local",
            "response_detail",
        ],
        "web_fetch" => &[
            "url",
            "max_chars",
            "extract_mode",
            "include_links",
            "pdf",
            "focus",
            "focus_max_chunks",
            "focus_max_chars",
            "render",
            "cursor",
            "response_detail",
        ],
        "repo_fetch" => &[
            "owner",
            "repo",
            "path",
            "ref",
            "commit_sha",
            "line_start",
            "line_end",
            "symbol",
            "max_chars",
            "response_detail",
        ],
        "security_search" => &[
            "query",
            "package",
            "version",
            "ecosystem",
            "assess_applicability",
            "dependency_files",
            "goal",
            "response_detail",
        ],
        "research_search" => &[
            "query",
            "domain",
            "research_domain",
            "goal",
            "depth",
            "compare_targets",
            "constraints",
            "known_context",
            "cursor",
            "response_detail",
        ],
        "batch_fetch" => &[
            "items",
            "max_chars_per_item",
            "total_max_chars",
            "cursor",
            "response_detail",
        ],
        "repo_map" => &[
            "owner",
            "repo",
            "host",
            "ref",
            "path",
            "max_depth",
            "response_detail",
        ],
        "build_evidence_bundle" => &[
            "goal",
            "sources",
            "fetches",
            "max_sources",
            "max_fetched_items",
            "max_total_chars",
            "include_unfetched_sources",
            "response_detail",
        ],
        _ => COMMON,
    };
    compact_schema_value(schema, allowed);
    compact_workflow_schema(name, schema);
    if name == "web_fetch" {
        if let Some(render) = schema.pointer_mut("/properties/render") {
            set_string_schema_enum(render, &["http_only", "auto", "browser"]);
        }
    }
    if name == "build_evidence_bundle" {
        compact_evidence_bundle_schema(schema);
    }
}

fn compact_workflow_schema(name: &str, schema: &mut serde_json::Value) {
    if name != "research_search" && name != "security_search" {
        return;
    }
    let Some(properties) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    if name == "research_search" {
        if let Some(domain) = properties.get_mut("research_domain") {
            set_string_schema_enum(
                domain,
                &[
                    "general",
                    "software_architecture",
                    "architecture",
                    "api_design",
                    "api",
                    "distributed_systems",
                    "distributed",
                    "security",
                    "performance",
                    "language_ecosystem",
                    "ecosystem",
                    "machine_learning",
                    "ml",
                    "infrastructure",
                    "infra",
                ],
            );
        }
        if let Some(goal) = properties.get_mut("goal") {
            set_string_schema_enum(goal, crate::mcp::tools::canonical::RESEARCH_GOAL_VALUES);
        }
    } else if let Some(goal) = properties.get_mut("goal") {
        set_string_schema_enum(goal, crate::mcp::tools::canonical::SECURITY_GOAL_VALUES);
    }
}

fn set_string_schema_enum(schema: &mut serde_json::Value, values: &[&str]) {
    match schema {
        serde_json::Value::Object(object) => {
            match object.get("type") {
                Some(serde_json::Value::String(kind)) if kind == "string" => {
                    object.insert(
                        "enum".into(),
                        serde_json::Value::Array(
                            values
                                .iter()
                                .map(|value| serde_json::Value::String((*value).into()))
                                .collect(),
                        ),
                    );
                }
                Some(serde_json::Value::Array(kinds))
                    if kinds.iter().any(|kind| kind.as_str() == Some("string")) =>
                {
                    let mut allowed: Vec<serde_json::Value> = values
                        .iter()
                        .map(|value| serde_json::Value::String((*value).into()))
                        .collect();
                    if kinds.iter().any(serde_json::Value::is_null) {
                        allowed.push(serde_json::Value::Null);
                    }
                    object.insert("enum".into(), serde_json::Value::Array(allowed));
                }
                _ => {}
            }
            for key in ["anyOf", "oneOf", "allOf"] {
                if let Some(variants) = object
                    .get_mut(key)
                    .and_then(serde_json::Value::as_array_mut)
                {
                    for variant in variants {
                        set_string_schema_enum(variant, values);
                    }
                }
            }
        }
        serde_json::Value::Array(variants) => {
            for value in variants {
                set_string_schema_enum(value, values);
            }
        }
        _ => {}
    }
}

fn compact_evidence_bundle_schema(schema: &mut serde_json::Value) {
    let Some(properties) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    properties.insert(
        "sources".into(),
        serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "url": {"type": "string"},
                    "title": {"type": "string"},
                    "snippet": {"type": "string"},
                    "providers": {"type": "array", "items": {"type": "string"}},
                    "trust": {"type": "string"},
                    "trust_markers": {"type": "object", "additionalProperties": true},
                    "metadata": {"type": "object", "additionalProperties": true},
                    "quality": {"type": "object", "additionalProperties": true}
                },
                "additionalProperties": true
            }
        }),
    );
    properties.insert(
        "fetches".into(),
        serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "source_id": {"type": "string"},
                    "url": {"type": "string"},
                    "fetched": {"type": "boolean"},
                    "content_type": {"type": "string"},
                    "text": {"type": "string"},
                    "truncated": {"type": "boolean"},
                    "trust": {"type": "string"},
                    "warnings": {"type": "array", "items": {"type": "string"}}
                },
                "additionalProperties": true
            }
        }),
    );
    let response_detail = schema.pointer("/$defs/ResponseDetail").cloned();
    if let Some(object) = schema.as_object_mut() {
        if let Some(response_detail) = response_detail {
            object.insert(
                "$defs".into(),
                serde_json::json!({"ResponseDetail": response_detail}),
            );
        } else {
            object.remove("$defs");
        }
    }
}

fn compact_schema_value(value: &mut serde_json::Value, allowed: &[&str]) {
    match value {
        serde_json::Value::Object(object) => {
            let required = object
                .get("required")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(properties) = object.get_mut("properties").and_then(|v| v.as_object_mut()) {
                properties.retain(|key, _| {
                    allowed.contains(&key.as_str())
                        || required.iter().any(|entry| entry.as_str() == Some(key))
                });
                for value in properties.values_mut() {
                    compact_schema_value(value, &[]);
                }
            }
            object.remove("description");
            object.remove("title");
            object.insert("additionalProperties".into(), serde_json::Value::Bool(true));
            if let Some(defs) = object.get_mut("$defs").and_then(|v| v.as_object_mut()) {
                for value in defs.values_mut() {
                    compact_schema_value(value, &[]);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                compact_schema_value(value, &[]);
            }
        }
        _ => {}
    }
}

#[tool_router]
impl EggsearchServer {
    #[tool(
        name = "web_search",
        description = "Find candidate public web sources as source cards. Use for general research with `query`; inspect content with `web_fetch`. Returns cards only, never page text.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn web_search(
        &self,
        Parameters(args): Parameters<WebSearchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        self.respond(run_web_search(state, args).await)
    }

    #[tool(
        name = "provider_status",
        description = "Report configured providers, capabilities, and recipes for diagnostics. Use only when availability is in question; not a normal first research step.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn provider_status(
        &self,
        Parameters(args): Parameters<ProviderStatusArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        self.respond(run_provider_status_async(state, args).await)
    }

    #[tool(
        name = "repo_search",
        description = "Discover evidence for a repository across source, docs, issues, releases, and package metadata. Use for codebase investigation; use `repo_fetch` only after a concrete file/span is known.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn repo_search(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<RepoSearchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        let host_scope = context.meta.0 .0.get("codegg/host-local-scope").cloned();
        self.respond(run_repo_search_with_host_scope(state, args, host_scope).await)
    }

    #[tool(
        name = "web_fetch",
        description = "Fetch one known HTTP(S) URL with bounded extracted text. Use after search with `url`; for several targets use `batch_fetch`. Do not use for search or crawling.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn web_fetch(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<WebFetchArgs>,
    ) -> Result<CallToolResponse, McpError> {
        let state = self.state.clone();
        let mut progress = ProgressSink::from_context(&context);
        self.respond_response(run_web_fetch_with_progress(state, args, &mut progress).await)
    }

    #[tool(
        name = "repo_fetch",
        description = "Fetch a known repository file or span by structured locator. Use after `repo_search` with `owner`, `repo`, `path`; for arbitrary URLs use `web_fetch`.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn repo_fetch(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<RepoFetchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        let host_scope = context.meta.0 .0.get("codegg/host-local-scope").cloned();
        self.respond(run_repo_fetch_with_host_scope(state, args, host_scope).await)
    }

    #[tool(
        name = "security_search",
        description = "Find vulnerability and advisory evidence with applicability context. Use for CVE/GHSA/advisory questions with `query`; for general background use `web_search`.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn security_search(
        &self,
        Parameters(args): Parameters<SecuritySearchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        self.respond(run_security_search(state, args).await)
    }

    #[tool(
        name = "research_search",
        description = "Gather multi-source evidence for complex architectural questions. Use when flat search is insufficient with `query`; does not synthesize answers or fetch pages.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn research_search(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<ResearchSearchArgs>,
    ) -> Result<CallToolResponse, McpError> {
        let eligible = is_eligible("research_search", args.depth.as_deref(), 0);
        let task_args = args.clone();
        if let Some(offloaded) = self.spawn_task(eligible, move |state| {
            Box::pin(async move {
                let mut sink = ProgressSink::disabled();
                run_research_search_with_progress(state, task_args, &mut sink).await
            })
        }) {
            return task_response(offloaded);
        }
        let state = self.state.clone();
        let mut progress = ProgressSink::from_context(&context);
        self.respond_response(run_research_search_with_progress(state, args, &mut progress).await)
    }

    #[tool(
        name = "batch_fetch",
        description = "Fetch several known URLs or repo locators in one bounded call. Use for fan-out over selected `items`; not a crawler. Each item reports its own result.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn batch_fetch(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<BatchFetchArgs>,
    ) -> Result<CallToolResponse, McpError> {
        let item_count = args.items.len();
        let eligible = is_eligible("batch_fetch", None, item_count);
        let task_args = args.clone();
        if let Some(offloaded) = self.spawn_task(eligible, move |state| {
            Box::pin(async move {
                let mut sink = ProgressSink::disabled();
                run_batch_fetch_with_progress(state, task_args, &mut sink).await
            })
        }) {
            return task_response(offloaded);
        }
        let state = self.state.clone();
        let mut progress = ProgressSink::from_context(&context);
        self.respond_response(run_batch_fetch_with_progress(state, args, &mut progress).await)
    }

    #[tool(
        name = "repo_map",
        description = "Describe a repository layout without file contents. Use to orient before `repo_search` with `owner`, `repo`; not a substitute for search or fetch.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn repo_map(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<RepoMapArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        let host_scope = context.meta.0 .0.get("codegg/host-local-scope").cloned();
        self.respond(run_repo_map_with_host_scope(state, args, host_scope).await)
    }

    #[tool(
        name = "build_evidence_bundle",
        description = "Package already-selected search and fetch evidence for handoff. Use with gathered `sources` and `fetches`; does not search, fetch, or summarize.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn build_evidence_bundle(
        &self,
        Parameters(args): Parameters<EvidenceBundleArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.respond(run_build_evidence_bundle(args))
    }
}

#[tool_handler]
impl ServerHandler for EggsearchServer {
    fn get_info(&self) -> ServerConfig {
        let builder = ServerCapabilities::builder().enable_tools();
        let capabilities = if self.tasks.is_some() {
            builder.enable_tasks().build()
        } else {
            builder.build()
        };
        let implementation = Implementation::new("eggsearch", env!("CARGO_PKG_VERSION"));
        InitializeResult::new(capabilities)
            .with_instructions(self.instructions.as_ref())
            .with_server_info(implementation)
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, McpError> {
        match self.tasks.as_deref() {
            Some(runtime) => runtime.get_task(&request),
            None => Err(McpError::method_not_found::<rmcp::model::GetTaskMethod>()),
        }
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        match self.tasks.as_deref() {
            Some(runtime) => runtime.update_task(&request),
            None => Err(McpError::method_not_found::<rmcp::model::UpdateTaskMethod>()),
        }
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        match self.tasks.as_deref() {
            Some(runtime) => runtime.cancel_task(&request),
            None => Err(McpError::method_not_found::<rmcp::model::CancelTaskMethod>()),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let _ = &self.tool_router;
        let tools = self.tool_definitions();
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }
}

/// Apply canonical contract descriptions and annotations to tool metadata.
///
/// Annotations are static hints only. In particular `provider_status`
/// reports `open_world_hint=false` even though `probe=true` performs bounded
/// live liveness checks; annotations never vary by arguments and are not
/// permission controls.
///
/// Output schemas are attached from generated response types where the tool
/// returns a typed envelope, or from a permissive stable-envelope schema
/// where the runtime payload is an ad-hoc `json!` envelope with open-ended
/// metadata. Tools are sorted by name so `tools/list` is deterministic and
/// cacheable across transports and protocol eras.
fn apply_contract_metadata(tools: Vec<rmcp::model::Tool>) -> Vec<rmcp::model::Tool> {
    let mut tools: Vec<rmcp::model::Tool> = tools
        .into_iter()
        .map(|mut tool| {
            if let Some(contract) = crate::mcp::tool_contract::lookup(tool.name.as_ref()) {
                tool.description = Some(contract.description.into());
                tool.annotations = Some(contract.annotations());
            }
            if let Some(schema) = crate::mcp::output_schema::output_schema_for(tool.name.as_ref()) {
                tool.output_schema = Some(schema);
            }
            tool
        })
        .collect();
    tools.sort_by(|a, b| a.name.as_ref().cmp(b.name.as_ref()));
    tools
}

/// Deterministic content fingerprint of advertised tool contracts.
///
/// Hashes canonical names, descriptions, annotations, serialized
/// input/output schemas, and canonical discovery metadata (domain,
/// disclosure, purpose/use-when/not-for, keywords, aliases, related/next)
/// with FNV-1a. Changes only when the public contract changes. Exposed for
/// client-side `tools/list` caching and regression tests. Cache by this
/// fingerprint plus the server version, never by tool count; apply
/// hydration state after retrieving a cached base surface.
fn contract_fingerprint(tools: &[rmcp::model::Tool]) -> String {
    use std::hash::Hasher;
    let mut hasher = fnv1a64();
    for tool in tools {
        hash_str(&mut hasher, tool.name.as_ref());
        hash_str(&mut hasher, tool.description.as_deref().unwrap_or_default());
        if let Some(annotations) = &tool.annotations {
            hash_str(
                &mut hasher,
                &serde_json::to_string(annotations).unwrap_or_default(),
            );
        }
        hash_str(
            &mut hasher,
            &serde_json::to_string(&tool.input_schema).unwrap_or_default(),
        );
        hash_str(
            &mut hasher,
            &serde_json::to_string(&tool.output_schema).unwrap_or_default(),
        );
        if let Some(contract) = crate::mcp::tool_contract::lookup(tool.name.as_ref()) {
            hash_str(&mut hasher, contract.domain.as_str());
            hash_str(&mut hasher, contract.disclosure.as_str());
            hash_str(&mut hasher, contract.purpose);
            hash_str(&mut hasher, contract.use_when);
            hash_str(&mut hasher, contract.not_for);
            for keyword in contract.keywords {
                hash_str(&mut hasher, keyword);
            }
            for alias in contract.aliases {
                hash_str(&mut hasher, alias);
            }
            for related in contract.related_tools {
                hash_str(&mut hasher, related);
            }
            for next in contract.next_tools {
                hash_str(&mut hasher, next);
            }
        }
    }
    format!("{:016x}", hasher.finish())
}

fn fnv1a64() -> impl std::hash::Hasher {
    struct Fnv1a(u64);
    impl std::hash::Hasher for Fnv1a {
        fn write(&mut self, bytes: &[u8]) {
            const OFFSET: u64 = 0xcbf29ce484222325;
            const PRIME: u64 = 0x100000001b3;
            if self.0 == 0 {
                self.0 = OFFSET;
            }
            for b in bytes {
                self.0 ^= u64::from(*b);
                self.0 = self.0.wrapping_mul(PRIME);
            }
        }
        fn finish(&self) -> u64 {
            if self.0 == 0 {
                0xcbf29ce484222325
            } else {
                self.0
            }
        }
    }
    Fnv1a(0)
}

fn hash_str(hasher: &mut impl std::hash::Hasher, s: &str) {
    hasher.write(s.as_bytes());
    hasher.write_u8(0xff);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::AppConfig;

    fn server(tasks_enabled: bool, max_concurrent: usize) -> EggsearchServer {
        let mut config = AppConfig::default();
        config.mcp.tasks.enabled = tasks_enabled;
        config.mcp.tasks.max_concurrent = max_concurrent;
        let state = Arc::new(ServerState::build(config).expect("state builds"));
        EggsearchServer::new(state)
    }

    fn blocked_task(runtime: &TaskRuntime) -> rmcp::model::CreateTaskResult {
        runtime.spawn("working", |context| {
            Box::pin(async move {
                crate::mcp::tasks::run_cancellable(Some(context), async {
                    std::future::pending::<Result<CallToolResult, McpError>>().await
                })
                .await
            })
        })
    }

    #[test]
    fn tasks_capability_is_opt_in() {
        assert!(!server(false, 4).get_info().capabilities.supports_tasks());
        assert!(server(true, 4).get_info().capabilities.supports_tasks());
    }

    #[test]
    fn task_methods_are_absent_without_config() {
        let server = server(false, 4);
        assert!(server.task_runtime().is_none());
        let spawn = server.spawn_task(true, |_state| {
            Box::pin(async { Ok(serde_json::json!({"ok": true})) })
        });
        assert!(spawn.is_none(), "disabled tasks never offload");
    }

    #[test]
    fn ineligible_calls_never_offload() {
        let server = server(true, 4);
        let spawn = server.spawn_task(false, |_state| {
            Box::pin(async { Ok(serde_json::json!({"ok": true})) })
        });
        assert!(spawn.is_none());
    }

    #[tokio::test]
    async fn saturation_falls_back_to_the_synchronous_path() {
        let server = server(true, 1);
        let runtime = server.task_runtime().expect("runtime");
        let _held = blocked_task(runtime);
        assert!(runtime.at_capacity());
        let spawn = server.spawn_task(true, |_state| {
            Box::pin(async { Ok(serde_json::json!({"ok": true})) })
        });
        assert!(
            spawn.is_none(),
            "a saturated runtime must not error or offload"
        );
    }
}
