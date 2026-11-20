# ADR-0021: Optional Local Knowledge-Delta Evidence Index

Status: proposed
Date: 2026-11-19
Revised: 2026-11-20 — proposed contract. Not implementation authorization.
Decision owners: project maintainers
Baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`

Related: ADR-0002 (eggfetch transport), ADR-0006 (deterministic retrieval),
ADR-0020 (proposed knowledge-brief tool).
Affected specification: `plans/000-long-term-specification.md` §§3–4, 6–8.
Affected subsystem: `plans/subsystems/knowledge-delta-roadmap.md`.
Implementation: `plans/implementation/knowledge-delta/002-local-evidence-index.md`.

## Context

The existing process-local search and fetch caches are short-lived
performance infrastructure, not durable records of changed external facts.
A local knowledge-delta index would preserve dated source evidence across
sessions so a later milestone can find a related change the caller did not
name. Persistence creates retention, locking, migration, corruption,
authorization, and release-footprint obligations. The canonical
specification excludes recursive crawling, autonomous browser interaction,
mandatory vector indexing, and multiple binary SKUs.

Fetch cache already partitions browser-profile bodies by an opaque
`prof_` ID. `should_cache_response` refuses `private` and `no-store`
bodies in the anonymous scope. Config lives under
`$XDG_CONFIG_HOME/eggsearch/`. There is no general data directory.

## Alternatives

1. No persistent index; always search live. Lowest complexity but no
   historical evidence recall. This remains the default.
2. Append-only JSON files plus a lexical scan. Simple to ship but weaker
   for atomic updates, constrained queries, deduplication, and crash
   recovery.
3. **Candidate:** opt-in SQLite with FTS5, bounded document and event
   metadata, and deterministic lexical indexing, with a versioned migration
   path. Acceptable only after the spike below.
4. Vector DB or embedded LLM extraction. Rejected as a required dependency.
   Neither solves source dating or veracity.

## Acceptance precondition

This ADR cannot be accepted until a recorded footprint spike qualifies
bundled versus system SQLite under Rust 1.89 on the default release
targets (Linux, macOS, and Windows). Measure added stripped binary size,
startup resident memory, and schema-init time against the current
default-binary budget. The experimental ARMv7 wheel is not a hard gate;
mark that physical run unrun. The spike is not M002 work. If the spike
misses the budget, the accepted decision is alternative 2, the JSON store.
No second SKU and no new Cargo feature. Until that record exists, this
ADR stays proposed, and M002 must not start.

## Proposed decision (requires acceptance)

- The index is optional infrastructure, disabled by default, and never
  required for `knowledge_brief` to perform bounded live discovery.
  Enabling it does not change a `live` call. `live` neither reads nor
  writes the index.
- Additive modes, rejected by M001 until this ADR is accepted:
  `retain` writes this call's already selected public evidence and does
  not change retrieval; `prefer_live` runs live search and then unions
  index hits by stable ID, labeling `live_checked` or `index_only`;
  `index` reads the index only and says freshness is unverified.
  `retain` is a boolean, not a `refresh` value. `prefer_live` is
  the union mode and `index` is the local-only mode; both are
  `refresh` values, and neither is the default. An operator setting
  cannot write on a call the caller did not ask to retain.
- Store document identity, canonical URL, content hash, timestamps with
  the ADR-0020 basis and precision, evidence excerpts and spans, the M001
  `kdelta_` ID, source provenance, and schema version. Do not store user
  prompts, credentials, session cookies, or personal browsing history.
  Do not add a feed-subscription table.
- Index a document only when the fetch used `CacheScope::Anonymous`,
  `browser_profile` was absent, and `should_cache_response` would
  accept the response. Refuse `private`, `no-store`, pass-cookie
  replays, and profile-scoped bodies.
- The path is explicit, or `<config_dir>/knowledge-delta/<partition>/`.
  `partition` is `anonymous` or the opaque `prof_<fnv>` ID, never
  the display name. Do not copy fetch-cache contents, and do not treat
  fetch-cache TTL as event freshness.
- This decision does not authorize feed crawling, a refresh command, an
  import daemon, or a scheduler. M002 persists only evidence a live
  `knowledge_brief` already selected. A later milestone must not add
  host discovery from page links under this ADR.
- Bound index size, document count, excerpt bytes, and time. Evict
  deterministically. On disk full, corrupt records, or lock contention,
  report the degraded state. Never report a failed read as a fresh search.
- Use a versioned schema, transactional writes, one writer, and read-safe
  access. On an incompatible schema, fail the index closed and preserve
  existing data. A `live` call is unchanged.
- File permissions are restrictive. No network listener beyond the
  existing loopback-only MCP transport. Document deletion and disable.
- Content retrieved from the index stays external-untrusted. Persistence
  does not promote trust. The index is not a crawler, an answer generator,
  or an authority on what a model knows.

## Dependency and scope

M002 has a hard dependency on M001 closure and on acceptance of this ADR,
which includes the spike record. M002 implements the named engine. It does
not choose the engine. No canonical long-term amendment is required for an
index that remains optional, deterministic, and non-crawling. If
implementation needs recursive traversal, a storage network service, or a
mandatory runtime dependency that breaks the release constraints, stop
for a new decision.

## Verification and consequences

M002 must prove schema migration and recovery, lexical recall, preserved
date precision, retention and eviction, no write on `live`, refusal of
authenticated bodies, concurrent use, offline semantics, no-network tests,
and default-target package sizes. Unrun targets are named. Operational
docs cover path, backup, delete, disk use, and disable.

Positive: persistent recall of already selected public evidence without a
model. Costs: storage compatibility, migrations, and a privacy review.
Those are gates, not assumed benefits.
