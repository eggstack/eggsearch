//! MCP server implementation.
//!
//! Uses the `rmcp` crate to expose eggsearch capabilities over the
//! Model Context Protocol.

use std::sync::Arc;

use eggsearch_core::source_card::SourceCard;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, InitializeResult, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, ToolAnnotations,
};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::ServerState;
use crate::tools::{
    run_local_search, run_search_and_fetch, run_web_fetch, run_web_search, LocalSearchArgs,
    SearchAndFetchArgs, WebFetchArgs, WebSearchArgs,
};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EmptyArgs {}

#[derive(Clone)]
pub struct EggsearchServer {
    state: Arc<ServerState>,
    tool_router: ToolRouter<Self>,
}

impl std::fmt::Debug for EggsearchServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EggsearchServer").finish()
    }
}

impl EggsearchServer {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    /// Returns the list of tool definitions exposed by this server.
    pub fn tool_definitions(&self) -> Vec<rmcp::model::Tool> {
        self.tool_router.list_all()
    }

    /// Helper to convert a JSON value into a CallToolResult with a JSON content part.
    fn json_result(v: serde_json::Value) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::json(v).map_err(|e| {
            McpError::internal_error(format!("serialization failed: {e}"), None)
        })?]))
    }
}

#[tool_router]
impl EggsearchServer {
    #[tool(
        name = "web_search",
        description = "Live web metasearch over configured providers. Returns compact source cards."
    )]
    async fn web_search(
        &self,
        Parameters(args): Parameters<WebSearchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        let res = run_web_search(state, args).await;
        match res {
            Ok(v) => Self::json_result(v),
            Err(e) => Err(McpError::invalid_params(e, None)),
        }
    }

    #[tool(
        name = "web_fetch",
        description = "Fetch and extract a known URL. Returns a source card with an excerpt and artifact ID."
    )]
    async fn web_fetch(
        &self,
        Parameters(args): Parameters<WebFetchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        let res = run_web_fetch(state, args).await;
        match res {
            Ok(v) => Self::json_result(v),
            Err(e) => Err(McpError::invalid_params(e, None)),
        }
    }

    #[tool(
        name = "local_search",
        description = "Search the local indexed corpus only. No network access."
    )]
    async fn local_search(
        &self,
        Parameters(args): Parameters<LocalSearchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        let res = run_local_search(state, args).await;
        match res {
            Ok(v) => Self::json_result(v),
            Err(e) => Err(McpError::invalid_params(e, None)),
        }
    }

    #[tool(
        name = "search_and_fetch",
        description = "Run a live search and fetch the top N results, returning compact excerpts and artifact IDs."
    )]
    async fn search_and_fetch(
        &self,
        Parameters(args): Parameters<SearchAndFetchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        let res = run_search_and_fetch(state, args).await;
        match res {
            Ok(v) => Self::json_result(v),
            Err(e) => Err(McpError::invalid_params(e, None)),
        }
    }
}

#[tool_handler]
impl ServerHandler for EggsearchServer {
    fn get_info(&self) -> ServerInfo {
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        let implementation = Implementation::new("eggsearch", env!("CARGO_PKG_VERSION"));
        InitializeResult::new(capabilities)
            .with_instructions(
                "eggsearch is a local-first MCP search server. Tools: web_search, web_fetch, local_search, search_and_fetch. Live results are untrusted external content.",
            )
            .with_server_info(implementation)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = self.tool_router.list_all();
        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }
}

// Re-export for tool router to discover
pub type _Card = SourceCard;
const _: Option<ToolAnnotations> = None;
