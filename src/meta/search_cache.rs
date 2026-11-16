//! Process-local cache and single-flight coalescer for `web_search` fan-outs.
//!
//! The cache stores the raw, pre-aggregation per-provider result lists so
//! that sanitization, trust framing, fusion, reranking, and evidence
//! postprocessing still run on every call. This mirrors the fetch cache's
//! raw-versus-derived split: only the provider payload is shared, never a
//! converted or framed card.
//!
//! Every dimension is bounded: time-to-live, entry count, and a per-entry
//! row cap. An entry whose total row count exceeds 1024 rows is not stored at
//! all, so a cache hit can never serve a truncated list.
//!
//! Only complete fan-outs are stored. A fan-out with any provider failure,
//! any provider that did not answer before the dispatch deadline, or a
//! panicking dispatch task bypasses the cache and is not published to
//! coalesced waiters; each waiter re-runs its own fan-out instead.
//!
//! Coalescing is process-local and keyed by the same digest as the store.
//! A coalesced waiter consumes the leader's dispatch budget: two identical
//! simultaneous calls share one `[search].timeout_ms` window, which is
//! intended.
//! A leader registers a `broadcast` sender, executes the fan-out, stores
//! the result when it is complete, removes the in-flight entry, and then
//! publishes to waiters. If the leader is cancelled or panics, its guard
//! removes the in-flight entry and the remaining senders drop, so waiters
//! wake with a receive error and execute their own fan-out. A waiter is
//! never left blocked.
//!
//! The TTL is a freshness bound, not a correctness guarantee: a hit can be
//! up to one TTL stale, and restarting the process clears everything.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use lru::LruCache;
use tokio::sync::broadcast;

use crate::core::identity::{write_str, write_usize, FnvHasher};
use crate::core::query::{Freshness, SafeSearch, SearchDateRange, SearchIntent, WebSearchRequest};
use crate::meta::engines::error::EngineError;
use crate::meta::engines::models::SearchResult;

/// Versioned key namespace, mirroring the `eggsearch-id-v1\0` convention in
/// [`crate::core::identity`].
const SEARCH_CACHE_KEY_PREFIX: &[u8] = b"eggsearch-search-cache-v1\0";

/// Payload schema version mixed into every key. Bumping it invalidates every
/// stored entry without touching the hashing algorithm.
const CACHE_SCHEMA_VERSION: u32 = 1;

/// Maximum rows (summed across providers) stored in one cache entry.
pub(crate) const MAX_CACHED_ROWS_PER_ENTRY: usize = 1024;

/// Maximum number of consecutive coalesced-wait failures before a caller
/// stops waiting and runs its own fan-out.
const MAX_COALESCED_FALLBACKS: u32 = 2;

const DEFAULT_TTL: Duration = Duration::from_secs(300);
const DEFAULT_MAX_ENTRIES: usize = 128;

type CachedLists = Vec<(String, Vec<SearchResult>)>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Settings for the process-local search-result cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchCacheSettings {
    /// Time-to-live for a stored entry. A zero TTL disables the cache.
    pub ttl: Duration,
    /// Maximum number of stored entries. Values below one are clamped to one.
    pub max_entries: usize,
}

impl SearchCacheSettings {
    /// Whether these settings enable the cache.
    pub fn is_enabled(&self) -> bool {
        !self.ttl.is_zero()
    }
}

impl Default for SearchCacheSettings {
    fn default() -> Self {
        Self {
            ttl: DEFAULT_TTL,
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }
}

/// Canonical cache key for one `web_search` fan-out.
///
/// The key covers every input that can change the raw per-provider lists or
/// the payload built from them. `query` is whitespace-normalized, provider
/// ids and domain filters are sorted, so semantically identical requests
/// share one entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchCacheKey {
    pub(crate) query: String,
    pub(crate) providers: Vec<String>,
    pub(crate) intent: Option<SearchIntent>,
    pub(crate) freshness: Freshness,
    pub(crate) date_range: Option<SearchDateRange>,
    pub(crate) include_domains: Vec<String>,
    pub(crate) exclude_domains: Vec<String>,
    pub(crate) language: Option<String>,
    pub(crate) region: Option<String>,
    pub(crate) safe_search: Option<SafeSearch>,
    pub(crate) effective_max_results: usize,
    pub(crate) candidate_limit: usize,
    pub(crate) excerpt_count: usize,
}

