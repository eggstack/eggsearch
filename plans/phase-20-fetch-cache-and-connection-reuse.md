# Phase 20 — Fetch Cache Sharing and Connection Reuse

Status: implemented
Depends on: phase 19 benchmark conventions preferred; phases 17-18 implemented
Baseline for planning: `205ab26fb03c6769035a1c05bb9b1f41c2a9ead1` (`main`)
Governing roadmap: `performance-optimization-roadmap.md`

## Why this phase exists

The Phase 17 migration correctly consolidated eggsearch-owned HTTP behind `eggfetch-core 0.1.7`, but one timeout convenience path still rebuilds the transport client.

`FetchClient::with_timeout_ms` constructs a new `eggfetch_core::Client` even though eggfetch clients are cloneable shared handles and individual requests already set request-level timeouts. This loses connection-pool reuse for a configuration change that should only affect request limits.

`batch_fetch` can invoke that constructor once per web item when a timeout override is supplied, multiplying the cost and isolating item connection pools.

The derived document cache has a separate copy problem: cache hits clone the complete cached extracted document while the LRU mutex is held, and response construction clones much of the same content again. Raw body storage already uses shared Arc bytes; the derived tier should receive a compatible internal shared path.

## Objective

Preserve existing fetch/cache semantics while eliminating timeout-only transport reconstruction and one full derived-cache deep copy from eggsearch's internal hot path.

The desired end state is:

- timeout overrides retain the existing shared eggfetch connection pool;
- request and DNS timeout semantics remain exactly equivalent;
- batch fetch computes a timeout-adjusted fetch handle at most once per batch rather than per item;
- derived-cache entries are internally shareable with Arc;
- existing public owned-return cache APIs remain source-compatible;
- cache locks are held for LRU bookkeeping and Arc cloning, not large document copying;
- correctness is demonstrated with deterministic loopback tests and targeted benchmarks.

## Work item 1 — Lock current timeout semantics with tests

Before changing `with_timeout_ms`, add focused tests around the behavior it is required to preserve.

Cover:

- zero/invalid timeout remains rejected at the MCP argument layer;
- timeout override changes `FetchLimits.timeout_ms` semantics for DNS validation;
- request-level eggfetch timeout remains derived from the override;
- timeout errors still report the effective timeout value;
- redirects continue to use the same effective timeout and existing per-hop authorization;
- `fetch_conditional` observes the same override semantics as normal `fetch`.

Use deterministic loopback/stalled-response fixtures. Do not use public network endpoints.

## Work item 2 — Make timeout-only FetchClient clones share the eggfetch client

Refactor `FetchClient::with_timeout_ms` so it does not call `Client::builder()` solely to change timeout.

The implementation should conceptually:

- clone `self.client`;
- clone `self.limits`;
- replace only `limits.timeout_ms`;
- preserve `user_agent` and `sanitize_output`;
- rely on the existing per-request `.timeout(client_timeout(self.limits.timeout_ms))` call for request phases.

This is valid only if the current eggfetch request-level timeout override continues to override the client-level timeout fields used by eggsearch. Add a focused regression test against the current eggfetch contract rather than relying on a comment.

Do not change the public method signature.

Do not move DNS/SSRF policy into eggfetch. `validate_fetch_target_with_resolved_addrs` remains eggsearch-owned and must continue deriving its timeout from the effective `FetchLimits`.

## Work item 3 — Build the timeout-adjusted batch client once

In `run_batch_fetch` or the closest stable setup seam:

- resolve the shared base `FetchClient`;
- if `args.timeout_ms` is present, construct one timeout-adjusted `Arc<FetchClient>`;
- pass that Arc to all web-item futures;
- remove per-item calls to `with_timeout_ms`.

Repo fetch items should remain unchanged unless they independently use the same fetch-client path.

Do not change wave scheduling or aggregate character budgeting in this phase. Continuous replenishment is explicitly out of scope because the current wave boundary participates in deterministic budget allocation.

The existing semaphore may be removed only if a correctness review proves the wave itself already provides the same concurrency bound in every branch. Treat that as optional cleanup, not a phase requirement.

## Work item 4 — Store derived cache values behind shared ownership internally

Change the internal derived LRU value representation so a cache hit can clone an Arc rather than recursively clone the extracted document while holding the mutex.

A suitable internal shape is:

~~~text
LruCache<DerivedCacheKey, Arc<DerivedDocumentCacheEntry>>
~~~

or equivalent.

Requirements:

- byte accounting continues to measure the document payload, not Arc pointer size;
- eviction accounting remains exact;
- scope invalidation semantics remain unchanged;
- LRU recency behavior remains unchanged;
- cache entries are immutable after insertion;
- oversized-entry rejection remains unchanged.

Retain the existing public:

~~~text
get_derived(&DerivedCacheKey) -> Option<DerivedDocumentCacheEntry>
~~~

as a compatibility wrapper if it is public API. Add a `pub(crate)` shared getter for eggsearch's internal hot path.

The compatibility wrapper may intentionally clone the underlying entry. The optimization target is eggsearch's own fetch path.

## Work item 5 — Remove the duplicate derived-cache copy from cache-hit response construction

Update web/batch fetch cache-hit code to use the shared derived entry.

The current shape is effectively:

~~~text
LRU value --deep clone--> local entry --field clones--> WebFetchResponse/JSON
~~~

After this phase it should be:

~~~text
LRU Arc --cheap Arc clone--> shared entry --one required output copy--> response
~~~

Do not introduce borrowed response fields or lifetime-heavy public APIs merely to eliminate the final output ownership copy. The MCP response must still own its serialized content.

Where a JSON envelope consumes owned values directly, prefer moving values from a newly-created owned response over cloning them again.

