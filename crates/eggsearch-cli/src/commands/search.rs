//! `eggsearch search`: manual live metasearch via the CLI.

use anyhow::Result;
use eggsearch_core::config::AppConfig;
use eggsearch_core::provider::SearchContext;
use eggsearch_core::query::SearchQuery;
use eggsearch_core::rank::reciprocal_rank_fusion;
use eggsearch_fetch::FetchProvider;
use eggsearch_mcp::ServerState;
use std::sync::Arc;

pub async fn run(
    cfg: &AppConfig,
    query: &str,
    provider: Option<&str>,
    max_results: usize,
    as_json: bool,
    fetch_top_n: usize,
) -> Result<()> {
    let state = Arc::new(ServerState::build(cfg.clone())?);
    let providers: Vec<String> = provider
        .map(|p| vec![p.to_string()])
        .unwrap_or_default();
    let mut sq = SearchQuery::new(query);
    sq.max_results = max_results;
    sq.providers = providers;
    let ctx = SearchContext::live();
    let responses = state.providers.search_all(sq.clone(), ctx).await?;
    let ranked: Vec<Vec<_>> = responses.iter().map(|r| r.results.clone()).collect();
    let cards = reciprocal_rank_fusion(&ranked, 60.0, max_results);

    let mut warnings: Vec<String> = Vec::new();
    for r in &responses {
        for w in &r.warnings {
            warnings.push(format!("[{}] {}", w.provider_id, w.message));
        }
    }

    if fetch_top_n > 0 {
        let top_n = fetch_top_n.min(cards.len());
        for card in cards.iter().take(top_n) {
            if let Some(url) = &card.url {
                if let Ok(url) = url::Url::parse(url) {
                    let req = eggsearch_fetch::FetchRequest {
                        url,
                        max_bytes: 2 * 1024 * 1024,
                        timeout_ms: cfg.search.live.timeout_ms,
                        extract_mode: eggsearch_fetch::ExtractMode::Readability,
                        respect_robots_txt: cfg.search.live.respect_robots_txt,
                    };
                    match state.fetch.fetch(req).await {
                        Ok(doc) => {
                            println!(
                                "\n# {}\n{}\n[excerpt] {}\n[artifact] {}\n",
                                doc.title.unwrap_or_else(|| card.title.clone()),
                                doc.url,
                                excerpt(&doc.text, 400),
                                doc.artifact_id,
                            );
                        }
                        Err(e) => {
                            warnings.push(format!("fetch failed for {}: {e}", card.url.as_deref().unwrap_or("?")));
                        }
                    }
                }
            }
        }
    }

    if as_json {
        let payload = serde_json::json!({
            "query": query,
            "results": cards,
            "warnings": warnings,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("# Results for '{}' ({} items, {} warnings)", query, cards.len(), warnings.len());
        for (i, c) in cards.iter().enumerate() {
            let snippet = c.snippet.as_deref().unwrap_or("").replace('\n', " ");
            let url = c.url.as_deref().unwrap_or("");
            println!("\n{}. {}\n   {}\n   {}", i + 1, c.title, url, snippet);
        }
        if !warnings.is_empty() {
            println!("\nWarnings:");
            for w in &warnings {
                println!("  - {w}");
            }
        }
    }
    Ok(())
}

fn excerpt(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = String::new();
    for (i, c) in trimmed.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}
