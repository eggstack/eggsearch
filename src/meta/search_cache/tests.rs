use super::*;

use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use tokio::sync::{oneshot, Barrier};

fn result(title: &str, url: &str) -> SearchResult {
    SearchResult {
        title: title.to_string(),
        url: url.to_string(),
        snippet: None,
        source_engine: "mock_a".to_string(),
        metadata: Default::default(),
        excerpts: Vec::new(),
        published_at: None,
    }
}

fn take_entered(slot: &Mutex<Option<oneshot::Sender<()>>>) -> Option<oneshot::Sender<()>> {
    slot.lock().unwrap().take()
}

fn take_release(slot: &Mutex<Option<oneshot::Receiver<()>>>) -> Option<oneshot::Receiver<()>> {
    slot.lock().unwrap().take()
}

fn signature(lists: &CachedLists) -> Vec<(String, Vec<String>)> {
    lists
        .iter()
        .map(|(provider, results)| {
            (
                provider.clone(),
                results.iter().map(|result| result.url.clone()).collect(),
            )
        })
        .collect()
}

fn complete_fanout(rows: usize) -> SearchFanout {
    SearchFanout {
        lists: vec![(
            "mock_a".to_string(),
            (0..rows)
                .map(|index| {
                    result(
                        &format!("title {index}"),
                        &format!("https://example.com/{index}"),
                    )
                })
                .collect(),
        )],
        failures: Vec::new(),
        result_latencies_ms: vec![1],
        failure_latencies_ms: Vec::new(),
        deadline_interrupted: false,
    }
}

fn settings(ttl: Duration, max_entries: usize) -> SearchCacheSettings {
    SearchCacheSettings { ttl, max_entries }
}

fn base_request() -> WebSearchRequest {
    let mut req = WebSearchRequest::new("rust async runtime");
    req.max_results = Some(10);
    req.freshness = Freshness::Any;
    req.include_domains = vec!["docs.rs".to_string()];
    req.exclude_domains = vec!["example.com".to_string()];
    req.language = Some("en".to_string());
    req.region = Some("US".to_string());
    req.safe_search = Some(SafeSearch::Moderate);
    req.excerpt_count = Some(1);
    req
}

fn providers() -> Vec<String> {
    vec!["mock_a".to_string(), "mock_b".to_string()]
}

fn base_key() -> SearchCacheKey {
    SearchCacheKey::from_request(
        &base_request(),
        Some(SearchIntent::Web),
        &providers(),
        10,
        20,
    )
}

fn key_for(req: &WebSearchRequest) -> SearchCacheKey {
    SearchCacheKey::from_request(req, Some(SearchIntent::Web), &providers(), 10, 20)
}

fn assert_invalidates(label: &str, left: &SearchCacheKey, right: &SearchCacheKey) {
    assert_ne!(
        left.digest(),
        right.digest(),
        "{label} must change the digest"
    );
}

#[test]
fn digest_is_stable_across_constructions() {
    let first = base_key();
    let second = SearchCacheKey::from_request(
        &base_request(),
        Some(SearchIntent::Web),
        &providers(),
        10,
        20,
    );
    assert_eq!(first.digest(), second.digest());
    assert_eq!(first, second);
}

#[test]
fn digest_matches_frozen_value() {
    assert_eq!(
        format!("{:016x}", base_key().digest()),
        "88c66bb4dc158d34",
        "the cache-key encoding is frozen; bump CACHE_SCHEMA_VERSION when it changes"
    );
}

#[test]
fn query_changes_invalidate_the_digest() {
    let mut req = base_request();
    req.query = "rust async runtimes".to_string();
    assert_invalidates("query", &base_key(), &key_for(&req));
}

#[test]
fn query_whitespace_is_normalized() {
    let mut padded = base_request();
    padded.query = "  rust   async\truntime \n".to_string();
    assert_eq!(base_key().digest(), key_for(&padded).digest());
}

#[test]
fn provider_set_changes_invalidate_the_digest() {
    let req = base_request();
    let one_provider = SearchCacheKey::from_request(
        &req,
        Some(SearchIntent::Web),
        &["mock_a".to_string()],
        10,
        20,
    );
    assert_invalidates("provider set", &base_key(), &one_provider);

    let reversed = vec!["mock_b".to_string(), "mock_a".to_string()];
    let reordered = SearchCacheKey::from_request(&req, Some(SearchIntent::Web), &reversed, 10, 20);
    assert_eq!(
        base_key().digest(),
        reordered.digest(),
        "provider order must not change the digest"
    );
}

