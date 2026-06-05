//! Compact `SourceCard` representation passed to agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::result::{SearchResult, SourceKind, TrustLevel};

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceCard {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub snippet: Option<String>,
    pub provider_id: String,
    pub source_kind: SourceKind,
    pub trust_level: TrustLevel,
    pub published_at: Option<DateTime<Utc>>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub artifact_id: Option<String>,
    pub score: Option<f32>,
    pub warnings: Vec<String>,
}

impl SourceCard {
    pub fn new(title: impl Into<String>, url: Option<&str>, provider_id: impl Into<String>) -> Self {
        Self {
            id: format!("src_{}", Uuid::new_v4().simple()),
            title: title.into(),
            url: url.map(String::from),
            path: None,
            snippet: None,
            provider_id: provider_id.into(),
            source_kind: SourceKind::Unknown,
            trust_level: TrustLevel::default(),
            published_at: None,
            fetched_at: None,
            artifact_id: None,
            score: None,
            warnings: Vec::new(),
        }
    }

    pub fn with_kind(mut self, k: SourceKind) -> Self {
        self.source_kind = k;
        self
    }

    pub fn with_trust(mut self, t: TrustLevel) -> Self {
        self.trust_level = t;
        self
    }

    pub fn with_snippet(mut self, s: impl Into<String>) -> Self {
        self.snippet = Some(s.into());
        self
    }

    pub fn with_warning(mut self, w: impl Into<String>) -> Self {
        self.warnings.push(w.into());
        self
    }
}

/// Convert a `SearchResult` into a `SourceCard`. Used by RRF and the
/// provider adapters.
pub fn make_source_card(r: &SearchResult) -> SourceCard {
    SourceCard {
        id: format!("src_{}", Uuid::new_v4().simple()),
        title: r.title.clone(),
        url: Some(r.url.to_string()),
        path: None,
        snippet: r.snippet.clone(),
        provider_id: r.provider_id.clone(),
        source_kind: r.source_kind,
        trust_level: r.trust_level,
        published_at: r.published_at,
        fetched_at: None,
        artifact_id: None,
        score: r.score,
        warnings: Vec::new(),
    }
}

/// Convert a path-only source (e.g. a local file result) into a card.
pub fn card_for_path(
    title: impl Into<String>,
    path: &str,
    snippet: Option<String>,
    score: Option<f32>,
) -> SourceCard {
    SourceCard {
        id: format!("src_{}", Uuid::new_v4().simple()),
        title: title.into(),
        url: None,
        path: Some(path.to_string()),
        snippet,
        provider_id: "local".to_string(),
        source_kind: SourceKind::LocalFile,
        trust_level: TrustLevel::LocalTrusted,
        published_at: None,
        fetched_at: Some(Utc::now()),
        artifact_id: None,
        score,
        warnings: Vec::new(),
    }
}

/// Helper to build a stable key for a URL (used for dedupe and indexing).
pub fn url_key(url: &Url) -> String {
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::SearchResult;

    #[test]
    fn new_card_defaults() {
        let c = SourceCard::new("hello", Some("https://example.com"), "duckduckgo_html");
        assert_eq!(c.provider_id, "duckduckgo_html");
        assert!(c.artifact_id.is_none());
    }

    #[test]
    fn make_card_from_result() {
        let r = SearchResult {
            title: "t".into(),
            url: Url::parse("https://example.com").unwrap(),
            snippet: Some("snip".into()),
            published_at: None,
            rank: 0,
            score: Some(0.5),
            provider_id: "p".into(),
            source_kind: SourceKind::Web,
            trust_level: TrustLevel::ExternalUntrusted,
        };
        let c = make_source_card(&r);
        assert_eq!(c.title, "t");
        assert_eq!(c.score, Some(0.5));
    }
}