## Work item 6 — Add connection-reuse evidence

Add a deterministic loopback test or characterization harness demonstrating that timeout-adjusted fetches can reuse the same shared transport client.

Preferred evidence is an HTTP/1.1 keep-alive loopback server that counts accepted TCP connections for sequential compatible requests. If platform/runtime behavior makes exact connection count flaky, use a lower-level deterministic structural test plus a targeted benchmark and document the limitation.

The test must not depend on external DNS or public services.

Also add a static/regression assertion preventing `with_timeout_ms` from reintroducing `Client::builder()` without an explicit reason.

## Work item 7 — Benchmark cache-hit and timeout-adjusted paths

Extend `benches/perf.rs` or an appropriate benchmark target with deterministic cases for:

- derived-cache shared hit of a small document;
- derived-cache shared hit of approximately 12K characters;
- derived-cache shared hit near the configured large-response cap;
- construction of timeout-adjusted `FetchClient` handles;
- batch setup with a timeout override for representative item counts.

Do not benchmark real network latency as the primary evidence. The targeted concern is local allocation/client construction overhead.

If the benchmark harness cannot call internal shared getters without widening public API, place a small benchmarkable helper behind an existing public/internal module boundary rather than exporting new API solely for Criterion.

## Work item 8 — Leave cache stats invariant scans alone unless evidence justifies change

`FetchCache::stats()` currently locks both tiers and recomputes byte sums to detect/self-heal counter drift.

This is O(cache size), but it is also an integrity check and may be called infrequently.

Do not optimize it speculatively.

If profiling shows `stats()` on a request hot path:

- separate a cheap counter-based snapshot from a debug/invariant verification path;
- preserve a way for tests/diagnostics to validate exact accounting;
- do not silently remove drift detection.

Otherwise record it as reviewed/deferred.

## Correctness and security invariants

Preserve:

- URL validation and SSRF policy;
- resolved-address pinning;
- redirect limits and redirect revalidation;
- request/body total timeout behavior;
- DNS timeout derivation;
- body byte caps and extraction character caps;
- cache scope and privacy boundaries;
- cache freshness/revalidation/304 semantics;
- OriginController retry/backoff ownership;
- sanitization/trust markers;
- browser escalation behavior;
- Phase 18 compression workaround boundaries.

## Required tests

At minimum:

1. timeout-adjusted `FetchClient` preserves effective timeout behavior;
2. normal and conditional fetch both use the override;
3. timeout adjustment does not rebuild transport state solely for the override;
4. batch timeout override is constructed once and shared across item futures;
5. derived shared getter updates LRU recency correctly;
6. public owned `get_derived` behavior remains intact;
7. byte accounting remains exact after replacement, capacity eviction, byte-pressure eviction, and scope invalidation;
8. oversized derived entries remain unstored;
9. cache-hit output equals the pre-change response contract;
10. deterministic loopback coverage exercises sequential compatible fetches and pool reuse where reliable.

## Non-goals

Phase 20 does not:

- change cache policy;
- increase cache limits;
- introduce persistent/disk caching;
- change OriginController retry behavior;
- add request singleflight/coalescing;
- redesign batch scheduling;
- change public fetch response types;
- remove the Phase 18 identity-encoding workaround;
- enable new eggfetch features.

## Suggested execution order

~~~text
1. add timeout semantics and cache compatibility tests
2. add baseline microbenchmarks
3. refactor with_timeout_ms to clone shared eggfetch client
4. move batch timeout adjustment outside item futures
5. change internal derived LRU values to Arc
6. add internal shared cache getter
7. update web/batch cache-hit paths
8. run loopback connection-reuse evidence
9. run make check and targeted benchmarks
10. record measured/deferred cache-stat decision
~~~

## Acceptance criteria

Phase 20 is complete only when:

1. `with_timeout_ms` no longer builds a new eggfetch client solely for timeout changes.
2. timeout-adjusted clients retain the shared eggfetch connection pool.
3. effective DNS/request/error timeout semantics are unchanged.
4. `batch_fetch` creates at most one timeout-adjusted fetch handle per batch.
5. internal derived cache hits clone shared ownership rather than deep-copying the complete cached document under the LRU mutex.
6. the public owned derived-cache API remains compatible.
7. cache byte accounting, eviction, invalidation, and freshness behavior remain exact.
8. cache-hit MCP responses remain contract-equivalent.
9. deterministic connection-reuse evidence or an explicitly documented structural substitute exists.
10. targeted before/after benchmarks are recorded.
11. no new transport/cache dependency is introduced.
12. `make check` passes on the exact implementation candidate.
13. Phase 18 transport/compression safeguards remain intact.

## Handoff notes

The primary optimization is not changing eggfetch itself. Eggfetch already provides a cloneable shared client and request-level timeouts. The downstream issue is that eggsearch currently discards that sharing in its timeout convenience method.

Likewise, do not pursue a fully borrowed cache-response architecture. One output copy into an owned MCP response is acceptable. The goal is to remove the extra full copy performed simply to get an LRU hit out from under the mutex.

## Implementation record

Implemented in performance candidate `5a8822ba538e89f9b8f441a328925fab78300754`. Timeout-only `FetchClient` clones now reuse the eggfetch client, batch web fetches share one adjusted handle, and derived cache values are stored/read internally through `Arc` while the public owned getter remains compatible. Exact byte accounting, eviction, invalidation, cache-hit response tests, and a static guard against transport reconstruction remain in place. Cache stats were reviewed and deliberately left as an integrity-checking O(cache-size) path because they are not on the request hot path. The existing loopback fetch suites and cache tests passed; no new transport or cache dependency was introduced.
