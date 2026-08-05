use std::collections::HashMap;
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
            fetched_at: Some(SystemTime::now()),
        };
        let mut validators = CacheValidators {
            etag: None,
            last_modified: None,
        };

        if let Some(cc) = headers.get("cache-control").and_then(|v| v.to_str().ok()) {
            let cc_lower = cc.to_lowercase();
            freshness.no_store = cc_lower.contains("no-store");
            freshness.no_cache = cc_lower.contains("no-cache");
            freshness.private = cc_lower.contains("private");

            if let Some(pos) = cc_lower.find("max-age=") {
                let val_start = pos + 8;
                if let Some(val_end) = cc_lower[val_start..].find(|c: char| !c.is_ascii_digit()) {
                    if let Ok(secs) = cc_lower[val_start..val_start + val_end].parse::<u64>() {
                        freshness.max_age = Some(Duration::from_secs(secs));
                    }
                } else if let Ok(secs) = cc_lower[val_start..].parse::<u64>() {
                    freshness.max_age = Some(Duration::from_secs(secs));
                }
            }
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
}

pub struct FetchCache {
    raw: Mutex<LruCache<RawCacheKey, RawFetchCacheEntry>>,
    derived: Mutex<LruCache<DerivedCacheKey, DerivedDocumentCacheEntry>>,
    raw_max_bytes: usize,
    current_raw_bytes: Mutex<usize>,
}

impl FetchCache {
    pub fn new(max_raw_entries: usize, max_derived_entries: usize, raw_max_bytes: usize) -> Self {
        let raw_cap = std::num::NonZeroUsize::new(max_raw_entries.max(1)).unwrap();
        let derived_cap = std::num::NonZeroUsize::new(max_derived_entries.max(1)).unwrap();
        Self {
            raw: Mutex::new(LruCache::new(raw_cap)),
            derived: Mutex::new(LruCache::new(derived_cap)),
            raw_max_bytes,
            current_raw_bytes: Mutex::new(0),
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
        let mut current = self.current_raw_bytes.lock().await;

        if let Some(evicted) = raw.pop(&key) {
            *current = current.saturating_sub(evicted.body.len());
        }

        while *current + body_len > self.raw_max_bytes && !raw.is_empty() {
            if let Some((_, evicted)) = raw.pop_lru() {
                *current = current.saturating_sub(evicted.body.len());
            } else {
                break;
            }
        }

        raw.put(key, entry);
        *current += body_len;
        true
    }

    pub async fn get_derived(&self, key: &DerivedCacheKey) -> Option<DerivedDocumentCacheEntry> {
        let mut derived = self.derived.lock().await;
        derived.get(key).cloned()
    }

    pub async fn insert_derived(&self, key: DerivedCacheKey, entry: DerivedDocumentCacheEntry) {
        let mut derived = self.derived.lock().await;
        derived.put(key, entry);
    }

    pub async fn invalidate_scope(&self, scope: &CacheScope) {
        let mut raw = self.raw.lock().await;
        let mut current = self.current_raw_bytes.lock().await;

        let keys_to_remove: Vec<RawCacheKey> = raw
            .iter()
            .filter(|(k, _)| k.scope == *scope)
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            if let Some(evicted) = raw.pop(&key) {
                *current = current.saturating_sub(evicted.body.len());
            }
        }

        drop(raw);
        drop(current);

        let mut derived = self.derived.lock().await;
        let derived_keys_to_remove: Vec<DerivedCacheKey> = derived
            .iter()
            .filter(|(k, _)| k.scope == *scope)
            .map(|(k, _)| k.clone())
            .collect();

        for key in derived_keys_to_remove {
            derived.pop(&key);
        }
    }

    pub async fn stats(&self) -> CacheStats {
        let raw = self.raw.lock().await;
        let derived = self.derived.lock().await;
        let current_raw = *self.current_raw_bytes.lock().await;
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
    match max_chars {
        0..=4096 => 4096,
        4097..=12000 => 12000,
        12001..=50000 => 50000,
        _ => max_chars,
    }
}

fn parse_http_date(s: &str) -> Option<SystemTime> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(s) {
        return Some(dt.into());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.into());
    }
    let normalized = s.trim().replace("GMT", "+0000");
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
        assert_eq!(classify_max_chars(1000), 4096);
        assert_eq!(classify_max_chars(8000), 12000);
        assert_eq!(classify_max_chars(30000), 50000);
        assert_eq!(classify_max_chars(60000), 60000);
    }

    #[tokio::test]
    async fn cache_insert_and_get_raw() {
        let cache = FetchCache::new(10, 10, 1024 * 1024);
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
        };

        assert!(cache.get_raw(&key).await.is_none());
        cache.insert_raw(key.clone(), entry).await;
        assert!(cache.get_raw(&key).await.is_some());
    }

    #[tokio::test]
    async fn cache_evicts_oldest_on_byte_pressure() {
        let cache = FetchCache::new(10, 10, 20);
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
            };
            cache.insert_raw(key, entry).await;
        }
        let stats = cache.stats().await;
        assert!(stats.raw_bytes <= 20);
    }

    #[tokio::test]
    async fn cache_derived_insert_and_get() {
        let cache = FetchCache::new(10, 10, 1024 * 1024);
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
            },
            created_at: SystemTime::now(),
        };

        assert!(cache.get_derived(&key).await.is_none());
        cache.insert_derived(key.clone(), entry).await;
        assert!(cache.get_derived(&key).await.is_some());
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
    fn build_derived_key_groups_same_max_chars_class() {
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
        assert_eq!(k1, k2);
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
