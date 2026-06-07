//! `eggsearch providers`: report provider configuration and status.

use anyhow::Result;
use eggsearch::core::config::AppConfig;
use eggsearch::core::provider::ProviderDescriptor;
use eggsearch::mcp::ServerState;

pub fn run(cfg: &AppConfig, as_json: bool) -> Result<()> {
    let state = ServerState::build(cfg.clone())?;
    let descriptors: Vec<ProviderDescriptor> = state.adapter.provider_status();

    if as_json {
        let payload = serde_json::json!({
            "providers": descriptors,
            "mode": format!("{:?}", cfg.search.mode),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        // Compute column widths dynamically for alignment
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

        println!(
            "{:<width_id$}  {:<8}  {:<8}  {:<width_kind$}  {:<5}  {:<12}  {:<width_caps$}",
            "ID",
            "Enabled",
            "Default",
            "Kind",
            "Key",
            "Configured",
            "Capabilities",
            width_id = id_w,
            width_kind = kind_w,
            width_caps = caps_w,
        );
        let total_width = id_w + 8 + 8 + kind_w + 5 + 12 + caps_w + 14; // separators
        println!("{}", "-".repeat(total_width));
        for d in &descriptors {
            let enabled = if d.enabled { "yes" } else { "no" };
            let default = if d.default { "yes" } else { "no" };
            let key = if d.requires_api_key { "yes" } else { "no" };
            let configured = if d.configured { "yes" } else { "no" };
            let caps = d.capabilities.summary();
            println!(
                "{:<width_id$}  {:<8}  {:<8}  {:<width_kind$}  {:<5}  {:<12}  {:<width_caps$}",
                d.id,
                enabled,
                default,
                kind_str(&d.kind),
                key,
                configured,
                caps,
                width_id = id_w,
                width_kind = kind_w,
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
    }
}
