# M002 — Search-Result Cache and Request Coalescing

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M002
Primary class: infrastructure
Baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71`
References: `architecture/meta.md` § Adapter execution paths; `architecture/fetch.md` § 3 (two-tier cache as the precedent); `src/meta/adapter/web.rs`; `src/core/identity.rs`; `src/core/config.rs`.
Dependencies: none hard. M001 is soft: a cache must never store a partial fan-out, and M001 makes partial-versus-empty classification truthful.

## Objective

Stop repeated identical searches from re-hitting providers, and make concurrent identical searches share one fan-out, without changing ranking, payloads, or trust handling.

Local-model evidence (standalone MCP local-inference M003) shows the same task issuing repeated and malformed-then-retried search calls in a single turn. Each retry currently re-dispatches five providers or more, which is the main avoidable rate-limit and latency cost.

## Non-goals

- No caching for domain workflows (`repo_search`, `research_search`, `security_search`). Their subquery sets and freshness semantics differ; a follow-up milestone may extend the cache after web search proves the design.
- No per-call cache controls on `web_search`; no schema change.
- No negative caching, no cross-request single-flight for fetch (that already has its own cache).
- No change to RRF, truncation, or candidate pool sizing.

## Invariants that cannot regress

- Cache hits return byte-identical payloads to a fresh call for the same key, including provider attribution, trust markers, warnings, and `retrieval_summary`.
- Partial or deadline-interrupted fan-outs are never cached. A call with any provider failure, timeout, or deadline interruption bypasses the cache and is not stored.
- Sanitization and trust framing are applied per call, not cached. The cache stores the raw pre-conversion provider lists, mirroring the fetch cache's raw-versus-derived split.
- Provider health recording still happens on real executions only; a cache hit must not fabricate health evidence.
- `req.providers` selection and explicit-provider rejection semantics are unchanged.
- Bound every dimension: entry count, per-entry rows, and TTL.

## Expected production changes

1. **`src/meta/search_cache.rs`.** Bounded LRU (`lru` is already a dependency) keyed by an FNV-1a digest over a canonical struct: normalized query fingerprint, resolved provider list in sorted order, intent, freshness, date range, include/exclude domains, language, region, safe search, candidate limit, and a `cache_schema_version` constant. Use the `eggsearch-id-v1\0`-style versioned prefix convention from `src/core/identity.rs`.
2. **Coalescing.** An in-flight map keyed by the same digest: the leader executes the fan-out and publishes to `oneshot` waiters; followers await. If the leader is cancelled or panics, its senders drop, followers receive a receive error, and each follower falls back to executing its own fan-out. Never leave a waiter blocked.
3. **Adapter integration.** Wrap the fan-out in `MetadataWebSearch::web_search` only. Record health, attempts, and latencies as today. Store only when `raw_failures` is empty and no provider was deadline-interrupted.
4. **Configuration.** `[search] cache_ttl_seconds` (default 300, `0` disables) and `[search] cache_max_entries` (default 128), validated with explicit bounds. Document that the TTL is a freshness bound, not a correctness guarantee.
5. **Diagnostics.** `provider_status` gains a diagnostic-only `search_cache { entries, hits, misses, coalesced, bypassed, evictions }` block.
6. **Lifetime note.** A follower inherits the leader's deadline. Document this: two identical simultaneous calls share one budget, which is intended.

## Storage and compatibility effects

- Process-local in-memory state only; no disk, no migration. Restart clears it.
- Optional diagnostic field is additive; compact and standard projections omit it.
- No change to `WebSearchResponse` fields consumed by existing hosts.

## Ordered work packages

1. Implement the key struct, canonical serialization, and digest with unit tests for stability and for every field that must invalidate the key.
2. Implement the bounded store and TTL with eviction and saturation tests.
3. Implement coalescing with leader-cancellation and leader-panic tests.
4. Integrate into `web_search`; assert the "never cache partial" rule with a fake adapter that returns one failure.
5. Add config fields, validation, and defaults; extend `tests/config_validation.rs`.
6. Add diagnostics and projection tests.
7. Update `architecture/meta.md` (adapter path), `architecture/config.md`, `docs/config.md`, and `docs/test-inventory.md`.

## Verification

- `cargo test --locked --features mock --test web_search_integration`
- `cargo test --locked --features mock --test mcp_tools --test config_validation`
- `cargo test --locked --all-features --test dispatch_fault_injection --test retrieval_attempt_ledger --test provider_routing`
- `make check`; `make docs-check`

## Acceptance and stop conditions

- Two identical sequential calls within the TTL produce one provider fan-out and identical payloads.
- N concurrent identical calls produce one fan-out; all callers receive identical payloads; a cancelled leader does not strand followers.
- A fan-out containing any provider failure is neither stored nor served.
- Stop if the design requires per-call schema changes, disk persistence, or caching across differing provider sets.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/002-status.md` records the candidate SHA, fan-out counts for hit/miss/coalesce scenarios, the partial-bypass test, eviction and TTL tests, and confirmation that health recording and the attempt ledger are unchanged on cache hits.
