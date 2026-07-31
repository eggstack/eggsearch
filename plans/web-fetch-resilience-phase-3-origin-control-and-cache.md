# Phase 3 — Origin Control and Cache

**Repository:** `eggstack/eggsearch`  
**Roadmap:** `plans/web-fetch-pdf-and-browser-resilience-roadmap.md`  
**Predecessor:** `plans/web-fetch-resilience-phase-1-pdf-quality-and-navigation.md`  
**Status:** Implementation handoff  
**Scope:** Per-origin concurrency, bounded retry/backoff, conditional revalidation, and small raw/derived caches

---

## 1. Objective

Add the minimum fetch resilience needed before browser fallback is introduced.

This phase should make repeated local-agent use more respectful of remote origins and more efficient by adding:

1. per-origin request concurrency limits;
2. explicit handling of `Retry-After`;
3. bounded exponential backoff with jitter for genuinely retryable failures;
4. a short-lived per-origin circuit breaker;
5. HTTP conditional revalidation using `ETag` and `Last-Modified`;
6. separation between cached raw response bytes and cached derived extraction output;
7. an in-memory cache by default;
8. optional SQLite persistence only after the cache contract is proven.

This is not a general-purpose HTTP cache, distributed scheduler, crawler frontier, or traffic-evasion system.

---

## 2. Fixed Decisions

### 2.1 Origin state is process-local

Eggsearch is primarily a local single-operator service. Origin backoff and concurrency state should remain in-process.

Do not introduce:

- Redis;
- distributed locks;
- cross-host coordination;
- a background queue service;
- a separate daemon;
- a scheduler framework.

### 2.2 HTTP semantics take priority

Honor server-provided cache and retry information where practical:

```text
Retry-After
Cache-Control
Expires
ETag
Last-Modified
Vary
no-store
private
```

Operator defaults may fill gaps but must not override explicit `no-store` or leak session-specific content.

### 2.3 Retries are narrow

Automatic retries may apply to:

- connection reset before a response;
- transient DNS/connection failures when the request deadline permits;
- selected 502/503/504 responses;
- 429 only after honoring `Retry-After`, and only if the remaining request deadline permits.

Do not retry automatically:

- 400;
- 401;
- 403;
- 404;
- interactive browser challenges;
- invalid redirects;
- private-network policy failures;
- oversized responses;
- unsupported content types;
- PDF parse errors;
- prompt-injection warnings.

Maximum automatic attempts should remain small: one initial attempt plus at most one or two retries.

### 2.4 Browser escalation must not be a retry strategy

Phase 4 will introduce system Chrome. This phase must define origin state and response classification so that Phase 4 cannot repeatedly alternate between HTTP and browser.

One logical request will have a shared attempt budget. Browser escalation is a transport change, not a reset of retry counters.

### 2.5 Cache content must be partitioned by trust/session scope

Never mix:

- anonymous HTTP content;
- authenticated HTTP content;
- one browser profile's content;
- another browser profile's content;
- live content and an archive source if archive support is later added.

Use an explicit cache scope identifier. The public/anonymous scope may be shared within the local process. Profile-scoped content must include a non-secret profile identifier in the key.

### 2.6 Start with memory; persist only optionally

Implement the cache contract and bounded memory implementation first. Add SQLite behind `cache-sqlite` only after tests prove keying, expiry, and revalidation.

Do not make SQLite a default dependency.

---

## 3. Origin State Model

### 3.1 Add a small origin controller

Recommended shape:

```rust
pub struct OriginController {
    states: DashMap<OriginKey, Arc<OriginState>>,
    defaults: OriginPolicy,
}

pub struct OriginState {
    semaphore: Semaphore,
    failures: Mutex<FailureState>,
}

pub struct FailureState {
    consecutive_retryable_failures: u8,
    next_allowed_at: Option<Instant>,
    circuit_open_until: Option<Instant>,
    last_failure_class: Option<OriginFailureClass>,
}
```

