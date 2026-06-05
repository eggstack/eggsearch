//! Engine construction: builds the `Arc<dyn SearchEngine>` list from
//! the server's effective provider configuration.
//!
//! This module is only compiled when the `metasearch` feature is on.

use std::sync::Arc;

use metadata_search_engine_rs::engines::{
    build_http_client, BraveEngine, DuckDuckGoEngine, SearchEngine, StartpageEngine, YahooEngine,
};

type EngineList = Vec<Arc<dyn SearchEngine>>;

/// The set of provider ids that ship with the upstream library and that
/// eggsearch can enable by default.
pub const KNOWN_PROVIDERS: &[&str] = &["duckduckgo", "brave", "startpage", "yahoo"];

/// Kind of an engine, for `provider_status` reporting.
pub fn provider_kind(id: &str) -> (&'static str, bool) {
    match id {
        "duckduckgo" | "startpage" | "yahoo" => ("html_scrape", false),
        "brave" => ("html_scrape", true),
        _other => ("unknown", false),
    }
}

/// Build a default shared HTTP client.
pub fn shared_http_client() -> anyhow::Result<Arc<reqwest::Client>> {
    let client = build_http_client()?;
    Ok(Arc::new(client))
}

/// Build the default engine set used by the server. Disabled providers
/// in the config are skipped; unknown ids are reported via the returned
/// `Vec<String>` of skipped ids.
pub fn build_default_engines(enabled_providers: &[String]) -> anyhow::Result<(EngineList, Vec<String>)> {
    let client = shared_http_client()?;
    let mut engines: EngineList = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for id in enabled_providers {
        match id.as_str() {
            "duckduckgo" => engines.push(Arc::new(DuckDuckGoEngine { client: client.clone() })),
            "brave" => engines.push(Arc::new(BraveEngine { client: client.clone() })),
            "startpage" => engines.push(Arc::new(StartpageEngine { client: client.clone() })),
            "yahoo" => engines.push(Arc::new(YahooEngine { client: client.clone() })),
            other => skipped.push(other.to_string()),
        }
    }

    Ok((engines, skipped))
}