#[test]
fn omitted_intent_differs_from_explicit_web() {
    let req = base_request();
    let omitted = SearchCacheKey::from_request(&req, None, &providers(), 10, 20);
    assert_invalidates("omitted intent", &base_key(), &omitted);
}

#[test]
fn intent_changes_invalidate_the_digest() {
    let req = base_request();
    let docs = SearchCacheKey::from_request(&req, Some(SearchIntent::Docs), &providers(), 10, 20);
    assert_invalidates("intent", &base_key(), &docs);
}

#[test]
fn freshness_changes_invalidate_the_digest() {
    let mut req = base_request();
    req.freshness = Freshness::Week;
    assert_invalidates("freshness", &base_key(), &key_for(&req));
}

#[test]
fn date_range_changes_invalidate_the_digest() {
    let mut req = base_request();
    req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-02-01"));
    let with_range = key_for(&req);
    assert_invalidates("date range presence", &base_key(), &with_range);

    req.date_range = Some(SearchDateRange::new("2024-01-01", "2024-03-01"));
    assert_invalidates("date range end", &with_range, &key_for(&req));
}

#[test]
fn include_domains_invalidate_the_digest() {
    let mut req = base_request();
    req.include_domains = vec!["docs.rs".to_string(), "crates.io".to_string()];
    assert_invalidates("include domains", &base_key(), &key_for(&req));

    req.include_domains = vec!["crates.io".to_string(), "docs.rs".to_string()];
    let reordered = key_for(&req);
    req.include_domains = vec!["docs.rs".to_string(), "crates.io".to_string()];
    assert_eq!(
        key_for(&req).digest(),
        reordered.digest(),
        "domain order must not change the digest"
    );
}

#[test]
fn exclude_domains_invalidate_the_digest() {
    let mut req = base_request();
    req.exclude_domains = vec!["example.com".to_string(), "spam.example".to_string()];
    assert_invalidates("exclude domains", &base_key(), &key_for(&req));
}

#[test]
fn language_changes_invalidate_the_digest() {
    let mut req = base_request();
    req.language = None;
    assert_invalidates("language presence", &base_key(), &key_for(&req));

    req.language = Some("de".to_string());
    assert_invalidates("language value", &base_key(), &key_for(&req));
}

#[test]
fn region_changes_invalidate_the_digest() {
    let mut req = base_request();
    req.region = Some("GB".to_string());
    assert_invalidates("region", &base_key(), &key_for(&req));
}

#[test]
fn safe_search_changes_invalidate_the_digest() {
    let mut req = base_request();
    req.safe_search = None;
    assert_invalidates("safe search presence", &base_key(), &key_for(&req));

    req.safe_search = Some(SafeSearch::Strict);
    assert_invalidates("safe search value", &base_key(), &key_for(&req));
}

#[test]
fn result_bounds_invalidate_the_digest() {
    let req = base_request();
    let other_effective =
        SearchCacheKey::from_request(&req, Some(SearchIntent::Web), &providers(), 11, 20);
    assert_invalidates("effective max results", &base_key(), &other_effective);

    let other_candidate =
        SearchCacheKey::from_request(&req, Some(SearchIntent::Web), &providers(), 10, 21);
    assert_invalidates("candidate limit", &base_key(), &other_candidate);
}

#[test]
fn excerpt_count_invalidates_the_digest() {
    let mut req = base_request();
    req.excerpt_count = Some(2);
    assert_invalidates("excerpt count", &base_key(), &key_for(&req));
}

#[test]
fn defaults_match_config_defaults() {
    let settings = SearchCacheSettings::default();
    let config = crate::core::config::AppConfig::default();
    assert_eq!(
        settings.ttl,
        Duration::from_secs(config.search.cache_ttl_seconds)
    );
    assert_eq!(settings.max_entries, config.search.cache_max_entries);
}

#[tokio::test]
async fn entries_expire_after_ttl() {
    let cache = SearchCache::new(settings(Duration::from_millis(250), 8));
    let key = base_key();

    let first = cache
        .resolve(key.clone(), || async { complete_fanout(2) })
        .await;
    assert_eq!(first.disposition, CacheDisposition::Miss);
    assert_eq!(cache.stats().entries, 1);

    let second = cache
        .resolve(key.clone(), || async { complete_fanout(2) })
        .await;
    assert_eq!(second.disposition, CacheDisposition::Hit);

    tokio::time::sleep(Duration::from_millis(400)).await;

    let third = cache.resolve(key, || async { complete_fanout(2) }).await;
    assert_eq!(third.disposition, CacheDisposition::Miss);
    assert_eq!(cache.stats().entries, 1);
    assert_eq!(cache.stats().hits, 1);
}

