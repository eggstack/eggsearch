//! Fetch request/response types for the canonical fetch tool.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::fetch::ExtractMode;

fn is_none_or_false(value: &Option<bool>) -> bool {
    !value.unwrap_or(false)
}

/// A single item in a batch fetch request.
///
/// Each item is either a web URL fetch or a structured repo fetch
/// request. Items are validated before any network I/O begins.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BatchFetchItem {
    /// Fetch an explicit HTTP(S) URL; controls match `fetch`.
    Web {
        /// The URL to fetch. Must be a valid HTTP(S) URL.
        url: String,
        /// Extraction mode for this item.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extract_mode: Option<ExtractMode>,
        /// Whether to include extracted links.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        include_links: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(description = "CSS selectors to include in document order.")]
        #[allow(missing_docs)]
        include_selectors: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(description = "CSS selectors whose subtrees are excluded.")]
        #[allow(missing_docs)]
        exclude_selectors: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Bounded CSS-selector HTML attribute extractions.")]
        #[allow(missing_docs)]
        attributes: Option<Vec<crate::core::fetch::FetchAttributeRequest>>,
        /// Maximum characters to extract for this item.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_chars: Option<usize>,
        /// Cache policy for this item. Omitted means `default`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_policy: Option<crate::core::fetch::FetchCachePolicy>,
        /// Caller maximum acceptable cache age in seconds for this
        /// item. Only tightens origin freshness.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_cache_age_seconds: Option<u64>,
        /// Optional focus query for deterministic query-focused chunk
        /// selection over the extracted document. Same validation as
        /// `fetch` focus. No extra URL traversal.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        focus: Option<String>,
        /// Maximum focused chunks to return (1-5, default 5).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        focus_max_chunks: Option<usize>,
        /// Maximum focused characters to return. Defaults to the
        /// effective per-item `max_chars` budget.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        focus_max_chars: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[allow(missing_docs)]
        snapshot: Option<crate::core::fetch::FetchSnapshotRequest>,
        /// PDF options for this item; same fields and validation as `fetch`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pdf: Option<crate::core::fetch::PdfFetchOptions>,
        /// Render policy: `http_only`, `auto`, or `browser`; same as `fetch`.
        #[schemars(with = "Option<crate::core::fetch::RenderPolicy>")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        render: Option<String>,
        /// Named browser profile; same authorization as `fetch`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        browser_profile: Option<String>,
        #[schemars(description = "Include a bounded viewport screenshot as MCP image content.")]
        #[allow(missing_docs)]
        #[serde(default, skip_serializing_if = "is_none_or_false")]
        include_screenshot: Option<bool>,
    },
    /// Fetch a repository file by structured locator; controls match `repo_fetch`.
    Repo {
        /// Code host. Accepted values: github (alias gh), gitlab (alias gl),
        /// codeberg (alias cb), gitea, forgejo, or workspace.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host: Option<String>,
        /// Structured repository target fields.
        #[serde(flatten)]
        target: crate::core::repo_fetch::RepoTarget,
        /// Optional focus query applied as a deterministic projection
        /// after the underlying repo fetch returns a bounded
        /// document/span. Same validation as fetch focus. No extra URL traversal.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        focus: Option<String>,
        /// Maximum focused chunks to return (1-5, default 5).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        focus_max_chunks: Option<usize>,
        /// Maximum focused characters to return. Defaults to the
        /// effective per-item max_chars budget.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        focus_max_chars: Option<usize>,
    },
    /// Return a bounded URL inventory without fetching discovered targets.
    Site {
        /// Entry URL whose origin defines the inventory boundary.
        url: String,
        /// Maximum returned URLs (default 200, maximum 2,000).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
        /// Allow sibling subdomains under the same registrable domain.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        include_subdomains: Option<bool>,
        /// Include same-site anchor links from one entry-page read (default true).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        link_discovery: Option<bool>,
        /// Keep or drop query strings when normalizing candidates (default keep).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        normalize_query: Option<SiteQueryNormalization>,
    },
}

/// Query-string handling for site URL inventory normalization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SiteQueryNormalization {
    /// Preserve query strings.
    #[default]
    Keep,
    /// Remove query strings.
    Drop,
}

