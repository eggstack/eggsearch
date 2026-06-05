//! Response types for the metasearch adapter.

use eggsearch_core::SourceCard;
use eggsearch_core::SearchWarning;
use serde::Serialize;

/// Status of a single configured provider.
#[derive(Clone, Debug, Serialize)]
pub struct ProviderStatus {
    /// Stable provider id, e.g. `"duckduckgo"`.
    pub id: String,
    /// Whether the provider is enabled in the server's effective config.
    pub enabled: bool,
    /// Provider kind. For HTML-scraped engines this is `"html_scrape"`.
    /// For engines that take an API key this is `"api_key"`.
    pub kind: String,
    /// Whether the provider requires an API key.
    pub requires_api_key: bool,
}

/// A failure record for a single provider, exposed to the MCP tool.
#[derive(Clone, Debug, Serialize)]
pub struct ProviderFailure {
    pub id: String,
    /// Coarse error class: `timeout`, `http_status`, `parse_error`,
    /// `network_error`, `rate_limited`, or `unknown`.
    pub error_class: String,
    /// Human-readable detail. The MCP tool surfaces this in provider
    /// failure metadata; raw HTTP bodies are never included.
    pub message: String,
}

/// Successful response from `MetadataSearchAdapter::web_search`.
#[derive(Clone, Debug)]
pub struct WebSearchResponse {
    pub query: String,
    pub mode: &'static str,
    pub results: Vec<SourceCard>,
    /// All provider ids that were queried.
    pub providers_queried: Vec<String>,
    /// Per-provider failures, if any.
    pub providers_failed: Vec<ProviderFailure>,
    /// Aggregated warnings (per-provider failures + the standard
    /// "untrusted external content" warning).
    pub warnings: Vec<SearchWarning>,
}
