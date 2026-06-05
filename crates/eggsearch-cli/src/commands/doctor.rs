//! `eggsearch doctor`: report environment / configuration status.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_mcp::ServerState;

pub async fn run(cfg: &AppConfig) -> Result<()> {
    let state = ServerState::build(cfg.clone())?;

    let out = serde_json::json!({
        "config_path": eggsearch_core::config::default_config_path().display().to_string(),
        "mode": format!("{:?}", cfg.search.mode),
        "providers": state.adapter.provider_ids(),
    });

    println!("{}", serde_json::to_string_pretty(&out)?);

    // The changeover is "healthy" if at least one provider is enabled
    // and the adapter could be constructed.
    let healthy = !state.adapter.provider_ids().is_empty();
    if !healthy {
        eprintln!("\nNo providers enabled. Enable at least one in [search].providers.");
        std::process::exit(1);
    }
    Ok(())
}