impl BatchFetchItem {
    /// Returns a human-readable label for this item.
    pub fn label(&self) -> String {
        match self {
            BatchFetchItem::Web { url, .. } => url.clone(),
            BatchFetchItem::Repo { target, host, .. } => {
                let h = host.as_deref().unwrap_or("github");
                format!("{h}:{}/{}/{}", target.owner, target.repo, target.path)
            }
            BatchFetchItem::Site { url, .. } => url.clone(),
        }
    }

    /// Returns the max_chars for this item, if set.
    pub fn max_chars(&self) -> Option<usize> {
        match self {
            BatchFetchItem::Web { max_chars, .. } => *max_chars,
            BatchFetchItem::Repo { target, .. } => target.max_chars,
            BatchFetchItem::Site { .. } => None,
        }
    }

    /// Returns the focus query for this item, if set.
    pub fn focus_query(&self) -> Option<&str> {
        match self {
            BatchFetchItem::Web { focus, .. } => focus.as_deref(),
            BatchFetchItem::Repo { focus, .. } => focus.as_deref(),
            BatchFetchItem::Site { .. } => None,
        }
    }

    /// Returns the focus chunk cap for this item, if set.
    pub fn focus_max_chunks(&self) -> Option<usize> {
        match self {
            BatchFetchItem::Web {
                focus_max_chunks, ..
            } => *focus_max_chunks,
            BatchFetchItem::Repo {
                focus_max_chunks, ..
            } => *focus_max_chunks,
            BatchFetchItem::Site { .. } => None,
        }
    }

    /// Returns the focus character cap for this item, if set.
    pub fn focus_max_chars(&self) -> Option<usize> {
        match self {
            BatchFetchItem::Web {
                focus_max_chars, ..
            } => *focus_max_chars,
            BatchFetchItem::Repo {
                focus_max_chars, ..
            } => *focus_max_chars,
            BatchFetchItem::Site { .. } => None,
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
    /// A bounded site URL inventory.
    Site,
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
    /// Serialized response payload (fetch or repo_fetch response shape).
    /// Budget-omitted items carry `null`; successful items carry their payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<serde_json::Value>,
    /// Error message. Present only when `ok = false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Approximate character count of the returned text content.
    pub chars_returned: usize,
    /// Unicode scalar count of the serialized response value after response-detail projection.
    #[serde(default)]
    pub payload_chars: usize,
    /// Response fields removed to fit the aggregate payload budget.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded_fields: Vec<String>,
    /// Whether this item's text was truncated at the per-item max_chars cap.
    pub truncated: bool,
}

/// Aggregate telemetry for a canonical `fetch` response.
///
/// All counters are deterministic derivations of the request and the
/// per-item outcomes. Focus projection never changes stable item IDs;
/// `focused_*` counters describe the additive focus projection only.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct BatchFetchTelemetry {
    /// Items requested after `max_items` clamping.
    pub items_requested: usize,
    /// Items with `ok = true`.
    pub items_completed: usize,
    /// Items with `ok = false`.
    pub items_failed: usize,
    /// Items with `truncated = true`.
    pub items_truncated: usize,
    /// Total characters returned across all items.
    pub total_chars_returned: usize,
    /// Items where a focus query was requested.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub focused_items: usize,
    /// Focused chunks selected across all items.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub focused_chunks_selected: usize,
    /// Focused characters returned across all items.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub focused_chars_returned: usize,
    /// Whether the aggregate `max_total_chars` budget was exhausted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub aggregate_budget_exhausted: bool,
    /// Web items served from cache without revalidation.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_hits: usize,
    /// Web items served after conditional revalidation (HTTP 304).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_revalidated: usize,
    /// Web items fetched fresh from the network.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_misses: usize,
    /// Web items that bypassed cache reads.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_bypassed: usize,
    /// Web items that could not be cached.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_not_cacheable: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Response type for the canonical `fetch` tool.
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
    /// Sum of serialized response-value characters across emitted items.
    #[serde(default)]
    pub total_payload_chars: usize,
    /// Per-item results, in input order.
    pub results: Vec<BatchFetchResult>,
    /// Batch-level warnings (e.g. budget exhaustion, partial failure).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Structured warnings with stable codes and severity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_warnings: Vec<crate::core::warning::AgentWarning>,
    /// Aggregate telemetry for items, budgets, focus, and cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<BatchFetchTelemetry>,
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
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            max_chars: None,
            cache_policy: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
            snapshot: None,
            pdf: None,
            render: None,
            browser_profile: None,
            include_screenshot: None,
        };
        let v = serde_json::to_value(&web).unwrap();
        assert_eq!(v["type"], "web");
        assert_eq!(v["url"], "https://example.com");