impl SearchCacheKey {
    /// Build a canonical key from the request, the caller's intent (or
    /// `None` when the caller omitted it), and the resolved provider list.
    pub(crate) fn from_request(
        req: &WebSearchRequest,
        caller_intent: Option<SearchIntent>,
        queried_ids: &[String],
        effective_max_results: usize,
        candidate_limit: usize,
    ) -> Self {
        Self {
            query: normalize_query(&req.query),
            providers: sorted_unique(queried_ids),
            intent: caller_intent,
            freshness: req.freshness,
            date_range: req.date_range.clone(),
            include_domains: sorted_unique(&req.include_domains),
            exclude_domains: sorted_unique(&req.exclude_domains),
            language: req.language.clone(),
            region: req.region.clone(),
            safe_search: req.safe_search,
            effective_max_results,
            candidate_limit,
            excerpt_count: req.effective_excerpt_count(),
        }
    }

    /// Stable FNV-1a digest over every key field.
    pub(crate) fn digest(&self) -> u64 {
        let mut hasher = FnvHasher::new();
        hasher.write(SEARCH_CACHE_KEY_PREFIX);
        hasher.write(&CACHE_SCHEMA_VERSION.to_le_bytes());
        write_str(&mut hasher, &self.query);
        write_list(&mut hasher, &self.providers);
        match self.intent {
            None => write_tag(&mut hasher, 0),
            Some(intent) => {
                write_tag(&mut hasher, 1);
                write_str(&mut hasher, intent.as_str());
            }
        }
        write_str(&mut hasher, self.freshness.as_str());
        match &self.date_range {
            None => write_tag(&mut hasher, 0),
            Some(range) => {
                write_tag(&mut hasher, 1);
                write_str(&mut hasher, &range.start);
                write_str(&mut hasher, &range.end);
            }
        }
        write_list(&mut hasher, &self.include_domains);
        write_list(&mut hasher, &self.exclude_domains);
        write_optional_str(&mut hasher, self.language.as_deref());
        write_optional_str(&mut hasher, self.region.as_deref());
        match self.safe_search {
            None => write_tag(&mut hasher, 0),
            Some(safe_search) => {
                write_tag(&mut hasher, 1);
                write_str(&mut hasher, safe_search.as_str());
            }
        }
        write_usize(&mut hasher, self.effective_max_results);
        write_usize(&mut hasher, self.candidate_limit);
        write_usize(&mut hasher, self.excerpt_count);
        hasher.finish()
    }
}

fn normalize_query(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sorted_unique(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort();
    values.dedup();
    values
}

fn write_tag(hasher: &mut FnvHasher, tag: u8) {
    hasher.write(&[tag]);
}

fn write_optional_str(hasher: &mut FnvHasher, value: Option<&str>) {
    match value {
        None => write_tag(hasher, 0),
        Some(value) => {
            write_tag(hasher, 1);
            write_str(hasher, value);
        }
    }
}

fn write_list(hasher: &mut FnvHasher, values: &[String]) {
    write_usize(hasher, values.len());
    for value in values {
        write_str(hasher, value);
    }
}

/// Raw per-provider dispatch outcome.
#[derive(Debug)]
pub(crate) struct SearchFanout {
    pub(crate) lists: CachedLists,
    pub(crate) failures: Vec<(String, EngineError)>,
    pub(crate) result_latencies_ms: Vec<u64>,
    pub(crate) failure_latencies_ms: Vec<u64>,
    pub(crate) deadline_interrupted: bool,
}

impl SearchFanout {
    /// A fan-out is cacheable only when every queried provider answered
    /// successfully before the deadline.
    pub(crate) fn is_cacheable(&self) -> bool {
        self.failures.is_empty() && !self.deadline_interrupted
    }

    fn row_count(&self) -> usize {
        self.lists.iter().map(|(_, results)| results.len()).sum()
    }

    fn from_cached(lists: Arc<CachedLists>) -> Self {
        Self {
            lists: (*lists).clone(),
            failures: Vec::new(),
            result_latencies_ms: Vec::new(),
            failure_latencies_ms: Vec::new(),
            deadline_interrupted: false,
        }
    }
}

/// How a call obtained its raw fan-out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheDisposition {
    /// Served from a stored entry.
    Hit,
    /// Served from another call's in-flight fan-out.
    Coalesced,
    /// This call executed its own fan-out.
    Miss,
    /// The cache is disabled; this call executed its own fan-out.
    Disabled,
}

