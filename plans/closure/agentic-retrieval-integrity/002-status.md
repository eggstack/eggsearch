# M002 Search-Result Cache and Request Coalescing — Closure Status

Status: closing (closure commit outstanding)

Source plan: `plans/implementation/agentic-retrieval-integrity/002-search-result-cache-and-coalescing.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71` (origin/main, 2026-11-10 UTC)

Implementation candidate: uncommitted worktree on the planning baseline. Closure commit outstanding.

## Result

`src/meta/search_cache.rs` adds a process-local bounded LRU plus a single-flight coalescer for
`web_search` only. Both the three-argument `web_search` and `web_search_with_intent` share the
wrapper, so an explicit and an omitted intent both benefit.

Key: FNV-1a over an `eggsearch-search-cache-v1` domain prefix and a length-prefixed tagged encoding
of the schema version, normalized query, sorted provider list, caller intent (`Option`, so omitted
and explicit `web` differ), freshness, exact date range, include/exclude domains (sorted),
language, region, safe search, effective max results, candidate limit, and `excerpt_count`. A pinned
digest test freezes the encoding; each field has its own invalidation test.

Coalescing: `reserve()` installs a capacity-1 broadcast sender for the leader and returns receivers
for followers; the leader removes the in-flight entry before publishing, so a late arrival sees the
store instead of a missed broadcast. Removal is `Drop`-based, so cancellation and panic unwind both
release waiters, which then run their own fan-out after up to two re-reservations.

## Behaviour on a hit

Skipped: provider fan-out and provider health recording.

Re-run per call from the cached raw per-provider lists: attempts ledger, timeout rows, `select()`
(RRF, conversion, sanitization and trust framing, domain filters, rerank, truncation), capability
telemetry and warnings, `providers_failed` ordering, `materialize_evidence_roles`,
`build_retrieval_failures`, and `postprocess()`. Only `Vec<(provider_id, Vec<SearchResult>)>` is
stored, mirroring the fetch cache's raw-versus-derived split.

Assertions: the whole `run_web_search` JSON compares equal between a miss and a hit; a hit leaves
`last_latency_ms`, `parse_shape`, `consecutive_failures`, and health status untouched while a miss
records a real latency.

## Never cache a partial fan-out

`is_cacheable()` requires zero provider failures and no deadline-interrupted provider. The
deadline flag is computed from the accounted set after the JoinSet loop, covering the zero-remaining
break, the timeout branch, and tasks aborted by the JoinSet drop; engine-task panics become
classified failure rows. A non-cacheable result stores nothing, publishes nothing, and increments
`bypassed`; waiters run their own fan-out.

Evidence: two providers re-dispatched with `entries == 0` and `bypassed == 2`; a truncated fan-out
re-dispatches the successful provider with `entries == 0`; a non-cacheable leader does not publish
its partial five-row list to waiters.

## Bounds

| Bound | Value | Behaviour |
|---|---|---|
| TTL | `cache_ttl_seconds` 300 s default, `0` disables, max 86,400 | expiry test with a short TTL |
| Entries | `cache_max_entries` 128 default, `1..=4096` | LRU eviction with promotion respected |
| Rows per entry | 1,024 across providers | Refuse to store, never truncate, so hits stay byte-identical |

## Verification (combined candidate)

- `cargo test --locked --features mock --test web_search_integration`: 70 passed, seven cache tests
  (sequential hit, concurrent coalescing, failure bypass, deadline bypass, health, payload
  equivalence, differing-request fan-out).
- `cargo test --locked --features mock --test mcp_tools --test config_validation`: 5 + 74 passed (six
  new config tests).
- `cargo test --locked --all-features --test dispatch_fault_injection --test
  retrieval_attempt_ledger --test provider_routing`: 34 + 46 + 87 passed.
- `cargo test --locked --all-features --test mcp_projection --test static_guards`: 24 + 60 passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `make check`: passed through the full `--all-features` suite (5,848 passed, 26 ignored, 0 failed)
  and hygiene; stops at the pre-existing `dependency-policy` advisories. `Cargo.lock` unchanged.

## Deviations

- `src/core/config.rs` grew past its 3,000-line ratchet, so the existing inline test module moved
  verbatim to `src/core/config/tests.rs` and the ceiling was lowered to 1,900 lines / 76,000 bytes.
  No test content changed.
- The per-entry row cap refuses to store rather than truncating, because truncation would break
  byte-identical hits.
- Cache entries carry the full key alongside the digest; a digest collision is treated as a miss and
  is covered by a test.
- `timeout_ms` is deliberately not in the key. A follower shares the leader's deadline; that is
  intended and documented in the module.

## Residual risk

- The TTL is a freshness bound, not a correctness guarantee. A freshness-sensitive caller should
  use `freshness`/`date_range` constraints (which are key dimensions) or a lower TTL.
- Cache state is process-local and disappears on restart; nothing is persisted.
- All verification is offline and mock-driven. Real provider fan-out counts were not observed live.
- Domain workflows (`repo_search`, `research_search`, `security_search`) remain uncached by design;
  extending the cache to them needs its own measured milestone.
