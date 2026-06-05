//! `eggsearch index`: manage the local Tantivy index.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_local::{ingest_path, IngestOptions, IndexedDocument};
use eggsearch_mcp::ServerState;
use std::sync::Arc;

pub async fn add(cfg: &AppConfig, path: &str, tags: Vec<String>) -> Result<()> {
    let state = Arc::new(ServerState::build(cfg.clone())?);
    let opts = IngestOptions::default();
    let docs: Vec<IndexedDocument> = ingest_path(path, &opts)
        .into_iter()
        .map(|mut d| {
            d.tags = tags.clone();
            d
        })
        .collect();
    if docs.is_empty() {
        println!("No documents ingested from {path}.");
        return Ok(());
    }
    state.corpus.add_many(&docs)?;
    println!("Indexed {} documents from {path}.", docs.len());
    Ok(())
}

pub async fn search(cfg: &AppConfig, query: &str, max_results: usize, as_json: bool) -> Result<()> {
    let state = Arc::new(ServerState::build(cfg.clone())?);
    let cards = state.corpus.search(query, max_results, &[])?;
    let count = state.corpus.count().unwrap_or(0);

    if as_json {
        let payload = serde_json::json!({
            "query": query,
            "index_size": count,
            "results": cards,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("# Local search for '{}' ({} docs in index, {} hits)", query, count, cards.len());
        for (i, c) in cards.iter().enumerate() {
            let snippet = c.snippet.as_deref().unwrap_or("").replace('\n', " ");
            let where_ = c.path.as_deref().or(c.url.as_deref()).unwrap_or("");
            println!("\n{}. {}\n   {}\n   {}", i + 1, c.title, where_, snippet);
        }
    }
    Ok(())
}

pub async fn stats(cfg: &AppConfig) -> Result<()> {
    let state = Arc::new(ServerState::build(cfg.clone())?);
    let count = state.corpus.count().unwrap_or(0);
    let payload = serde_json::json!({
        "index_dir": cfg.search.local.index_dir.display().to_string(),
        "documents": count,
    });
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}
