//! `eggsearch fetch`: fetch and extract content from a URL.

use anyhow::{anyhow, Result};
use eggsearch::core::config::AppConfig;
use eggsearch::core::fetch::ExtractMode;
use eggsearch::fetch::FetchClient;

pub async fn run(
    cfg: &AppConfig,
    url: &str,
    max_chars: Option<usize>,
    timeout_ms: Option<u64>,
    metadata_only: bool,
    include_links: bool,
    as_json: bool,
) -> Result<()> {
    if !cfg.fetch.enabled {
        anyhow::bail!("fetch is disabled in config; enable [fetch].enabled to use this command");
    }

    let mut limits = cfg.fetch_limits();
    if let Some(t) = timeout_ms {
        limits.timeout_ms = t;
    }

    let client = FetchClient::new(limits, cfg.fetch_user_agent())?;

    let extract_mode = if metadata_only {
        ExtractMode::MetadataOnly
    } else {
        ExtractMode::Text
    };

    let response = client
        .fetch(url, max_chars, extract_mode, include_links)
        .await
        .map_err(|e| anyhow!("fetch failed: {}: {}", e.error_code(), e))?;

    if as_json {
        let payload = serde_json::json!({
            "url": response.url,
            "final_url": response.final_url,
            "title": response.title,
            "description": response.description,
            "content_type": response.content_type,
            "status": response.status,
            "fetched": response.fetched,
            "truncated": response.truncated,
            "trust": "external_untrusted",
            "text": response.text,
            "links": response.links,
            "warnings": response.warnings,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("# Fetch: {}\n", url);
        println!("Final URL: {}", response.final_url);
        if let Some(title) = &response.title {
            println!("Title: {}", title);
        }
        if let Some(desc) = &response.description {
            println!("Description: {}", desc);
        }
        println!("Status: {}", response.status);
        println!(
            "Content-Type: {}",
            response.content_type.as_deref().unwrap_or("unknown")
        );
        println!("Fetched: {}", response.fetched);
        println!("Truncated: {}", response.truncated);
        if let Some(text) = &response.text {
            println!("\n--- Content ({} chars) ---", text.chars().count());
            println!("{}", text);
        }
        if !response.links.is_empty() {
            println!("\n--- Links ({} links) ---", response.links.len());
            for link in response.links.iter().take(20) {
                println!("  - {}: {}", link.text, link.url);
            }
        }
        if !response.warnings.is_empty() {
            println!("\nWarnings:");
            for w in &response.warnings {
                println!("  - {}", w);
            }
        }
    }

    Ok(())
}
