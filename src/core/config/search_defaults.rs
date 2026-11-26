//! Field-level defaults for the `[search]` configuration section.
//!
//! Serde attributes and `SearchSection::default` call these functions so a
//! partial table and an in-memory default cannot drift apart.

use super::Mode;

pub(super) fn default_search_mode() -> Mode {
    Mode::Live
}

pub(super) fn default_search_max_results() -> usize {
    10
}

pub(super) fn default_search_results_cap() -> usize {
    50
}

pub(super) fn default_search_max_query_chars() -> usize {
    512
}

pub(super) fn default_search_timeout_ms() -> u64 {
    8000
}

pub(super) fn default_default_providers() -> Vec<String> {
    vec![
        "duckduckgo".to_string(),
        "brave".to_string(),
        "yahoo".to_string(),
    ]
}

pub(super) fn default_provider_map() -> std::collections::BTreeMap<String, bool> {
    let mut providers = std::collections::BTreeMap::new();
    providers.insert("duckduckgo".to_string(), true);
    providers.insert("brave".to_string(), true);
    providers.insert("startpage".to_string(), true);
    providers.insert("yahoo".to_string(), true);
    providers.insert("mojeek".to_string(), false);
    providers.insert("searxng".to_string(), false);
    providers.insert("osv".to_string(), true);
    providers.insert("firecrawl_developer".to_string(), false);
    providers.insert("wikipedia".to_string(), false);
    providers.insert("arxiv".to_string(), false);
    providers.insert("pubmed".to_string(), false);
    providers.insert("hn_algolia".to_string(), false);
    providers.insert("github_repositories".to_string(), false);
    providers
}