#[tokio::test]
async fn lru_evicts_at_the_entry_cap() {
    let cache = SearchCache::new(settings(Duration::from_secs(60), 2));
    let mut keys = Vec::new();
    for query in ["alpha", "beta", "gamma"] {
        let mut req = base_request();
        req.query = query.to_string();
        keys.push(key_for(&req));
    }

    cache
        .resolve(keys[0].clone(), || async { complete_fanout(1) })
        .await;
    cache
        .resolve(keys[1].clone(), || async { complete_fanout(1) })
        .await;
    assert_eq!(
        cache
            .resolve(keys[0].clone(), || async { complete_fanout(1) })
            .await
            .disposition,
        CacheDisposition::Hit,
        "re-reading key 0 must promote it to most-recently used"
    );

    cache
        .resolve(keys[2].clone(), || async { complete_fanout(1) })
        .await;

    let stats = cache.stats();
    assert_eq!(stats.entries, 2);
    assert_eq!(stats.evictions, 1);
    assert_eq!(
        cache
            .resolve(keys[0].clone(), || async { complete_fanout(1) })
            .await
            .disposition,
        CacheDisposition::Hit
    );
    assert_eq!(
        cache
            .resolve(keys[1].clone(), || async { complete_fanout(1) })
            .await
            .disposition,
        CacheDisposition::Miss,
        "key 1 must have been evicted as least-recently used"
    );
}

#[tokio::test]
async fn entries_above_the_row_cap_are_not_stored() {
    let cache = SearchCache::new(settings(Duration::from_secs(60), 8));
    let key = base_key();

    let oversized = cache
        .resolve(key.clone(), || async {
            complete_fanout(MAX_CACHED_ROWS_PER_ENTRY + 1)
        })
        .await;
    assert_eq!(oversized.disposition, CacheDisposition::Miss);
    assert_eq!(
        oversized.fanout.lists[0].1.len(),
        MAX_CACHED_ROWS_PER_ENTRY + 1
    );

    let stats = cache.stats();
    assert_eq!(stats.entries, 0);
    assert_eq!(stats.bypassed, 1);

    let again = cache.resolve(key, || async { complete_fanout(1) }).await;
    assert_eq!(again.disposition, CacheDisposition::Miss);
    assert_eq!(again.fanout.lists[0].1.len(), 1);
}

#[tokio::test]
async fn entries_at_the_row_cap_are_stored() {
    let cache = SearchCache::new(settings(Duration::from_secs(60), 8));
    let key = base_key();

    cache
        .resolve(key.clone(), || async {
            complete_fanout(MAX_CACHED_ROWS_PER_ENTRY)
        })
        .await;
    assert_eq!(cache.stats().entries, 1);

    let hit = cache.resolve(key, || async { complete_fanout(1) }).await;
    assert_eq!(hit.disposition, CacheDisposition::Hit);
    assert_eq!(hit.fanout.lists[0].1.len(), MAX_CACHED_ROWS_PER_ENTRY);
}

#[tokio::test]
async fn concurrent_identical_calls_share_one_fanout() {
    let cache = Arc::new(SearchCache::new(settings(Duration::from_secs(60), 8)));
    let key = base_key();
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = oneshot::channel::<()>();

    let leader = {
        let cache = Arc::clone(&cache);
        let calls = Arc::clone(&calls);
        let key = key.clone();
        let entered_tx = Arc::new(Mutex::new(Some(entered_tx)));
        tokio::spawn(async move {
            cache
                .resolve(key, move || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    async move {
                        if let Some(tx) = take_entered(&entered_tx) {
                            let _ = tx.send(());
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        complete_fanout(3)
                    }
                })
                .await
        })
    };

    entered_rx.await.expect("leader started");

    let followers: Vec<_> = (0..3)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            let key = key.clone();
            tokio::spawn(async move {
                cache
                    .resolve(key, move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async move { complete_fanout(3) }
                    })
                    .await
            })
        })
        .collect();

    let leader_result = tokio::time::timeout(Duration::from_secs(5), leader)
        .await
        .expect("leader completes")
        .expect("leader joins");
    assert_eq!(leader_result.disposition, CacheDisposition::Miss);

    for follower in followers {
        let result = tokio::time::timeout(Duration::from_secs(5), follower)
            .await
            .expect("follower completes")
            .expect("follower joins");
        assert_eq!(result.disposition, CacheDisposition::Coalesced);
        assert_eq!(
            signature(&result.fanout.lists),
            signature(&leader_result.fanout.lists)
        );
    }

    assert_eq!(calls.load(Ordering::SeqCst), 1, "one fan-out total");
    let stats = cache.stats();
    assert_eq!(stats.misses, 4);
    assert_eq!(stats.coalesced, 3);
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.entries, 1);
}