Use existing synchronization dependencies where possible. If `DashMap` would be a new dependency, a `tokio::sync::Mutex<HashMap<...>>` is sufficient at Eggsearch scale.

### 3.2 Define origin keys consistently

An origin key should include:

```text
scheme
lowercased host
explicit/effective port
```

Do not key only by hostname. Preserve normal URL semantics.

For browser profile state, origin state remains origin-wide, while cache scope distinguishes profiles. A rate-limited origin should not be hammered independently by multiple profiles.

### 3.3 Bound origin-state memory

Avoid an unbounded map for one-off hosts.

Use one simple strategy:

- maximum origin entries with oldest-idle eviction; or
- periodic opportunistic cleanup during insertion; or
- an existing bounded cache crate only if already present.

Do not add a background maintenance task solely for this map unless necessary.

### 3.4 Concurrency defaults

Suggested initial defaults:

```text
HTTP in-flight per origin: 2
browser in-flight per origin: 1
maximum total browser in-flight: 1 or 2
```

This phase implements HTTP origin concurrency and exposes the hook Phase 4 will use for browser concurrency.

Keep the settings configurable but capped. Do not expose arbitrary unbounded values.

---

## 4. Retry and Backoff

### 4.1 Parse `Retry-After`

Support:

- delta seconds;
- HTTP date when parseable.

Clamp the result to a configured maximum. If the requested wait exceeds the remaining request deadline, return a rate-limited result without sleeping past the deadline.

Recommended metadata:

```text
retry_after_ms
retry_scheduled
attempt_count
origin_backoff_active
```

### 4.2 Use bounded exponential backoff with full jitter

Recommended calculation:

```text
cap = min(max_backoff, base * 2^failure_count)
delay = random(0..cap)
```

Keep constants simple and centralized. Example defaults:

```text
base: 250 ms
max automatic delay: 4 s
circuit threshold: 3 consecutive retryable failures
circuit duration: 30-120 s
```

These values are starting points, not protocol requirements.

Use a deterministic injectable jitter source in unit tests. Do not test statistical distributions.

### 4.3 Circuit-breaker behavior

Open the circuit only for repeated origin-level transient or rate-limit failures. Do not open it for caller mistakes or content extraction failures.

When open:

- fail quickly with a structured origin-backoff result;
- include the next allowed time or remaining duration;
- allow cache hits to be served if valid;
- allow an explicit cache-only response if such a mode exists;
- do not attempt browser escalation.

A successful network response resets consecutive failure state.

### 4.4 Preserve request deadline

The logical request deadline covers:

```text
origin semaphore wait
backoff sleep
DNS resolution
connection
redirects
body read
optional browser escalation in Phase 4
extraction
```

Do not create a fresh full timeout for every retry. Pass a remaining-time budget through orchestration.

If a major refactor is needed to propagate deadlines, keep it local to `fetch` and transport orchestration. Do not redesign all search providers in this phase.

---

## 5. Cache Contract

### 5.1 Separate raw and derived caches

Raw cache entry:

```rust
pub struct RawFetchCacheEntry {
    pub final_url: String,
    pub status: u16,
    pub headers: CachedHeaders,
    pub body: Arc<[u8]>,
    pub fetched_at: SystemTime,
    pub freshness: CacheFreshness,
    pub validators: CacheValidators,
    pub scope: CacheScope,
}
```

Derived cache entry:

```rust
pub struct DerivedDocumentCacheEntry {
    pub raw_content_hash: String,
    pub extraction_key: ExtractionCacheKey,
    pub response: CachedExtractedDocument,
    pub created_at: SystemTime,
}
```

The raw cache allows a PDF to be re-extracted for a different page range without redownloading it. The derived cache avoids repeating HTML/PDF/OCR work for identical options.

### 5.2 Key raw responses correctly

Raw key should include:

```text
normalized requested URL
cache scope
representation-relevant request headers under Eggsearch control
```

Do not include secrets directly. Hash any bounded header subset needed for authenticated representations.

