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
//! - IDs use FNV-1a 64-bit — a simple, well-known hash with zero
//!   external dependencies and deterministic output across platforms.
//!   The 64-bit output is formatted as 16 hex chars. FNV-1a has known
//!   collision tradeoffs vs cryptographic hashes but is sufficient for
//!   content-derived identity keys at the scale of search results.
//! - IDs include a versioned input prefix (`eggsearch-id-v1\0`) and
//!   entity sub-namespace to prevent cross-entity collisions and enable
//!   future algorithm migration.
//! - IDs are prefixed with a human-readable tag (`src_`, `fetch_`,
//!   `suggested_`, `batch_`, `loc_`, `doc_`, `chunk_`) so callers can
//!   distinguish entity types at a glance.
//! - The existing random UUID-based `id` on `SourceCard` is preserved
//!   for backward compatibility; the new `stable_id` field carries the
//!   deterministic identity.

use crate::core::repo_fetch::RepoLocator;
use crate::core::source_card::SourceKind;

// ---------------------------------------------------------------------------
// FNV-1a 64-bit Hash
// ---------------------------------------------------------------------------

/// FNV-1a 64-bit hash — explicit, stable, zero external dependencies.
///
/// Algorithm reference: <http://www.isthe.com/chongo/tech/comp/fnv/>
///
/// This replaces `DefaultHasher` (SipHash 1-3) so the identity system
/// does not depend on stdlib hasher internals. FNV-1a is simple, fast,
/// and produces deterministic output across all platforms.
///
/// **Collision tradeoff:** FNV-1a is a non-cryptographic hash with
/// higher collision probability than SipHash at large input volumes.
/// For eggsearch's use case (content-derived identity keys over
/// bounded search result sets) this is acceptable — the hash is
/// combined with a versioned prefix and entity namespace, and
/// collisions only affect deduplication, not correctness.
pub struct FnvHasher {
    state: u64,
}

impl Default for FnvHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl FnvHasher {
    /// Create a new FNV-1a hasher with the standard offset basis.
    pub fn new() -> Self {
        Self {
            state: 14_695_981_039_346_656_037,
        }
    }

    /// Feed bytes into the hasher.
    pub fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= byte as u64;
            self.state = self.state.wrapping_mul(1_099_511_628_211);
        }
    }

    /// Finalize and return the 64-bit hash value.
    pub fn finish(self) -> u64 {
        self.state
    }
}

// ---------------------------------------------------------------------------
// Versioned Input Prefix
// ---------------------------------------------------------------------------

/// Versioned prefix for identity hashing. Every input is prefixed with
/// this to prevent cross-entity collisions and to enable future
/// algorithm migration by bumping the version string.
const ID_VERSION_PREFIX: &[u8] = b"eggsearch-id-v1\0";

/// Build a versioned entity prefix: `eggsearch-id-v1\0{entity}\0`.
pub fn entity_prefix(entity: &str) -> Vec<u8> {
    let mut prefix = ID_VERSION_PREFIX.to_vec();
    prefix.extend_from_slice(entity.as_bytes());
    prefix.push(0); // null separator
    prefix
}

/// Write the versioned entity prefix directly to `hasher` without an
/// intermediate allocation. Equivalent to hashing `entity_prefix(entity)`.
pub fn write_entity_prefix(hasher: &mut FnvHasher, entity: &str) {
    hasher.write(ID_VERSION_PREFIX);
    hasher.write(entity.as_bytes());
    hasher.write(&[0]);
}

/// Write a length-prefixed byte slice to the hasher.
/// Prefix prevents field-boundary ambiguity (e.g. "ab"+"c" vs "a"+"bc").
/// Lengths above u32::MAX are clamped to u32::MAX to keep the prefix
/// stable; in practice inputs are bounded by upstream extract limits.
pub fn write_str(hasher: &mut FnvHasher, s: &str) {
    let len = u32::try_from(s.len()).unwrap_or(u32::MAX);
    hasher.write(&len.to_le_bytes());
    hasher.write(s.as_bytes());
}

