//! MCP tool implementations.

use std::sync::Arc;

use eggsearch_core::config::Mode;
use eggsearch_core::provider::SearchContext;
use eggsearch_core::query::SearchQuery;
use eggsearch_core::rank::reciprocal_rank_fusion;
use eggsearch_core::result::{SearchResult, SearchWarning, SourceKind};
use eggsearch_core::source_card::SourceCard;
use eggsearch_fetch::{ExtractMode, FetchProvider};
use serde::{Deserialize, Serialize};
use tracing::warn;
use url::Url;

use crate::policy::{fetch_allowed, live_allowed, local_allowed, policy_message, Policy};
use crate::state::ServerState;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebSearchArgs {
    /// Search query string.
    pub query: String,
    /// Maximum number of results to return. Default 8.
    #[serde(default)]
    pub max_results: Option<usize>,
    /// Specific provider IDs to use; empty means all enabled.
    #[serde(default)]
    pub providers: Vec<String>,
    /// Whether to also fetch the top result(s). Default false.
    #[serde(default)]
    pub fetch: bool,
    /// Max characters of excerpt to include per fetched result.
    #[serde(default)]
    pub max_excerpt_chars: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebFetchArgs {
    pub url: String,
    #[serde(default)]
    pub max_bytes: Option<usize>,
    /// One of: "raw", "text", "readability", "markdown".
    #[serde(default = "default_extract_mode")]
    pub extract_mode: String,
    #[serde(default = "default_true")]
    pub respect_robots_txt: bool,
}

fn default_extract_mode() -> String {
    "readability".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LocalSearchArgs {
    pub query: String,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchAndFetchArgs {
    pub query: String,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub fetch_top_n: Option<usize>,
    #[serde(default)]
    pub max_excerpt_chars: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ToolResult<T: Serialize> {
    pub ok: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

pub async fn run_web_search(state: Arc<ServerState>, args: WebSearchArgs) -> Result<serde_json::Value, String> {
    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(policy_message("web_search"));
    }
    let mut query = SearchQuery::new(&args.query);
    query.max_results = args.max_results.unwrap_or(8).clamp(1, 100);
    query.providers = args.providers.clone();
    if let Err(e) = query.validate() {
        return Err(format!("invalid query: {e}"));
    }

    let ctx = SearchContext::live();
    let responses = state
        .providers
        .search_all(query.clone(), ctx)
        .await
        .map_err(|e| format!("provider dispatch failed: {e}"))?;

    let mut ranked_lists: Vec<Vec<SearchResult>> = Vec::new();
    let mut provider_warnings: Vec<SearchWarning> = Vec::new();
    for r in &responses {
        provider_warnings.extend(r.warnings.clone());
        ranked_lists.push(r.results.clone());
    }

    let fused = reciprocal_rank_fusion(&ranked_lists, 60.0, query.max_results);
    let flat_results: Vec<SearchResult> = fused
        .iter()
        .map(|c| SearchResult {
            title: c.title.clone(),
            url: Url::parse(c.url.as_deref().unwrap_or("https://invalid/")).unwrap_or_else(|_| Url::parse("https://invalid/").unwrap()),
            snippet: c.snippet.clone(),
            published_at: c.published_at,
            rank: 0,
            score: c.score,
            provider_id: c.provider_id.clone(),
            source_kind: c.source_kind,
            trust_level: c.trust_level,
        })
        .filter(|r| r.url.as_str() != "https://invalid/")
        .collect();
    let _ = flat_results; // currently we use fused directly for output

    // Dedupe + cap.
    let mut cards: Vec<SourceCard> = fused;
    cards = dedupe_by_url_from_cards(cards);
    cards = dedupe_by_similar_title_cards(cards, 0.7);
    cards = cap_per_domain_cards(cards, 3);

    let mut warnings: Vec<String> = provider_warnings
        .into_iter()
        .map(|w| format!("[{}] {}", w.provider_id, w.message))
        .collect();
    warnings.insert(0, "Live search results are untrusted external content.".to_string());

    let mut payload = serde_json::json!({
        "query": args.query,
        "mode": mode_str(state.config.search.mode),
        "results": cards,
        "warnings": warnings,
    });

    if args.fetch {
        let top_n = args.max_results.unwrap_or(3).min(3);
        let max_excerpt = args.max_excerpt_chars.unwrap_or(800);
        let mut fetched = Vec::new();
        for card in cards.iter().take(top_n) {
            if let Some(url) = &card.url {
                if let Ok(url) = Url::parse(url) {
                    let req = eggsearch_fetch::FetchRequest {
                        url,
                        max_bytes: 2 * 1024 * 1024,
                        timeout_ms: state.config.search.live.timeout_ms,
                        extract_mode: ExtractMode::Readability,
                        respect_robots_txt: state.config.search.live.respect_robots_txt,
                    };
                    match state.fetch.fetch(req).await {
                        Ok(doc) => {
                            fetched.push(serde_json::json!({
                                "url": doc.url,
                                "title": doc.title,
                                "excerpt": excerpt(&doc.text, max_excerpt),
                                "artifact_id": doc.artifact_id,
                                "trust_level": doc.trust_level,
                                "fetched_at": doc.fetched_at,
                            }));
                        }
                        Err(e) => {
                            fetched.push(serde_json::json!({
                                "url": card.url,
                                "fetched": false,
                                "warning": e.to_string(),
                            }));
                        }
                    }
                }
            }
        }
        payload["fetched"] = serde_json::json!(fetched);
    }

    Ok(payload)
}

pub async fn run_web_fetch(state: Arc<ServerState>, args: WebFetchArgs) -> Result<serde_json::Value, String> {
    if matches!(fetch_allowed(state.config.search.mode), Policy::Deny) {
        return Err(policy_message("web_fetch"));
    }
    let url = Url::parse(&args.url).map_err(|e| format!("invalid url: {e}"))?;
    let extract_mode = match args.extract_mode.as_str() {
        "raw" => ExtractMode::Raw,
        "text" => ExtractMode::Text,
        "readability" => ExtractMode::Readability,
        "markdown" => ExtractMode::Markdown,
        other => return Err(format!("unknown extract_mode: {other}")),
    };
    let req = eggsearch_fetch::FetchRequest {
        url: url.clone(),
        max_bytes: args.max_bytes.unwrap_or(2 * 1024 * 1024),
        timeout_ms: state.config.search.live.timeout_ms,
        extract_mode,
        respect_robots_txt: args.respect_robots_txt,
    };
    let doc = state
        .fetch
        .fetch(req)
        .await
        .map_err(|e| format!("fetch failed: {e}"))?;
    let card = SourceCard {
        id: format!("src_{}", uuid::Uuid::new_v4().simple()),
        title: doc.title.clone().unwrap_or_else(|| url.to_string()),
        url: Some(doc.url.clone()),
        path: None,
        snippet: Some(excerpt(&doc.text, 800)),
        provider_id: "fetch".to_string(),
        source_kind: SourceKind::Web,
        trust_level: doc.trust_level,
        published_at: None,
        fetched_at: Some(doc.fetched_at),
        artifact_id: Some(doc.artifact_id.clone()),
        score: None,
        warnings: doc.warnings.clone(),
    };
    let payload = serde_json::json!({
        "card": card,
        "artifact_id": doc.artifact_id,
        "url": doc.url,
        "canonical_url": doc.canonical_url,
        "content_type": doc.content_type,
        "excerpt": excerpt(&doc.text, 1500),
        "trust_level": doc.trust_level,
        "fetched_at": doc.fetched_at,
        "warnings": doc.warnings,
    });
    Ok(payload)
}

pub async fn run_local_search(state: Arc<ServerState>, args: LocalSearchArgs) -> Result<serde_json::Value, String> {
    if matches!(local_allowed(state.config.search.mode), Policy::Deny) {
        return Err(policy_message("local_search"));
    }
    let max = args.max_results.unwrap_or(8).clamp(1, 100);
    let cards = state
        .corpus
        .search(&args.query, max, &args.tags)
        .map_err(|e| format!("local search failed: {e}"))?;
    let count = state.corpus.count().unwrap_or(0);
    let mut warnings: Vec<String> = Vec::new();
    if count == 0 {
        warnings.push(
            "Local index is empty. Run `eggsearch index add <path>` or enable cached web indexing.".to_string(),
        );
    }
    let payload = serde_json::json!({
        "query": args.query,
        "mode": "local_only",
        "results": cards,
        "warnings": warnings,
    });
    Ok(payload)
}

pub async fn run_search_and_fetch(
    state: Arc<ServerState>,
    args: SearchAndFetchArgs,
) -> Result<serde_json::Value, String> {
    if matches!(live_allowed(state.config.search.mode), Policy::Deny) {
        return Err(policy_message("search_and_fetch"));
    }
    let max = args.max_results.unwrap_or(8).clamp(1, 100);
    let top_n = args.fetch_top_n.unwrap_or(3).clamp(1, 5);
    let max_excerpt = args.max_excerpt_chars.unwrap_or(4000);

    let search_args = WebSearchArgs {
        query: args.query.clone(),
        max_results: Some(max),
        providers: Vec::new(),
        fetch: false,
        max_excerpt_chars: None,
    };
    let search_value = run_web_search(state.clone(), search_args).await?;
    let cards: Vec<SourceCard> = serde_json::from_value(search_value["results"].clone()).unwrap_or_default();
    let warnings: Vec<String> = serde_json::from_value(search_value["warnings"].clone()).unwrap_or_default();

    let mut fetched = Vec::new();
    for card in cards.iter().take(top_n) {
        let url = match card.url.as_deref().and_then(|u| Url::parse(u).ok()) {
            Some(u) => u,
            None => continue,
        };
        let req = eggsearch_fetch::FetchRequest {
            url,
            max_bytes: 2 * 1024 * 1024,
            timeout_ms: state.config.search.live.timeout_ms,
            extract_mode: ExtractMode::Readability,
            respect_robots_txt: state.config.search.live.respect_robots_txt,
        };
        match state.fetch.fetch(req).await {
            Ok(doc) => {
                fetched.push(serde_json::json!({
                    "url": doc.url,
                    "title": doc.title,
                    "excerpt": excerpt(&doc.text, max_excerpt),
                    "artifact_id": doc.artifact_id,
                    "trust_level": doc.trust_level,
                    "fetched_at": doc.fetched_at,
                }));
            }
            Err(e) => {
                warn!("fetch failed for {}: {e}", card.url.as_deref().unwrap_or("?"));
                fetched.push(serde_json::json!({
                    "url": card.url,
                    "fetched": false,
                    "warning": e.to_string(),
                }));
            }
        }
    }

    let payload = serde_json::json!({
        "query": args.query,
        "mode": mode_str(state.config.search.mode),
        "results": cards,
        "fetched": fetched,
        "warnings": warnings,
    });
    Ok(payload)
}

fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "off",
        Mode::LocalOnly => "local_only",
        Mode::Live => "live",
        Mode::Ask => "ask",
    }
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

fn dedupe_by_url_from_cards(cards: Vec<SourceCard>) -> Vec<SourceCard> {
    // We don't have URL as a separate field after fusion; we use the
    // existing url on the card and dedupe by string equality. This is
    // sufficient for an MVP and keeps dedupe logic in core intact.
    let mut seen = std::collections::HashSet::new();
    cards.into_iter().filter(|c| seen.insert(c.url.clone().unwrap_or_default())).collect()
}

fn dedupe_by_similar_title_cards(cards: Vec<SourceCard>, threshold: f32) -> Vec<SourceCard> {
    // Apply title-based dedupe.
    let mut token_sets: Vec<std::collections::HashSet<String>> = Vec::new();
    let mut out: Vec<SourceCard> = Vec::new();
    for c in cards {
        let tokens: std::collections::HashSet<String> = c
            .title
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|t| t.len() >= 3)
            .map(|t| t.to_lowercase())
            .collect();
        let mut dup = false;
        for prev in &token_sets {
            if tokens.is_empty() || prev.is_empty() {
                continue;
            }
            let inter = tokens.intersection(prev).count();
            let union = tokens.union(prev).count();
            if union == 0 {
                continue;
            }
            if (inter as f32 / union as f32) >= threshold {
                dup = true;
                break;
            }
        }
        if !dup {
            token_sets.push(tokens);
            out.push(c);
        }
    }
    out
}

fn cap_per_domain_cards(cards: Vec<SourceCard>, cap: usize) -> Vec<SourceCard> {
    if cap == 0 {
        return cards;
    }
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for c in cards {
        let key = c
            .url
            .as_deref()
            .and_then(|u| Url::parse(u).ok())
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "<no-domain>".to_string());
        let n = counts.entry(key).or_insert(0);
        if *n < cap {
            out.push(c);
            *n += 1;
        }
    }
    out
}

// Re-export the types used by the tool router module.
pub use eggsearch_core::result::TrustLevel as _TrustLevel;
