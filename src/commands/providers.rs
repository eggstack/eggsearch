//! `eggsearch providers`: report provider configuration and status.

use anyhow::Result;
use eggsearch::core::config::AppConfig;
use eggsearch::core::provider::ProviderDescriptor;
use eggsearch::mcp::ServerState;

pub fn run(cfg: &AppConfig, as_json: bool) -> Result<()> {
    let state = ServerState::build(cfg.clone())?;
    let mut descriptors: Vec<ProviderDescriptor> = state.adapter.provider_status();
    if let Some(desc) = descriptors.iter_mut().find(|d| d.id == "local_workspace") {
        let backend_enabled = state.local_backend.is_some();
        desc.enabled = backend_enabled;
        desc.configured = backend_enabled;
    }

    // Build per-provider health views
    let health_registry = state.adapter.health();
    let health_views: Vec<_> = descriptors
        .iter()
        .map(|d| health_registry.health_view(&d.id))
        .collect();

    if as_json {
        let providers_with_health: Vec<_> = descriptors
            .iter()
            .zip(health_views.iter())
            .map(|(desc, hv)| {
                serde_json::json!({
                    "id": desc.id,
                    "enabled": desc.enabled,
                    "default": desc.default,
                    "kind": kind_str(&desc.kind),
                    "requires_api_key": desc.requires_api_key,
                    "configured": desc.configured,
                    "routable": desc.routable,
                    "skip_reason": desc.skip_reason,
                    "skip_code": desc.skip_code.map(|c| c.as_str()),
                    "capabilities": desc.capabilities.summary(),
                    "health": hv,
                })
            })
            .collect();
        let payload = serde_json::json!({
            "providers": providers_with_health,
            "mode": format!("{:?}", cfg.search.mode),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        let id_w = descriptors
            .iter()
            .map(|d| d.id.len())
            .max()
            .unwrap_or(2)
            .max(2);
        let kind_w = descriptors
            .iter()
            .map(|d| kind_str(&d.kind).len())
            .max()
            .unwrap_or(10)
            .max(10);
        let caps_w = descriptors
            .iter()
            .map(|d| d.capabilities.summary().len())
            .max()
            .unwrap_or(12)
            .max(12);
        let skip_w = descriptors
            .iter()
            .map(|d| d.skip_code.map(|c| c.as_str().len()).unwrap_or(1).max(1))
            .max()
            .unwrap_or(5)
            .max(5);
        let health_w = health_views
            .iter()
            .map(|hv| format!("{:?}", hv.status).len())
            .max()
            .unwrap_or(7)
            .max(7);

        println!(
            "{:<width_id$}  {:<8}  {:<8}  {:<width_kind$}  {:<5}  {:<12}  {:<8}  {:<width_skip$}  {:<width_health$}  {:<width_caps$}",
            "ID",
            "Enabled",
            "Default",
            "Kind",
            "Key",
            "Configured",
            "Routable",
            "SkipCode",
            "Health",
            "Capabilities",
            width_id = id_w,
            width_kind = kind_w,
            width_skip = skip_w,
            width_health = health_w,
            width_caps = caps_w,
        );
        let total_width = id_w + 8 + 8 + kind_w + 5 + 12 + 8 + skip_w + health_w + caps_w + 22;
        println!("{}", "-".repeat(total_width));
        for (d, hv) in descriptors.iter().zip(health_views.iter()) {
            let enabled = if d.enabled { "yes" } else { "no" };
            let default = if d.default { "yes" } else { "no" };
            let key = if d.requires_api_key { "yes" } else { "no" };
            let configured = if d.configured { "yes" } else { "no" };
            let routable = if d.routable { "yes" } else { "no" };
            let skip_code = d
                .skip_code
                .map(|c| c.as_str().to_string())
                .unwrap_or_else(|| "-".to_string());
            let health_status = format!("{:?}", hv.status);
            let caps = d.capabilities.summary();
            println!(
                "{:<width_id$}  {:<8}  {:<8}  {:<width_kind$}  {:<5}  {:<12}  {:<8}  {:<width_skip$}  {:<width_health$}  {:<width_caps$}",
                d.id,
                enabled,
                default,
                kind_str(&d.kind),
                key,
                configured,
                routable,
                skip_code,
                health_status,
                caps,
                width_id = id_w,
                width_kind = kind_w,
                width_skip = skip_w,
                width_health = health_w,
                width_caps = caps_w,
            );
        }
    }
    Ok(())
}

fn kind_str(kind: &eggsearch::core::provider::ProviderKind) -> &'static str {
    match kind {
        eggsearch::core::provider::ProviderKind::HtmlScrape => "html_scrape",
        eggsearch::core::provider::ProviderKind::JsonApi => "json_api",
        eggsearch::core::provider::ProviderKind::ApiKey => "api_key",
        eggsearch::core::provider::ProviderKind::Local => "local",
    }
}
