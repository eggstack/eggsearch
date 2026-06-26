//! `eggsearch fetch`: fetch and extract content from a URL.

use anyhow::{anyhow, Result};
use eggsearch::core::config::AppConfig;
use eggsearch::core::fetch::ExtractMode;
use eggsearch::fetch::FetchClient;

/// CLI display cap for links. Distinct from [`crate::fetch::extract::MAX_LINKS`]
/// (the in-memory extractor cap) because the CLI is human-facing and only
/// needs enough links to be useful for a glance at the page; the extractor
/// returns more so that programmatic consumers can pick from a richer set.
const CLI_DISPLAY_MAX_LINKS: usize = 20;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    cfg: &AppConfig,
    url: &str,
    max_chars: Option<usize>,
    timeout_ms: Option<u64>,
    metadata_only: bool,
    markdown: bool,
    include_links: bool,
    as_json: bool,
) -> Result<()> {
    if !cfg.fetch.enabled {
        anyhow::bail!("fetch is disabled in config; set [fetch].enabled = true to enable");
    }

    if let Some(0) = max_chars {
        anyhow::bail!("max_chars must be > 0");
    }

    let mut limits = cfg.fetch_limits();
    if let Some(t) = timeout_ms {
        limits.timeout_ms = t;
    }

    let client = FetchClient::new(limits, cfg.fetch_user_agent(), cfg.fetch.sanitize_output)?;

    let extract_mode = if metadata_only {
        ExtractMode::MetadataOnly
    } else if markdown {
        ExtractMode::Markdown
    } else {
        ExtractMode::Text
    };

    let include_links = include_links || cfg.fetch.include_links_default;

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
            "trust_markers": response.trust_markers,
            "document": response.document,
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
            println!(
                "\n--- Links ({} returned, {} seen{}) ---",
                response.links.len(),
                response.links_seen.unwrap_or(response.links.len()),
                if response.links_truncated {
                    ", truncated"
                } else {
                    ""
                }
            );
            for link in response.links.iter().take(CLI_DISPLAY_MAX_LINKS) {
                println!(
                    "  - [{}] {}: {}",
                    format!("{:?}", link.link_kind).to_lowercase(),
                    link.text,
                    link.url
                );
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

#[cfg(test)]
mod tests {
    use super::*;
    use eggsearch::core::config::AppConfig;

    #[tokio::test]
    async fn run_zero_max_chars_returns_error() {
        let cfg = AppConfig::default();
        let err = run(
            &cfg,
            "https://example.com",
            Some(0),
            None,
            false,
            false,
            false,
            false,
        )
        .await
        .expect_err("expected max_chars validation error");
        assert!(
            err.to_string().contains("max_chars must be > 0"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn run_disabled_by_config_returns_error() {
        let mut cfg = AppConfig::default();
        cfg.fetch.enabled = false;
        let err = run(
            &cfg,
            "https://example.com",
            None,
            None,
            false,
            false,
            false,
            false,
        )
        .await
        .expect_err("expected fetch-disabled error");
        assert!(err.to_string().contains("disabled"), "got: {err}");
        assert!(err.to_string().contains("[fetch].enabled"), "got: {err}");
    }
}