/// Write an `Option<&str>` to the hasher (None = empty).
fn write_opt_str(hasher: &mut FnvHasher, s: Option<&str>) {
    write_str(hasher, s.unwrap_or(""));
}

/// Write an `Option<u32>` to the hasher (`None` = sentinel 0,
/// `Some(0)` = sentinel u32::MAX so the encoding stays injective).
/// `Some(u32::MAX)` appends a one-byte disambiguator so it cannot
/// collide with the `Some(0)` sentinel; all other values are written
/// verbatim, keeping existing hash streams byte-for-byte stable.
fn write_opt_u32(hasher: &mut FnvHasher, v: Option<u32>) {
    let encoded = match v {
        None => 0u32,
        Some(0) => u32::MAX,
        Some(n) => n,
    };
    hasher.write(&encoded.to_le_bytes());
    if v == Some(u32::MAX) {
        hasher.write(&[1u8]);
    }
}

/// Write a `usize` to the hasher.
fn write_usize(hasher: &mut FnvHasher, v: usize) {
    hasher.write(&(v as u64).to_le_bytes());
}

// ---------------------------------------------------------------------------
// URL Canonicalization
// ---------------------------------------------------------------------------

/// Canonicalize a URL for identity-stable hashing.
///
/// Normalization rules:
/// - Lowercase scheme and host
/// - Strip `www.` prefix (deliberate deduplication heuristic —
///   `www.example.com` and `example.com` are treated as equivalent)
/// - Remove default ports (`:80` for HTTP, `:443` for HTTPS)
/// - Strip fragments (`#...`)
/// - Normalize percent-encoding (decode unreserved chars, normalize hex
///   casing)
/// - Strip trailing slashes (except bare root `/`)
/// - Preserve query parameters (identity-significant)
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

    // Split authority from path+query+fragment
    let (authority, path_query_frag) = if let Some(slash_pos) = rest.find('/') {
        (&rest[..slash_pos], &rest[slash_pos..])
    } else {
        // No slash after authority: entire rest is authority.
        // Split fragment and query from authority.
        let authority = if let Some(hash_pos) = rest.find('#') {
            &rest[..hash_pos]
        } else if let Some(qmark_pos) = rest.find('?') {
            &rest[..qmark_pos]
        } else {
            rest
        };
        let suffix = &rest[authority.len()..];
        (authority, suffix)
    };

    // Lowercase only the host portion of the authority, preserving
    // case-sensitive userinfo (usernames and passwords).
    let authority = if let Some(at_pos) = authority.rfind('@') {
        let (userinfo, host_port) = authority.split_at(at_pos);
        let host_port = host_port[1..].to_ascii_lowercase();
        format!("{userinfo}@{host_port}")
    } else {
        authority.to_ascii_lowercase()
    };

    // Strip www. prefix
    let authority = if let Some(at_pos) = authority.rfind('@') {
        let (userinfo, host_port) = authority.split_at(at_pos);
        let host_port = host_port[1..]
            .strip_prefix("www.")
            .unwrap_or(&host_port[1..]);
        format!("{userinfo}@{host_port}")
    } else {
        authority
            .strip_prefix("www.")
            .unwrap_or(&authority)
            .to_string()
    };

    // Strip default ports
    let authority = strip_default_port(&authority, &scheme);

    // Strip fragment from path+query+fragment
    let path_query_frag = if let Some(hash_pos) = path_query_frag.find('#') {
        &path_query_frag[..hash_pos]
    } else {
        path_query_frag
    };

    // Normalize percent-encoding in path and query: decode unreserved
    // chars and re-encode with consistent casing.
    let path_query_frag = normalize_percent_encoding(path_query_frag);

    let path_query_frag = {
        let (path, query) = if let Some(qpos) = path_query_frag.find('?') {
            (&path_query_frag[..qpos], &path_query_frag[qpos..])
        } else {
            (path_query_frag.as_str(), "")
        };
        let trimmed = path.trim_end_matches('/');
        if trimmed.is_empty() {
            format!("/{query}")
        } else {
            format!("{trimmed}{query}")
        }
    };

    format!("{scheme}://{authority}{path_query_frag}")
}

