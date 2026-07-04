//! `eggsearch doctor`: report environment / configuration status.

use anyhow::Result;
use eggsearch::core::config::AppConfig;
use eggsearch::core::provider::KNOWN_PROVIDER_IDS;
use eggsearch::mcp::ServerState;
use std::path::PathBuf;

pub async fn run(cfg: &AppConfig, config_path: Option<&PathBuf>, probe: bool) -> Result<()> {
    cfg.validate().map_err(|e| anyhow::anyhow!("{e}"))?;

    let resolved_path = match config_path {
        Some(p) => p.clone(),
        None => eggsearch::core::config::default_config_path(),
    };
    let path_display = resolved_path.display().to_string();
    let config_file_exists = resolved_path.exists();
    let config_file_loaded = if config_file_exists {
        AppConfig::load(&resolved_path).is_ok()
    } else {
        false
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "config_path": path_display,
            "config_file_exists": config_file_exists,
            "config_file_loaded": config_file_loaded,
            "mode": format!("{:?}", cfg.search.mode),
            "providers": {
                "enabled": cfg.effective_provider_ids(),
                "default": cfg.search.default_providers,
                "disabled": {
                    "known": KNOWN_PROVIDER_IDS.iter()
                        .filter(|id| cfg.search.providers.get(**id).is_some_and(|v| !*v))
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>(),
                },
                "capabilities": provider_capability_summary(cfg),
            },
            "search": {
                "default_max_results": cfg.search.default_max_results,
                "max_results_cap": cfg.search.max_results_cap,
            },
            "searxng": searxng_status(cfg),
            "api_providers": api_credential_status(cfg),
            "fetch": fetch_status(cfg),
            "warnings": collect_warnings(cfg),
        }))?
    );

    if cfg.search.mode == eggsearch::core::config::Mode::Off {
        if probe {
            println!("\n--- Skipping provider probe (mode=off) ---");
        }
        return Ok(());
    }

    let state = ServerState::build(cfg.clone())?;
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

fn provider_capability_summary(cfg: &AppConfig) -> Vec<serde_json::Value> {
    use eggsearch::core::provider::built_in_provider_descriptor;

    let mut out = Vec::new();
    let default_set: std::collections::BTreeSet<&str> = cfg
        .search
        .default_providers
        .iter()
        .map(|s| s.as_str())
        .collect();

    for id in KNOWN_PROVIDER_IDS {
        let enabled = cfg.search.providers.get(*id).copied().unwrap_or(false);
        let is_default = default_set.contains(id);
        let configured = match *id {
            "searxng" => {
                cfg.search.searxng.enabled
                    && cfg
                        .search
                        .searxng
                        .base_url
                        .as_deref()
                        .is_some_and(|u| !u.is_empty())
            }
            _ => true,
        };
        if let Some(desc) = built_in_provider_descriptor(id, enabled, is_default, configured) {
            out.push(serde_json::json!({
                "id": desc.id,
                "enabled": desc.enabled,
                "default": desc.default,
                "kind": provider_kind_str(&desc.kind),
                "configured": desc.configured,
                "capabilities": desc.capabilities.summary(),
            }));
        }
    }
    out
}

fn provider_kind_str(kind: &eggsearch::core::provider::ProviderKind) -> &'static str {
    match kind {
        eggsearch::core::provider::ProviderKind::HtmlScrape => "html_scrape",
        eggsearch::core::provider::ProviderKind::JsonApi => "json_api",
        eggsearch::core::provider::ProviderKind::ApiKey => "api_key",
        eggsearch::core::provider::ProviderKind::Local => "local",
    }
}

fn searxng_status(cfg: &AppConfig) -> serde_json::Value {
    let base_url_valid = cfg
        .search
        .searxng
        .base_url
        .as_ref()
        .map(|u| url::Url::parse(u).is_ok())
        .unwrap_or(false);
    serde_json::json!({
        "enabled": cfg.search.searxng.enabled,
        "base_url_set": cfg.search.searxng.base_url.is_some(),
        "base_url_valid": base_url_valid,
    })
}

