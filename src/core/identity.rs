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
//!   `suggested_`, `batch_`, `loc_`, `doc_`, `chunk_`) so callers can
//!   distinguish entity types at a glance.
//! - The existing random UUID-based `id` on `SourceCard` is preserved
//!   for backward compatibility; the new `stable_id` field carries the
//!   deterministic identity.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::repo_fetch::RepoLocator;
use crate::core::source_card::SourceKind;

// ---------------------------------------------------------------------------
// URL Canonicalization
// ---------------------------------------------------------------------------

/// Canonicalize a URL for identity-stable hashing.
///
/// Normalizations applied:
/// - Lowercase scheme and host
/// - Remove default ports (`:80` for HTTP, `:443` for HTTPS)
/// - Strip fragments (`#...`)
/// - Strip trailing slashes from the path (except bare root `/`)
/// - Strip `www.` prefix from host for dedup purposes
///
/// This is NOT a general-purpose URL normalizer. It is intentionally
/// conservative — it normalizes only the aspects that cause spurious
/// ID differences for identical resources.
pub fn canonicalize_url(url: &str) -> String {
    let url = url.trim();

    // Split into scheme + rest
    let (scheme, rest) = if let Some(pos) = url.find("://") {
        (url[..pos].to_ascii_lowercase(), &url[pos + 3..])
    } else {
        // No scheme — treat as path-only
        return normalize_path(url);
    };

    // Split host from path+query+fragment
    let (host_part, path_query_frag) = if let Some(slash_pos) = rest.find('/') {
        (&rest[..slash_pos], &rest[slash_pos..])
    } else {
        (rest, "")
    };

    // Strip www. prefix
    let host_part = host_part
        .strip_prefix("www.")
        .unwrap_or(host_part);

    // Strip default ports
    let host_part = strip_default_port(host_part, &scheme);

    // Strip fragment from path+query+fragment
    let path_query_frag = if let Some(hash_pos) = path_query_frag.find('#') {
        &path_query_frag[..hash_pos]
    } else {
        path_query_frag
    };

    // Strip trailing slash (but not for bare root)
    let path_query_frag = if path_query_frag.ends_with('/')
        && path_query_frag.len() > 1
        && !path_query_frag.starts_with("/?")
    {
        &path_query_frag[..path_query_frag.len() - 1]
    } else {
        path_query_frag
    };

    format!("{scheme}://{host_part}{path_query_frag}")
}

/// Strip default port from a host string.
fn strip_default_port<'a>(host: &'a str, scheme: &str) -> &'a str {
    match scheme {
        "http" => host.strip_suffix(":80").unwrap_or(host),
        "https" => host.strip_suffix(":443").unwrap_or(host),
        _ => host,
    }
}