/// Strip default port from a host string.
fn strip_default_port(host: &str, scheme: &str) -> String {
    let default_port = match scheme {
        "http" => "80",
        "https" => "443",
        _ => return host.to_string(),
    };
    let host_start = host.rfind('@').map_or(0, |pos| pos + 1);
    let host_port = &host[host_start..];
    let Some(port_start) = host_port.rfind(':') else {
        return host.to_string();
    };
    if &host_port[port_start + 1..] != default_port {
        return host.to_string();
    }
    format!("{}{}", &host[..host_start], &host_port[..port_start])
}

/// Normalize percent-encoding in a URL path+query string.
///
/// Decodes unreserved characters (alphanumerics, `-`, `.`, `_`, `~`)
/// and re-encodes the path portion with consistent hex casing. This
/// ensures `%41` and `A` produce the same canonical form. Reserved
/// characters and their encodings (e.g. `%2F` for `/`) are left as-is.
fn normalize_percent_encoding(path_query: &str) -> String {
    // Split path from query at the first '?'
    let (path, query) = if let Some(qpos) = path_query.find('?') {
        (&path_query[..qpos], &path_query[qpos..])
    } else {
        (path_query, "")
    };

    // Decode only unreserved characters in the path, preserving
    // reserved encodings like %2F (encoded slash).
    let decoded = decode_unreserved(path);
    let normalized = percent_encode_path(&decoded);
    let normalized_query = normalize_percent_encoding_component(query);

    format!("{normalized}{normalized_query}")
}

fn normalize_percent_encoding_component(component: &str) -> String {
    let bytes = component.as_bytes();
    let mut result = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                let byte = hi * 16 + lo;
                if is_unreserved(byte) {
                    result.push(byte as char);
                } else {
                    result.push('%');
                    result.push(hex_digit(hi));
                    result.push(hex_digit(lo));
                }
                i += 3;
                continue;
            }
        }
        let ch = component[i..].chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }
    result
}

fn hex_digit(value: u8) -> char {
    b"0123456789ABCDEF"[value as usize] as char
}