fn api_credential_status(cfg: &AppConfig) -> Vec<serde_json::Value> {
    cfg.search
        .api
        .iter()
        .map(|(id, api_cfg)| {
            let env_set = api_cfg
                .api_key_env
                .as_ref()
                .map(|env| std::env::var(env).is_ok())
                .unwrap_or(false);
            serde_json::json!({
                "id": id,
                "enabled": api_cfg.enabled,
                "api_key_env": api_cfg.api_key_env,
                "api_key_set": env_set,
            })
        })
        .collect()
}

fn fetch_status(cfg: &AppConfig) -> serde_json::Value {
    serde_json::json!({
        "enabled": cfg.fetch.enabled,
        "timeout_ms": cfg.fetch.timeout_ms,
        "max_bytes": cfg.fetch.max_bytes,
        "max_chars_default": cfg.fetch.max_chars_default,
        "max_chars_cap": cfg.fetch.max_chars_cap,
        "redirect_limit": cfg.fetch.redirect_limit,
        "allow_private_network": cfg.fetch.allow_private_network,
        "allow_localhost": cfg.fetch.allow_localhost,
        "include_links_default": cfg.fetch.include_links_default,
    })
}

fn collect_warnings(cfg: &AppConfig) -> Vec<String> {
    let mut warnings = Vec::new();

    let disabled_defaults = cfg.misconfigured_default_providers();
    if !disabled_defaults.is_empty() {
        warnings.push(format!(
            "default_providers contains unavailable provider(s): {}",
            disabled_defaults.join(", ")
        ));
    }

    // SearXNG configured but disabled
    if cfg.search.searxng.enabled
        && cfg
            .search
            .searxng
            .base_url
            .as_deref()
            .is_some_and(|u| !u.is_empty())
        && !cfg
            .search
            .providers
            .get("searxng")
            .copied()
            .unwrap_or(false)
    {
        warnings.push(
            "[search].searxng is configured but [search].providers.searxng is disabled".to_string(),
        );
    }

    // API providers enabled without key
    for (id, api_cfg) in &cfg.search.api {
        if api_cfg.enabled {
            let key_set = api_cfg
                .api_key_env
                .as_ref()
                .map(|env| std::env::var(env).is_ok())
                .unwrap_or(false);
            if !key_set {
                warnings.push(format!(
                    "API provider '{}' is enabled but its api_key_env is not set",
                    id
                ));
            }
        }
    }

    // Fetch policy warnings
    if !cfg.fetch.allow_private_network && !cfg.fetch.allow_localhost {
        // This is the secure default; not a warning.
    } else if cfg.fetch.allow_private_network || cfg.fetch.allow_localhost {
        let mut parts = Vec::new();
        if cfg.fetch.allow_private_network {
            parts.push("allow_private_network=true");
        }
        if cfg.fetch.allow_localhost {
            parts.push("allow_localhost=true");
        }
        warnings.push(format!(
            "fetch network policy is permissive: {}",
            parts.join(", ")
        ));
    }

    warnings
}

async fn probe_providers(state: &ServerState) -> Result<()> {
    use eggsearch::core::WebSearchRequest;

    let probe_query = "test";
    let timeout_per_provider = 3000;

    let mut all_failed = true;
    for provider_id in state.adapter.provider_ids() {
        let req = WebSearchRequest {
            query: probe_query.to_string(),
            max_results: Some(1),
            providers: vec![provider_id.clone()],
            safe_search: None,
            timeout_ms: Some(timeout_per_provider),
            intent: eggsearch::core::query::SearchIntent::default(),
            freshness: eggsearch::core::query::Freshness::default(),
        };

        let start = std::time::Instant::now();
        let resp = state
            .adapter
            .web_search(&req, 1, state.config.search.max_results_cap)
            .await;
        let elapsed = start.elapsed().as_millis() as u64;

        if resp.providers_failed.is_empty() {
            println!(
                "  [OK]     {} ({}ms, {} result(s))",
                provider_id,
                elapsed,
                resp.results.len()
            );
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
                "  [FAIL]   {} ({}ms) - {}: {}",
                provider_id, elapsed, class, msg
            );
            if !resp.results.is_empty() {
                println!(
                    "           (returned {} result(s) despite failure)",
                    resp.results.len()
                );
            }
        }
    }

    if all_failed {
        anyhow::bail!("all providers failed");
    }

    Ok(())
}
