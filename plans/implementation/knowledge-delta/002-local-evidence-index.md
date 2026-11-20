# M002 — Optional Local Evidence Index

Status: proposed
Draft: yes — implementation prohibited until predecessors and ADR-0021 close
Date: 2026-11-19
Subsystem: knowledge-delta
Milestone class: infrastructure
Planning baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`
Roadmap: `plans/subsystems/knowledge-delta-roadmap.md` M002

References: `plans/000-long-term-specification.md` §§3–4, 6–8;
`plans/001-terminology-and-domain-model.md` §§1–3;
`plans/003-planning-process.md`; ADR-0002, ADR-0006;
proposed ADR-0020 and ADR-0021.

## Objective and readiness

Provide optional, bounded, crash-safe persistence for *selected external
evidence*, with lexical FTS5 retrieval, source/version provenance, and
explicit freshness state. A local run can answer from historical evidence
without an LLM or network call, but must not claim current coverage.

**Hard:** M001 must be closed and ADR-0021 accepted; confirm this branch's
actual storage/packaging constraints before implementation. **Operational:**
cross-platform SQLite and release footprint qualification before claiming
the feature is shippable in a default binary. This plan is proposed, not
ready.

## Current evidence

`src/meta/search_cache.rs` provides bounded process-local query-result
reuse; `src/fetch/cache.rs` provides in-memory raw/derived extraction
caches. Neither is an event index or persistent store. `src/core/config.rs`
and `src/config.rs` own typed TOML defaults, validation and settings
paths; `src/fetch/` owns bounded URL extraction, with `eggfetch-core`
as transport authority. M001 will own the candidate-evidence contract.

## Invariants and non-goals

- Keep the M001 live-only path functional when index is disabled, corrupt,
  unavailable, or its schema is too new. No silent fallback that makes a
  stale result appear freshly searched.
- Do not persist raw user prompts, authenticated browser/session state,
  cookies, upstream credentials, or arbitrary private workspace content.
- Keep external source trust markers and stable `SourceCard`/delta IDs;
  index hits do not become trusted instructions.
- Never use FTS scores as fact confidence or publication dates as
  event dates. Preserve original observation/date evidence.
- No recursive URL traversal, resident model, embedding dependency,
  general background crawler, cross-profile evidence leakage, or second
  release SKU.

## Storage and public compatibility contract

SQLite with FTS5 is **conditional on accepted ADR-0021**, including its
release-footprint/dependency gate. Suggested typed entities:
`documents` (canonical URL, content hash, source, publish/update/
observed dates, capped excerpt, trust); `candidate_events` (stable ID,
subject/type, date precision/basis, document span, ambiguity flags);
`event_sources` (event and document identity); `subscriptions`
(operator-approved feed/API source, validators, last/next check, status);
and `schema_meta` (schema version, last migration).

Index path under the platform's eggsearch data directory, not a repository
checkout; do not migrate/copy the existing fetch-cache contents. Default
runtime configuration disabled. Proposed settings: `[knowledge_delta.index]`
enabled, path, `max_bytes` (suggested 64 MiB), `max_documents`,
`max_excerpt_chars`, retention days, per-cycle limits. Specify and
validate all defaults/caps in the implementation's config docs.

## Ordered work packages

1. **Dependency/footprint spike:** qualify SQLite bundled vs system
   library under Rust MSRV 1.89, Linux/macOS/Windows and target SBC
   distributions; measure added stripped binary size, startup resident
   memory and schema-init time. If it violates accepted constraints,
   stop and revise ADR-0021 rather than adding another binary SKU.
2. **Local schema and transactions:** define schema version and
   migrations, bounded excerpt storage and FTS5 columns. Persist
   original source IDs, source URL canonicalization, validated date
   basis/precision and content-derived event IDs. Atomic inserts,
   idempotent replay, concurrent-reader/single-writer behavior,
   crash recovery, and deterministic schema-downgrade refusal.
3. **Ingestion:** index only already selected public source evidence
   from M001 with explicit per-item/aggregate limits. Avoid arbitrary
   link following. Permit operator-provided approved source feeds or
   APIs using existing bounded fetch/forges; cap item and origin
   rates. ETag/Last-Modified are validators, not verified event dates.
4. **Query and budget:** use FTS5 for bounded topic/subject lookup with
   deterministic tie-breaks. Union index and live candidates by stable
   identity while preserving trust and freshness metadata; report
   `index_only`, `live_checked`, `stale`, `refresh_failed`,
   `budget_exhausted`, and `unknown` without overstating coverage.
5. **Operations:** add a bounded operator CLI index-status/prune/refresh
   path only if needed; keep no autonomous in-process timer. Allow an
   external scheduler to invoke opt-in refresh safely. Document local
   permissions, retention, backup/delete, recovery and migration.
6. **Docs/SDK:** update `architecture/research.md`,
   `architecture/config.md`, `architecture/fetch.md`,
   `architecture/mcp.md`, `docs/config.md`,
   `docs/features.md`, `docs/agent-workflows.md`,
   `docs/test-inventory.md`, and the Python/native API surfaces only
   where their contracts change.

## Failure and recovery

Use SQLite transactional boundaries for each selected evidence batch.
A malformed date, source update conflict, bad UTF-8, lock contention,
corrupt database, unsupported schema, disk full, and cancelled refresh
have explicit bounded failure states. Do not delete existing data on
migration failure or convert `refresh_failed` to current. If a
partial failure leaves the index usable, report its coverage; otherwise
run M001 live-only with an index-degraded warning if the caller allows
network, or return a marked offline failure.

## Verification (exact implementation candidate)

- Pure tests: stable document/event identity, date preservation,
  canonicalization, lexical FTS recall, bounded indexing, dedup,
  deterministic eviction and no-query-persistence.
- Fault injection: crash mid-transaction, two processes/locks, disk
  full/read-only/corrupt DB, schema-forward incompatibility, missing
  FTS5, malformed feed, ETag 304 and upstream 429/timeout.
- Security: hostile source HTML/feed text remains untrusted;
  feed input may not redirect around SSRF or origin policy; stored
  credentials and cross-profile leakage remain absent.
- Focused: implement `tests/knowledge_delta_index.rs` (network-free
  deterministic fixtures) and extend `tests/mcp_tools.rs`,
  `tests/fetch_safety.rs`, `tests/static_guards.rs`,
  `tests/docs_config_snippets.rs` as applicable. Run
  `cargo test --locked --features mock --test knowledge_delta_index`,
  `cargo test --locked --features mock --test mcp_tools`,
  `cargo test --locked --features mock --test static_guards`.
- Broad: `make check`, `make docs-check`,
  `make packaging-check` and applicable Python wheel/ARMv7
  qualification. Record CPU, RSS, disk, elapsed time, net requests,
  and indexed-event counts on at least one low-memory target or
  clearly mark physical validation unrun.

## Acceptance and stop conditions

Index survives restarts and returns bounded source-linked evidence;
disabled and offline paths remain correct; migrations are non-
destructive; stale coverage and refresh failure are visible;
no new general crawl is introduced; release/target size and
dependency policy pass. M002 is infrastructure, **not** proof of
better knowledge-gap recall.

**Stop:** unaccepted ADR; unsupported SQLite build requirements;
loss of source identity/trust; any unbounded background work; migrations
requiring deletion without a plan; default binary/wheel fallout; or
unmeasured footprint changes.

## Closure evidence

Record baseline and candidate SHA, ADR acceptance, exact migration
and corruption/recovery tests, target matrix/footprint evidence,
source-retention/privacy review, offline vs stale outcomes, command
results, unresolved risks and decision. Write
`plans/closure/knowledge-delta/002-status.md`; update roadmap and
registry in the same closure change.
