//! `eggsearch providers`: report provider configuration and status.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_mcp::ServerState;

pub async fn run(cfg: &AppConfig, as_json: bool) -> Result<()> {
    let state = ServerState::build(cfg.clone())?;

    let mut rows: Vec<serde_json::Value> = Vec::new();
    for diag in &state.diagnostics.diagnostics {
        let extra = cfg.search.providers.get(&diag.id);
        let api_key_env = extra.and_then(|p| p.api_key_env.clone());
        let base_url = extra.and_then(|p| p.base_url.clone());
        let api_key_present = api_key_env
            .as_ref()
            .map(|v| !v.is_empty() && std::env::var(v).is_ok())
            .unwrap_or(false);

        rows.push(serde_json::json!({
            "id": diag.id,
            "enabled": diag.enabled,
            "status": diag.status,
            "message": diag.message,
            "base_url": base_url,
            "api_key_env": api_key_env,
            "api_key_present": api_key_present,
        }));
    }

    let summary = serde_json::json!({
        "loaded": state.diagnostics.loaded,
        "providers": rows,
    });

    if as_json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("Loaded providers ({}): ", state.diagnostics.loaded.len());
        for id in &state.diagnostics.loaded {
            println!("  - {id}");
        }
        println!();
        println!("Per-provider status:");
        for r in &rows {
            let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            let enabled = r.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let status = r.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            let mut line = format!("  {id:<20} enabled={enabled:<5} status={status:<14}");
            if let Some(env) = r.get("api_key_env").and_then(|v| v.as_str()) {
                let present = r
                    .get("api_key_present")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                line.push_str(&format!(" key={env} present={present}"));
            }
            if let Some(url) = r.get("base_url").and_then(|v| v.as_str()) {
                line.push_str(&format!(" base_url={url}"));
            }
            if let Some(msg) = r.get("message").and_then(|v| v.as_str()) {
                line.push_str(&format!(" -- {msg}"));
            }
            println!("{line}");
        }

        let misconfigured: Vec<&serde_json::Value> = rows
            .iter()
            .filter(|r| r.get("status").and_then(|v| v.as_str()) == Some("misconfigured"))
            .collect();
        if !misconfigured.is_empty() {
            println!(
                "\nMisconfigured providers will be skipped at search time. Fix the config or unset the env var to silence these."
            );
            std::process::exit(1);
        }
    }
    Ok(())
}
