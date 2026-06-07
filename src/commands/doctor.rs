//! `eggsearch doctor`: report environment / configuration status.

use anyhow::Result;
use eggsearch::core::config::AppConfig;
use eggsearch::mcp::ServerState;
use std::path::PathBuf;

pub async fn run(cfg: &AppConfig, config_path: Option<&PathBuf>, probe: bool) -> Result<()> {
    cfg.validate().map_err(|e| anyhow::anyhow!("{e}"))?;
    let state = ServerState::build(cfg.clone())?;

    let path_display = match config_path {
        Some(p) => p.display().to_string(),
        None => eggsearch::core::config::default_config_path()
            .display()
            .to_string(),
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "config_path": path_display,
            "mode": format!("{:?}", cfg.search.mode),
            "providers": state.adapter.provider_ids(),
        }))?
    );

    let healthy = !state.adapter.provider_ids().is_empty();
    if !healthy {
        anyhow::bail!("no providers enabled; enable at least one in [search].providers");
    }

    if probe {
        println!("\n--- Probing providers ---");
        probe_providers(&state).await?;
    }

    Ok(())
}

async fn probe_providers(state: &ServerState) -> Result<()> {
    use eggsearch::core::WebSearchRequest;

    let probe_query = "rust programming language";
    let timeout_per_provider = 5000;

    let mut all_failed = true;
    for provider_id in state.adapter.provider_ids() {
        let req = WebSearchRequest {
            query: probe_query.to_string(),
            max_results: Some(1),
            providers: vec![provider_id.clone()],
            safe_search: None,
            timeout_ms: Some(timeout_per_provider),
        };

        let start = std::time::Instant::now();
        let resp = state.adapter.web_search(&req, 1, 1).await;
        let elapsed = start.elapsed().as_millis() as u64;

        if resp.providers_failed.is_empty() {
            println!("  [OK]   {} ({}ms)", provider_id, elapsed);
            all_failed = false;
        } else {
            let msg = resp
                .providers_failed
                .first()
                .map(|f| f.message.as_str())
                .unwrap_or("unknown");
            let class = resp
                .providers_failed
                .first()
                .map(|f| f.error_class.as_str())
                .unwrap_or("unknown");
            println!(
                "  [FAIL] {} ({}ms) - {}: {}",
                provider_id, elapsed, class, msg
            );
        }
    }

    if all_failed {
        anyhow::bail!("all providers failed");
    }

    Ok(())
}
