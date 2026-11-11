use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lru::LruCache;
use tokio::sync::{Mutex, RwLock};

use crate::core::fetch::ExtractMode;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RawCacheKey {
    pub url: String,
    pub scope: CacheScope,
    pub accept_policy: crate::fetch::accept::AcceptPolicy,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct DerivedCacheKey {
    pub scope: CacheScope,
    pub raw_content_hash: u64,
    pub extraction_key: ExtractionCacheKey,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ExtractionCacheKey {
    pub extract_mode: ExtractMode,
    pub max_chars_class: usize,
    pub include_links: bool,
    pub pdf_pages: Option<String>,
    pub pdf_ocr: Option<String>,
    pub include_media: bool,
    pub renderer_version: u32,
    pub sanitize_output: bool,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ProfileId(pub String);

impl ProfileId {
    pub fn opaque(id: String) -> Self {
        Self(id)
    }
}

impl AsRef<str> for ProfileId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProfileId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ProfileId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum CacheScope {
    Anonymous,
    Profile(ProfileId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawRepresentation {
    Http,
    BrowserDom,
}

#[derive(Clone, Debug)]
pub struct CacheValidators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CacheFreshness {
    pub max_age: Option<Duration>,
    pub expires: Option<SystemTime>,
    pub no_store: bool,
    pub no_cache: bool,
    pub private: bool,
    pub vary: Option<String>,
    pub fetched_at: Option<SystemTime>,
}

impl CacheFreshness {
    pub fn is_fresh(&self) -> bool {
        if self.no_store || self.no_cache {
            return false;
        }
        let now = SystemTime::now();
        if let Some(max_age) = self.max_age {
            if let Some(fetched_at) = self.fetched_at {
                if let Ok(elapsed) = now.duration_since(fetched_at) {
                    return elapsed < max_age;
                }
            }
            return false;
        }
        if let Some(expires) = self.expires {
            return now < expires;
        }
        false
    }

    pub fn from_headers(headers: &http::HeaderMap) -> (Self, CacheValidators) {
        let mut freshness = CacheFreshness {
            max_age: None,
            expires: None,
            no_store: false,
            no_cache: false,
            private: false,
            vary: None,
            fetched_at: None,
        };
        let mut validators = CacheValidators {
            etag: None,
            last_modified: None,
        };

        if let Some(cc) = headers.get("cache-control").and_then(|v| v.to_str().ok()) {
            let mut max_age = None;
            let mut s_maxage = None;
            for directive in cc.split(',') {
                let (name, value) = directive.split_once('=').unwrap_or((directive, ""));
                match name.trim().to_ascii_lowercase().as_str() {
                    "no-store" => freshness.no_store = true,
                    "no-cache" => freshness.no_cache = true,
                    "private" => freshness.private = true,
                    "max-age" | "s-maxage" => {
                        let parsed = value
                            .trim()
                            .trim_matches('"')
                            .parse::<u64>()
                            .ok()
                            .map(Duration::from_secs);
                        if name.trim().eq_ignore_ascii_case("s-maxage") {
                            s_maxage = parsed;
                        } else {
                            max_age = parsed;
                        }
                    }
                    _ => {}
                }
            }
            // RFC 7234: s-maxage takes precedence over max-age for shared caches.
            freshness.max_age = s_maxage.or(max_age);
        }

        if freshness.max_age.is_none() {
            if let Some(expires) = headers.get("expires").and_then(|v| v.to_str().ok()) {
                if let Some(exp) = parse_http_date(expires) {
                    freshness.expires = Some(exp);
                }
            }
        }

        if let Some(etag) = headers.get("etag").and_then(|v| v.to_str().ok()) {
            validators.etag = Some(etag.to_string());
        }
        if let Some(lm) = headers.get("last-modified").and_then(|v| v.to_str().ok()) {
            validators.last_modified = Some(lm.to_string());
        }

        if let Some(vary) = headers.get("vary").and_then(|v| v.to_str().ok()) {
            freshness.vary = Some(vary.to_string());
        }

        (freshness, validators)
    }
}

#[derive(Clone, Debug)]
pub struct RawFetchCacheEntry {
    pub final_url: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Arc<[u8]>,
    pub fetched_at: SystemTime,
    pub freshness: CacheFreshness,
    pub validators: CacheValidators,
    pub scope: CacheScope,
    pub content_type: Option<String>,
    pub content_length_header: Option<usize>,
    pub redirect_count: usize,
    pub representation: RawRepresentation,
    pub truncated: bool,
    pub browser_escalated: bool,
}

#[derive(Clone, Debug)]
pub struct DerivedDocumentCacheEntry {
    pub raw_content_hash: u64,
    pub extraction_key: ExtractionCacheKey,
    pub response: CachedExtractedDocument,
    pub created_at: SystemTime,
}

#[derive(Clone, Debug)]
pub struct CachedExtractedDocument {
    pub title: Option<String>,
    pub description: Option<String>,
    pub text: Option<String>,
    pub raw_text: Option<String>,
    pub raw_text_truncated: bool,
    pub raw_text_cap: Option<usize>,
    pub links: Vec<crate::core::fetch::ExtractedLink>,
    pub links_seen: Option<usize>,
    pub links_truncated: bool,
    pub truncated: bool,
    pub document: Option<crate::core::document::FetchDocument>,
    pub trust_markers: crate::core::sanitize::TrustMarkers,
    pub transport: Option<String>,
    pub browser_escalated: bool,
    pub representation: Option<crate::core::fetch::FetchRepresentation>,
}

/// Approximate in-memory size of a derived cache entry: the byte
/// length of every stored text payload plus a fixed overhead covering
/// small scalar fields (`transport`, `trust_markers`, flags).
fn derived_entry_bytes(entry: &DerivedDocumentCacheEntry) -> usize {
    const DERIVED_ENTRY_OVERHEAD: usize = 128;
    let r = &entry.response;
    let mut bytes = DERIVED_ENTRY_OVERHEAD;
    if let Some(t) = &r.title {
        bytes += t.len();
    }
    if let Some(d) = &r.description {
        bytes += d.len();
    }
    if let Some(t) = &r.text {
        bytes += t.len();
    }
    if let Some(t) = &r.raw_text {
        bytes += t.len();
    }
    for link in &r.links {
        bytes += link.url.len() + link.text.len();
    }
    bytes += r.transport.as_ref().map(|s| s.len()).unwrap_or(0);
    bytes += std::mem::size_of::<crate::core::sanitize::TrustMarkers>();
    if r.links_truncated {
        bytes += 1;
    }
    if r.truncated {
        bytes += 1;
    }
    if r.browser_escalated {
        bytes += 1;
    }
    if let Some(doc) = &r.document {
        bytes += std::mem::size_of_val(doc);
        bytes += doc.text_format.len();
        if let Some(metadata) = &doc.metadata {
            bytes += std::mem::size_of_val(metadata);
            bytes += metadata.charset.as_ref().map_or(0, String::len);
            bytes += metadata.source_extension.as_ref().map_or(0, String::len);
            bytes += metadata.detected_language.as_ref().map_or(0, String::len);
        }
        bytes +=
            doc.outline.len() * std::mem::size_of::<crate::core::document::DocumentOutlineEntry>();
        for block in &doc.blocks {
            bytes += std::mem::size_of_val(block);
            bytes += block.text.len();
        }
        for outline_entry in &doc.outline {
            bytes += outline_entry.title.len();
            bytes += outline_entry.anchor.as_ref().map_or(0, String::len);
        }
        for chunk in &doc.chunks {
            bytes += std::mem::size_of_val(chunk);
            bytes += chunk.text.len();
        }
    }
    bytes
}

fn saturating_sub_counter(counter: &AtomicUsize, n: usize) {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_sub(n);
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }
}

pub struct FetchCache {
    operation_gate: RwLock<()>,
    raw: Mutex<LruCache<RawCacheKey, RawFetchCacheEntry>>,
    derived: Mutex<LruCache<DerivedCacheKey, Arc<DerivedDocumentCacheEntry>>>,
    raw_max_bytes: usize,
    derived_max_bytes: usize,
    // Byte counters live outside the per-tier Mutex but are only mutated
    // while that Mutex is held by the writer. The Mutex provides
    // happens-before, so Relaxed atomic ordering is sufficient: any
    // reader that observes a counter update also observes the matching
    // LruCache mutation. `invalidate_scope` upgrades the operation gate
    // to a write lock, which serializes against concurrent inserts.
    current_raw_bytes: AtomicUsize,
    current_derived_bytes: AtomicUsize,
}

impl FetchCache {
    pub fn new(
        max_raw_entries: usize,
        max_derived_entries: usize,
        raw_max_bytes: usize,
        derived_max_bytes: usize,
    ) -> Self {
        let raw_cap = std::num::NonZeroUsize::new(max_raw_entries.max(1))
            .unwrap_or(std::num::NonZeroUsize::MIN);
        let derived_cap = std::num::NonZeroUsize::new(max_derived_entries.max(1))
            .unwrap_or(std::num::NonZeroUsize::MIN);
        Self {
            operation_gate: RwLock::new(()),
            raw: Mutex::new(LruCache::new(raw_cap)),
            derived: Mutex::new(LruCache::new(derived_cap)),
            raw_max_bytes,
            derived_max_bytes,
            current_raw_bytes: AtomicUsize::new(0),
            current_derived_bytes: AtomicUsize::new(0),
        }
    }

    pub async fn get_raw(&self, key: &RawCacheKey) -> Option<RawFetchCacheEntry> {
        let _operation = self.operation_gate.read().await;
        let mut raw = self.raw.lock().await;
        let entry = raw.get(key).cloned()?;
        if crate::core::identity::canonicalize_url(&entry.final_url)
            != crate::core::identity::canonicalize_url(&key.url)
        {
            return None;
        }
        Some(entry)
    }

    pub async fn insert_raw(&self, key: RawCacheKey, entry: RawFetchCacheEntry) -> bool {
        let _operation = self.operation_gate.read().await;
        let body_len = entry.body.len();
        if body_len > self.raw_max_bytes {
            return false;
        }
        let mut raw = self.raw.lock().await;

        if let Some(evicted) = raw.pop(&key) {
            saturating_sub_counter(&self.current_raw_bytes, evicted.body.len());
        }

        while self
            .current_raw_bytes
            .load(Ordering::Relaxed)
            .saturating_add(body_len)
            > self.raw_max_bytes
            && !raw.is_empty()
        {
            if let Some((_, evicted)) = raw.pop_lru() {
                saturating_sub_counter(&self.current_raw_bytes, evicted.body.len());
            } else {
                break;
            }
        }

        if let Some((_, evicted)) = raw.push(key, entry) {
            saturating_sub_counter(&self.current_raw_bytes, evicted.body.len());
        }
        self.current_raw_bytes
            .fetch_add(body_len, Ordering::Relaxed);
        true
    }

    pub async fn get_derived(&self, key: &DerivedCacheKey) -> Option<DerivedDocumentCacheEntry> {
        self.get_derived_shared(key)
            .await
            .map(|entry| (*entry).clone())
    }

    /// Return a shared immutable derived entry for internal hot paths.
    #[doc(hidden)]
    pub async fn get_derived_shared(
        &self,
        key: &DerivedCacheKey,
    ) -> Option<Arc<DerivedDocumentCacheEntry>> {
        let _operation = self.operation_gate.read().await;
        let mut derived = self.derived.lock().await;
        derived.get(key).cloned()
    }

    pub async fn insert_derived(&self, key: DerivedCacheKey, entry: DerivedDocumentCacheEntry) {
        let _operation = self.operation_gate.read().await;
        let entry_len = derived_entry_bytes(&entry);
        if entry_len > self.derived_max_bytes {
            return;
        }
        let mut derived = self.derived.lock().await;

        if let Some(evicted) = derived.pop(&key) {
            saturating_sub_counter(&self.current_derived_bytes, derived_entry_bytes(&evicted));
        }

        while self
            .current_derived_bytes
            .load(Ordering::Relaxed)
            .saturating_add(entry_len)
            > self.derived_max_bytes
            && !derived.is_empty()
        {
            if let Some((_, evicted)) = derived.pop_lru() {
                saturating_sub_counter(&self.current_derived_bytes, derived_entry_bytes(&evicted));
            } else {
                break;
            }
        }

        if let Some((_, evicted)) = derived.push(key, Arc::new(entry)) {
            saturating_sub_counter(&self.current_derived_bytes, derived_entry_bytes(&evicted));
        }
        self.current_derived_bytes
            .fetch_add(entry_len, Ordering::Relaxed);
    }

    /// Remove all raw and derived entries for a scope. Derived entries are
    /// content-addressed and can otherwise outlive their raw cache entry
    /// after ordinary LRU eviction.
    pub async fn invalidate_scope(&self, scope: &CacheScope) {
        // Intentionally holds a single write gate across both raw and
        // derived scans to prevent concurrent gets/inserts from
        // observing a half-invalidated state. Profile deletion is
        // infrequent; the latency spike under load is acceptable.
        let _operation = self.operation_gate.write().await;
        let mut raw = self.raw.lock().await;

        let keys_to_remove: Vec<RawCacheKey> = raw
            .iter()
            .filter(|(k, _)| k.scope == *scope)
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            if let Some(evicted) = raw.pop(&key) {
                saturating_sub_counter(&self.current_raw_bytes, evicted.body.len());
            }
        }

        drop(raw);

        let mut derived = self.derived.lock().await;
        let derived_keys_to_remove: Vec<DerivedCacheKey> = derived
            .iter()
            .filter(|(k, _)| k.scope == *scope)
            .map(|(k, _)| k.clone())
            .collect();

        for key in derived_keys_to_remove {
            if let Some(evicted) = derived.pop(&key) {
                saturating_sub_counter(&self.current_derived_bytes, derived_entry_bytes(&evicted));
            }
        }
    }

    pub async fn stats(&self) -> CacheStats {
        let _operation = self.operation_gate.read().await;
        let raw = self.raw.lock().await;
        let derived = self.derived.lock().await;
        let mut current_raw = self.current_raw_bytes.load(Ordering::Relaxed);
        let summed_raw: usize = raw.iter().map(|(_, e)| e.body.len()).sum();
        if current_raw != summed_raw {
            tracing::error!(
                current_raw,
                summed_raw,
                "raw byte counter drifted from LruCache contents"
            );
            self.current_raw_bytes.store(summed_raw, Ordering::Relaxed);
            current_raw = summed_raw;
        }
        let mut current_derived = self.current_derived_bytes.load(Ordering::Relaxed);
        let summed_derived: usize = derived.iter().map(|(_, e)| derived_entry_bytes(e)).sum();
        if current_derived != summed_derived {
            tracing::error!(
                current_derived,
                summed_derived,
                "derived byte counter drifted from LruCache contents"
            );
            self.current_derived_bytes
                .store(summed_derived, Ordering::Relaxed);
            current_derived = summed_derived;
        }
        debug_assert_eq!(current_raw, summed_raw, "raw byte counter drifted");
        debug_assert_eq!(
            current_derived, summed_derived,
            "derived byte counter drifted"
        );
        CacheStats {
            raw_entries: raw.len(),
            derived_entries: derived.len(),
            raw_bytes: current_raw,
            derived_bytes: current_derived,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CacheStats {
    pub raw_entries: usize,
    pub derived_entries: usize,
    pub raw_bytes: usize,
    pub derived_bytes: usize,
}

#[derive(Clone, Debug, Default)]
pub struct FetchCacheMetadata {
    pub cache_status: CacheStatus,
    pub attempt_count: usize,
    pub retry_after_ms: Option<u64>,
    pub origin_backoff_ms: Option<u64>,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    #[default]
    Miss,
    Hit,
    Revalidated,
    Bypassed,
    NotCacheable,
}

pub fn build_raw_cache_key(
    url: &str,
    scope: &CacheScope,
    accept_policy: crate::fetch::accept::AcceptPolicy,
) -> RawCacheKey {
    let normalized = crate::core::identity::canonicalize_url(url);
    RawCacheKey {
        url: normalized,
        scope: scope.clone(),
        accept_policy,
    }
}

pub fn build_raw_response_hash(body: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(body)
}

#[allow(clippy::too_many_arguments)]
pub fn build_derived_key(
    scope: &CacheScope,
    raw_hash: u64,
    extract_mode: ExtractMode,
    max_chars: usize,
    include_links: bool,
    pdf_pages: Option<&str>,
    pdf_ocr: Option<&str>,
    include_media: bool,
    sanitize_output: bool,
) -> DerivedCacheKey {
    let max_chars_class = max_chars;
    DerivedCacheKey {
        scope: scope.clone(),
        raw_content_hash: raw_hash,
        extraction_key: ExtractionCacheKey {
            extract_mode,
            max_chars_class,
            include_links,
            pdf_pages: pdf_pages.map(|s| s.to_string()),
            pdf_ocr: pdf_ocr.map(|s| s.to_string()),
            include_media,
            renderer_version: 2,
            sanitize_output,
        },
    }
}

fn parse_http_date(s: &str) -> Option<SystemTime> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(s) {
        return Some(dt.into());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.into());
    }
    let normalized_input = crate::core::sanitize::normalize_whitespace(s);
    let mut parts = normalized_input.split_whitespace().collect::<Vec<_>>();
    let zone = parts.last().copied().unwrap_or_default();
    let normalized_zone = match zone {
        "UT" | "GMT" | "Z" => Some("+0000"),
        "EST" => Some("-0500"),
        "EDT" => Some("-0400"),
        "CST" => Some("-0600"),
        "CDT" => Some("-0500"),
        "MST" => Some("-0700"),
        "MDT" => Some("-0600"),
        "PST" => Some("-0800"),
        "PDT" => Some("-0700"),
        "A" => Some("+0100"),
        "B" => Some("+0200"),
        "C" => Some("+0300"),
        "D" => Some("+0400"),
        "E" => Some("+0500"),
        "F" => Some("+0600"),
        "G" => Some("+0700"),
        "H" => Some("+0800"),
        "I" => Some("+0900"),
        "K" => Some("+1000"),
        "L" => Some("+1100"),
        "M" => Some("+1200"),
        "N" => Some("-0100"),
        "O" => Some("-0200"),
        "P" => Some("-0300"),
        "Q" => Some("-0400"),
        "R" => Some("-0500"),
        "S" => Some("-0600"),
        "T" => Some("-0700"),
        "U" => Some("-0800"),
        "V" => Some("-0900"),
        "W" => Some("-1000"),
        "X" => Some("-1100"),
        "Y" => Some("-1200"),
        _ => None,
    };
    let normalized = if let Some(normalized_zone) = normalized_zone {
        parts.pop();
        parts.push(normalized_zone);
        parts.join(" ")
    } else {
        parts.join(" ")
    };
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(&normalized) {
        return Some(dt.into());
    }
    None
}

pub fn should_cache_response(
    status: u16,
    content_type: Option<&str>,
    freshness: &CacheFreshness,
    scope: &CacheScope,
) -> bool {
    if freshness.no_store {
        return false;
    }
    if !(200..300).contains(&status) {
        return false;
    }
    if freshness.private && matches!(scope, CacheScope::Anonymous) {
        return false;
    }
    if let Some(ref vary) = freshness.vary {
        let unsupported = vary
            .split(',')
            .map(|h| h.trim().to_lowercase())
            .filter(|h| !h.is_empty())
            .any(|h| h != "accept-encoding" && h != "accept");
        if unsupported {
            return false;
        }
    }
    if let Some(ct) = content_type {
        let ct_lower = ct.to_lowercase();
        let ct_base = ct_lower.split(';').next().unwrap_or("").trim();
        if ct_base.starts_with("image/")
            || ct_base.starts_with("audio/")
            || ct_base.starts_with("video/")
            || ct_base == "application/octet-stream"
        {
            return false;
        }
    }
    true
}

pub fn build_request_conditional_headers(validators: &CacheValidators) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if let Some(ref etag) = validators.etag {
        headers.push(("If-None-Match".to_string(), etag.clone()));
    } else if let Some(ref lm) = validators.last_modified {
        headers.push(("If-Modified-Since".to_string(), lm.clone()));
    }
    headers
}

/// Merge header fields supplied by a 304 response into the stored
/// entry's freshness and validators (RFC 9111 §4.3.4): only fields
/// present in the 304 replace the stored values; absent fields keep
/// their cached values.
pub fn apply_304_headers(
    freshness: &mut CacheFreshness,
    validators: &mut CacheValidators,
    headers_304: &HashMap<String, String>,
) {
    if headers_304.is_empty() {
        return;
    }
    let mut map = http::HeaderMap::new();
    for (name, value) in headers_304 {
        if let (Ok(name), Ok(value)) = (
            http::header::HeaderName::from_bytes(name.as_bytes()),
            http::header::HeaderValue::from_str(value),
        ) {
            map.insert(name, value);
        }
    }
    if map.is_empty() {
        return;
    }
    let has_cache_control = map.contains_key("cache-control");
    let has_expires = map.contains_key("expires");
    let has_vary = map.contains_key("vary");
    // Note: from_headers() does not set fetched_at; the caller
    // preserves the original fetched_at and refreshes it explicitly.
    let (updated, updated_validators) = CacheFreshness::from_headers(&map);
    debug_assert!(
        updated.fetched_at.is_none(),
        "from_headers must never set fetched_at"
    );

    if has_cache_control {
        freshness.max_age = updated.max_age;
        freshness.no_store = updated.no_store;
        freshness.no_cache = updated.no_cache;
        freshness.private = updated.private;
    }
    if has_expires && updated.expires.is_some() {
        freshness.expires = updated.expires;
    }
    if has_vary {
        freshness.vary = updated.vary;
    }
    if let Some(etag) = updated_validators.etag {
        validators.etag = Some(etag);
    }
    if let Some(lm) = updated_validators.last_modified {
        validators.last_modified = Some(lm);
    }
}

#[cfg(test)]
mod tests;
