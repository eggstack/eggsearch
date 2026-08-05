# Closure Pass 2 — Cache and Profile Correctness

**Repository:** `eggstack/eggsearch`  
**Roadmap:** `plans/web-fetch-resilience-closure-roadmap.md`  
**Predecessor:** `plans/web-fetch-resilience-closure-pass-1-browser-orchestration.md`  
**Planning baseline:** `2b95328e409e5f19074c1d8e2118fc4a7ce5561d`  
**Status:** Corrective implementation handoff  
**Scope:** Repair raw/derived cache semantics, profile scoping, invalidation, and byte accounting without expanding into an RFC-complete or persistent cache

---

## 1. Objective

Make the existing process-local fetch cache correct enough for HTML, PDFs, HTTP revalidation, and persistent browser profiles.

At the baseline, the cache has the right broad shape—raw and derived tiers, HTTP validators, freshness metadata, anonymous/profile scopes, and bounded LRU storage—but several implementation details violate the intended contract:

- the raw tier stores `resp.raw_text` rather than original response bytes;
- profile scope is built from the display name instead of the opaque profile ID;
- derived keys do not include scope;
- scope invalidation removes raw entries only;
- raw insertion/accounting performs multiple cache mutations and can mishandle replacement or oversized entries;
- a cached PDF cannot be re-extracted for a different page selection without network access.

This pass must fix those defects while preserving a small in-memory implementation.

---

## 2. Fixed Cache Model

### 2.1 Raw tier

A raw entry represents one fetched transport response before content extraction:

```rust
pub struct RawFetchCacheEntry {
    pub requested_url: String,
    pub final_url: String,
    pub status: u16,
    pub headers: CachedHeaders,
    pub body: Arc<[u8]>,
    pub fetched_at: SystemTime,
    pub freshness: CacheFreshness,
    pub validators: CacheValidators,
    pub scope: CacheScope,
    pub content_type: Option<String>,
    pub transport: FetchTransportKind,
}
```

The exact field names may follow existing types. Required properties:

- `body` is the original bounded response body or browser DOM bytes supplied to the extraction layer;
- the body is stored before HTML/PDF extraction;
- no unbounded body is ever retained;
- a truncated body is not represented as complete cacheable content;
- cache eligibility is determined before insertion;
- browser profile cookies and storage are never copied into the entry.

For ordinary HTTP, raw bytes are the bounded HTTP entity bytes after content decoding performed by the HTTP client.

For browser rendering, raw bytes are the bounded serialized DOM passed into the shared HTML extraction path. The cache must mark the transport so browser-derived DOM is not confused with an ordinary HTTP entity representation.

### 2.2 Derived tier

A derived entry represents extraction/sanitization output generated from a raw body:

```rust
pub struct DerivedCacheKey {
    pub scope: CacheScope,
    pub raw_content_hash: u64,
    pub extraction_key: ExtractionCacheKey,
}
```

Required extraction dimensions:

```text
transport/representation kind when relevant
extract mode
max-char budget class
include links
PDF page selection
PDF OCR policy
include media
renderer/extractor version
sanitation mode
```

The scope must be part of the key even when two profiles produce byte-identical content. Isolation is more important than cross-profile deduplication.

### 2.3 Scope

Use:

```rust
pub enum CacheScope {
    Anonymous,
    Profile(ProfileId),
}
```

`ProfileId` may remain a string newtype or existing opaque ID representation.

Rules:

- profile display names never appear in cache keys;
- resolving a profile returns its opaque ID and display name separately;
- deleting/recreating the same display name produces a distinct cache scope;
- anonymous and all profile scopes remain isolated;
- no raw cookie, authorization header, or secret is included in a key.

---

## 3. Preserve Original Bytes Through Fetch Execution

### 3.1 Ordinary HTTP

The current `FetchClient::fetch()` returns an extracted `WebFetchResponse`, which is too late for a correct raw cache write.

Refactor narrowly so the orchestration layer can obtain a bounded transport response before extraction. Recommended direction:

```rust
pub struct RawTransportResponse {
    pub requested_url: String,
    pub final_url: String,
    pub status: u16,
    pub headers: HeaderMap,
    pub content_type: Option<String>,
    pub body: Arc<[u8]>,
    pub transport: FetchTransportKind,
}
```

Then use one extraction function:

```rust
fn extract_transport_response(
    raw: &RawTransportResponse,
    options: &FetchExtractionOptions,
) -> Result<WebFetchResponse, FetchError>
```

Exact placement may differ. The central requirement is that network acquisition and extraction are separable enough to:

