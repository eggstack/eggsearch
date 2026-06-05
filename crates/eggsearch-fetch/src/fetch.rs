//! Fetch request / response types and the `FetchProvider` trait.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use eggsearch_core::result::TrustLevel;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::FetchError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExtractMode {
    /// Just the raw bytes (still capped to max_bytes).
    Raw,
    /// Plain text extraction (HTML stripped).
    Text,
    /// HTML readability-style extraction (headings/lists preserved).
    Readability,
    /// Markdown-style extraction.
    Markdown,
}

impl Default for ExtractMode {
    fn default() -> Self {
        Self::Readability
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FetchRequest {
    pub url: Url,
    pub max_bytes: usize,
    pub timeout_ms: u64,
    pub extract_mode: ExtractMode,
    pub respect_robots_txt: bool,
}

impl Default for FetchRequest {
    fn default() -> Self {
        Self {
            url: Url::parse("https://example.com").unwrap(),
            max_bytes: 2 * 1024 * 1024,
            timeout_ms: 8000,
            extract_mode: ExtractMode::Readability,
            respect_robots_txt: true,
        }
    }
}

impl FetchRequest {
    pub fn new(url: Url) -> Self {
        Self {
            url,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FetchedDocument {
    pub url: String,
    pub canonical_url: String,
    pub title: Option<String>,
    pub text: String,
    pub excerpt: String,
    pub content_type: String,
    pub content_hash: String,
    pub artifact_id: String,
    pub fetched_at: DateTime<Utc>,
    pub status: u16,
    pub trust_level: TrustLevel,
    pub warnings: Vec<String>,
    pub from_cache: bool,
}

#[async_trait]
pub trait FetchProvider: Send + Sync {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchedDocument, FetchError>;
}
