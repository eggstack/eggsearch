//! `eggsearch doctor`: report environment / configuration status.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_mcp::ServerState;

pub async fn run(cfg: &AppConfig) -> Result<()> {
    let mut out = serde_json::json!({
        "config_path": eggsearch_core::config::default_config_path().display().to_string(),
        "mode": format!("{:?}", cfg.search.mode),
        "providers": cfg.search.providers.iter().map(|(k, v)| {
            serde_json::json!({ "id": k, "enabled": v.enabled })
        }).collect::<Vec<_>>(),
        "checks": serde_json::json!({}),
    });

    let checks = run_checks(cfg).await;
    out["checks"] = serde_json::to_value(&checks)?;

    let all_ok = checks.iter().all(|c| c.ok);
    out["healthy"] = serde_json::Value::Bool(all_ok);

    println!("{}", serde_json::to_string_pretty(&out)?);
    if !all_ok {
        std::process::exit(1);
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
}

async fn run_checks(cfg: &AppConfig) -> Vec<Check> {
    let mut out: Vec<Check> = Vec::new();

    // Config readable
    out.push(Check {
        name: "config_loaded",
        ok: true,
        detail: format!("max_results={}", cfg.search.max_results),
    });

    // Cache dir writable
    let cache_dir = &cfg.search.cache_dir;
    let cache_ok = check_dir_writable(cache_dir);
    out.push(Check {
        name: "cache_dir_writable",
        ok: cache_ok.0,
        detail: cache_dir.display().to_string() + " " + &cache_ok.1,
    });

    // Artifact dir writable
    let art_dir = &cfg.search.artifact_dir;
    let art_ok = check_dir_writable(art_dir);
    out.push(Check {
        name: "artifact_dir_writable",
        ok: art_ok.0,
        detail: art_dir.display().to_string() + " " + &art_ok.1,
    });

    // Local index accessible
    let idx_dir = &cfg.search.local.index_dir;
    let idx_ok = check_dir_writable(idx_dir);
    out.push(Check {
        name: "local_index_accessible",
        ok: idx_ok.0,
        detail: idx_dir.display().to_string() + " " + &idx_ok.1,
    });

    // Server state can be built
    match ServerState::build(cfg.clone()) {
        Ok(_) => out.push(Check {
            name: "mcp_server_instantiable",
            ok: true,
            detail: "ok".to_string(),
        }),
        Err(e) => out.push(Check {
            name: "mcp_server_instantiable",
            ok: false,
            detail: e.to_string(),
        }),
    }

    out
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
