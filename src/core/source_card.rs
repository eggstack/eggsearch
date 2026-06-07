//! Compact `SourceCard` representation passed to agents.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::result::TrustLevel;
use crate::core::sanitize::TrustMarkers;

/// A single normalized result returned to MCP callers.
///
/// This is the canonical, provider-agnostic output model. It is deliberately
/// small: agents should fetch full content via a separate `web_fetch` tool
/// rather than rely on snippets.
///
/// `web_search` is discovery-only and returns `SourceCard` values with
/// `fetched = false`. `web_fetch` returns a separate fetched-document
/// response for one explicit URL.
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
    /// Whether the underlying content was fetched. `web_search` is
    /// discovery-only and always returns cards with `fetched = false`;
    /// full-page retrieval is handled by the separate `web_fetch`
    /// tool, which returns its own response type rather than a
    /// `SourceCard`.
    pub fetched: bool,
    /// What eggsearch did to the title/snippet text on this card
    /// (control-char stripping, length bounding, framing, marker
    /// scanning). Default-initialized to a zero record on cards that
    /// have not yet been sanitized; later pipeline stages replace it
    /// with the actual counts.
    #[serde(default)]
    pub trust_markers: TrustMarkers,
}

impl SourceCard {
    /// Build a fresh `SourceCard` with the given title, url, providers, score,
    /// and trust label. A unique id of the form `src_<uuid>` is generated.
    ///
    /// # Examples
    ///
    /// ```
    /// use eggsearch::core::{SourceCard, TrustLevel};
    ///
    /// let card = SourceCard::new(
    ///     "tower-http - Rust",
    ///     "https://docs.rs/tower-http",
    ///     vec!["duckduckgo".to_string(), "brave".to_string()],
    ///     Some(0.0327),
    ///     TrustLevel::ExternalUntrusted,
    /// )
    /// .with_snippet("Middleware and utilities for HTTP clients and servers.");
    ///
    /// assert_eq!(card.title, "tower-http - Rust");
    /// assert!(card.id.starts_with("src_"));
    /// assert!(!card.fetched);
    /// assert!(card.snippet.is_some());
    /// ```
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
            trust_markers: TrustMarkers::default(),
        }
    }

    /// Attach a snippet to this card. Convenience for the
    /// `SourceCard::new(...).with_snippet(...)` builder pattern.
    pub fn with_snippet(mut self, s: impl Into<String>) -> Self {
        self.snippet = Some(s.into());
        self
    }

    /// Attach `TrustMarkers` describing what eggsearch did to the
    /// title/snippet text on this card. The pipeline populates this
    /// after sanitization; the constructor leaves it at
    /// `TrustMarkers::default()`.
    pub fn with_trust_markers(mut self, m: TrustMarkers) -> Self {
        self.trust_markers = m;
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

    #[test]
    fn new_card_default_trust_markers_is_zero() {
        let c = SourceCard::new(
            "t",
            "https://example.com",
            vec!["a".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        );
        assert_eq!(c.trust_markers, TrustMarkers::default());
        assert!(!c.trust_markers.text_sanitized);
        assert_eq!(c.trust_markers.injection_hits, 0);
    }

    #[test]
    fn with_trust_markers_sets_field() {
        let markers = TrustMarkers {
            text_sanitized: true,
            text_truncated: true,
            text_framed: false,
            control_chars_removed: 2,
            injection_hits: 1,
        };
        let c = SourceCard::new(
            "t",
            "https://example.com",
            vec!["a".to_string()],
            None,
            TrustLevel::ExternalUntrusted,
        )
        .with_trust_markers(markers.clone());
        assert_eq!(c.trust_markers, markers);
    }
}
