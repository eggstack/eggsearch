use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lru::LruCache;
use tokio::sync::Mutex;

use crate::core::fetch::ExtractMode;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RawCacheKey {
    pub url: String,
    pub scope: CacheScope,
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
pub enum CacheScope {
    Anonymous,
    Profile(String),
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

    pub fn from_headers(headers: &reqwest::header::HeaderMap) -> (Self, CacheValidators) {
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

        freshness.fetched_at = Some(SystemTime::now());

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
    pub links: Vec<crate::core::fetch::ExtractedLink>,
    pub links_seen: Option<usize>,
    pub links_truncated: bool,
    pub truncated: bool,
    pub document: Option<crate::core::document::FetchDocument>,
    pub trust_markers: crate::core::sanitize::TrustMarkers,
    pub transport: Option<String>,
    pub browser_escalated: bool,
}

/// Approximate in-memory size of a derived cache entry: the byte
/// length of every stored text payload.
fn derived_entry_bytes(entry: &DerivedDocumentCacheEntry) -> usize {
    let r = &entry.response;
    let mut bytes = 0usize;
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
    if let Some(doc) = &r.document {
        for block in &doc.blocks {
            bytes += block.text.len();
        }
        for outline_entry in &doc.outline {
            bytes += outline_entry.title.len();
        }
        for chunk in &doc.chunks {
            bytes += chunk.text.len();
        }
    }
    bytes
}

pub struct FetchCache {
    raw: Mutex<LruCache<RawCacheKey, RawFetchCacheEntry>>,
    derived: Mutex<LruCache<DerivedCacheKey, DerivedDocumentCacheEntry>>,
    raw_max_bytes: usize,
    derived_max_bytes: usize,
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
        let raw_cap = std::num::NonZeroUsize::new(max_raw_entries.max(1)).unwrap();
        let derived_cap = std::num::NonZeroUsize::new(max_derived_entries.max(1)).unwrap();
        Self {
            raw: Mutex::new(LruCache::new(raw_cap)),
            derived: Mutex::new(LruCache::new(derived_cap)),
            raw_max_bytes,
            derived_max_bytes,
            current_raw_bytes: AtomicUsize::new(0),
            current_derived_bytes: AtomicUsize::new(0),
        }
    }

    pub async fn get_raw(&self, key: &RawCacheKey) -> Option<RawFetchCacheEntry> {
        let mut raw = self.raw.lock().await;
        raw.get(key).cloned()
    }

    pub async fn insert_raw(&self, key: RawCacheKey, entry: RawFetchCacheEntry) -> bool {
        let body_len = entry.body.len();
        if body_len > self.raw_max_bytes {
            return false;
        }
        let mut raw = self.raw.lock().await;

        if let Some(evicted) = raw.pop(&key) {
            self.current_raw_bytes
                .fetch_sub(evicted.body.len(), Ordering::Relaxed);
        }

        while self.current_raw_bytes.load(Ordering::Relaxed) + body_len > self.raw_max_bytes
            && !raw.is_empty()
        {
            if let Some((_, evicted)) = raw.pop_lru() {
                self.current_raw_bytes
                    .fetch_sub(evicted.body.len(), Ordering::Relaxed);
            } else {
                break;
            }
        }

        raw.put(key, entry);
        self.current_raw_bytes
            .fetch_add(body_len, Ordering::Relaxed);
        true
    }

    pub async fn get_derived(&self, key: &DerivedCacheKey) -> Option<DerivedDocumentCacheEntry> {
        let mut derived = self.derived.lock().await;
        derived.get(key).cloned()
    }

    pub async fn insert_derived(&self, key: DerivedCacheKey, entry: DerivedDocumentCacheEntry) {
        let entry_len = derived_entry_bytes(&entry);
        if entry_len > self.derived_max_bytes {
            return;
        }
        let mut derived = self.derived.lock().await;

        if let Some(evicted) = derived.pop(&key) {
            self.current_derived_bytes
                .fetch_sub(derived_entry_bytes(&evicted), Ordering::Relaxed);
        }

        while self.current_derived_bytes.load(Ordering::Relaxed) + entry_len
            > self.derived_max_bytes
            && !derived.is_empty()
        {
            if let Some((_, evicted)) = derived.pop_lru() {
                self.current_derived_bytes
                    .fetch_sub(derived_entry_bytes(&evicted), Ordering::Relaxed);
            } else {
                break;
            }
        }

        derived.put(key, entry);
        self.current_derived_bytes
            .fetch_add(entry_len, Ordering::Relaxed);
    }

    pub async fn invalidate_scope(&self, scope: &CacheScope) {
        let mut raw = self.raw.lock().await;

        let keys_to_remove: Vec<RawCacheKey> = raw
            .iter()
            .filter(|(k, _)| k.scope == *scope)
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            if let Some(evicted) = raw.pop(&key) {
                self.current_raw_bytes
                    .fetch_sub(evicted.body.len(), Ordering::Relaxed);
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
                self.current_derived_bytes
                    .fetch_sub(derived_entry_bytes(&evicted), Ordering::Relaxed);
            }
        }
    }

    pub async fn stats(&self) -> CacheStats {
        let raw = self.raw.lock().await;
        let derived = self.derived.lock().await;
        let current_raw = self.current_raw_bytes.load(Ordering::Relaxed);
        CacheStats {
            raw_entries: raw.len(),
            derived_entries: derived.len(),
            raw_bytes: current_raw,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CacheStats {
    pub raw_entries: usize,
    pub derived_entries: usize,
    pub raw_bytes: usize,
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

pub fn build_raw_cache_key(url: &str, scope: &CacheScope) -> RawCacheKey {
    let normalized = crate::core::identity::canonicalize_url(url);
    RawCacheKey {
        url: normalized,
        scope: scope.clone(),
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
    let max_chars_class = classify_max_chars(max_chars);
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
            renderer_version: 1,
            sanitize_output,
        },
    }
}

fn classify_max_chars(max_chars: usize) -> usize {
    max_chars
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
        s.trim().to_string()
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
            .any(|h| h != "accept-encoding");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fetch::ExtractMode;
    use reqwest::header::HeaderMap;

    #[test]
    fn cache_scope_anonymous() {
        let scope = CacheScope::Anonymous;
        assert_ne!(scope, CacheScope::Profile("test".into()));
    }

    #[test]
    fn cache_scope_profile_partitioning() {
        let s1 = CacheScope::Profile("alice".into());
        let s2 = CacheScope::Profile("bob".into());
        assert_ne!(s1, s2);
    }

    #[test]
    fn raw_key_normalizes_url() {
        let k1 = build_raw_cache_key("https://Example.com:443/path", &CacheScope::Anonymous);
        let k2 = build_raw_cache_key("https://example.com/path", &CacheScope::Anonymous);
        assert_eq!(k1.url, k2.url);
    }

    #[test]
    fn raw_key_scope_partitioning() {
        let k1 = build_raw_cache_key("https://x.com", &CacheScope::Anonymous);
        let k2 = build_raw_cache_key("https://x.com", &CacheScope::Profile("test".into()));
        assert_ne!(k1, k2);
    }

    #[test]
    fn freshness_from_headers_max_age() {
        let mut headers = HeaderMap::new();
        headers.insert("cache-control", "max-age=300".parse().unwrap());
        let (freshness, _) = CacheFreshness::from_headers(&headers);
        assert_eq!(freshness.max_age, Some(Duration::from_secs(300)));
        assert!(!freshness.no_store);
    }

    #[test]
    fn freshness_from_headers_matches_cache_control_tokens() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "cache-control",
            "no-cacheable, custom-no-store-extension, not-private"
                .parse()
                .unwrap(),
        );
        let (freshness, _) = CacheFreshness::from_headers(&headers);
        assert!(!freshness.no_store);
        assert!(!freshness.no_cache);
        assert!(!freshness.private);
    }

    #[test]
    fn freshness_from_headers_prefers_s_maxage() {
        let mut headers = HeaderMap::new();
        headers.insert("cache-control", "max-age=300, s-maxage=60".parse().unwrap());
        let (freshness, _) = CacheFreshness::from_headers(&headers);
        assert_eq!(freshness.max_age, Some(Duration::from_secs(60)));
    }

    #[test]
    fn freshness_from_headers_no_store() {
        let mut headers = HeaderMap::new();
        headers.insert("cache-control", "no-store".parse().unwrap());
        let (freshness, _) = CacheFreshness::from_headers(&headers);
        assert!(freshness.no_store);
    }

    #[test]
    fn freshness_from_headers_etag() {
        let mut headers = HeaderMap::new();
        headers.insert("etag", "\"abc123\"".parse().unwrap());
        let (_, validators) = CacheFreshness::from_headers(&headers);
        assert_eq!(validators.etag.as_deref(), Some("\"abc123\""));
    }

    #[test]
    fn freshness_from_headers_last_modified() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "last-modified",
            "Wed, 01 Jan 2025 00:00:00 GMT".parse().unwrap(),
        );
        let (_, validators) = CacheFreshness::from_headers(&headers);
        assert!(validators.last_modified.is_some());
    }

    #[test]
    fn freshness_from_headers_expires() {
        let mut headers = HeaderMap::new();
        headers.insert("expires", "Wed, 01 Jan 2025 00:00:00 GMT".parse().unwrap());
        let (freshness, _) = CacheFreshness::from_headers(&headers);
        assert!(freshness.expires.is_some());
    }

    #[test]
    fn freshness_from_headers_parses_legacy_http_date_zones() {
        for zone in ["UT", "Z", "PST"] {
            let mut headers = HeaderMap::new();
            headers.insert(
                "expires",
                format!("Wed, 01 Jan 2025 00:00:00 {zone}").parse().unwrap(),
            );
            let (freshness, _) = CacheFreshness::from_headers(&headers);
            assert!(freshness.expires.is_some(), "failed to parse {zone}");
        }
    }

    #[test]
    fn freshness_no_cache_not_fresh() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(3600)),
            no_cache: true,
            ..CacheFreshness::default()
        };
        assert!(!freshness.is_fresh());
    }

    #[test]
    fn should_cache_rejects_no_store() {
        let freshness = CacheFreshness {
            no_store: true,
            ..CacheFreshness::default()
        };
        assert!(!should_cache_response(
            200,
            Some("text/html"),
            &freshness,
            &CacheScope::Anonymous
        ));
    }

    #[test]
    fn should_cache_rejects_error_status() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            ..CacheFreshness::default()
        };
        assert!(!should_cache_response(
            404,
            Some("text/html"),
            &freshness,
            &CacheScope::Anonymous
        ));
        assert!(!should_cache_response(
            500,
            Some("text/html"),
            &freshness,
            &CacheScope::Anonymous
        ));
    }

    #[test]
    fn should_cache_rejects_images() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            ..CacheFreshness::default()
        };
        assert!(!should_cache_response(
            200,
            Some("image/png"),
            &freshness,
            &CacheScope::Anonymous
        ));
    }

    #[test]
    fn should_cache_allows_text_html() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            ..CacheFreshness::default()
        };
        assert!(should_cache_response(
            200,
            Some("text/html"),
            &freshness,
            &CacheScope::Anonymous
        ));
    }

    #[test]
    fn should_cache_rejects_private_in_anonymous_scope() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            private: true,
            ..CacheFreshness::default()
        };
        assert!(!should_cache_response(
            200,
            Some("text/html"),
            &freshness,
            &CacheScope::Anonymous
        ));
    }

    #[test]
    fn should_cache_allows_private_in_profile_scope() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            private: true,
            ..CacheFreshness::default()
        };
        assert!(should_cache_response(
            200,
            Some("text/html"),
            &freshness,
            &CacheScope::Profile("alice".into())
        ));
    }

    #[test]
    fn should_cache_rejects_unsupported_vary() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            vary: Some("Authorization".into()),
            ..CacheFreshness::default()
        };
        assert!(!should_cache_response(
            200,
            Some("text/html"),
            &freshness,
            &CacheScope::Anonymous
        ));
    }

    #[test]
    fn should_cache_allows_vary_accept_encoding_only() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            vary: Some("Accept-Encoding".into()),
            ..CacheFreshness::default()
        };
        assert!(should_cache_response(
            200,
            Some("text/html"),
            &freshness,
            &CacheScope::Anonymous
        ));
    }

    #[test]
    fn should_cache_rejects_mixed_vary_with_unsupported() {
        let freshness = CacheFreshness {
            max_age: Some(Duration::from_secs(300)),
            vary: Some("Accept-Encoding, Cookie".into()),
            ..CacheFreshness::default()
        };
        assert!(!should_cache_response(
            200,
            Some("text/html"),
            &freshness,
            &CacheScope::Anonymous
        ));
    }

    #[test]
    fn build_conditional_headers_with_etag() {
        let validators = CacheValidators {
            etag: Some("\"abc\"".into()),
            last_modified: None,
        };
        let headers = build_request_conditional_headers(&validators);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "If-None-Match");
        assert_eq!(headers[0].1, "\"abc\"");
    }

    #[test]
    fn build_conditional_headers_with_last_modified() {
        let validators = CacheValidators {
            etag: None,
            last_modified: Some("Wed, 01 Jan 2025 00:00:00 GMT".into()),
        };
        let headers = build_request_conditional_headers(&validators);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "If-Modified-Since");
    }

    #[test]
    fn build_conditional_headers_etag_takes_precedence() {
        let validators = CacheValidators {
            etag: Some("\"abc\"".into()),
            last_modified: Some("Wed, 01 Jan 2025 00:00:00 GMT".into()),
        };
        let headers = build_request_conditional_headers(&validators);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "If-None-Match");
    }

    #[test]
    fn classify_max_chars_works() {
        assert_eq!(classify_max_chars(1000), 1000);
        assert_eq!(classify_max_chars(8000), 8000);
        assert_eq!(classify_max_chars(30000), 30000);
        assert_eq!(classify_max_chars(60000), 60000);
    }

    #[test]
    fn derived_keys_do_not_share_max_chars_buckets() {
        let low = build_derived_key(
            &CacheScope::Anonymous,
            1,
            ExtractMode::Text,
            1000,
            false,
            None,
            None,
            false,
            false,
        );
        let high = build_derived_key(
            &CacheScope::Anonymous,
            1,
            ExtractMode::Text,
            4096,
            false,
            None,
            None,
            false,
            false,
        );
        assert_ne!(low, high);
    }

    #[tokio::test]
    async fn cache_insert_and_get_raw() {
        let cache = FetchCache::new(10, 10, 1024 * 1024, 1024 * 1024);
        let key = RawCacheKey {
            url: "https://x.com".into(),
            scope: CacheScope::Anonymous,
        };
        let entry = RawFetchCacheEntry {
            final_url: "https://x.com".into(),
            status: 200,
            headers: HashMap::new(),
            body: Arc::from(b"hello" as &[u8]),
            fetched_at: SystemTime::now(),
            freshness: CacheFreshness {
                max_age: Some(Duration::from_secs(300)),
                ..CacheFreshness::default()
            },
            validators: CacheValidators {
                etag: None,
                last_modified: None,
            },
            scope: CacheScope::Anonymous,
            content_type: Some("text/html".into()),
            content_length_header: Some(5),
            redirect_count: 0,
            representation: RawRepresentation::Http,
            truncated: false,
            browser_escalated: false,
        };

        assert!(cache.get_raw(&key).await.is_none());
        cache.insert_raw(key.clone(), entry).await;
        assert!(cache.get_raw(&key).await.is_some());
    }

    #[tokio::test]
    async fn cache_evicts_oldest_on_byte_pressure() {
        let cache = FetchCache::new(10, 10, 20, 20);
        for i in 0..5 {
            let key = RawCacheKey {
                url: format!("https://x.com/{i}"),
                scope: CacheScope::Anonymous,
            };
            let entry = RawFetchCacheEntry {
                final_url: format!("https://x.com/{i}"),
                status: 200,
                headers: HashMap::new(),
                body: Arc::from(vec![0u8; 10]),
                fetched_at: SystemTime::now(),
                freshness: CacheFreshness {
                    max_age: Some(Duration::from_secs(300)),
                    ..CacheFreshness::default()
                },
                validators: CacheValidators {
                    etag: None,
                    last_modified: None,
                },
                scope: CacheScope::Anonymous,
                content_type: Some("text/html".into()),
                content_length_header: Some(10),
                redirect_count: 0,
                representation: RawRepresentation::Http,
                truncated: false,
                browser_escalated: false,
            };
            cache.insert_raw(key, entry).await;
        }
        let stats = cache.stats().await;
        assert!(stats.raw_bytes <= 20);
    }

    #[tokio::test]
    async fn cache_derived_insert_and_get() {
        let cache = FetchCache::new(10, 10, 1024 * 1024, 1024 * 1024);
        let key = DerivedCacheKey {
            scope: CacheScope::Anonymous,
            raw_content_hash: 12345,
            extraction_key: ExtractionCacheKey {
                extract_mode: ExtractMode::Text,
                max_chars_class: 12000,
                include_links: false,
                pdf_pages: None,
                pdf_ocr: None,
                include_media: false,
                renderer_version: 1,
                sanitize_output: true,
            },
        };
        let entry = DerivedDocumentCacheEntry {
            raw_content_hash: 12345,
            extraction_key: key.extraction_key.clone(),
            response: CachedExtractedDocument {
                title: Some("test".into()),
                description: None,
                text: Some("hello".into()),
                raw_text: Some("hello".into()),
                links: Vec::new(),
                links_seen: None,
                links_truncated: false,
                truncated: false,
                document: None,
                trust_markers: crate::core::sanitize::TrustMarkers::default(),
                transport: Some("http".into()),
                browser_escalated: false,
            },
            created_at: SystemTime::now(),
        };

        assert!(cache.get_derived(&key).await.is_none());
        cache.insert_derived(key.clone(), entry).await;
        assert!(cache.get_derived(&key).await.is_some());
    }

    fn make_derived_entry(raw_content_hash: u64, text_len: usize) -> DerivedDocumentCacheEntry {
        DerivedDocumentCacheEntry {
            raw_content_hash,
            extraction_key: ExtractionCacheKey {
                extract_mode: ExtractMode::Text,
                max_chars_class: 12000,
                include_links: false,
                pdf_pages: None,
                pdf_ocr: None,
                include_media: false,
                renderer_version: 1,
                sanitize_output: true,
            },
            response: CachedExtractedDocument {
                title: None,
                description: None,
                text: Some("x".repeat(text_len)),
                raw_text: Some("y".repeat(text_len)),
                links: Vec::new(),
                links_seen: None,
                links_truncated: false,
                truncated: false,
                document: None,
                trust_markers: crate::core::sanitize::TrustMarkers::default(),
                transport: Some("http".into()),
                browser_escalated: false,
            },
            created_at: SystemTime::now(),
        }
    }

    fn make_derived_key(raw_content_hash: u64) -> DerivedCacheKey {
        DerivedCacheKey {
            scope: CacheScope::Anonymous,
            raw_content_hash,
            extraction_key: ExtractionCacheKey {
                extract_mode: ExtractMode::Text,
                max_chars_class: 12000,
                include_links: false,
                pdf_pages: None,
                pdf_ocr: None,
                include_media: false,
                renderer_version: 1,
                sanitize_output: true,
            },
        }
    }

    #[tokio::test]
    async fn cache_derived_evicts_oldest_on_byte_pressure() {
        let cache = FetchCache::new(10, 10, 500, 500);
        for i in 0..5u64 {
            cache
                .insert_derived(make_derived_key(i), make_derived_entry(i, 100))
                .await;
        }

        let oldest = make_derived_key(0);
        let kept_old = make_derived_key(3);
        let newest = make_derived_key(4);
        assert!(
            cache.get_derived(&oldest).await.is_none(),
            "oldest derived entry should be evicted under byte pressure"
        );
        assert!(cache.get_derived(&kept_old).await.is_some());
        assert!(cache.get_derived(&newest).await.is_some());

        let stats = cache.stats().await;
        assert_eq!(stats.derived_entries, 2);
    }

    #[tokio::test]
    async fn cache_derived_oversized_entry_not_stored() {
        let cache = FetchCache::new(10, 10, 64, 64);
        let key = make_derived_key(1);
        cache
            .insert_derived(key.clone(), make_derived_entry(1, 4096))
            .await;
        assert!(cache.get_derived(&key).await.is_none());
        let stats = cache.stats().await;
        assert_eq!(stats.derived_entries, 0);
    }

    #[tokio::test]
    async fn cache_derived_budget_is_independent_of_raw_budget() {
        let cache = FetchCache::new(10, 10, 8, 4096);
        for i in 0..4u64 {
            cache
                .insert_derived(make_derived_key(i), make_derived_entry(i, 100))
                .await;
        }
        for i in 0..4u64 {
            assert!(
                cache.get_derived(&make_derived_key(i)).await.is_some(),
                "derived entry {i} should survive: derived budget is independent of raw budget"
            );
        }
        let stats = cache.stats().await;
        assert_eq!(stats.derived_entries, 4);
    }

    #[tokio::test]
    async fn cache_raw_oversized_entry_not_stored_with_separate_derived_budget() {
        let cache = FetchCache::new(10, 10, 16, 4096);
        let key = RawCacheKey {
            url: "https://x.com".into(),
            scope: CacheScope::Anonymous,
        };
        let entry = RawFetchCacheEntry {
            final_url: "https://x.com".into(),
            status: 200,
            headers: HashMap::new(),
            body: Arc::from(vec![0u8; 128]),
            fetched_at: SystemTime::now(),
            freshness: CacheFreshness::default(),
            validators: CacheValidators {
                etag: None,
                last_modified: None,
            },
            scope: CacheScope::Anonymous,
            content_type: Some("text/html".into()),
            content_length_header: Some(128),
            redirect_count: 0,
            representation: RawRepresentation::Http,
            truncated: false,
            browser_escalated: false,
        };
        assert!(!cache.insert_raw(key.clone(), entry).await);
        assert!(cache.get_raw(&key).await.is_none());

        let dkey = make_derived_key(1);
        cache
            .insert_derived(dkey.clone(), make_derived_entry(1, 100))
            .await;
        assert!(cache.get_derived(&dkey).await.is_some());
    }

    #[test]
    fn build_derived_key_distinguishes_modes() {
        let scope = CacheScope::Anonymous;
        let k1 = build_derived_key(
            &scope,
            100,
            ExtractMode::Text,
            12000,
            false,
            None,
            None,
            false,
            true,
        );
        let k2 = build_derived_key(
            &scope,
            100,
            ExtractMode::Markdown,
            12000,
            false,
            None,
            None,
            false,
            true,
        );
        assert_ne!(k1, k2);
    }

    #[test]
    fn build_derived_key_distinguishes_pdf_pages() {
        let scope = CacheScope::Anonymous;
        let k1 = build_derived_key(
            &scope,
            100,
            ExtractMode::Text,
            12000,
            false,
            Some("1-3"),
            None,
            false,
            true,
        );
        let k2 = build_derived_key(
            &scope,
            100,
            ExtractMode::Text,
            12000,
            false,
            Some("1-5"),
            None,
            false,
            true,
        );
        assert_ne!(k1, k2);
    }

    #[test]
    fn build_derived_key_distinguishes_max_chars() {
        let scope = CacheScope::Anonymous;
        let k1 = build_derived_key(
            &scope,
            100,
            ExtractMode::Text,
            8000,
            false,
            None,
            None,
            false,
            true,
        );
        let k2 = build_derived_key(
            &scope,
            100,
            ExtractMode::Text,
            10000,
            false,
            None,
            None,
            false,
            true,
        );
        assert_ne!(k1, k2);
    }

    #[test]
    fn stale_entry_with_etag_gets_conditional_headers() {
        let validators = CacheValidators {
            etag: Some("\"abc123\"".into()),
            last_modified: None,
        };
        let headers = build_request_conditional_headers(&validators);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "If-None-Match");
        assert_eq!(headers[0].1, "\"abc123\"");
    }

    #[test]
    fn stale_entry_with_last_modified_gets_conditional_headers() {
        let validators = CacheValidators {
            etag: None,
            last_modified: Some("Wed, 01 Jan 2025 00:00:00 GMT".into()),
        };
        let headers = build_request_conditional_headers(&validators);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "If-Modified-Since");
        assert_eq!(headers[0].1, "Wed, 01 Jan 2025 00:00:00 GMT");
    }

    #[test]
    fn no_validators_means_no_conditional_headers() {
        let validators = CacheValidators {
            etag: None,
            last_modified: None,
        };
        let headers = build_request_conditional_headers(&validators);
        assert!(headers.is_empty());
    }

    #[test]
    fn stale_freshness_is_not_fresh() {
        let freshness = CacheFreshness {
            max_age: Some(std::time::Duration::from_secs(300)),
            fetched_at: Some(std::time::SystemTime::now() - std::time::Duration::from_secs(600)),
            ..CacheFreshness::default()
        };
        assert!(!freshness.is_fresh());
    }

    #[test]
    fn fresh_freshness_is_fresh() {
        let freshness = CacheFreshness {
            max_age: Some(std::time::Duration::from_secs(300)),
            fetched_at: Some(std::time::SystemTime::now()),
            ..CacheFreshness::default()
        };
        assert!(freshness.is_fresh());
    }

    #[test]
    fn no_store_never_fresh() {
        let freshness = CacheFreshness {
            no_store: true,
            max_age: Some(std::time::Duration::from_secs(3600)),
            fetched_at: Some(std::time::SystemTime::now()),
            ..CacheFreshness::default()
        };
        assert!(!freshness.is_fresh());
    }
}