1. read fresh raw cache entries;
2. revalidate stale entries;
3. extract with request-specific options;
4. write derived entries;
5. avoid re-downloading the body.

Do not redesign all provider clients. Restrict this separation to `web_fetch`/fetch client behavior.

### 3.2 Browser

Pass 1 should produce browser DOM bytes through the same raw/extraction boundary.

Do not cache:

- screenshots;
- CDP event streams;
- cookies;
- local/session storage;
- downloaded files;
- challenge pages;
- login forms identified as profile attention cases.

### 3.3 PDFs

The raw tier must retain original bounded PDF bytes. This is necessary for:

- different `pdf.pages` selections;
- different `include_media` values;
- future extraction-version changes;
- avoiding another network request when only derived options change.

Encrypted/decrypted PDF handling:

- do not persist decrypted output;
- the in-memory anonymous raw entry may retain original encrypted bytes when otherwise cacheable;
- do not include the password in cache keys or logs;
- safest initial behavior is to bypass raw/derived caching whenever a PDF password is supplied.

---

## 4. Cache Read Flow

Recommended sequence:

```text
resolve scope
build raw key
apply cache policy
read raw entry
    fresh -> derive/read derived entry
    stale + validators -> conditional revalidate
    stale without validators -> network fetch
network/browser fetch -> raw response
extract under request options
write eligible raw and derived entries
return response metadata
```

### 4.1 Fresh raw entry

- Build the derived key from scope, raw hash, and extraction options.
- If the derived entry exists, return it.
- If it does not exist, extract from cached raw bytes and insert the new derived entry.
- Do not treat a missing derived entry as a network miss.

### 4.2 Stale entry and 304

- Send validators through the ordinary HTTP transport only.
- On 304, update freshness/validator metadata according to returned headers and reuse the original cached body.
- Reuse or regenerate the derived entry as needed.
- Do not perform conditional HTTP validation for browser-DOM entries.

### 4.3 Browser-DOM freshness

Browser-DOM cache entries may use operator-configured/default local TTL but do not claim HTTP validator semantics unless the browser layer deliberately captures reliable main-document response validators.

Keep the initial behavior simple:

- no conditional revalidation for browser DOM;
- stale browser DOM triggers a new browser fetch when the selected policy permits;
- profile scope remains mandatory for profile-derived content.

---

## 5. Cache Write and Accounting

### 5.1 Single insertion path

Replace the current multi-step `push`/evict/`put` behavior with one clear insertion operation.

Required algorithm:

1. reject an entry whose body exceeds `raw_max_bytes`;
2. remove an existing value for the same key and subtract its body length;
3. evict LRU entries until the new body fits;
4. insert exactly once;
5. add the new body length exactly once.

The cache must never report `current_raw_bytes > raw_max_bytes` after insertion.

### 5.2 Derived bounds

The current derived tier is bounded by entry count. Keep this unless profiling demonstrates a real memory issue.

A simple estimated-byte cap is acceptable only if it remains small to implement. It is not required for this closure if:

- derived entry count stays conservatively bounded;
- raw bytes remain the dominant bounded resource;
- output text is already bounded by fetch limits.

Do not introduce a custom weighted eviction framework.

### 5.3 Non-cacheable content

Do not cache as successful content:

- `no-store` responses;
- unsupported `Vary` representations;
- challenge pages;
- login/attention pages;
- 401/403/429 and ordinary error bodies;
- policy-blocked results;
- truncated raw bodies;
- password-supplied PDF extraction;
- response bodies whose content type is intentionally excluded;
- browser results marked incomplete or timed out.

`private` content may be cached only in an explicit profile scope. Anonymous scope must not store it.

---

## 6. Profile Removal and Invalidation

### 6.1 Return opaque scope identity

`ProfileManager::remove_profile()` already returns the removed opaque ID. Use that ID to invalidate cache state.

The CLI/state integration should:

1. resolve/remove the profile;
2. receive the opaque ID;
3. call cache invalidation for `CacheScope::Profile(id)`;
4. remove both raw and derived entries;
5. report success without printing internal cache keys.

If the CLI and MCP server run in different processes, process-local cache invalidation applies only to the active process. Document that process-local limitation; do not add IPC or a cache daemon.

### 6.2 Invalidate both tiers

Implement:

```rust
pub async fn invalidate_scope(&self, scope: &CacheScope)
```

It must remove:

- all raw entries whose key scope matches;
- all derived entries whose key scope matches;
- associated byte accounting for raw entries.

Do not attempt content-hash-based partial invalidation.

### 6.3 Recreated display name