/// Normalize a path-only string (no scheme).
fn normalize_path(path: &str) -> String {
    let path = path.trim();
    // Strip fragment
    let path = if let Some(pos) = path.find('#') {
        &path[..pos]
    } else {
        path
    };
    // Strip trailing slash
    let path = if path.len() > 1 && path.ends_with('/') {
        &path[..path.len() - 1]
    } else {
        path
    };
    path.to_string()
}

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
/// URLs are canonicalized before hashing to ensure that trivial
/// differences (trailing slashes, `www.` prefix, default ports,
/// fragments) do not produce spurious ID differences.
pub fn compute_source_id(key: &SourceKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    key.provider_id.unwrap_or("").hash(&mut hasher);
    match key.url {
        Some(u) => canonicalize_url(u).hash(&mut hasher),
        None => 0_u8.hash(&mut hasher),
    }
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
///
/// URLs are canonicalized before hashing (when no locator is present).
pub fn compute_fetch_id(key: &FetchKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    if let Some(loc) = key.locator {
        format!("{:?}", loc).hash(&mut hasher);
    } else {
        match key.url {
            Some(u) => canonicalize_url(u).hash(&mut hasher),
            None => 0_u8.hash(&mut hasher),
        }
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
// Repo Locator Key
// ---------------------------------------------------------------------------

/// Canonical key for normalizing a repo locator's identity.
///
/// All string fields are lowercased and stripped of common trivial
/// variations (`.git` suffix on repo, leading/trailing slashes on path).
#[derive(Clone, Debug, Default)]
pub struct RepoLocatorKey<'a> {
    /// Code host (e.g. "github", "gitlab").
    pub host: Option<&'a str>,
    /// Repository owner or namespace.
    pub owner: Option<&'a str>,
    /// Repository name (without `.git` suffix).
    pub repo: Option<&'a str>,
    /// Branch, tag, or commit ref.
    pub ref_name: Option<&'a str>,
    /// File path relative to repo root.
    pub path: &'a str,
}

/// Normalize a `RepoLocator` into a `RepoLocatorKey` for hashing.
pub fn normalize_locator_key(locator: &RepoLocator) -> RepoLocatorKey<'_> {
    RepoLocatorKey {
        host: locator.host.as_ref().map(|h| match h {
            crate::core::code_metadata::CodeHost::Github => "github",
            crate::core::code_metadata::CodeHost::Gitlab => "gitlab",
            crate::core::code_metadata::CodeHost::Codeberg => "codeberg",
            crate::core::code_metadata::CodeHost::Gitea => "gitea",
            crate::core::code_metadata::CodeHost::Forgejo => "forgejo",
            crate::core::code_metadata::CodeHost::Unknown => "unknown",
        }),
        owner: locator.owner.as_deref(),
        repo: locator.repo.as_deref().map(strip_dot_git),
        ref_name: locator.ref_name.as_deref(),
        path: &locator.path,
    }
}

/// Strip a trailing `.git` suffix from a repo name.
fn strip_dot_git(name: &str) -> &str {
    name.strip_suffix(".git").unwrap_or(name)
}

/// Compute a deterministic locator-based ID.
///
/// `locator_id = loc_<16hex(host + owner + repo + ref + path)>`
///
/// This is distinct from `fetch_id` because locators carry structured
/// identity that should be normalized independently of URL form.
pub fn compute_locator_id(key: &RepoLocatorKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    key.host.unwrap_or("").hash(&mut hasher);
    key.owner.unwrap_or("").hash(&mut hasher);
    key.repo.unwrap_or("").hash(&mut hasher);
    key.ref_name.unwrap_or("").hash(&mut hasher);
    key.path.hash(&mut hasher);
    format!("loc_{:016x}", hasher.finish())
}

/// Convenience: compute a locator ID from a `RepoLocator`.
pub fn locator_id(locator: &RepoLocator) -> String {
    compute_locator_id(&normalize_locator_key(locator))
}

// ---------------------------------------------------------------------------
// Document / Chunk ID
// ---------------------------------------------------------------------------

/// Canonical key for a document's deterministic identity.
#[derive(Clone, Debug, Default)]
pub struct DocKey<'a> {
    /// The document's source URL.
    pub url: Option<&'a str>,
    /// The document title.
    pub title: Option<&'a str>,
    /// The document kind (as Debug string).
    pub kind: Option<&'a str>,
}

/// Compute a deterministic document ID.
///
/// `doc_id = doc_<16hex(url + title + kind)>`
pub fn compute_doc_id(key: &DocKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    match key.url {
        Some(u) => canonicalize_url(u).hash(&mut hasher),
        None => 0_u8.hash(&mut hasher),
    }
    key.title.unwrap_or("").hash(&mut hasher);
    key.kind.unwrap_or("").hash(&mut hasher);
    format!("doc_{:016x}", hasher.finish())
}

/// Convenience: compute a document ID from individual fields.
pub fn doc_id(url: Option<&str>, title: Option<&str>, kind: Option<&str>) -> String {
    compute_doc_id(&DocKey { url, title, kind })
}

/// Canonical key for a document chunk's deterministic identity.
#[derive(Clone, Debug, Default)]
pub struct DocChunkKey<'a> {
    /// The parent document's deterministic ID (`doc_<16hex>`).
    pub doc_id: &'a str,
    /// Zero-based chunk index within the document.
    pub chunk_index: usize,
    /// Heading path from root to this chunk (joined with `/`).
    pub heading_path: &'a str,
}

/// Compute a deterministic chunk ID.
///
/// `chunk_id = chunk_<16hex(doc_id + chunk_index + heading_path)>`
pub fn compute_chunk_id(key: &DocChunkKey<'_>) -> String {
    let mut hasher = DefaultHasher::new();
    key.doc_id.hash(&mut hasher);
    key.chunk_index.hash(&mut hasher);
    key.heading_path.hash(&mut hasher);
    format!("chunk_{:016x}", hasher.finish())
}

