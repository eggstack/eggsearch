use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::repo_fetch::RepoFetchRequest;

/// The common structured repository locator used by fetch surfaces.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct RepoTarget {
    /// Repository owner or workspace root name.
    #[schemars(description = "Repository owner or workspace root name.")]
    pub owner: String,
    /// Repository name.
    #[schemars(description = "Repository name.")]
    pub repo: String,
    /// Branch, tag, or commit ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Branch, tag, or commit ref.")]
    pub ref_name: Option<String>,
    /// Full commit SHA for stable permalink construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Full commit SHA for stable permalink construction.")]
    pub commit_sha: Option<String>,
    /// File path relative to the repository or workspace root.
    #[schemars(description = "File path relative to the repository or workspace root.")]
    pub path: String,
    /// First line to return, 1-indexed and inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "First line to return, 1-indexed and inclusive.")]
    pub line_start: Option<u32>,
    /// Last line to return, 1-indexed and inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Last line to return, 1-indexed and inclusive.")]
    pub line_end: Option<u32>,
    /// Extra context lines before the selected range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Extra context lines before the selected range.")]
    pub context_before: Option<u32>,
    /// Extra context lines after the selected range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Extra context lines after the selected range.")]
    pub context_after: Option<u32>,
    /// Maximum characters to return for this item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum characters to return for this item.")]
    pub max_chars: Option<usize>,
    /// Symbol name to select.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Symbol name to select.")]
    pub symbol: Option<String>,
    /// Symbol kind filter applied with symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Symbol kind filter applied with symbol.")]
    pub symbol_kind: Option<String>,
    /// Select the span containing this text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Select the span containing this text.")]
    pub match_text: Option<String>,
    /// Expand the selected line or span to its enclosing block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Expand the selected line or span to its enclosing block.")]
    pub expand_to_block: Option<bool>,
    /// Maximum lines returned when expanding to a block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum lines returned when expanding to a block.")]
    pub max_block_lines: Option<usize>,
    /// Prefer the local workspace copy when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Prefer the local workspace copy when one exists.")]
    pub prefer_local: Option<bool>,
}

impl From<RepoTarget> for RepoFetchRequest {
    fn from(target: RepoTarget) -> Self {
        Self {
            host: None,
            target,
            timeout_ms: None,
        }
    }
}