Add a deterministic test:

1. create profile `portal`, capture opaque ID A;
2. insert scoped raw and derived entries under A;
3. remove profile and invalidate A;
4. recreate display name `portal`, capture opaque ID B;
5. assert A != B;
6. assert no A content is readable through B.

---

## 7. Cache Policy Semantics

Retain the existing public values:

```text
default
bypass
refresh
```

Clarify implementation:

- `default`: read fresh cache, revalidate stale eligible HTTP entries, otherwise fetch and store;
- `bypass`: do not read raw or derived cache; after a successful fetch, storing may occur unless forbidden;
- `refresh`: skip fresh derived return, revalidate/fetch, then update cache.

Do not add caller-selected TTLs, cache-only mode, stale-while-revalidate, or background refresh in this pass.

---

## 8. Metadata Contract

Return concise, accurate metadata:

```text
cache_status: hit | miss | revalidated | bypassed | not_cacheable
attempt_count
retry_after_ms
origin_backoff_ms
transport
```

`cache_status=hit` is valid for:

- a direct derived hit; or
- extraction performed from a fresh raw cache entry without network access.

If distinguishing these is useful, add an internal trace or one optional bounded field such as `cache_layer=raw|derived`; it is not required for closure.

Never expose:

- profile opaque IDs to ordinary MCP callers unless already part of an explicitly documented local diagnostic;
- cache keys;
- cache paths;
- validators containing sensitive custom headers;
- body hashes.

---

## 9. Expected Files

Primary changes:

```text
src/fetch/cache.rs
src/fetch/client.rs
src/fetch/types.rs
src/mcp/tools.rs
src/mcp/state.rs
src/fetch/browser/navigate.rs
src/fetch/browser/profiles.rs
src/commands/browser_profiles.rs
```

Possible supporting changes:

```text
src/core/fetch.rs
src/core/config.rs
src/fetch/mod.rs
```

Do not add SQLite or another cache crate.

---

## 10. Focused Verification

Use deterministic fixtures only.

Required tests:

1. raw cache stores original HTML bytes, not extracted text.
2. raw cache stores original PDF bytes.
3. fresh raw HTML entry can produce a new extraction mode without network access.
4. fresh raw PDF entry can produce a different page selection without network access.
5. derived keys differ across anonymous and profile scopes.
6. profile display name is not used as the scope key.
7. two profiles with identical response bytes do not share derived entries.
8. profile removal invalidates both raw and derived tiers.
9. recreating the same display name cannot access the removed profile scope.
10. replacement insertion updates byte accounting correctly.
11. one entry larger than `raw_max_bytes` is rejected and not counted.
12. LRU eviction leaves total bytes within the cap.
13. `no-store` and unsupported `Vary` remain non-cacheable.
14. anonymous `private` responses are not cached.
15. profile-scoped `private` content may be cached without crossing scopes.
16. 304 revalidation preserves body and refreshes metadata.
17. browser-DOM entries do not attempt HTTP conditional revalidation.
18. password-supplied PDFs bypass cache.
19. challenge/login pages are not inserted.
20. `bypass` and `refresh` semantics match documentation.

Recommended commands:

```bash
cargo test --locked --all-features cache
cargo test --locked --all-features --test integration web_fetch
cargo test --locked --features browser --test browser_profiles
make check
```

Use repository-appropriate filters. Do not add live public-site tests, a SQLite suite, or a cache CI matrix.

---

## 11. Acceptance Criteria

- [ ] Raw entries store original bounded transport bytes or bounded browser DOM bytes.
- [ ] Raw and derived extraction are separable.
- [ ] Cached HTML can be re-extracted under different options without network access.
- [ ] Cached PDF bytes can be re-extracted for a different page selection without network access.
- [ ] Derived keys include anonymous/profile scope.
- [ ] Profile cache scope uses opaque ID, not display name.
- [ ] Profile removal invalidates both cache tiers for the opaque scope.
- [ ] Recreating a display name cannot expose removed-profile cache content.
- [ ] Raw insertion mutates the LRU once and maintains exact byte accounting.
- [ ] Entries larger than the total byte cap are rejected.
- [ ] `no-store`, unsupported `Vary`, anonymous `private`, challenge, error, truncated, and password-PDF results are not cached incorrectly.
- [ ] 304 handling reuses the original body and updates freshness.
- [ ] Browser-DOM entries do not claim unsupported conditional HTTP semantics.
- [ ] No persistent/distributed cache, background worker, or new cache framework was added.
- [ ] Focused tests pass.
- [ ] `make check` passes.
