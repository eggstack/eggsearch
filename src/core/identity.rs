//! Deterministic cross-tool identity model.
//!
//! Every tool output type (source cards, suggested fetches, fetch
//! responses, batch fetch results, evidence bundle entries) carries a
//! stable, content-derived ID alongside the existing random per-response
//! ID. This module provides the canonical hashing functions and key
//! structs used to generate those IDs.
//!
//! **Design principles:**
//!
//! - IDs are deterministic: identical inputs always produce the same ID.
//! - IDs are content-derived: they hash the fields that define identity,
//!   not incidental metadata.
//! - IDs use `DefaultHasher` (SipHash 1-3) for speed and zero external
//!   dependencies. The 64-bit output is formatted as 16 hex chars.
//! - IDs are prefixed with a human-readable tag (`src_`, `fetch_`,
//!   `suggested_`) so callers can distinguish entity types at a glance.
//! - The existing random UUID-based `id` on `SourceCard` is preserved
//!   for backward compatibility; the new `stable_id` field carries the
//!   deterministic identity.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::repo_fetch::RepoLocator;
use crate::core::source_card::SourceKind;

// ---------------------------------------------------------------------------
// Source ID
// ---------------------------------------------------------------------------

/// Canonical key for a source card's deterministic identity.
///
/// Two source cards with the same `(provider_id, url, title, source_kind)`
/// tuple will produce the same `stable_id`.
#[derive(Clone, Debug, Default)]
pub struct SourceKey<'a> {
    /// The provider that returned this source (first provider wins).
    pub provider_id: Option<&'a str>,
    /// The source URL.
    pub url: Option<&'a str>,
    /// The source title.
    pub title: Option<&'a str>,
    /// The classified source kind.
    pub source_kind: Option<SourceKind>,
}

/// Compute a deterministic source ID from a canonical key.
///
/// `stable_id = src_<16hex(priority_provider + url + title + source_kind)>`
///
/// The first provider in the list is treated as the "priority" provider
/// for hashing purposes, matching the existing `compute_source_id` convention.
pub fn compute_source_id(key: &SourceKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    key.provider_id.unwrap_or("").hash(&mut hasher);
    key.url.unwrap_or("").hash(&mut hasher);
    key.title.unwrap_or("").hash(&mut hasher);
    format!("{:?}", key.source_kind).hash(&mut hasher);
    format!("src_{:016x}", hasher.finish())
}

/// Convenience: compute a source ID from individual fields.
pub fn source_id(
    provider_id: Option<&str>,
    url: Option<&str>,
    title: Option<&str>,
    source_kind: Option<SourceKind>,
) -> String {
    compute_source_id(&SourceKey {
        provider_id,
        url,
        title,
        source_kind,
    })
}

// ---------------------------------------------------------------------------
// Fetch ID
// ---------------------------------------------------------------------------

/// Canonical key for a fetch response's deterministic identity.
#[derive(Clone, Debug, Default)]
pub struct FetchKey<'a> {
    /// The fetched URL (used when no locator is present).
    pub url: Option<&'a str>,
    /// Structured repo locator (takes precedence over URL for hashing).
    pub locator: Option<&'a RepoLocator>,
    /// Start line of the fetched range.
    pub line_start: Option<u32>,
    /// End line of the fetched range.
    pub line_end: Option<u32>,
    /// First 64 chars of the fetched text, used for content stability.
    pub text_prefix: Option<&'a str>,
}

/// Compute a deterministic fetch ID from a canonical key.
///
/// `fetch_id = fetch_<16hex(locator_or_url + line_range + text_hash_prefix)>`
pub fn compute_fetch_id(key: &FetchKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    if let Some(loc) = key.locator {
        format!("{:?}", loc).hash(&mut hasher);
    } else {
        key.url.unwrap_or("").hash(&mut hasher);
    }
    key.line_start.hash(&mut hasher);
    key.line_end.hash(&mut hasher);
    let prefix = key.text_prefix.unwrap_or("");
    let prefix = if prefix.len() > 64 {
        &prefix[..64]
    } else {
        prefix
    };
    prefix.hash(&mut hasher);
    format!("fetch_{:016x}", hasher.finish())
}