#[tokio::test]
async fn non_cacheable_leader_does_not_publish_to_waiters() {
    let cache = Arc::new(SearchCache::new(settings(Duration::from_secs(60), 8)));
    let key = base_key();
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = oneshot::channel::<()>();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let release_rx = Arc::new(Mutex::new(Some(release_rx)));

    let leader = {
        let cache = Arc::clone(&cache);
        let calls = Arc::clone(&calls);
        let key = key.clone();
        let entered_tx = Arc::new(Mutex::new(Some(entered_tx)));
        tokio::spawn(async move {
            cache
                .resolve(key, move || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    async move {
                        if let Some(tx) = take_entered(&entered_tx) {
                            let _ = tx.send(());
                        }
                        if let Some(rx) = take_release(&release_rx) {
                            let _ = rx.await;
                        }
                        let mut fanout = complete_fanout(5);
                        fanout.failures.push((
                            "mock_b".to_string(),
                            EngineError::Timeout { engine: "mock_b" },
                        ));
                        fanout
                    }
                })
                .await
        })
    };

    entered_rx.await.expect("leader started");

    let followers: Vec<_> = (0..2)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            let key = key.clone();
            tokio::spawn(async move {
                cache
                    .resolve(key, move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async move { complete_fanout(2) }
                    })
                    .await
            })
        })
        .collect();

    let digest = base_key().digest();
    while cache.inflight_waiters(digest) < 2 {
        tokio::task::yield_now().await;
    }

    let _ = release_tx.send(());

    let leader_result = tokio::time::timeout(Duration::from_secs(5), leader)
        .await
        .expect("leader completes")
        .expect("leader joins");
    assert_eq!(leader_result.disposition, CacheDisposition::Miss);
    assert!(!leader_result.fanout.is_cacheable());

    let expected = signature(&complete_fanout(2).lists);
    for follower in followers {
        let result = tokio::time::timeout(Duration::from_secs(5), follower)
            .await
            .expect("follower completes")
            .expect("follower joins");
        assert!(
            matches!(
                result.disposition,
                CacheDisposition::Miss | CacheDisposition::Coalesced
            ),
            "a partial fan-out must never be served: {:?}",
            result.disposition
        );
        assert_eq!(
            signature(&result.fanout.lists),
            expected,
            "followers must run their own fan-out, not receive the partial one"
        );
    }

    assert!(
        calls.load(Ordering::SeqCst) >= 2,
        "followers must dispatch after a partial leader"
    );
    let stats = cache.stats();
    assert_eq!(stats.entries, 1);
    assert_eq!(stats.bypassed, 1);
    assert_eq!(cache.inflight_entries(), 0);
}

#[tokio::test]
async fn cancelled_leader_releases_followers() {
    let cache = Arc::new(SearchCache::new(settings(Duration::from_secs(60), 8)));
    let key = base_key();
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = oneshot::channel::<()>();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let release_rx = Arc::new(Mutex::new(Some(release_rx)));

    let leader = {
        let cache = Arc::clone(&cache);
        let key = key.clone();
        let entered_tx = Arc::new(Mutex::new(Some(entered_tx)));
        tokio::spawn(async move {
            cache
                .resolve(key, move || async move {
                    if let Some(tx) = take_entered(&entered_tx) {
                        let _ = tx.send(());
                    }
                    if let Some(rx) = take_release(&release_rx) {
                        let _ = rx.await;
                    }
                    complete_fanout(1)
                })
                .await
        })
    };

    entered_rx.await.expect("leader started");

    let followers: Vec<_> = (0..2)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            let key = key.clone();
            tokio::spawn(async move {
                cache
                    .resolve(key, move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async move { complete_fanout(2) }
                    })
                    .await
            })
        })
        .collect();

    let digest = base_key().digest();
    while cache.inflight_waiters(digest) < 2 {
        tokio::task::yield_now().await;
    }

    leader.abort();
    let _ = leader.await;
    drop(release_tx);

    let expected = signature(&complete_fanout(2).lists);
    for follower in followers {
        let result = tokio::time::timeout(Duration::from_secs(5), follower)
            .await
            .expect("follower completes")
            .expect("follower joins");
        assert!(
            matches!(
                result.disposition,
                CacheDisposition::Miss | CacheDisposition::Coalesced
            ),
            "cancelled leader must not serve a value: {:?}",
            result.disposition
        );
        assert_eq!(signature(&result.fanout.lists), expected);
    }

    assert!(calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(cache.inflight_entries(), 0);
    assert_eq!(cache.stats().entries, 1);
}