/// Decode only unreserved percent-encoded characters in a string.
///
/// Unreserved characters: `A-Z`, `a-z`, `0-9`, `-`, `.`, `_`, `~`.
/// Reserved characters and their encodings (e.g. `%2F` for `/`) are
/// left as-is to preserve resource identity. Operates on bytes so raw
/// multi-byte UTF-8 passes through unchanged.
fn decode_unreserved(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                let byte = h * 16 + l;
                if is_unreserved(byte) {
                    result.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    result
}

/// Check if a byte is an unreserved URL character.
fn is_unreserved(byte: u8) -> bool {
    matches!(byte,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'
    )
}

/// Convert a hex ASCII character to its numeric value.
fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Percent-encode a path byte string, encoding all bytes except
/// unreserved characters, `/` (path separator), and existing
/// percent-encoded sequences (`%XX`).
///
/// This produces a canonical percent-encoding form: unreserved chars
/// are literal, hex digits are lowercase, path separators are
/// preserved, and already-encoded sequences are left as-is. Raw
/// multi-byte UTF-8 is encoded once (byte-wise), matching the
/// encoding of the same text supplied pre-encoded.
fn percent_encode_path(path: &[u8]) -> String {
    let mut result = String::with_capacity(path.len());
    let mut i = 0;
    while i < path.len() {
        if path[i] == b'%'
            && i + 2 < path.len()
            && hex_val(path[i + 1]).is_some()
            && hex_val(path[i + 2]).is_some()
        {
            // Existing percent-encoded sequence: preserve with uppercase hex
            result.push('%');
            result.push(path[i + 1].to_ascii_uppercase() as char);
            result.push(path[i + 2].to_ascii_uppercase() as char);
            i += 3;
        } else if matches!(path[i],
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'.' | b'_' | b'~' | b'/'
        ) {
            result.push(path[i] as char);
            i += 1;
        } else {
            result.push('%');
            result.push_str(&format!("{:02X}", path[i]));
            i += 1;
        }
    }
    result
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
    normalize_percent_encoding(path)
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

fn source_kind_str(kind: Option<SourceKind>) -> &'static str {
    match kind {
        None => "",
        Some(SourceKind::Unknown) => "unknown",
        Some(SourceKind::OfficialDocs) => "official_docs",
        Some(SourceKind::PackageRegistry) => "package_registry",
        Some(SourceKind::SourceRepository) => "source_repository",
        Some(SourceKind::RepositoryRoot) => "repository_root",
        Some(SourceKind::SourceDirectory) => "source_directory",
        Some(SourceKind::SourceFile) => "source_file",
        Some(SourceKind::IssueThread) => "issue_thread",
        Some(SourceKind::PullRequest) => "pull_request",
        Some(SourceKind::ReleaseNotes) => "release_notes",
        Some(SourceKind::Tag) => "tag",
        Some(SourceKind::Commit) => "commit",
        Some(SourceKind::SecurityAdvisory) => "security_advisory",
        Some(SourceKind::Reference) => "reference",
        Some(SourceKind::News) => "news",
        Some(SourceKind::Tutorial) => "tutorial",
        Some(SourceKind::Forum) => "forum",
    }
}

/// Compute a deterministic source ID from a canonical key.
///
/// `stable_id = src_<16hex(source\0provider\0url\0title\0kind)>`
///
/// URLs are canonicalized before hashing to ensure that trivial
/// differences (trailing slashes, `www.` prefix, default ports,
/// fragments) do not produce spurious ID differences.
pub fn compute_source_id(key: &SourceKey<'_>) -> String {
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "source");
    write_opt_str(&mut hasher, key.provider_id);
    match key.url {
        Some(u) => write_str(&mut hasher, &canonicalize_url(u)),
        None => write_str(&mut hasher, ""),
    }
    write_opt_str(&mut hasher, key.title);
    write_str(&mut hasher, source_kind_str(key.source_kind));
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
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "fetch");
    if let Some(loc) = key.locator {
        write_str(&mut hasher, &format!("{loc:?}"));
    } else {
        match key.url {
            Some(u) => write_str(&mut hasher, &canonicalize_url(u)),
            None => write_str(&mut hasher, ""),
        }
    }
    write_opt_u32(&mut hasher, key.line_start);
    write_opt_u32(&mut hasher, key.line_end);
    let prefix = key.text_prefix.unwrap_or("");
    let prefix: String = prefix.chars().take(64).collect();
    write_str(&mut hasher, &prefix);
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
///
/// URLs are canonicalized before hashing (same rules as source and
/// fetch IDs) so cosmetic differences do not produce spurious IDs.
pub fn compute_suggested_fetch_id(key: &SuggestedFetchKey<'_>) -> String {
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "suggested");
    write_str(&mut hasher, &canonicalize_url(key.url));
    write_str(&mut hasher, key.group);
    hasher.write(&[key.priority]);
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
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "batch");
    write_str(&mut hasher, key.label);
    write_usize(&mut hasher, key.index);
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
// Code Span ID
// ---------------------------------------------------------------------------

/// Canonical key for a code span's deterministic identity.
#[derive(Clone, Debug, Default)]
pub struct CodeSpanKey<'a> {
    /// The locator string (URL or structured locator debug form).
    pub locator: &'a str,
    /// Start line of the span (1-indexed).
    pub line_start: Option<u32>,
    /// End line of the span (1-indexed).
    pub line_end: Option<u32>,
    /// Symbol name matched or enclosing the span.
    pub symbol: Option<&'a str>,
}