/// Convenience: compute a fetch ID from individual fields.
pub fn fetch_id(
    url: Option<&str>,
    locator: Option<&RepoLocator>,
    line_start: Option<u32>,
    line_end: Option<u32>,
    text_prefix: Option<&str>,
) -> String {
    compute_fetch_id(&FetchKey {
        url,
        locator,
        line_start,
        line_end,
        text_prefix,
    })
}

// ---------------------------------------------------------------------------
// Suggested Fetch ID
// ---------------------------------------------------------------------------

/// Canonical key for a suggested fetch's deterministic identity.
#[derive(Clone, Debug, Default)]
pub struct SuggestedFetchKey<'a> {
    /// The suggested fetch URL.
    pub url: &'a str,
    /// The result group kind (as Debug string).
    pub group: &'a str,
    /// The priority rank (1-based).
    pub priority: u8,
}

/// Compute a deterministic suggested-fetch ID from a canonical key.
///
/// `suggested_id = suggested_<16hex(url + group + priority)>`
pub fn compute_suggested_fetch_id(key: &SuggestedFetchKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    key.url.hash(&mut hasher);
    key.group.hash(&mut hasher);
    key.priority.hash(&mut hasher);
    format!("suggested_{:016x}", hasher.finish())
}

/// Convenience: compute a suggested-fetch ID from individual fields.
pub fn suggested_fetch_id(url: &str, group: &str, priority: u8) -> String {
    compute_suggested_fetch_id(&SuggestedFetchKey {
        url,
        group,
        priority,
    })
}

// ---------------------------------------------------------------------------
// Batch Fetch ID
// ---------------------------------------------------------------------------

/// Canonical key for a batch-fetch item's deterministic identity.
#[derive(Clone, Debug, Default)]
pub struct BatchFetchKey<'a> {
    /// The item label (URL or locator string).
    pub label: &'a str,
    /// The input order index.
    pub index: usize,
}

/// Compute a deterministic batch-fetch ID from a canonical key.
///
/// `batch_id = batch_<16hex(label + index)>`
pub fn compute_batch_fetch_id(key: &BatchFetchKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    key.label.hash(&mut hasher);
    key.index.hash(&mut hasher);
    format!("batch_{:016x}", hasher.finish())
}

/// Convenience: compute a batch-fetch ID from individual fields.
pub fn batch_fetch_id(label: &str, index: usize) -> String {
    compute_batch_fetch_id(&BatchFetchKey { label, index })
}

// ---------------------------------------------------------------------------
// Bundle ID (re-export from evidence_bundle)
// ---------------------------------------------------------------------------

