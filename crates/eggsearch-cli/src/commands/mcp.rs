//! `eggsearch mcp stdio`: run the MCP server over stdio.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_mcp::{EggsearchServer, ServerState};
use rmcp::ServiceExt;
use std::sync::Arc;
use tracing::info;

pub async fn run_stdio(cfg: &AppConfig) -> Result<()> {
    let state = Arc::new(ServerState::build(cfg.clone())?);
    let server = EggsearchServer::new(state);
    info!("starting eggsearch MCP server over stdio");
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