/// Compute a deterministic code-span ID from a canonical key.
///
/// `span_id = span_<16hex(locator + line_start + line_end + symbol)>`
pub fn compute_code_span_id(key: &CodeSpanKey<'_>) -> String {
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "code_span");
    write_str(&mut hasher, key.locator);
    write_opt_u32(&mut hasher, key.line_start);
    write_opt_u32(&mut hasher, key.line_end);
    write_opt_str(&mut hasher, key.symbol);
    format!("span_{:016x}", hasher.finish())
}

/// Convenience: compute a code-span ID from individual fields.
pub fn code_span_id(
    locator: &str,
    line_start: Option<u32>,
    line_end: Option<u32>,
    symbol: Option<&str>,
) -> String {
    compute_code_span_id(&CodeSpanKey {
        locator,
        line_start,
        line_end,
        symbol,
    })
}

// ---------------------------------------------------------------------------
// Repo Locator Key
// ---------------------------------------------------------------------------

/// Canonical key for normalizing a repo locator's identity.
///
/// Identity hashing lowercases all string fields and strips common trivial
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
    let host = key.host.map(str::to_ascii_lowercase);
    let owner = key.owner.map(str::to_ascii_lowercase);
    let repo = key
        .repo
        .map(str::to_ascii_lowercase)
        .map(|repo| strip_dot_git(&repo).to_string());
    let ref_name = key.ref_name.map(str::to_ascii_lowercase);
    let path = key.path.to_ascii_lowercase();
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "locator");
    write_opt_str(&mut hasher, host.as_deref());
    write_opt_str(&mut hasher, owner.as_deref());
    write_opt_str(&mut hasher, repo.as_deref());
    write_opt_str(&mut hasher, ref_name.as_deref());
    write_str(&mut hasher, path.trim_matches('/'));
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
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "doc");
    match key.url {
        Some(u) => write_str(&mut hasher, &canonicalize_url(u)),
        None => write_str(&mut hasher, ""),
    }
    write_opt_str(&mut hasher, key.title);
    write_opt_str(&mut hasher, key.kind);
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
    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "chunk");
    write_str(&mut hasher, key.doc_id);
    write_usize(&mut hasher, key.chunk_index);
    write_str(&mut hasher, key.heading_path);
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
        let a = source_id(
            Some("p"),
            Some("https://a.com"),
            None,
            Some(SourceKind::OfficialDocs),
        );
        let b = source_id(
            Some("p"),
            Some("https://a.com"),
            None,
            Some(SourceKind::PackageRegistry),
        );
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
    fn fetch_id_non_ascii_prefix_does_not_panic() {
        let multibyte = "日本語テキストです漢字も含むテストデータです。これが長すぎる場合にどうなりますか確認します。".repeat(4);
        let _ = fetch_id(
            Some("https://a.com"),
            None,
            Some(1),
            Some(10),
            Some(&multibyte),
        );
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
        let from_fn = fetch_id(
            Some("https://example.com"),
            None,
            Some(1),
            Some(50),
            Some("hello"),
        );
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
    fn canonicalize_url_uses_last_at_for_userinfo() {
        assert_eq!(
            canonicalize_url("HTTP://user@name@WWW.example.com:80/path"),
            "http://user@name@example.com/path"
        );
    }

    #[test]
    fn canonicalize_url_preserves_non_default_port() {
        let a = canonicalize_url("https://example.com:8443/path");
        assert!(a.contains(":8443"));
    }

    #[test]
    fn canonicalize_url_preserves_ipv6_non_default_port() {
        assert_eq!(
            canonicalize_url("https://[::1]:4430/path"),
            "https://[::1]:4430/path"
        );
    }

    #[test]
    fn canonicalize_url_strips_ipv6_default_port() {
        assert_eq!(
            canonicalize_url("http://[::1]:80/path"),
            "http://[::1]/path"
        );
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
    fn canonicalize_url_no_scheme_normalizes_percent_encoding() {
        let a = canonicalize_url("example.com/path%41");
        let b = canonicalize_url("example.com/pathA");
        assert_eq!(a, b);
    }

    #[test]
    fn source_id_canonicalizes_urls() {
        // Trailing slash, fragment, www, port should not affect ID
        let base = source_id(Some("p"), Some("https://example.com/path"), None, None);
        let variant1 = source_id(Some("p"), Some("https://example.com/path/"), None, None);
        let variant2 = source_id(
            Some("p"),
            Some("https://example.com/path#section"),
            None,
            None,
        );
        let variant3 = source_id(Some("p"), Some("https://www.example.com/path"), None, None);
        let variant4 = source_id(Some("p"), Some("https://example.com:443/path"), None, None);
        assert_eq!(base, variant1);
        assert_eq!(base, variant2);
        assert_eq!(base, variant3);
        assert_eq!(base, variant4);
    }

    #[test]
    fn canonicalize_url_normalizes_query_percent_encoding() {
        assert_eq!(
            canonicalize_url("https://example.com/path?x=a%2fb&y=%7e"),
            "https://example.com/path?x=a%2Fb&y=~"
        );
    }

    #[test]
    fn fetch_id_canonicalizes_urls() {
        let base = fetch_id(Some("https://example.com/file.rs"), None, None, None, None);
        let variant = fetch_id(Some("https://example.com/file.rs/"), None, None, None, None);
        let fragment = fetch_id(
            Some("https://example.com/file.rs#L10"),
            None,
            None,
            None,
            None,
        );
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
    fn locator_id_normalizes_case_and_path_slashes() {
        let make = |owner: &str, repo: &str, ref_name: &str, path: &str| RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(crate::core::code_metadata::CodeHost::Github),
            owner: Some(owner.to_string()),
            repo: Some(repo.to_string()),
            ref_name: Some(ref_name.to_string()),
            commit_sha: None,
            path: path.to_string(),
            workspace_root: None,
        };

        assert_eq!(
            locator_id(&make("Owner", "Repo.GIT", "MAIN", "/SRC/MAIN.RS/")),
            locator_id(&make("owner", "repo", "main", "src/main.rs"))
        );
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
        assert_ne!(
            locator_id(&make(crate::core::code_metadata::CodeHost::Github)),
            locator_id(&make(crate::core::code_metadata::CodeHost::Gitlab))
        );
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
        assert_ne!(
            locator_id(&make("src/main.rs")),
            locator_id(&make("src/lib.rs"))
        );
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

    #[test]
    fn locator_id_field_ordering_independent() {
        // Construct the same logical locator with fields in different
        // conceptual orders (Rust struct field order is fixed by the type,
        // but this tests that normalize_locator_key → compute_locator_id
        // produces the same result regardless of how the locator was built).
        let mut loc_a = RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(crate::core::code_metadata::CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("tokio".to_string()),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            workspace_root: None,
        };
        let mut loc_b = RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(crate::core::code_metadata::CodeHost::Github),
            owner: Some("tokio-rs".to_string()),
            repo: Some("tokio".to_string()),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            workspace_root: None,
        };

        // Same logical locator → same ID
        assert_eq!(locator_id(&loc_a), locator_id(&loc_b));

        // Mutate loc_a: different ref → different ID
        loc_a.ref_name = Some("develop".to_string());
        assert_ne!(locator_id(&loc_a), locator_id(&loc_b));

        // Restore loc_a, mutate loc_b: different path → different ID
        loc_a.ref_name = Some("main".to_string());
        loc_b.path = "src/runtime.rs".to_string();
        assert_ne!(locator_id(&loc_a), locator_id(&loc_b));

        // Restore loc_b: same again → same ID
        loc_b.path = "src/lib.rs".to_string();
        assert_eq!(locator_id(&loc_a), locator_id(&loc_b));
    }

    #[test]
    fn query_params_produce_different_ids() {
        let a = source_id(
            Some("p"),
            Some("https://example.com/search?q=rust"),
            None,
            None,
        );
        let b = source_id(
            Some("p"),
            Some("https://example.com/search?q=python"),
            None,
            None,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn percent_encoding_normalized() {
        // %41 (encoded 'A') should normalize to the same as literal 'A'
        let a = source_id(
            Some("p"),
            Some("https://example.com/path%41/file"),
            None,
            None,
        );
        let b = source_id(
            Some("p"),
            Some("https://example.com/pathA/file"),
            None,
            None,
        );
        assert_eq!(a, b);

        // %2f (encoded '/') should NOT normalize to '/' (different resources)
        let c = source_id(Some("p"), Some("https://example.com/a%2Fb"), None, None);
        let d = source_id(Some("p"), Some("https://example.com/a/b"), None, None);
        assert_ne!(c, d);

        // Hex casing: %2f and %2F should produce the same ID
        let e = source_id(Some("p"), Some("https://example.com/a%2fb"), None, None);
        let f = source_id(Some("p"), Some("https://example.com/a%2Fb"), None, None);
        assert_eq!(e, f);
    }

    #[test]
    fn raw_utf8_and_percent_encoded_path_converge() {
        let raw = canonicalize_url("https://example.com/docs/café");
        let encoded = canonicalize_url("https://example.com/docs/caf%C3%A9");
        assert_eq!(raw, encoded);

        let raw_id = source_id(Some("p"), Some("https://example.com/docs/café"), None, None);
        let encoded_id = source_id(
            Some("p"),
            Some("https://example.com/docs/caf%C3%A9"),
            None,
            None,
        );
        assert_eq!(raw_id, encoded_id);
    }

    #[test]
    fn opt_u32_encoding_is_injective() {
        let none = fetch_id(Some("https://a.com"), None, None, None, None);
        let some_zero = fetch_id(Some("https://a.com"), None, Some(0), None, None);
        let some_max = fetch_id(Some("https://a.com"), None, Some(u32::MAX), None, None);
        assert_ne!(none, some_zero);
        assert_ne!(none, some_max);
        assert_ne!(some_zero, some_max);
    }

    // -- Doc / Chunk ID tests --

    #[test]
    fn doc_id_deterministic() {
        let a = doc_id(
            Some("https://docs.rs/axum"),
            Some("axum docs"),
            Some("html"),
        );
        let b = doc_id(
            Some("https://docs.rs/axum"),
            Some("axum docs"),
            Some("html"),
        );
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

    #[test]
    fn source_id_golden() {
        let id = source_id(
            Some("duckduckgo"),
            Some("https://example.com/page"),
            Some("Example Page"),
            Some(SourceKind::OfficialDocs),
        );
        assert_eq!(id, "src_17b1daa97bc304f0");
    }

    #[test]
    fn fetch_id_golden() {
        let id = fetch_id(Some("https://example.com/file.rs"), None, None, None, None);
        assert_eq!(id, "fetch_351d8b4af32d6573");
    }

    #[test]
    fn suggested_fetch_id_golden() {
        let id = suggested_fetch_id("https://example.com/path", "OfficialDocs", 1);
        assert_eq!(id, "suggested_ad40b2173a8c41f6");
    }

    #[test]
    fn batch_fetch_id_golden() {
        let id = batch_fetch_id("https://example.com/path", 0);
        assert_eq!(id, "batch_d85a5a3267858a74");
    }

    #[test]
    fn locator_id_golden() {
        let id = locator_id(&RepoLocator {
            kind: crate::core::repo_fetch::RepoLocatorKind::Remote,
            host: Some(crate::core::code_metadata::CodeHost::Github),
            owner: Some("a".to_string()),
            repo: Some("r".to_string()),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            workspace_root: None,
        });
        assert_eq!(id, "loc_91cbe152399f0d98");
    }

    #[test]
    fn doc_id_golden() {
        let id = doc_id(
            Some("https://example.com/page"),
            Some("Example"),
            Some("html"),
        );
        assert_eq!(id, "doc_378ae4bb554d051c");
    }

    #[test]
    fn chunk_id_golden() {
        let id = chunk_id("doc_aabbccdd11223344", 0, "intro");
        assert_eq!(id, "chunk_c777b483a3765f9f");
    }

    // -- Code Span ID tests --

    #[test]
    fn code_span_id_deterministic() {
        let a = code_span_id(
            "https://example.com/src.rs",
            Some(10),
            Some(20),
            Some("main"),
        );
        let b = code_span_id(
            "https://example.com/src.rs",
            Some(10),
            Some(20),
            Some("main"),
        );
        assert_eq!(a, b);
        assert!(a.starts_with("span_"));
        assert_eq!(a.len(), 21); // "span_" + 16 hex
    }

    #[test]
    fn code_span_id_differs_on_locator() {
        let a = code_span_id("https://a.com/f.rs", Some(1), Some(10), None);
        let b = code_span_id("https://b.com/f.rs", Some(1), Some(10), None);
        assert_ne!(a, b);
    }

    #[test]
    fn code_span_id_differs_on_line_range() {
        let a = code_span_id("https://a.com/f.rs", Some(1), Some(10), None);
        let b = code_span_id("https://a.com/f.rs", Some(5), Some(15), None);
        assert_ne!(a, b);
    }

    #[test]
    fn code_span_id_differs_on_symbol() {
        let a = code_span_id("https://a.com/f.rs", Some(1), Some(10), Some("foo"));
        let b = code_span_id("https://a.com/f.rs", Some(1), Some(10), Some("bar"));
        assert_ne!(a, b);
    }

    #[test]
    fn code_span_id_none_symbol() {
        let id = code_span_id("https://a.com/f.rs", Some(1), Some(10), None);
        assert!(id.starts_with("span_"));
        assert_eq!(id.len(), 21);
    }

    #[test]
    fn code_span_key_struct_matches_convenience_fn() {
        let key = CodeSpanKey {
            locator: "https://example.com/src.rs",
            line_start: Some(10),
            line_end: Some(20),
            symbol: Some("main"),
        };
        let from_struct = compute_code_span_id(&key);
        let from_fn = code_span_id(
            "https://example.com/src.rs",
            Some(10),
            Some(20),
            Some("main"),
        );
        assert_eq!(from_struct, from_fn);
    }

    #[test]
    fn code_span_id_golden() {
        let id = code_span_id(
            "https://example.com/src.rs",
            Some(10),
            Some(20),
            Some("main"),
        );
        assert_eq!(id, "span_2b241f6240cde0ab");
    }

    #[test]
    fn www_stripping_is_deliberate_dedup() {
        let id1 = source_id(
            Some("test"),
            Some("https://www.example.com/page"),
            Some("Test"),
            Some(SourceKind::Reference),
        );
        let id2 = source_id(
            Some("test"),
            Some("https://example.com/page"),
            Some("Test"),
            Some(SourceKind::Reference),
        );
        assert_eq!(id1, id2);
    }

    #[test]
    fn query_params_are_identity_significant() {
        let id1 = source_id(
            Some("test"),
            Some("https://example.com/page?q=foo"),
            Some("Test"),
            Some(SourceKind::Reference),
        );
        let id2 = source_id(
            Some("test"),
            Some("https://example.com/page?q=bar"),
            Some("Test"),
            Some(SourceKind::Reference),
        );
        assert_ne!(id1, id2);
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
        let span = code_span_id("https://a.com", Some(1), Some(10), None);

        assert!(src.starts_with("src_"));
        assert!(fetch.starts_with("fetch_"));
        assert!(suggested.starts_with("suggested_"));
        assert!(batch.starts_with("batch_"));
        assert!(loc.starts_with("loc_"));
        assert!(doc.starts_with("doc_"));
        assert!(chunk.starts_with("chunk_"));
        assert!(span.starts_with("span_"));

        // No two prefix-bearing IDs should be equal
        let all = [&src, &fetch, &suggested, &batch, &loc, &doc, &chunk, &span];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(
                    all[i], all[j],
                    "IDs should differ: {} vs {}",
                    all[i], all[j]
                );
            }
        }
    }
}