/// Re-export the existing bundle ID computation for convenience.
pub use crate::core::evidence_bundle::compute_bundle_id;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_id_deterministic() {
        let a = source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        let b = source_id(
            Some("duckduckgo"),
            Some("https://docs.rs/axum"),
            Some("axum - Rust"),
            Some(SourceKind::OfficialDocs),
        );
        assert_eq!(a, b);
        assert!(a.starts_with("src_"));
        assert_eq!(a.len(), 20); // "src_" + 16 hex
    }

    #[test]
    fn source_id_differs_on_url() {
        let a = source_id(Some("p"), Some("https://a.com"), None, None);
        let b = source_id(Some("p"), Some("https://b.com"), None, None);
        assert_ne!(a, b);
    }

    #[test]
    fn source_id_differs_on_provider() {
        let a = source_id(Some("duckduckgo"), Some("https://a.com"), None, None);
        let b = source_id(Some("brave"), Some("https://a.com"), None, None);
        assert_ne!(a, b);
    }

    #[test]
    fn source_id_differs_on_title() {
        let a = source_id(Some("p"), Some("https://a.com"), Some("foo"), None);
        let b = source_id(Some("p"), Some("https://a.com"), Some("bar"), None);
        assert_ne!(a, b);
    }

    #[test]
    fn source_id_differs_on_kind() {
        let a = source_id(Some("p"), Some("https://a.com"), None, Some(SourceKind::OfficialDocs));
        let b = source_id(Some("p"), Some("https://a.com"), None, Some(SourceKind::PackageRegistry));
        assert_ne!(a, b);
    }

    #[test]
    fn source_id_null_fields() {
        let a = source_id(None, None, None, None);
        assert!(a.starts_with("src_"));
        assert_eq!(a.len(), 20);
    }

    #[test]
    fn fetch_id_deterministic() {
        let a = fetch_id(
            Some("https://raw.githubusercontent.com/a/b/main/Cargo.toml"),
            None,
            Some(1),
            Some(10),
            Some("name = \"axum\""),
        );
        let b = fetch_id(
            Some("https://raw.githubusercontent.com/a/b/main/Cargo.toml"),
            None,
            Some(1),
            Some(10),
            Some("name = \"axum\""),
        );
        assert_eq!(a, b);
        assert!(a.starts_with("fetch_"));
        assert_eq!(a.len(), 22); // "fetch_" + 16 hex
    }

    #[test]
    fn fetch_id_differs_on_line_range() {
        let a = fetch_id(Some("https://a.com"), None, Some(1), Some(10), None);
        let b = fetch_id(Some("https://a.com"), None, Some(5), Some(15), None);
        assert_ne!(a, b);
    }

    #[test]
    fn fetch_id_with_locator() {
        let loc = RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(crate::core::code_metadata::CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("tokio".to_string()),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            workspace_root: None,
        };
        let a = fetch_id(None, Some(&loc), None, None, None);
        assert!(a.starts_with("fetch_"));
        assert_eq!(a.len(), 22);
    }

    #[test]
    fn suggested_fetch_id_deterministic() {
        let a = suggested_fetch_id("https://docs.rs/axum", "OfficialDocs", 1);
        let b = suggested_fetch_id("https://docs.rs/axum", "OfficialDocs", 1);
        assert_eq!(a, b);
        assert!(a.starts_with("suggested_"));
        assert_eq!(a.len(), 26); // "suggested_" + 16 hex
    }

    #[test]
    fn suggested_fetch_id_differs_on_priority() {
        let a = suggested_fetch_id("https://a.com", "g", 1);
        let b = suggested_fetch_id("https://a.com", "g", 2);
        assert_ne!(a, b);
    }

    #[test]
    fn batch_fetch_id_deterministic() {
        let a = batch_fetch_id("https://docs.rs/axum", 0);
        let b = batch_fetch_id("https://docs.rs/axum", 0);
        assert_eq!(a, b);
        assert!(a.starts_with("batch_"));
        assert_eq!(a.len(), 22); // "batch_" + 16 hex
    }

    #[test]
    fn batch_fetch_id_differs_on_index() {
        let a = batch_fetch_id("https://a.com", 0);
        let b = batch_fetch_id("https://a.com", 1);
        assert_ne!(a, b);
    }

    #[test]
    fn source_key_struct_matches_convenience_fn() {
        let key = SourceKey {
            provider_id: Some("brave"),
            url: Some("https://example.com"),
            title: Some("Example"),
            source_kind: Some(SourceKind::Reference),
        };
        let from_struct = compute_source_id(&key);
        let from_fn = source_id(
            Some("brave"),
            Some("https://example.com"),
            Some("Example"),
            Some(SourceKind::Reference),
        );
        assert_eq!(from_struct, from_fn);
    }

    #[test]
    fn fetch_key_struct_matches_convenience_fn() {
        let key = FetchKey {
            url: Some("https://example.com"),
            locator: None,
            line_start: Some(1),
            line_end: Some(50),
            text_prefix: Some("hello"),
        };
        let from_struct = compute_fetch_id(&key);
        let from_fn = fetch_id(Some("https://example.com"), None, Some(1), Some(50), Some("hello"));
        assert_eq!(from_struct, from_fn);
    }
}
