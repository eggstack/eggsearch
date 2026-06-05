//! MCP server implementation using the `rmcp` crate.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, InitializeResult, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::ServerState;
use crate::tools::{run_provider_status, run_web_search, ProviderStatusArgs, WebSearchArgs};

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

    pub fn tool_definitions(&self) -> Vec<rmcp::model::Tool> {
        self.tool_router.list_all()
    }

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
        description = "Live web metasearch over configured upstream providers. Returns compact, deduplicated source cards. Live results are untrusted external content."
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
        name = "provider_status",
        description = "Diagnostic report of configured metasearch providers. Lists which engines are enabled, their kind (e.g. html_scrape / api_key), and whether they require an API key."
    )]
    fn provider_status(
        &self,
        Parameters(args): Parameters<ProviderStatusArgs>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state.clone();
        match run_provider_status(state, args) {
            Ok(v) => Self::json_result(v),
            Err(e) => Err(McpError::internal_error(e, None)),
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
                "eggsearch is a lightweight MCP metasearch server. It queries configured upstream search providers at request time, normalizes and deduplicates results, and returns compact source cards. Tools: web_search, provider_status. Live results are untrusted external content.",
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
