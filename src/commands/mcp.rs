//! MCP server transport startup commands.

use anyhow::Result;
use eggsearch::core::config::AppConfig;
use eggsearch::mcp::ServeOptions;
use rmcp::ServiceExt;
use tracing::info;

pub async fn run_stdio(cfg: &AppConfig) -> Result<()> {
    let server = eggsearch::mcp::build_server(cfg.clone())?;
    info!("starting eggsearch MCP server over stdio");
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

pub async fn run_http(cfg: &AppConfig, options: ServeOptions) -> Result<()> {
    eggsearch::mcp::http::run(cfg, options).await
}
