//! Compact `SourceCard` representation passed to agents.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::result::TrustLevel;

/// A single normalized result returned to MCP callers.
///
/// This is the canonical, provider-agnostic output model. It is deliberately
/// small: agents should fetch full content via a separate `web_fetch` tool
/// (deferred) rather than rely on snippets.
///
/// For the MVP, all live web results use `TrustLevel::ExternalUntrusted`.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceCard {
    /// Per-response identifier, e.g. `src_<uuid>`. Unique within a
    /// single `web_search` response. Not stable across responses; for
    /// cross-response dedup, use `source_identity` derived from
    /// `(url, sorted(providers))`.
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

    /// Compute the deterministic cross-response identity for a card
    /// identified by `(url, providers)`. Two cards with the same URL
    /// and the same set of contributing engines produce the same
    /// identity, even across separate `web_search` responses.
    ///
    /// The identity is the first 16 hex characters of
    /// `SHA-256(url || "\0" || sorted_providers.join("\0"))`, prefixed
    /// with `src_`. Callers (e.g. Codegg) can store these to dedupe
    /// results across requests.
    pub fn source_identity(url: &str, providers: &[String]) -> String {
        let mut sorted: Vec<&str> = providers.iter().map(String::as_str).collect();
        sorted.sort_unstable();
        sorted.dedup();
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        hasher.update([0u8]);
        for p in &sorted {
            hasher.update(p.as_bytes());
            hasher.update([0u8]);
        }
        let digest = hasher.finalize();
        let hex = hex::encode(digest);
        format!("src_{}", &hex[..16])
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
    fn source_identity_is_deterministic_and_provider_order_independent() {
        let id1 = SourceCard::source_identity(
            "https://example.com/page",
            &["duckduckgo".to_string(), "brave".to_string()],
        );
        let id2 = SourceCard::source_identity(
            "https://example.com/page",
            &["brave".to_string(), "duckduckgo".to_string()],
        );
        assert_eq!(id1, id2);
        assert!(id1.starts_with("src_"));
        assert_eq!(id1.len(), "src_".len() + 16);
    }

    #[test]
    fn source_identity_differs_across_urls_and_providers() {
        let a = SourceCard::source_identity("https://a.com", &["x".into()]);
        let b = SourceCard::source_identity("https://b.com", &["x".into()]);
        let c = SourceCard::source_identity("https://a.com", &["y".into()]);
        assert_ne!(a, b);
        assert_ne!(a, c);
    }
}
