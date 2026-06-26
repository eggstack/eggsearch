use serde::{Deserialize, Serialize};

use crate::core::source_card::{IssueMetadata, ReleaseMetadata};

#[allow(missing_docs)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ResultMetadata {
    #[default]
    None,
    Issue(IssueMetadata),
    Release(ReleaseMetadata),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub source_engine: String,
    #[serde(default)]
    pub metadata: ResultMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub engines: Vec<String>,
    pub score: f64,
    #[serde(default)]
    pub metadata: ResultMetadata,
}