        let repo = BatchFetchItem::Repo {
            host: Some("github".to_string()),
            target: crate::core::repo_fetch::RepoTarget {
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
                symbol: None,
                symbol_kind: None,
                match_text: None,
                expand_to_block: None,
                max_block_lines: None,
                prefer_local: None,
            },
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
        };
        let v = serde_json::to_value(&repo).unwrap();
        assert_eq!(v["type"], "repo");
        assert_eq!(v["owner"], "tokio-rs");

        let site = BatchFetchItem::Site {
            url: "https://example.com/docs/".to_string(),
            limit: None,
            include_subdomains: None,
            link_discovery: None,
            normalize_query: None,
        };
        let value = serde_json::to_value(site).unwrap();
        assert_eq!(value["type"], "site");
        assert_eq!(value["url"], "https://example.com/docs/");
    }

    #[test]
    fn batch_fetch_item_label() {
        let web = BatchFetchItem::Web {
            url: "https://example.com".to_string(),
            extract_mode: None,
            include_links: None,
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            max_chars: None,
            cache_policy: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
            snapshot: None,
            pdf: None,
            render: None,
            browser_profile: None,
            include_screenshot: None,
        };
        assert_eq!(web.label(), "https://example.com");

        let repo = BatchFetchItem::Repo {
            host: None,
            target: crate::core::repo_fetch::RepoTarget {
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
                symbol: None,
                symbol_kind: None,
                match_text: None,
                expand_to_block: None,
                max_block_lines: None,
                prefer_local: None,
            },
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
        };
        assert_eq!(repo.label(), "github:tokio-rs/axum/src/lib.rs");
    }

    #[test]
    fn batch_fetch_target_options_default_and_round_trip() {
        let legacy_web = serde_json::json!({
            "type": "web",
            "url": "https://example.com"
        });
        let parsed_web: BatchFetchItem = serde_json::from_value(legacy_web.clone()).unwrap();
        match &parsed_web {
            BatchFetchItem::Web {
                pdf,
                render,
                browser_profile,
                ..
            } => {
                assert!(pdf.is_none());
                assert!(render.is_none());
                assert!(browser_profile.is_none());
            }
            _ => panic!("expected web item"),
        }
        assert_eq!(serde_json::to_value(parsed_web).unwrap(), legacy_web);

        let configured_web = serde_json::json!({
            "type": "web",
            "url": "https://example.com",
            "pdf": {"pages": "1-2"},
            "render": "auto",
            "browser_profile": "docs"
        });
        let parsed_web: BatchFetchItem = serde_json::from_value(configured_web.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed_web).unwrap(), configured_web);

        let legacy_repo = serde_json::json!({
            "type": "repo",
            "owner": "tokio-rs",
            "repo": "axum",
            "path": "src/lib.rs"
        });
        let parsed_repo: BatchFetchItem = serde_json::from_value(legacy_repo.clone()).unwrap();
        match &parsed_repo {
            BatchFetchItem::Repo { target, .. } => {
                assert!(target.symbol.is_none());
                assert!(target.symbol_kind.is_none());
                assert!(target.match_text.is_none());
                assert!(target.expand_to_block.is_none());
                assert!(target.max_block_lines.is_none());
                assert!(target.prefer_local.is_none());
            }
            _ => panic!("expected repo item"),
        }
        assert_eq!(serde_json::to_value(parsed_repo).unwrap(), legacy_repo);

        let configured_repo = serde_json::json!({
            "type": "repo",
            "owner": "tokio-rs",
            "repo": "axum",
            "path": "src/lib.rs",
            "symbol": "Router",
            "symbol_kind": "struct",
            "match_text": "pub struct Router",
            "expand_to_block": true,
            "max_block_lines": 80,
            "prefer_local": true
        });
        let parsed_repo: BatchFetchItem = serde_json::from_value(configured_repo.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed_repo).unwrap(), configured_repo);
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