At minimum distinguish:

```text
anonymous HTTP
named browser profile
explicit authenticated HTTP scope if custom authentication is later supported
```

### 5.3 Key derived output by extraction semantics

Derived key should include:

```text
raw body/content hash
extract mode
max chars or normalized output budget class
include links
PDF page selection
PDF OCR policy
include media
renderer/extractor version
sanitation mode if it changes serialized output
```

Do not key derived output only by URL.

### 5.4 Honor cache directives

Required minimum behavior:

- `no-store`: do not persist raw or derived content;
- `max-age`: calculate freshness;
- `Expires`: use when `max-age` is absent;
- `ETag`: send `If-None-Match` on stale revalidation;
- `Last-Modified`: send `If-Modified-Since` when no stronger validator exists;
- 304: refresh metadata and reuse cached body;
- `private`: anonymous local-process memory cache may store it only if scope is private and non-shared; persistent storage should default to disabled for it;
- `Vary`: support a bounded known subset or mark the response non-cacheable when unsupported headers affect representation.

Do not attempt RFC-complete shared-cache semantics.

### 5.5 Do not cache unusable responses as content

Do not store as successful content:

- CAPTCHA pages;
- interactive challenge pages;
- authentication failures;
- ordinary 403/429 errors;
- truncated raw bodies represented as complete;
- invalid redirects;
- policy-blocked results;
- decrypted PDF output by default.

A short negative/suppression record may live in origin state, not the content cache.

### 5.6 Memory bounds

Recommended configurable bounds:

```text
maximum raw entries
maximum raw total bytes
maximum derived entries
maximum derived total characters/estimated bytes
maximum entry body size already constrained by fetch max_bytes
```

Use simple least-recently-used or oldest-access eviction. Avoid a custom complex eviction policy.

### 5.7 Optional SQLite backend

After the memory cache contract is stable, add `cache-sqlite` if implementation remains small.

Requirements:

- operator-selected path;
- WAL mode;
- busy timeout;
- schema version integer;
- bounded rows and/or total approximate bytes;
- oldest-access eviction;
- no secrets or raw cookies;
- cache scope stored explicitly;
- no decrypted PDF persistence by default;
- graceful corruption/unavailable fallback to memory or no-cache mode;
- no migration framework beyond a small explicit schema version handler.

If SQLite adds disproportionate code, leave it for a follow-up. Memory caching and conditional requests are the core phase requirement.

---

## 6. Request and Response Integration

### 6.1 Add cache policy argument

Recommended public enum:

```rust
pub enum FetchCachePolicy {
    Default,
    Bypass,
    Refresh,
}
```

Semantics:

- `default`: use fresh cache, otherwise revalidate/fetch;
- `bypass`: do not read cache; storage may still occur unless response forbids it;
- `refresh`: revalidate or fetch even when locally fresh, then update cache.

Do not add a caller-controlled arbitrary TTL in the initial public MCP schema. Keep operator policy in config.

### 6.2 Expose concise metadata

Recommended response metadata:

```text
cache_status: hit | miss | revalidated | bypassed | not_cacheable
attempt_count
retry_after_ms
origin_backoff_ms
```

Do not expose internal cache database paths or keys.

### 6.3 Batch fetch integration

`batch_fetch` should share the same origin controller and cache.

It must not bypass per-origin limits merely because batch concurrency allows more items. Existing batch global concurrency remains a separate outer limit.

Do not redesign batch response schemas beyond adding the same per-item metadata.

---

## 7. Configuration

Recommended settings:

```toml
[fetch]
retry_max_attempts = 2
retry_base_delay_ms = 250
retry_max_delay_ms = 4000
origin_http_concurrency = 2
origin_browser_concurrency = 1
origin_circuit_failure_threshold = 3
origin_circuit_duration_ms = 60000

[fetch.cache]
enabled = true
memory_max_entries = 256
memory_max_bytes = 67108864
default_ttl_seconds = 900
persistent_enabled = false
persistent_path = ""
persistent_max_entries = 10000
```