impl CacheDisposition {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Coalesced => "coalesced",
            Self::Miss => "miss",
            Self::Disabled => "disabled",
        }
    }

    /// Whether this call actually dispatched providers and may therefore
    /// record provider health.
    pub(crate) fn conducted_fanout(self) -> bool {
        matches!(self, Self::Miss | Self::Disabled)
    }
}

/// A resolved fan-out plus how it was obtained.
pub(crate) struct ResolvedSearch {
    pub(crate) fanout: SearchFanout,
    pub(crate) disposition: CacheDisposition,
}

impl ResolvedSearch {
    pub(crate) fn into_parts(self) -> (SearchFanout, CacheDisposition) {
        (self.fanout, self.disposition)
    }
}

/// Diagnostic counters for the cache.
///
/// `hits` counts calls served from a stored entry. `misses` counts calls
/// that did not find a fresh entry, whether they then coalesced or led.
/// `coalesced` counts calls served by another call's in-flight fan-out.
/// `bypassed` counts fan-outs that were not stored (incomplete, or above
/// the per-entry row cap). `evictions` counts capacity evictions only.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SearchCacheStats {
    pub(crate) entries: usize,
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) coalesced: u64,
    pub(crate) bypassed: u64,
    pub(crate) evictions: u64,
}

#[derive(Debug, Default)]
struct SearchCacheCounters {
    hits: AtomicU64,
    misses: AtomicU64,
    coalesced: AtomicU64,
    bypassed: AtomicU64,
    evictions: AtomicU64,
}

#[derive(Debug)]
struct CachedSearchEntry {
    key: SearchCacheKey,
    lists: Arc<CachedLists>,
    stored_at: Instant,
}

enum InflightSlot<'a> {
    Lead(InflightGuard<'a>),
    Follow(broadcast::Receiver<Arc<CachedLists>>),
}

/// Owns one in-flight registration. Removing the entry on every exit path is
/// the guard's responsibility, so a cancelled or panicking leader can never
/// leave a stale sender behind.
struct InflightGuard<'a> {
    cache: &'a SearchCache,
    digest: u64,
    armed: bool,
}

impl InflightGuard<'_> {
    fn finish(mut self, lists: Option<Arc<CachedLists>>) {
        self.armed = false;
        self.cache.release(self.digest, lists);
    }
}

impl Drop for InflightGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.cache.release(self.digest, None);
        }
    }
}

/// Bounded LRU store plus single-flight coalescer for `web_search` fan-outs.
pub(crate) struct SearchCache {
    settings: SearchCacheSettings,
    entries: Mutex<LruCache<u64, CachedSearchEntry>>,
    inflight: Mutex<HashMap<u64, broadcast::Sender<Arc<CachedLists>>>>,
    counters: SearchCacheCounters,
}

impl SearchCache {
    pub(crate) fn new(settings: SearchCacheSettings) -> Self {
        let capacity = std::num::NonZeroUsize::new(settings.max_entries.max(1))
            .unwrap_or(std::num::NonZeroUsize::MIN);
        Self {
            settings,
            entries: Mutex::new(LruCache::new(capacity)),
            inflight: Mutex::new(HashMap::new()),
            counters: SearchCacheCounters::default(),
        }
    }

    pub(crate) fn stats(&self) -> SearchCacheStats {
        SearchCacheStats {
            entries: lock(&self.entries).len(),
            hits: self.counters.hits.load(Ordering::Relaxed),
            misses: self.counters.misses.load(Ordering::Relaxed),
            coalesced: self.counters.coalesced.load(Ordering::Relaxed),
            bypassed: self.counters.bypassed.load(Ordering::Relaxed),
            evictions: self.counters.evictions.load(Ordering::Relaxed),
        }
    }