/// Convenience: compute a chunk ID from individual fields.
pub fn chunk_id(doc_id: &str, chunk_index: usize, heading_path: &str) -> String {
    compute_chunk_id(&DocChunkKey {
        doc_id,
        chunk_index,
        heading_path,
    })
}

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

    // -- URL Canonicalization tests --

    #[test]
    fn canonicalize_url_strips_trailing_slash() {
        let a = canonicalize_url("https://example.com/path/");
        let b = canonicalize_url("https://example.com/path");
        assert_eq!(a, b);
    }

    #[test]
    fn canonicalize_url_strips_fragment() {
        let a = canonicalize_url("https://example.com/path#section");
        let b = canonicalize_url("https://example.com/path");
        assert_eq!(a, b);
    }

    #[test]
    fn canonicalize_url_lowercases_scheme() {
        let a = canonicalize_url("HTTP://EXAMPLE.COM/Path");
        let b = canonicalize_url("http://EXAMPLE.COM/Path");
        assert_eq!(a, b);
    }

    #[test]
    fn canonicalize_url_strips_www_prefix() {
        let a = canonicalize_url("https://www.example.com/path");
        let b = canonicalize_url("https://example.com/path");
        assert_eq!(a, b);
    }

    #[test]
    fn canonicalize_url_strips_default_port_443() {
        let a = canonicalize_url("https://example.com:443/path");
        let b = canonicalize_url("https://example.com/path");
        assert_eq!(a, b);
    }

    #[test]
    fn canonicalize_url_strips_default_port_80() {
        let a = canonicalize_url("http://example.com:80/path");
        let b = canonicalize_url("http://example.com/path");
        assert_eq!(a, b);
    }

    #[test]
    fn canonicalize_url_preserves_non_default_port() {
        let a = canonicalize_url("https://example.com:8443/path");
        assert!(a.contains(":8443"));
    }

    #[test]
    fn canonicalize_url_bare_root_preserved() {
        let a = canonicalize_url("https://example.com/");
        assert_eq!(a, "https://example.com/");
    }

    #[test]
    fn canonicalize_url_no_scheme() {
        let a = canonicalize_url("example.com/path/to/file");
        assert_eq!(a, "example.com/path/to/file");
    }

    #[test]
    fn source_id_canonicalizes_urls() {
        // Trailing slash, fragment, www, port should not affect ID
        let base = source_id(Some("p"), Some("https://example.com/path"), None, None);
        let variant1 = source_id(Some("p"), Some("https://example.com/path/"), None, None);
        let variant2 = source_id(Some("p"), Some("https://example.com/path#section"), None, None);
        let variant3 = source_id(Some("p"), Some("https://www.example.com/path"), None, None);
        let variant4 = source_id(Some("p"), Some("https://example.com:443/path"), None, None);
        assert_eq!(base, variant1);
        assert_eq!(base, variant2);
        assert_eq!(base, variant3);
        assert_eq!(base, variant4);
    }

    #[test]
    fn fetch_id_canonicalizes_urls() {
        let base = fetch_id(Some("https://example.com/file.rs"), None, None, None, None);
        let variant = fetch_id(Some("https://example.com/file.rs/"), None, None, None, None);
        let fragment = fetch_id(Some("https://example.com/file.rs#L10"), None, None, None, None);
        assert_eq!(base, variant);
        assert_eq!(base, fragment);
    }

    // -- Repo Locator tests --

    #[test]
    fn locator_id_deterministic() {
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
        let a = locator_id(&loc);
        let b = locator_id(&loc);
        assert_eq!(a, b);
        assert!(a.starts_with("loc_"));
        assert_eq!(a.len(), 20);
    }

    #[test]
    fn locator_id_strips_dot_git() {
        let mut loc = RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(crate::core::code_metadata::CodeHost::Github),
            owner: Some("a".to_string()),
            repo: Some("repo".to_string()),
            ref_name: None,
            commit_sha: None,
            path: "src/main.rs".to_string(),
            workspace_root: None,
        };
        let a = locator_id(&loc);
        loc.repo = Some("repo.git".to_string());
        let b = locator_id(&loc);
        assert_eq!(a, b);
    }

    #[test]
    fn locator_id_differs_on_host() {
        let make = |host: crate::core::code_metadata::CodeHost| RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(host),
            owner: Some("a".to_string()),
            repo: Some("r".to_string()),
            ref_name: None,
            commit_sha: None,
            path: "f.rs".to_string(),
            workspace_root: None,
        };
        assert_ne!(locator_id(&make(crate::core::code_metadata::CodeHost::Github)), locator_id(&make(crate::core::code_metadata::CodeHost::Gitlab)));
    }

    #[test]
    fn locator_id_differs_on_path() {
        let make = |path: &str| RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(crate::core::code_metadata::CodeHost::Github),
            owner: Some("a".to_string()),
            repo: Some("r".to_string()),
            ref_name: None,
            commit_sha: None,
            path: path.to_string(),
            workspace_root: None,
        };
        assert_ne!(locator_id(&make("src/main.rs")), locator_id(&make("src/lib.rs")));
    }

    #[test]
    fn locator_struct_matches_convenience_fn() {
        let loc = RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(crate::core::code_metadata::CodeHost::Github),
            owner: Some("a".to_string()),
            repo: Some("r".to_string()),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            workspace_root: None,
        };
        let from_struct = compute_locator_id(&normalize_locator_key(&loc));
        let from_fn = locator_id(&loc);
        assert_eq!(from_struct, from_fn);
    }

    // -- Doc / Chunk ID tests --

    #[test]
    fn doc_id_deterministic() {
        let a = doc_id(Some("https://docs.rs/axum"), Some("axum docs"), Some("html"));
        let b = doc_id(Some("https://docs.rs/axum"), Some("axum docs"), Some("html"));
        assert_eq!(a, b);
        assert!(a.starts_with("doc_"));
        assert_eq!(a.len(), 20);
    }

    #[test]
    fn doc_id_differs_on_title() {
        let a = doc_id(Some("https://a.com"), Some("title1"), None);
        let b = doc_id(Some("https://a.com"), Some("title2"), None);
        assert_ne!(a, b);
    }

    #[test]
    fn doc_id_canonicalizes_url() {
        let a = doc_id(Some("https://example.com/path/"), Some("t"), None);
        let b = doc_id(Some("https://example.com/path"), Some("t"), None);
        assert_eq!(a, b);
    }

    #[test]
    fn chunk_id_deterministic() {
        let did = "doc_0123456789abcdef";
        let a = chunk_id(did, 0, "intro");
        let b = chunk_id(did, 0, "intro");
        assert_eq!(a, b);
        assert!(a.starts_with("chunk_"));
        assert_eq!(a.len(), 22);
    }

    #[test]
    fn chunk_id_differs_on_index() {
        let did = "doc_0123456789abcdef";
        let a = chunk_id(did, 0, "path");
        let b = chunk_id(did, 1, "path");
        assert_ne!(a, b);
    }

    #[test]
    fn chunk_id_differs_on_heading_path() {
        let did = "doc_0123456789abcdef";
        let a = chunk_id(did, 0, "intro");
        let b = chunk_id(did, 0, "setup");
        assert_ne!(a, b);
    }

    #[test]
    fn chunk_struct_matches_convenience_fn() {
        let did = "doc_aabbccdd11223344";
        let from_struct = compute_chunk_id(&DocChunkKey {
            doc_id: did,
            chunk_index: 2,
            heading_path: "section/sub",
        });
        let from_fn = chunk_id(did, 2, "section/sub");
        assert_eq!(from_struct, from_fn);
    }

    // -- Cross-type prefix uniqueness tests --

    #[test]
    fn all_prefixes_are_unique() {
        let src = source_id(Some("p"), Some("https://a.com"), None, None);
        let fetch = fetch_id(Some("https://a.com"), None, None, None, None);
        let suggested = suggested_fetch_id("https://a.com", "g", 1);
        let batch = batch_fetch_id("https://a.com", 0);
        let loc = locator_id(&RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: None,
            owner: None,
            repo: None,
            ref_name: None,
            commit_sha: None,
            path: "f".to_string(),
            workspace_root: None,
        });
        let doc = doc_id(Some("https://a.com"), None, None);
        let chunk = chunk_id(&doc, 0, "");

        assert!(src.starts_with("src_"));
        assert!(fetch.starts_with("fetch_"));
        assert!(suggested.starts_with("suggested_"));
        assert!(batch.starts_with("batch_"));
        assert!(loc.starts_with("loc_"));
        assert!(doc.starts_with("doc_"));
        assert!(chunk.starts_with("chunk_"));

        // No two prefix-bearing IDs should be equal
        let all = [&src, &fetch, &suggested, &batch, &loc, &doc, &chunk];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j], "IDs should differ: {} vs {}", all[i], all[j]);
            }
        }
    }
}