#[tokio::test]
async fn panicking_leader_does_not_strand_followers() {
    let cache = Arc::new(SearchCache::new(settings(Duration::from_secs(60), 8)));
    let key = base_key();
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = oneshot::channel::<()>();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let release_rx = Arc::new(Mutex::new(Some(release_rx)));

    let leader = {
        let cache = Arc::clone(&cache);
        let key = key.clone();
        let entered_tx = Arc::new(Mutex::new(Some(entered_tx)));
        tokio::spawn(async move {
            cache
                .resolve(key, move || async move {
                    if let Some(tx) = take_entered(&entered_tx) {
                        let _ = tx.send(());
                    }
                    if let Some(rx) = take_release(&release_rx) {
                        let _ = rx.await;
                    }
                    panic!("leader panic");
                })
                .await
        })
    };

    entered_rx.await.expect("leader started");

    let followers: Vec<_> = (0..2)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            let key = key.clone();
            tokio::spawn(async move {
                cache
                    .resolve(key, move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async move { complete_fanout(2) }
                    })
                    .await
            })
        })
        .collect();

    let digest = base_key().digest();
    while cache.inflight_waiters(digest) < 2 {
        tokio::task::yield_now().await;
    }

    let _ = release_tx.send(());
    let leader_join = tokio::time::timeout(Duration::from_secs(5), leader)
        .await
        .expect("leader task returns");
    assert!(leader_join.is_err(), "leader task must panic");

    let expected = signature(&complete_fanout(2).lists);
    for follower in followers {
        let result = tokio::time::timeout(Duration::from_secs(5), follower)
            .await
            .expect("follower completes")
            .expect("follower joins");
        assert!(
            matches!(
                result.disposition,
                CacheDisposition::Miss | CacheDisposition::Coalesced
            ),
            "panicking leader must not strand a waiter: {:?}",
            result.disposition
        );
        assert_eq!(signature(&result.fanout.lists), expected);
    }

    assert!(calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(cache.inflight_entries(), 0);
    assert_eq!(cache.stats().entries, 1);
}

#[tokio::test]
async fn key_collision_with_a_different_struct_is_not_served() {
    let cache = Arc::new(SearchCache::new(settings(Duration::from_secs(60), 8)));
    let key = base_key();
    let mut mutated = base_key();
    mutated.query = "another query".to_string();

    {
        let mut entries = cache.entries.lock().unwrap();
        entries.push(
            key.digest(),
            CachedSearchEntry {
                key: mutated,
                lists: Arc::new(complete_fanout(3).lists),
                stored_at: Instant::now(),
            },
        );
    }

    let resolved = cache
        .resolve(key.clone(), || async { complete_fanout(1) })
        .await;
    assert_eq!(resolved.disposition, CacheDisposition::Miss);
    assert_eq!(
        signature(&resolved.fanout.lists),
        signature(&complete_fanout(1).lists)
    );
    assert_eq!(
        cache
            .resolve(key, || async { complete_fanout(1) })
            .await
            .disposition,
        CacheDisposition::Hit
    );
}

#[tokio::test]
async fn barrier_waiters_all_complete() {
    let cache = Arc::new(SearchCache::new(settings(Duration::from_secs(60), 8)));
    let key = base_key();
    let calls = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(4));

    let tasks: Vec<_> = (0..4)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            let barrier = Arc::clone(&barrier);
            let key = key.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                cache
                    .resolve(key, move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async move {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            complete_fanout(2)
                        }
                    })
                    .await
            })
        })
        .collect();

    for task in tasks {
        let resolved = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("caller completes")
            .expect("caller joins");
        assert_eq!(
            signature(&resolved.fanout.lists),
            signature(&complete_fanout(2).lists)
        );
    }

    assert!(calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(cache.stats().entries, 1);
    assert_eq!(cache.inflight_entries(), 0);
}
