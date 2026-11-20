# ADR-0021: Optional Local Knowledge-Delta Evidence Index

Status: proposed
Draft: yes — storage decision pending; not implementation authorization
Date: 2026-11-19
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
sessions and make related changes discoverable without an in-process model.
Persistence creates new data-retention, locking, migration, index-corruption,
source-authorization, and release-footprint obligations. The canonical
specification excludes recursive crawling, autonomous browser interaction,
mandatory vector indexing, and multiple binary SKUs.

## Alternatives

1. No persistent index; always search live. Lowest complexity but no
   historical change detection or offline evidence recall.
2. Append-only JSON files plus a lexical scan. Simple to ship but weak for
   atomic updates, constrained queries, deduplication, and crash recovery.
3. **Proposed:** opt-in SQLite with FTS5, bounded document/event metadata,
   and deterministic lexical indexing, with a format/version migration path.
4. Vector DB or embedded LLM extraction. Rejected as a required dependency;
   neither solves source dating or veracity.

## Proposed decision (requires acceptance)

- The index is **optional infrastructure**, disabled by default, and never
  necessary for `knowledge_brief` to perform bounded live discovery.
  Queries use local FTS5 first when enabled; the caller may request a live
  refresh. Offline mode explicitly reports that freshness is unverified.
- Store document identity, canonical URL, content hash, timestamps with
  separate semantic bases, evidence excerpts/spans, event identity, source
  provenance, source subscriptions, and schema version. Avoid storing whole
  user prompts, private fetched content, credentials, session cookies, and
  personal browsing history by default.
- SQLite/FTS5 is the candidate storage engine. Before accepting, qualify
  its C dependency (bundled versus system SQLite), MSRV Rust 1.89,
  macOS/Linux/Windows/release matrix, wheel matrix and low-memory ARM
  support. If footprint or packaging qualification fails, choose the
  JSON-only alternative and revise this ADR.
- Run scheduled refresh **only** for operator-approved feed/API subscriptions
  and under explicit per-cycle limits. Refresh can be invoked by the operator
  or an external scheduler. No daemon-owned general crawl and no following
  arbitrary links from discovered pages. No mandatory resident background
  process.
- All network acquisition reuses existing search/fetch authorization,
  source limits, SSRF and redirect policies. Feed parsing has byte,
  entry-count, field-length and deadline limits; use ETag/Last-Modified only
  as cache validators, never as proof of an event date.
- Bound index size, document count, excerpt bytes, per-origin frequency,
  batch operations and time. Evict deterministically and report degraded/
  partial results on disk full, corrupt records, stale feeds, and lock
  contention. Never silently report failed refresh as current.
- Use a versioned schema, transactional migrations, atomic updates, a
  single-writer design and read-safe access. On incompatible schema,
  fail the index component closed and preserve unmodified existing data.
- Data is local to an explicitly scoped operator config directory, with
  restrictive file permissions, no network listener beyond eggsearch's
  existing loopback-only MCP transport, and documented deletion/disable
  behavior.
- Content is external-untrusted on retrieval from the index as it was on
  original fetch; persistence cannot promote trust. The index is not a
  crawler, answer generator, or authority on what a model truly knows.

## Dependency and scope

M002 has a hard dependency on M001 closure and acceptance of this ADR.
The existing fetch cache stays the owner of raw/derived response caching;
the knowledge index owns only durable *selected evidence*. Do not quietly
reuse the fetch-cache TTL as an event-freshness guarantee.

No canonical long-term amendment is required for an index that remains
optional, deterministic, and non-crawling. If implementation requires
recursive traversal, a new storage network service, or a mandatory runtime
dependency violating the release constraints, stop for a new decision.

## Verification and consequences

M002 must prove reproducible schema migrations and recovery, FTS recall,
source-date precision, retention and eviction, concurrent invocations,
input sanitization, adversarial feeds, offline semantics, no-network tests,
package sizes, and release-target support. Operational docs must cover
storage path, backups, export/delete, disk use, and disable behavior.

Positive: cheap persistent recall and source revalidation without a model.
Costs: SQLite compatibility, state migrations, privacy/safety review, and
potential maintenance burden. These are gates rather than assumed benefits.
