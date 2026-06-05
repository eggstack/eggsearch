//! `eggsearch doctor`: report environment / configuration status.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_mcp::ServerState;
use std::path::PathBuf;

pub async fn run(cfg: &AppConfig, config_path: Option<&PathBuf>) -> Result<()> {
    let state = ServerState::build(cfg.clone())?;

    let path_display = match config_path {
        Some(p) => p.display().to_string(),
        None => eggsearch_core::config::default_config_path().display().to_string(),
    };

    let out = serde_json::json!({
        "config_path": path_display,
        "mode": format!("{:?}", cfg.search.mode),
        "providers": state.adapter.provider_ids(),
    });

    println!("{}", serde_json::to_string_pretty(&out)?);

    // The changeover is "healthy" if at least one provider is enabled
    // and the adapter could be constructed.
    let healthy = !state.adapter.provider_ids().is_empty();
    if !healthy {
        anyhow::bail!("no providers enabled; enable at least one in [search].providers");
    }
    Ok(())
}
