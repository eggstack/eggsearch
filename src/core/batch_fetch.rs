//! Batch fetch request/response types for the `batch_fetch` tool.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::fetch::ExtractMode;

/// A single item in a batch fetch request.
///
/// Each item is either a web URL fetch or a structured repo fetch
/// request. Items are validated before any network I/O begins.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BatchFetchItem {
    /// Fetch an explicit HTTP(S) URL.
    Web {
        /// The URL to fetch. Must be a valid HTTP(S) URL.
        url: String,
        /// Extraction mode for this item.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extract_mode: Option<ExtractMode>,
        /// Whether to include extracted links.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        include_links: Option<bool>,
        /// Maximum characters to extract for this item.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_chars: Option<usize>,
    },
    /// Fetch a repository file by structured locator.
    Repo {
        /// Code host. Accepted values: `github`, `gitlab`, `workspace`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host: Option<String>,
        /// Repository owner (or workspace root name for workspace locators).
        owner: String,
        /// Repository name (or root-relative file path for workspace locators).
        repo: String,
        /// Branch, tag, or commit ref.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ref_name: Option<String>,
        /// Full commit SHA for stable permalink construction.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commit_sha: Option<String>,
        /// File path relative to repository root.
        path: String,
        /// First line to return (1-indexed, inclusive).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line_start: Option<u32>,
        /// Last line to return (1-indexed, inclusive).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line_end: Option<u32>,
        /// Extra lines of context before `line_start`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_before: Option<u32>,
        /// Extra lines of context after `line_end`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_after: Option<u32>,
        /// Maximum characters to return.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_chars: Option<usize>,
    },
}

impl BatchFetchItem {
    /// Returns a human-readable label for this item.
    pub fn label(&self) -> String {
        match self {
            BatchFetchItem::Web { url, .. } => url.clone(),
            BatchFetchItem::Repo {
                owner,
                repo,
                path,
                host,
                ..
            } => {
                let h = host.as_deref().unwrap_or("github");
                format!("{h}:{owner}/{repo}/{path}")
            }
        }
    }

    /// Returns the max_chars for this item, if set.
    pub fn max_chars(&self) -> Option<usize> {
        match self {
            BatchFetchItem::Web { max_chars, .. } => *max_chars,
            BatchFetchItem::Repo { max_chars, .. } => *max_chars,
        }
    }
}

/// The type of a batch fetch result item.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BatchFetchItemType {
    /// A web URL fetch.
    Web,
    /// A repository file fetch.
    Repo,
}

/// The result of fetching a single item in a batch.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BatchFetchResult {
    /// Index of this item in the original request (preserves input order).
    pub index: usize,
    /// Type of this item.
    pub item_type: BatchFetchItemType,
    /// Label identifying this item (URL or repo locator).
    pub label: String,
    /// Deterministic, content-derived identifier stable across runs.
    /// Format: `batch_<16hex>`. Derived from (label, index).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    /// Whether this item was fetched successfully.
    pub ok: bool,
    /// Serialized response payload (web_fetch or repo_fetch response shape).
    /// Present only when `ok = true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<serde_json::Value>,
    /// Error message. Present only when `ok = false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Approximate character count of the returned text content.
    pub chars_returned: usize,
    /// Whether this item's text was truncated at the per-item max_chars cap.
    pub truncated: bool,
}

/// Response type for the `batch_fetch` tool.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct BatchFetchResponse {
    /// Number of items successfully fetched.
    pub fetched: usize,
    /// Number of items that failed.
    pub failed: usize,
    /// Whether any item's text was truncated at the per-item cap.
    pub truncated: bool,
    /// Approximate total characters returned across all items.
    pub total_chars_returned: usize,
    /// Per-item results, in input order.
    pub results: Vec<BatchFetchResult>,
    /// Batch-level warnings (e.g. budget exhaustion, partial failure).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Structured warnings with stable codes and severity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_warnings: Vec<crate::core::warning::AgentWarning>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_fetch_item_tagged_serialization() {
        let web = BatchFetchItem::Web {
            url: "https://example.com".to_string(),
            extract_mode: None,
            include_links: None,
            max_chars: None,
        };
        let v = serde_json::to_value(&web).unwrap();
        assert_eq!(v["type"], "web");
        assert_eq!(v["url"], "https://example.com");

        let repo = BatchFetchItem::Repo {
            host: Some("github".to_string()),
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: None,
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
        };
        let v = serde_json::to_value(&repo).unwrap();
        assert_eq!(v["type"], "repo");
        assert_eq!(v["owner"], "tokio-rs");
    }

    #[test]
    fn batch_fetch_item_label() {
        let web = BatchFetchItem::Web {
            url: "https://example.com".to_string(),
            extract_mode: None,
            include_links: None,
            max_chars: None,
        };
        assert_eq!(web.label(), "https://example.com");

        let repo = BatchFetchItem::Repo {
            host: None,
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: None,
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
        };
        assert_eq!(repo.label(), "github:tokio-rs/axum/src/lib.rs");
    }

    #[test]
    fn batch_fetch_response_default() {
        let r = BatchFetchResponse::default();
        assert_eq!(r.fetched, 0);
        assert_eq!(r.failed, 0);
        assert!(!r.truncated);
        assert!(r.results.is_empty());
    }
}
