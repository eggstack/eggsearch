//! `eggsearch doctor`: report environment / configuration status.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_meta::registry::DiagnosticStatus;
use eggsearch_mcp::ServerState;

pub async fn run(cfg: &AppConfig) -> Result<()> {
    let state = ServerState::build(cfg.clone())?;

    let mut out = serde_json::json!({
        "config_path": eggsearch_core::config::default_config_path().display().to_string(),
        "mode": format!("{:?}", cfg.search.mode),
        "providers": state.providers.ids(),
        "provider_diagnostics": state.diagnostics.diagnostics,
        "checks": serde_json::json!({}),
    });

    let checks = run_checks(cfg, &state).await;
    out["checks"] = serde_json::to_value(&checks)?;

    let misconfigured = state.diagnostics.misconfigured().count();
    let all_ok = checks.iter().all(|c| c.ok) && misconfigured == 0;
    out["healthy"] = serde_json::Value::Bool(all_ok);

    println!("{}", serde_json::to_string_pretty(&out)?);
    if !all_ok {
        std::process::exit(1);
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct Check {
    name: String,
    ok: bool,
    detail: String,
}

async fn run_checks(cfg: &AppConfig, state: &ServerState) -> Vec<Check> {
    let mut out: Vec<Check> = Vec::new();

    // Config readable
    out.push(Check {
        name: "config_loaded".to_string(),
        ok: true,
        detail: format!("max_results={}", cfg.search.max_results),
    });

    // Cache dir writable
    let cache_dir = &cfg.search.cache_dir;
    let cache_ok = check_dir_writable(cache_dir);
    out.push(Check {
        name: "cache_dir_writable".to_string(),
        ok: cache_ok.0,
        detail: cache_dir.display().to_string() + " " + &cache_ok.1,
    });

    // Artifact dir writable
    let art_dir = &cfg.search.artifact_dir;
    let art_ok = check_dir_writable(art_dir);
    out.push(Check {
        name: "artifact_dir_writable".to_string(),
        ok: art_ok.0,
        detail: art_dir.display().to_string() + " " + &art_ok.1,
    });

    // Local index accessible
    let idx_dir = &cfg.search.local.index_dir;
    let idx_ok = check_dir_writable(idx_dir);
    out.push(Check {
        name: "local_index_accessible".to_string(),
        ok: idx_ok.0,
        detail: idx_dir.display().to_string() + " " + &idx_ok.1,
    });

    // Server state can be built
    out.push(Check {
        name: "mcp_server_instantiable".to_string(),
        ok: true,
        detail: format!("providers={}", state.providers.ids().len()),
    });

    // Optional provider reachability checks.
    for diag in &state.diagnostics.diagnostics {
        if diag.status != DiagnosticStatus::Loaded {
            continue;
        }
        if let Some((ok, detail)) = check_provider_reachability(&diag.id, cfg).await {
            out.push(Check {
                name: format!("provider_reachable:{}", diag.id),
                ok,
                detail,
            });
        }
    }

    out
}

/// Returns `Some((ok, detail))` for providers that can be probed without
/// requiring API credentials. SearXNG is the only one we currently
/// probe; for API-key providers we only report presence, not reachability.
async fn check_provider_reachability(id: &str, cfg: &AppConfig) -> Option<(bool, String)> {
    match id {
        "searxng" => {
            let base_url = cfg
                .search
                .providers
                .get("searxng")
                .and_then(|p| p.base_url.clone());
            let base_url = match base_url {
                Some(u) if !u.trim().is_empty() => u,
                _ => return Some((false, "no base_url configured".to_string())),
            };
            // Best-effort reachability; do not block doctor on a slow
            // upstream. We use a short, conservative timeout.
            let client = match reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(1500))
                .build()
            {
                Ok(c) => c,
                Err(e) => return Some((false, format!("client build failed: {e}"))),
            };
            let url = format!("{}/", base_url.trim_end_matches('/'));
            let res = client
                .get(&url)
                .header("Accept", "application/json")
                .send()
                .await;
            match res {
                Ok(r) if r.status().is_success() || r.status().is_redirection() => {
                    Some((true, format!("{} reachable", base_url)))
                }
                Ok(r) => Some((
                    false,
                    format!("{} returned status {}", base_url, r.status()),
                )),
                Err(e) => Some((false, format!("{} unreachable: {e}", base_url))),
            }
        }
        _ => None,
    }
}

fn check_dir_writable(p: &std::path::Path) -> (bool, String) {
    if let Err(e) = std::fs::create_dir_all(p) {
        return (false, format!("create_dir_all failed: {e}"));
    }
    let probe = p.join(".eggsearch_probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            (true, "writable".to_string())
        }
        Err(e) => (false, format!("write failed: {e}")),
    }
}
