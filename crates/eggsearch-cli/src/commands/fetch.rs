//! `eggsearch fetch`: fetch and extract a URL from the command line.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_fetch::{ExtractMode, FetchProvider};
use eggsearch_mcp::ServerState;
use std::sync::Arc;

pub async fn run(
    cfg: &AppConfig,
    url: &str,
    max_bytes: Option<usize>,
    extract_mode: &str,
    as_json: bool,
) -> Result<()> {
    let state = Arc::new(ServerState::build(cfg.clone())?);
    let parsed = url::Url::parse(url)?;
    let mode = match extract_mode {
        "raw" => ExtractMode::Raw,
        "text" => ExtractMode::Text,
        "readability" => ExtractMode::Readability,
        "markdown" => ExtractMode::Markdown,
        other => anyhow::bail!("unknown extract_mode: {other}"),
    };
    let req = eggsearch_fetch::FetchRequest {
        url: parsed.clone(),
        max_bytes: max_bytes.unwrap_or(2 * 1024 * 1024),
        timeout_ms: cfg.search.live.timeout_ms,
        extract_mode: mode,
        respect_robots_txt: cfg.search.live.respect_robots_txt,
    };
    let doc = state.fetch.fetch(req).await?;
    if as_json {
        let v = serde_json::to_value(&doc)?;
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        println!("# {}", doc.title.clone().unwrap_or_else(|| url.to_string()));
        println!("{}", doc.url);
        println!();
        println!("{}", doc.text);
    }
    Ok(())
}
