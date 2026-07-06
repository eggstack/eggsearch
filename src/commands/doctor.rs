//! `eggsearch doctor`: report environment / configuration status.

use anyhow::Result;
use eggsearch::core::config::AppConfig;
use eggsearch::core::provider::{is_api_provider, provider_configured_state, KNOWN_PROVIDER_IDS};
use eggsearch::mcp::ServerState;
use eggsearch::meta::local_backend::LocalWorkspaceBackend;
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
    let local_backend_available = LocalWorkspaceBackend::new(cfg.local.clone())
        .map(|backend| backend.is_enabled())
        .unwrap_or(false);
    let enabled_ids = KNOWN_PROVIDER_IDS
        .iter()
        .filter(|id| provider_enabled_state(cfg, id, local_backend_available))
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "config_path": path_display,
            "config_file_exists": config_file_exists,
            "config_file_loaded": config_file_loaded,
            "mode": format!("{:?}", cfg.search.mode),
            "providers": {
                "enabled": enabled_ids,
                "default": cfg.search.default_providers,
                "disabled": {
                    "known": KNOWN_PROVIDER_IDS.iter()
                        .filter(|id| !provider_enabled_state(cfg, id, local_backend_available))
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>(),
                },
                "capabilities": provider_capability_summary(cfg, local_backend_available),
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

fn provider_capability_summary(
    cfg: &AppConfig,
    local_backend_available: bool,
) -> Vec<serde_json::Value> {
    use eggsearch::core::provider::built_in_provider_descriptor;

    let mut out = Vec::new();
    let enabled_set: std::collections::BTreeSet<&str> = KNOWN_PROVIDER_IDS
        .iter()
        .copied()
        .filter(|id| provider_enabled_state(cfg, id, local_backend_available))
        .collect();
    let default_set: std::collections::BTreeSet<&str> = cfg
        .search
        .default_providers
        .iter()
        .map(|s| s.as_str())
        .collect();

    for id in KNOWN_PROVIDER_IDS {
        let enabled = enabled_set.contains(id);
        let is_default = default_set.contains(id);
        let searxng_configured = cfg
            .search
            .searxng
            .base_url
            .as_deref()
            .is_some_and(|u| url::Url::parse(u).is_ok());
        let api_configured = cfg.search.api.get(*id).is_some_and(|api_cfg| {
            api_cfg.enabled
                && api_cfg
                    .api_key_env
                    .as_deref()
                    .is_some_and(|env| std::env::var(env).is_ok())
        });
        let configured = provider_configured_state(
            id,
            searxng_configured,
            api_configured,
            local_backend_available,
        );
        if let Some(desc) =
            built_in_provider_descriptor(id, enabled, is_default, configured, false, None)
        {
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

fn provider_enabled_state(cfg: &AppConfig, id: &str, local_backend_available: bool) -> bool {
    match id {
        "local_workspace" => local_backend_available,
        "searxng" => {
            cfg.search.providers.get(id).copied().unwrap_or(false) && cfg.search.searxng.enabled
        }
        _ if is_api_provider(id) => cfg
            .search
            .api
            .get(id)
            .is_some_and(|api_cfg| api_cfg.enabled),
        _ => cfg.search.providers.get(id).copied().unwrap_or(false),
    }
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
                    "API provider '{id}' is enabled but its api_key_env is not set"
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
            println!("  [FAIL]   {provider_id} ({elapsed}ms) - {class}: {msg}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use eggsearch::core::config::{ApiProviderConfig, AppConfig, SearxngConfig};

    #[test]
    fn provider_capability_summary_reflects_default_configuration() {
        let cfg = AppConfig::default();
        let summary = provider_capability_summary(&cfg, false);

        let duck = summary
            .iter()
            .find(|p| p["id"].as_str() == Some("duckduckgo"))
            .expect("duckduckgo provider");
        assert_eq!(duck["enabled"], true);
        assert_eq!(duck["configured"], true);

        let osv = summary
            .iter()
            .find(|p| p["id"].as_str() == Some("osv"))
            .expect("osv provider");
        assert_eq!(osv["enabled"], true);
        assert_eq!(osv["configured"], true);

        let brave_api = summary
            .iter()
            .find(|p| p["id"].as_str() == Some("brave_api"))
            .expect("brave_api provider");
        assert_eq!(brave_api["enabled"], false);
        assert_eq!(brave_api["configured"], false);

        let local = summary
            .iter()
            .find(|p| p["id"].as_str() == Some("local_workspace"))
            .expect("local_workspace provider");
        assert_eq!(local["enabled"], false);
        assert_eq!(local["configured"], false);
    }

    #[test]
    fn provider_capability_summary_marks_api_provider_configured_when_env_set() {
        let env = "EGGSEARCH_DOCTOR_TEST_API_ENABLED_KEY";
        std::env::set_var(env, "test_key");

        let mut cfg = AppConfig::default();
        cfg.search.api.insert(
            "brave_api".to_string(),
            ApiProviderConfig {
                enabled: true,
                api_key_env: Some(env.to_string()),
                base_url: None,
            },
        );

        let summary = provider_capability_summary(&cfg, false);
        std::env::remove_var(env);

        let brave_api = summary
            .iter()
            .find(|p| p["id"].as_str() == Some("brave_api"))
            .expect("brave_api provider");
        assert_eq!(brave_api["enabled"], true);
        assert_eq!(brave_api["configured"], true);
    }

    #[test]
    fn provider_capability_summary_marks_api_provider_unconfigured_when_env_missing() {
        let env = "EGGSEARCH_DOCTOR_TEST_API_MISSING_KEY";
        std::env::remove_var(env);

        let mut cfg = AppConfig::default();
        cfg.search.api.insert(
            "brave_api".to_string(),
            ApiProviderConfig {
                enabled: true,
                api_key_env: Some(env.to_string()),
                base_url: None,
            },
        );

        let summary = provider_capability_summary(&cfg, false);
        let brave_api = summary
            .iter()
            .find(|p| p["id"].as_str() == Some("brave_api"))
            .expect("brave_api provider");
        assert_eq!(brave_api["enabled"], true);
        assert_eq!(brave_api["configured"], false);
    }

    #[test]
    fn provider_capability_summary_marks_searxng_configured_when_base_url_set() {
        let mut cfg = AppConfig::default();
        cfg.search.providers.insert("searxng".to_string(), true);
        cfg.search.searxng = SearxngConfig {
            enabled: true,
            base_url: Some("https://search.example.org".to_string()),
        };

        let summary = provider_capability_summary(&cfg, false);
        let searxng = summary
            .iter()
            .find(|p| p["id"].as_str() == Some("searxng"))
            .expect("searxng provider");
        assert_eq!(searxng["enabled"], true);
        assert_eq!(searxng["configured"], true);
    }

    #[test]
    fn provider_capability_summary_marks_local_workspace_available_when_backend_exists() {
        let cfg = AppConfig::default();
        let summary = provider_capability_summary(&cfg, true);
        let local = summary
            .iter()
            .find(|p| p["id"].as_str() == Some("local_workspace"))
            .expect("local_workspace provider");
        assert_eq!(local["enabled"], true);
        assert_eq!(local["configured"], true);
    }
}
