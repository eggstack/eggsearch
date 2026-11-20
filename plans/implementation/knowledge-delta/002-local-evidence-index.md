# M002 — Optional Local Evidence Index

Status: proposed
Date: 2026-11-19
Revised: 2026-11-20 — ADR-0021 accepted bundled rusqlite. Not implementation authorization.
Subsystem: knowledge-delta
Milestone class: infrastructure
Planning baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`
Roadmap: `plans/subsystems/knowledge-delta-roadmap.md` M002

References: `plans/000-long-term-specification.md` §§3–4, 6–8;
`plans/001-terminology-and-domain-model.md` §§1–3;
`plans/003-planning-process.md`; ADR-0002, ADR-0006;
accepted ADR-0020; accepted ADR-0021.

## Objective and readiness

Provide optional, bounded, crash-safe persistence for *selected public
evidence*, with lexical lookup, source provenance, and explicit
freshness state. A caller can read historical evidence without an LLM.
The index must not claim current coverage, and enabling it must not
change an M001 `live` call.

**Hard:** M001 closed. ADR-0021 was accepted 2026-11-20 and names
bundled rusqlite. This milestone does not choose the engine and does
not re-run the footprint spike. **Operational:** default-target
footprint evidence, including the unrun Linux and Windows targets,
before calling the feature shippable in the default binary.
The experimental ARMv7 wheel is not a hard gate; mark that physical
run unrun. This plan is proposed, not ready.

## Current evidence

`src/meta/search_cache.rs` is process-local query reuse.
`src/fetch/cache.rs` is an in-memory raw and derived cache.
`CacheScope::Profile` partitions by an opaque `prof_` ID, never the
display name. `should_cache_response` refuses `no-store`, non-2xx,
and `private` in the anonymous scope. Neither cache is an event
index. Config lives at `$XDG_CONFIG_HOME/eggsearch/config.toml`
(`default_config_path`). There is no general eggsearch data
directory. M001 owns the candidate-evidence contract and the
`kdelta_` ID.

## Invariants and non-goals

- A `live` call neither reads nor writes the index. Disabled, corrupt,
  missing, and too-new schemas leave that path unchanged. No silent
  fallback that makes a stale hit look freshly searched.
- Do not persist raw user prompts, browser-profile or cookie-backed
  bodies, pass-cookie replays, upstream credentials, or private
  workspace content.
- Keep trust markers and M001 `kdelta_` / `src_` IDs. An index hit
  is not a trusted instruction. Do not invent a second URL
  canonicalizer; use `canonicalize_url`.
- Never use lexical scores as fact confidence, or a publication date
  as an event date. Store the M001 basis and precision unchanged.
- No feed ingestion, refresh command, scheduler, recursive traversal,
  resident model, embedding dependency, cross-profile leakage, or
  second release SKU.

## Storage and public compatibility contract

The engine is whatever accepted ADR-0021 names. The SQLite/FTS5 schema
below applies only if that decision is SQLite. Suggested entities:
`documents` (canonical URL, content hash, source, dates with basis
and precision, capped excerpt, trust); `candidate_events` (M001
`kdelta_` ID, basis, precision, document span, ambiguity flags);
`event_sources`; and `schema_meta`. No `subscriptions` table.

Default path is explicit `[knowledge_delta.index].path`, or
`<config_dir>/knowledge-delta/<partition>/`. `partition` is
`anonymous` or the opaque `prof_<fnv>` ID. Never the display name,
and never a repository checkout. Do not copy fetch-cache contents.
Runtime default is disabled. Settings: `enabled`, `path`,
`max_bytes` (suggested 64 MiB), `max_documents`,
`max_excerpt_chars`, and retention days. Validate them in
`docs/config.md`.

A document is indexable only when the fetch used
`CacheScope::Anonymous`, `browser_profile` was absent, and
`should_cache_response` would accept the response. Refuse `private`,
`no-store`, pass-cookie replays, and profile-scoped bodies.

Request modes, additive over M001:

- omitted or `live`: M001 behavior. No index read or write.
- `retain: true`: rejected unless the index is enabled. When accepted,
  write this call's already selected public evidence. `retain` does
  not change retrieval.
- `prefer_live`: live search, then union index hits by stable ID.
  Label each `live_checked` or `index_only`. An index-only hit
  stays stale. It is not a fresh search.
- `index`: index only. Coverage says freshness is unverified.

`retain` is a boolean, not a `refresh` value. `prefer_live` is
the union mode and `index` is the local-only mode; both are
`refresh` values, and neither is the default. M001 callers that
omit `refresh` stay live and write nothing.

## Ordered work packages

1. **Accepted engine.** Implement bundled rusqlite as named by
   accepted ADR-0021. Do not re-run the footprint spike here. Do not add
   an eggsearch Cargo feature, a second SKU, or a system SQLite link.
2. **Schema and transactions.** Version the schema. Persist M001 IDs,
   `canonicalize_url`, and the original basis and precision. Schema v1
   migrates atomically to v2 by clearing caller-topic fields and rebuilding
   subjects and edges solely from stored public titles and excerpts; evidence
   rows remain. Atomic inserts, idempotent replay, concurrent readers, one
   writer, crash recovery, and deterministic refusal of a newer schema. Do
   not fold a month or year to midnight UTC.
3. **Ingestion.** Index only public evidence an already completed
   `knowledge_brief` selected, and only when the caller set `retain`.
   Enabling the index is not a write. No link following and no
   feed parser.
4. **Query.** Bounded lexical lookup with deterministic tie-breaks.
   Apply the modes above. Report `index_only`, `live_checked`,
   `stale`, `budget_exhausted`, and `unknown`. Do not report a
   failed refresh as current; this milestone has no refresh.
5. **Docs.** Update `architecture/research.md`,
   `architecture/config.md`, `architecture/mcp.md`,
   `architecture/testing.md`, `docs/config.md`,
   `docs/features.md`, `docs/agent-workflows.md`, and
   `docs/test-inventory.md`. No new CLI surface is required for
   closure. Document delete, disable, permissions, and backup.

## Failure and recovery

Use one transactional boundary per retained batch. Malformed dates,
conflicts, bad UTF-8, lock contention, a corrupt database, an
unsupported schema, a full disk, and cancellation have explicit
bounded states. Do not delete existing rows because a migration
failed. If the index cannot be read, a `live` call is unchanged. A
`prefer_live` or `index` call returns a marked degraded or offline
result and does not pretend the miss was a fresh search.

## Verification (exact implementation candidate)

- Tests: stable IDs, precision preservation, `canonicalize_url`
  reuse, bounded indexing, dedup, deterministic eviction, and no
  write on `live`.
- Fault injection: crash mid-write, two processes, full disk,
  read-only path, corrupt file, and a newer schema.
- Security: hostile stored text stays untrusted; profile-scoped,
  `private`, `no-store`, and pass-cookie bodies are refused; no
  cross-profile read.
- Focused: `tests/knowledge_delta_index.rs`, plus
  `tests/mcp_tools.rs`, `tests/fetch_safety.rs`,
  `tests/static_guards.rs`, and `tests/docs_config_snippets.rs`
  as applicable. Add the suite row to `architecture/testing.md`.
  `cargo test --locked --features mock --test knowledge_delta_index`;
  `cargo test --locked --features mock --test mcp_tools`;
  `cargo test --locked --features mock --test static_guards`.
- Broad: `make check`, `make docs-check`, and
  `make packaging-check`. Record size and RSS on the default
  release targets, or mark a target unrun. Do not block on the
  experimental ARMv7 wheel.

## Acceptance and stop conditions

The index survives restart and returns bounded source-linked
evidence. `live` is unchanged. Migrations are non-destructive.
Stale and offline states are visible. No feed, scheduler, or second
binary. M002 is infrastructure, not proof of better recall.

**Stop:** ADR unaccepted or naming a different engine than the code
about to be written; the spike being treated as this milestone;
loss of source identity or trust; indexing an authenticated body;
any background crawl; a migration that deletes data without a
written plan.

## Closure evidence

Record baseline and candidate SHA, the accepted ADR and its spike
record, migration and recovery tests, footprint evidence with unrun
targets named, the privacy review, `live` versus `index`
outcomes, command results, residual risks, and the decision. Write
`plans/closure/knowledge-delta/002-status.md` and update the roadmap
and registry in the same change.
