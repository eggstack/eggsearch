//! In-memory pass-cookie cache, scoped by origin and bounded by size and expiry.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use url::Url;

/// Default number of origins the cache holds.
pub const DEFAULT_CAPACITY: usize = 32;

/// Default byte ceiling on the stored pair sets.
pub const DEFAULT_MAX_BYTES: usize = 64 * 1024;

struct Entry {
    value: String,
    expires_at: Instant,
    sequence: u64,
    bytes: usize,
}

struct CacheState {
    entries: HashMap<String, Entry>,
    bytes: usize,
    sequence: u64,
}

/// Process-local, origin-keyed pass-cookie cache.
///
/// The key is an origin, so a pair set is only ever presented to the origin
/// that issued it. The cache is bounded in entry count and in stored bytes.
pub struct FetchPassCookieCache {
    capacity: usize,
    max_bytes: usize,
    state: Mutex<CacheState>,
}

impl FetchPassCookieCache {
    /// Create a cache holding at most `capacity` origins and `max_bytes` bytes.
    #[must_use]
    pub fn new(capacity: usize, max_bytes: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            max_bytes,
            state: Mutex::new(CacheState {
                entries: HashMap::new(),
                bytes: 0,
                sequence: 0,
            }),
        }
    }

    /// Return the unexpired pair set for `origin`, dropping an expired entry.
    #[must_use]
    pub fn get(&self, origin: &str) -> Option<String> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if state
            .entries
            .get(origin)
            .is_some_and(|entry| entry.expires_at <= now)
        {
            if let Some(expired) = state.entries.remove(origin) {
                state.bytes = state.bytes.saturating_sub(expired.bytes);
            }
        }
        state.entries.get(origin).map(|entry| entry.value.clone())
    }

    /// Store a pair set for `origin`. A zero `ttl` stores nothing.
    ///
    /// An insertion that would exceed the entry cap or the byte ceiling evicts
    /// the entry closest to expiry first. An insertion larger than the byte
    /// ceiling on its own is not stored.
    pub fn put(&self, origin: &str, value: &str, ttl: Duration) {
        if ttl.is_zero() || value.is_empty() {
            return;
        }
        let bytes = origin.len() + value.len();
        if bytes > self.max_bytes {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        purge_expired(&mut state, now);
        let replacing = state.entries.contains_key(origin);
        if let Some(existing) = state.entries.remove(origin) {
            state.bytes = state.bytes.saturating_sub(existing.bytes);
        }
        while state.bytes + bytes > self.max_bytes {
            let Some(victim) = eviction_victim(&state) else {
                return;
            };
            let Some(removed) = state.entries.remove(&victim) else {
                return;
            };
            state.bytes = state.bytes.saturating_sub(removed.bytes);
        }
        if !replacing && state.entries.len() >= self.capacity {
            let Some(victim) = eviction_victim(&state) else {
                return;
            };
            let Some(removed) = state.entries.remove(&victim) else {
                return;
            };
            state.bytes = state.bytes.saturating_sub(removed.bytes);
        }
        state.sequence += 1;
        let sequence = state.sequence;
        state.entries.insert(
            origin.to_string(),
            Entry {
                value: value.to_string(),
                expires_at: now.checked_add(ttl).unwrap_or(now),
                sequence,
                bytes,
            },
        );
        state.bytes += bytes;
    }

    /// Drop the entry for one origin, leaving every other origin intact.
    pub fn remove(&self, origin: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(removed) = state.entries.remove(origin) {
            state.bytes = state.bytes.saturating_sub(removed.bytes);
        }
    }

    /// Number of live entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .len()
    }

    /// Whether the cache holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Bytes currently held by the stored pair sets.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).bytes
    }

    /// Configured entry cap.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Configured byte ceiling.
    #[must_use]
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Drop every entry.
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.entries.clear();
        state.bytes = 0;
    }
}

impl std::fmt::Debug for FetchPassCookieCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchPassCookieCache")
            .field("capacity", &self.capacity)
            .field("max_bytes", &self.max_bytes)
            .field("entries", &self.len())
            .finish()
    }
}

fn purge_expired(state: &mut CacheState, now: Instant) {
    state.entries.retain(|_, entry| {
        if entry.expires_at > now {
            true
        } else {
            state.bytes = state.bytes.saturating_sub(entry.bytes);
            false
        }
    });
}

fn eviction_victim(state: &CacheState) -> Option<String> {
    state
        .entries
        .iter()
        .min_by(|(left_key, left), (right_key, right)| {
            left.expires_at
                .cmp(&right.expires_at)
                .then(left.sequence.cmp(&right.sequence))
                .then(left_key.cmp(right_key))
        })
        .map(|(key, _)| key.clone())
}

static FETCH_PASS_COOKIE_CACHE: OnceLock<FetchPassCookieCache> = OnceLock::new();

/// Process-wide fetch-side pass-cookie cache.
#[must_use]
pub fn fetch_pass_cookie_cache() -> &'static FetchPassCookieCache {
    FETCH_PASS_COOKIE_CACHE
        .get_or_init(|| FetchPassCookieCache::new(DEFAULT_CAPACITY, DEFAULT_MAX_BYTES))
}

/// Origin key for a URL: scheme, host, and effective port.
///
/// `None` when the URL has no cacheable origin — a scheme other than
/// `http` or `https`, a host the URL does not carry, or a URL that does not
/// parse.
#[must_use]
pub fn origin_key(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    origin_key_of(&parsed)
}

