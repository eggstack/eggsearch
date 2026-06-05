//! Result types returned by providers and combined by the orchestrator.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Web,
    Documentation,
    PackageRegistry,
    Reference,
    News,
    LocalFile,
    LocalArtifact,
    Unknown,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Documentation => "documentation",
            Self::PackageRegistry => "package_registry",
            Self::Reference => "reference",
            Self::News => "news",
            Self::LocalFile => "local_file",
            Self::LocalArtifact => "local_artifact",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Content originated from a configured local source (trusted by the operator).
    LocalTrusted,
    /// Indexed cached or local content that was originally fetched from the web.
    LocalCachedExternal,
    /// Live content fetched from the open web; treat as untrusted.
    ExternalUntrusted,
}

impl Default for TrustLevel {
    fn default() -> Self {
        Self::ExternalUntrusted
    }
}

impl TrustLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LocalTrusted => "local_trusted",
            Self::LocalCachedExternal => "local_cached_external",
            Self::ExternalUntrusted => "external_untrusted",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchWarning {
    pub provider_id: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchResult {
    pub title: String,
    pub url: Url,
    pub snippet: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub rank: usize,
    pub score: Option<f32>,
    pub provider_id: String,
    pub source_kind: SourceKind,
    pub trust_level: TrustLevel,
}

impl SearchResult {
    pub fn domain(&self) -> Option<String> {
        self.url.domain().map(|s| s.to_string())
    }
}