Use repository conventions and validation caps. Defaults should be conservative.

Cache may be enabled in memory by default if the implementation is demonstrably small and does not alter correctness. Persistent cache remains opt-in.

---

## 8. Non-Goals

Do not implement:

- distributed rate limiting;
- global crawl delay enforcement from robots.txt;
- proxy rotation;
- IP rotation;
- automatic user-agent rotation;
- background prefetching;
- link crawling;
- stale-while-revalidate background workers;
- distributed or remote cache;
- content deduplication across unrelated URLs beyond hash-based derived reuse if trivial;
- browser transport;
- browser cookies;
- archive.org fallback;
- cache compression unless profiling shows it is needed;
- cache encryption framework;
- cache metrics database;
- CI live rate-limit testing.

---

## 9. Focused Verification

### 9.1 Deterministic HTTP fixtures

Use `httpmock` or the existing local fixture style for:

```text
200 with max-age
stale ETag response followed by 304
Last-Modified revalidation
no-store
429 with delta Retry-After
503 then success
403 no retry
slow response exceeding logical deadline
same-origin concurrent requests
batch items sharing an origin
```

Do not use public sites.

### 9.2 Required tests

- origin key normalization;
- per-origin semaphore limits;
- retry classification;
- attempt cap;
- `Retry-After` parsing and clamping;
- remaining-deadline behavior;
- circuit opens and resets;
- cache directive parsing;
- fresh hit;
- stale revalidation/304;
- `no-store` exclusion;
- raw and derived key separation;
- PDF page-range derived cache distinction;
- profile scope partitioning contract for Phase 4;
- memory eviction bounds;
- batch fetch obeys origin limits;
- SQLite unavailable fallback if SQLite is implemented.

Avoid timing-fragile sleeps. Use paused Tokio time or injected clocks where practical, but do not create a generic time framework across the repository.

### 9.3 Commands

Use focused tests during development, then:

```bash
make check
```

If SQLite is implemented:

```bash
cargo check --locked --features cache-sqlite
cargo test --locked --features cache-sqlite --test fetch_cache
```

Do not add a separate CI job or matrix for SQLite.

---

## 10. Documentation Updates

Document:

- retryable versus non-retryable failures;
- per-origin concurrency;
- `Retry-After` behavior;
- cache modes;
- memory versus optional persistent cache;
- session-scope partitioning;
- fields that prevent caching;
- interaction with PDF page extraction;
- absence of proxy/IP rotation.

Keep documentation operational and concise. Do not add a release-evidence process.

---

## 11. Acceptance Criteria

- [ ] HTTP fetches use a shared bounded per-origin controller.
- [ ] Batch fetch cannot exceed per-origin concurrency through outer batch parallelism.
- [ ] Retry classification is narrow and documented.
- [ ] One logical request has a bounded total attempt count and deadline.
- [ ] `Retry-After` is honored and clamped.
- [ ] Repeated retryable failures open a short origin circuit.
- [ ] Cache hits remain available while an origin circuit is open.
- [ ] Raw bytes and derived documents use separate cache entries.
- [ ] Derived PDF cache keys distinguish page selection and OCR policy.
- [ ] Cache scope prevents anonymous/profile content mixing.
- [ ] `no-store`, ETag, Last-Modified, and 304 behavior are implemented.
- [ ] Challenge/error pages are not stored as successful content.
- [ ] Memory cache is bounded.
- [ ] SQLite, if implemented, remains optional and bounded.
- [ ] No distributed system, background crawler, or proxy mechanism is added.
- [ ] No new CI workflow or external live test is added.
- [ ] `make check` passes.

---

## 12. Handoff Notes

Prioritize correct origin gating, conditional revalidation, and cache key separation. Persistent SQLite is secondary; it may be deferred if it materially expands the implementation. Phase 4 needs a stable origin/cache contract more than it needs a database.