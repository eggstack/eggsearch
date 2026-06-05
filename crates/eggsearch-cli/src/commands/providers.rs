//! `eggsearch providers`: report provider configuration and status.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_mcp::ServerState;

pub async fn run(cfg: &AppConfig, as_json: bool) -> Result<()> {
    let state = ServerState::build(cfg.clone())?;
    let statuses = state.adapter.provider_status();

    if as_json {
        let payload = serde_json::json!({
            "providers": statuses,
            "mode": format!("{:?}", cfg.search.mode),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("Configured providers:");
        for s in &statuses {
            let key = if s.requires_api_key { " (api key required)" } else { "" };
            let enabled = if s.enabled { "on " } else { "off" };
            println!(
                "  {} {:<12} kind={:<12}{}",
                enabled,
                s.id,
                s.kind,
                key
            );
        }
    }
    Ok(())
}
