//! Response types for the metasearch adapter.

use crate::core::sanitize::TrustMarkers;
use crate::core::SearchWarning;
use crate::core::SourceCard;
use serde::{Deserialize, Serialize};

/// A failure record for a single provider, exposed to the MCP tool.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderFailure {
    /// Stable provider id, e.g. `"duckduckgo"`.
    pub id: String,
    /// Coarse error class: `timeout`, `http_status`, `parse_error`,
    /// `network_error`, `rate_limited`, or `unknown`.
    pub error_class: String,
    /// Human-readable detail. The MCP tool surfaces this in provider
    /// failure metadata; raw HTTP bodies are never included.
    pub message: String,
}

/// Successful response from `MetadataSearchAdapter::web_search`.
#[derive(Clone, Debug, Default)]
pub struct WebSearchResponse {
    /// Echo of the original query.
    pub query: String,
    /// Mode the adapter ran in (always `"live_metasearch"` for now).
    pub mode: &'static str,
    /// Deduplicated, ranked source cards.
    pub results: Vec<SourceCard>,
    /// All provider ids that were queried.
    pub providers_queried: Vec<String>,
    /// Per-provider failures, if any.
    pub providers_failed: Vec<ProviderFailure>,
    /// Aggregated warnings (per-provider failures + the standard
    /// "untrusted external content" warning).
    pub warnings: Vec<SearchWarning>,
    /// Aggregate `TrustMarkers` rolled up across every `SourceCard`
    /// in `results` (sum of `control_chars_removed`, sum of
    /// `injection_hits`, OR of the booleans). Default-initialized to
    /// a zero record; the adapter populates it after each card is
    /// sanitized.
    pub trust_markers: TrustMarkers,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_failure_serde_roundtrip() {
        let failure = ProviderFailure {
            id: "brave".to_string(),
            error_class: "timeout".to_string(),
            message: "request timed out".to_string(),
        };
        let json = serde_json::to_string(&failure).unwrap();
        let parsed: ProviderFailure = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, failure.id);
        assert_eq!(parsed.error_class, failure.error_class);
        assert_eq!(parsed.message, failure.message);
    }
}