    /// Serve `key` from the store, from an in-flight leader, or by running
    /// `run` as the leader. A waiter whose leader disappears (cancelled,
    /// panicked, or produced a non-cacheable fan-out) falls back to running
    /// its own fan-out.
    pub(crate) async fn resolve<F, Fut>(&self, key: SearchCacheKey, run: F) -> ResolvedSearch
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = SearchFanout>,
    {
        let digest = key.digest();
        if let Some(lists) = self.lookup(digest, &key) {
            self.counters.hits.fetch_add(1, Ordering::Relaxed);
            return ResolvedSearch {
                fanout: SearchFanout::from_cached(lists),
                disposition: CacheDisposition::Hit,
            };
        }
        self.counters.misses.fetch_add(1, Ordering::Relaxed);

        let mut slot = self.reserve(digest);
        let mut fallbacks = 0u32;
        loop {
            match slot {
                InflightSlot::Lead(guard) => return self.lead(key, digest, guard, run).await,
                InflightSlot::Follow(mut receiver) => {
                    if let Ok(lists) = receiver.recv().await {
                        self.counters.coalesced.fetch_add(1, Ordering::Relaxed);
                        return ResolvedSearch {
                            fanout: SearchFanout::from_cached(lists),
                            disposition: CacheDisposition::Coalesced,
                        };
                    }
                    fallbacks += 1;
                    if fallbacks >= MAX_COALESCED_FALLBACKS {
                        self.counters.bypassed.fetch_add(1, Ordering::Relaxed);
                        return ResolvedSearch {
                            fanout: run().await,
                            disposition: CacheDisposition::Miss,
                        };
                    }
                    slot = self.reserve(digest);
                }
            }
        }
    }

    async fn lead<F, Fut>(
        &self,
        key: SearchCacheKey,
        digest: u64,
        guard: InflightGuard<'_>,
        run: F,
    ) -> ResolvedSearch
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = SearchFanout>,
    {
        let fanout = run().await;
        let shareable = fanout.is_cacheable() && fanout.row_count() <= MAX_CACHED_ROWS_PER_ENTRY;
        let shared = shareable.then(|| Arc::new(fanout.lists.clone()));
        match shared.as_ref() {
            Some(lists) => self.store(key, digest, Arc::clone(lists)),
            None => {
                self.counters.bypassed.fetch_add(1, Ordering::Relaxed);
            }
        }
        guard.finish(shared);
        ResolvedSearch {
            fanout,
            disposition: CacheDisposition::Miss,
        }
    }

    fn lookup(&self, digest: u64, key: &SearchCacheKey) -> Option<Arc<CachedLists>> {
        let mut entries = lock(&self.entries);
        if let Some(entry) = entries.get(&digest) {
            if entry.stored_at.elapsed() < self.settings.ttl && entry.key == *key {
                return Some(Arc::clone(&entry.lists));
            }
        }
        entries.pop(&digest);
        None
    }

    fn store(&self, key: SearchCacheKey, digest: u64, lists: Arc<CachedLists>) {
        let entry = CachedSearchEntry {
            key,
            lists,
            stored_at: Instant::now(),
        };
        let mut entries = lock(&self.entries);
        if entries.push(digest, entry).is_some() {
            self.counters.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn reserve(&self, digest: u64) -> InflightSlot<'_> {
        let mut inflight = lock(&self.inflight);
        if let Some(sender) = inflight.get(&digest) {
            return InflightSlot::Follow(sender.subscribe());
        }
        let (sender, _) = broadcast::channel(1);
        inflight.insert(digest, sender);
        InflightSlot::Lead(InflightGuard {
            cache: self,
            digest,
            armed: true,
        })
    }

    fn release(&self, digest: u64, lists: Option<Arc<CachedLists>>) {
        let sender = lock(&self.inflight).remove(&digest);
        if let (Some(sender), Some(lists)) = (sender, lists) {
            let _ = sender.send(lists);
        }
    }

    #[cfg(test)]
    fn inflight_entries(&self) -> usize {
        lock(&self.inflight).len()
    }

    #[cfg(test)]
    fn inflight_waiters(&self, digest: u64) -> usize {
        lock(&self.inflight)
            .get(&digest)
            .map_or(0, |sender| sender.receiver_count())
    }
}

#[cfg(test)]
mod tests;
