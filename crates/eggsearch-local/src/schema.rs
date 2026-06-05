//! Schema for indexed documents.

use chrono::{DateTime, Utc};
use eggsearch_core::result::{SourceKind, TrustLevel};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IndexedDocument {
    pub id: String,
    pub title: String,
    pub body: String,
    pub url: Option<String>,
    pub path: Option<PathBuf>,
    pub source_kind: SourceKind,
    pub trust_level: TrustLevel,
    pub fetched_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub content_hash: String,
    pub tags: Vec<String>,
}

impl IndexedDocument {
    pub fn local_file(path: PathBuf, title: String, body: String, content_hash: String) -> Self {
        Self {
            id: format!("local:{}", content_hash),
            title,
            body,
            url: None,
            path: Some(path),
            source_kind: SourceKind::LocalFile,
            trust_level: TrustLevel::LocalTrusted,
            fetched_at: Some(Utc::now()),
            published_at: None,
            content_hash,
            tags: Vec::new(),
        }
    }

    pub fn cached_artifact(url: String, title: String, body: String, content_hash: String) -> Self {
        Self {
            id: format!("cache:{content_hash}"),
            title,
            body,
            url: Some(url),
            path: None,
            source_kind: SourceKind::LocalArtifact,
            trust_level: TrustLevel::LocalCachedExternal,
            fetched_at: Some(Utc::now()),
            published_at: None,
            content_hash,
            tags: Vec::new(),
        }
    }
}