fn origin_key_of(parsed: &Url) -> Option<String> {
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default()?;
    Some(format!("{}://{}:{}", parsed.scheme(), host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache() -> FetchPassCookieCache {
        FetchPassCookieCache::new(2, 64)
    }

    #[test]
    fn stores_and_returns_a_pair_set() {
        let cache = cache();
        cache.put("http://host.test:80", "chllg=abc", Duration::from_secs(60));
        assert_eq!(
            cache.get("http://host.test:80").as_deref(),
            Some("chllg=abc")
        );
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
    }

    #[test]
    fn a_zero_ttl_stores_nothing() {
        let cache = cache();
        cache.put("http://host.test:80", "chllg=abc", Duration::ZERO);
        assert_eq!(cache.get("http://host.test:80"), None);
        assert!(cache.is_empty());
    }

    #[test]
    fn an_expired_entry_is_dropped_on_read() {
        let cache = cache();
        cache.put("http://host.test:80", "chllg=abc", Duration::from_secs(60));
        assert_eq!(
            cache.get("http://host.test:80").as_deref(),
            Some("chllg=abc")
        );
        {
            let mut state = cache.state.lock().unwrap_or_else(|e| e.into_inner());
            state
                .entries
                .get_mut("http://host.test:80")
                .expect("stored entry")
                .expires_at = Instant::now();
        }
        assert_eq!(cache.get("http://host.test:80"), None);
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.bytes(), 0);
    }

    #[test]
    fn the_entry_cap_holds() {
        let cache = cache();
        cache.put("http://one.test:80", "a=1", Duration::from_secs(60));
        cache.put("http://two.test:80", "b=2", Duration::from_secs(60));
        cache.put("http://three.test:80", "c=3", Duration::from_secs(60));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.capacity(), 2);
        assert!(
            cache.bytes() <= cache.max_bytes(),
            "the byte ceiling holds: {} > {}",
            cache.bytes(),
            cache.max_bytes()
        );
    }

    #[test]
    fn the_byte_ceiling_holds() {
        let cache = FetchPassCookieCache::new(8, 32);
        cache.put("http://one.test:80", "a=1", Duration::from_secs(60));
        cache.put("http://two.test:80", "b=2", Duration::from_secs(60));
        assert!(
            cache.bytes() <= cache.max_bytes(),
            "the byte ceiling holds: {} > {}",
            cache.bytes(),
            cache.max_bytes()
        );
        assert!(cache.len() <= 1);
    }

    #[test]
    fn an_oversized_pair_set_is_not_stored() {
        let cache = FetchPassCookieCache::new(8, 16);
        cache.put(
            "http://one.test:80",
            &"x".repeat(64),
            Duration::from_secs(60),
        );
        assert!(cache.is_empty());
        assert_eq!(cache.bytes(), 0);
    }

    #[test]
    fn removing_one_origin_leaves_the_rest() {
        let cache = cache();
        cache.put("http://one.test:80", "a=1", Duration::from_secs(60));
        cache.put("http://two.test:80", "b=2", Duration::from_secs(60));
        cache.remove("http://one.test:80");
        assert_eq!(cache.get("http://one.test:80"), None);
        assert_eq!(cache.get("http://two.test:80").as_deref(), Some("b=2"));
        cache.remove("http://absent.test:80");
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.bytes(), "http://two.test:80".len() + 3);
    }

    #[test]
    fn replacing_an_origin_keeps_the_count_and_the_bytes_stable() {
        let cache = cache();
        cache.put("http://one.test:80", "a=1", Duration::from_secs(60));
        cache.put("http://one.test:80", "b=2", Duration::from_secs(60));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.bytes(), "http://one.test:80".len() + 3);
        assert_eq!(cache.get("http://one.test:80").as_deref(), Some("b=2"));
    }

    #[test]
    fn debug_redacts_the_stored_value() {
        let cache = cache();
        cache.put(
            "http://host.test:80",
            "chllg=super-secret",
            Duration::from_secs(60),
        );
        let rendered = format!("{cache:?}");
        assert!(!rendered.contains("super-secret"), "got: {rendered}");
        assert!(!rendered.contains("chllg"), "got: {rendered}");
    }

    #[test]
    fn the_origin_key_is_scheme_host_and_effective_port() {
        assert_eq!(
            origin_key("http://host.test/page?q=1").as_deref(),
            Some("http://host.test:80")
        );
        assert_eq!(
            origin_key("https://host.test/page").as_deref(),
            Some("https://host.test:443")
        );
        assert_eq!(
            origin_key("https://host.test:8443/page").as_deref(),
            Some("https://host.test:8443")
        );
    }

    #[test]
    fn a_default_port_and_an_explicit_one_share_a_key() {
        assert_eq!(
            origin_key("http://host.test/page"),
            origin_key("http://host.test:80/page")
        );
    }

    #[test]
    fn a_non_http_url_has_no_origin_key() {
        assert_eq!(origin_key("ftp://host.test/file"), None);
        assert_eq!(origin_key("file:///tmp/file"), None);
        assert_eq!(origin_key("not a url"), None);
        assert_eq!(origin_key(""), None);
    }

    #[test]
    fn the_process_cache_is_shared() {
        let first = fetch_pass_cookie_cache();
        let second = fetch_pass_cookie_cache();
        assert!(std::ptr::eq(first, second));
        assert_eq!(first.capacity(), DEFAULT_CAPACITY);
        assert_eq!(first.max_bytes(), DEFAULT_MAX_BYTES);
    }
}
