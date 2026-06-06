//! `eggsearch search`: manual live metasearch via the CLI.

use anyhow::{anyhow, Result};
use eggsearch::core::config::AppConfig;
use eggsearch::core::WebSearchRequest;
use eggsearch::mcp::ServerState;
use std::sync::Arc;

pub async fn run(
    cfg: &AppConfig,
    query: &str,
    max_results: usize,
    as_json: bool,
    providers: &[String],
) -> Result<()> {
    let state = Arc::new(ServerState::build(cfg.clone())?);

    let effective_providers = cfg
        .resolve_providers(providers)
        .map_err(|e| anyhow!("{}", e))?;
    let (_, unknown) = state.adapter.select_engines(&effective_providers);
    if !unknown.is_empty() {
        anyhow::bail!("unknown provider id(s): {}", unknown.join(", "));
    }

    let req = WebSearchRequest {
        query: query.to_string(),
        max_results: Some(max_results),
        providers: effective_providers,
        safe_search: None,
        timeout_ms: None,
    };

    if let Err(e) = req.validate(cfg.search.max_query_chars, cfg.search.max_results_cap) {
        return Err(anyhow!("invalid query: {e}"));
    }

    let resp = state
        .adapter
        .web_search(&req, cfg.search.max_results, cfg.search.max_results_cap)
        .await;

    if as_json {
        let payload = serde_json::json!({
            "query": resp.query,
            "mode": resp.mode,
            "results": resp.results,
            "providers_queried": resp.providers_queried,
            "providers_failed": resp.providers_failed,
            "warnings": resp.warnings.iter().map(|w| format!("[{}] {}", w.provider_id, w.message)).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "# Results for '{}' ({} items, {} failed)",
            query,
            resp.results.len(),
            resp.providers_failed.len()
        );
        for (i, c) in resp.results.iter().enumerate() {
            let snippet = c.snippet.as_deref().unwrap_or("").replace('\n', " ");
            let providers = c.providers.join(", ");
            println!(
                "\n{}. {}\n   {}\n   [{}]\n   {}",
                i + 1,
                c.title,
                c.url,
                providers,
                snippet
            );
        }
        if !resp.warnings.is_empty() {
            println!("\nWarnings:");
            for w in &resp.warnings {
                println!("  - [{}] {}", w.provider_id, w.message);
            }
        }
        if !resp.providers_failed.is_empty() {
            println!("\nFailed providers:");
            for f in &resp.providers_failed {
                println!("  - {}: {} ({})", f.id, f.message, f.error_class);
            }
        }
    }
    Ok(())
}
