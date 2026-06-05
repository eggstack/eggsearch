//! Compact `SourceCard` representation passed to agents.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::result::TrustLevel;

/// A single normalized result returned to MCP callers.
///
/// This is the canonical, provider-agnostic output model. It is deliberately
/// small: agents should fetch full content via a separate `web_fetch` tool
/// (deferred) rather than rely on snippets.
///
/// For the MVP, all live web results use `TrustLevel::ExternalUntrusted`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceCard {
    /// Per-response identifier, e.g. `src_<uuid>`. Unique within a
    /// single `web_search` response.
    pub id: String,
    /// Result title.
    pub title: String,
    /// Canonical URL.
    pub url: String,
    /// Short text snippet (truncated, never full content).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// All upstream engines that contributed to this card.
    #[serde(default)]
    pub providers: Vec<String>,
    /// Optional aggregate score (e.g. RRF). Higher is more relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Trust label; for live web results this is `external_untrusted`.
    pub trust: TrustLevel,
    /// `true` if the underlying content was fetched and cached locally.
    /// For the MVP this is always `false`.
    pub fetched: bool,
}

impl SourceCard {
    /// Build a fresh `SourceCard` with the given title, url, providers, score,
    /// and trust label. A unique id of the form `src_<uuid>` is generated.
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        providers: Vec<String>,
        score: Option<f64>,
        trust: TrustLevel,
    ) -> Self {
        Self {
            id: format!("src_{}", Uuid::new_v4().simple()),
            title: title.into(),
            url: url.into(),
            snippet: None,
            providers,
            score,
            trust,
            fetched: false,
        }
    }

    pub fn with_snippet(mut self, s: impl Into<String>) -> Self {
        self.snippet = Some(s.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_card_defaults() {
        let c = SourceCard::new(
            "hello",
            "https://example.com",
            vec!["duckduckgo".to_string()],
            Some(0.5),
            TrustLevel::ExternalUntrusted,
        );
        assert_eq!(c.title, "hello");
        assert_eq!(c.url, "https://example.com");
        assert_eq!(c.providers, vec!["duckduckgo".to_string()]);
        assert_eq!(c.score, Some(0.5));
        assert!(!c.fetched);
        assert!(c.snippet.is_none());
    }

    #[test]
    fn with_snippet_sets_field() {
        let c = SourceCard::new(
            "t",
            "https://example.com",
            vec!["a".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        )
        .with_snippet("a snippet");
        assert_eq!(c.snippet.as_deref(), Some("a snippet"));
    }

    #[test]
    fn id_starts_with_src_prefix() {
        let c = SourceCard::new(
            "t",
            "https://example.com",
            vec!["a".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        assert!(c.id.starts_with("src_"));
    }

    #[test]
    fn serde_roundtrip() {
        let c = SourceCard::new(
            "Example",
            "https://example.com",
            vec!["duckduckgo".to_string(), "brave".to_string()],
            Some(0.016),
            TrustLevel::ExternalUntrusted,
        )
        .with_snippet("An example snippet.");
        let json = serde_json::to_string(&c).unwrap();
        let parsed: SourceCard = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, c.title);
        assert_eq!(parsed.url, c.url);
        assert_eq!(parsed.providers, c.providers);
        assert_eq!(parsed.score, c.score);
        assert_eq!(parsed.trust, c.trust);
        assert_eq!(parsed.snippet, c.snippet);
    }

    #[test]
    fn serde_skips_none_optional_fields() {
        let c = SourceCard::new(
            "Example",
            "https://example.com",
            vec!["duckduckgo".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("\"snippet\":null"));
        assert!(!json.contains("\"score\":null"));
        let parsed: SourceCard = serde_json::from_str(&json).unwrap();
        assert!(parsed.snippet.is_none());
        assert!(parsed.score.is_none());
    }
}
