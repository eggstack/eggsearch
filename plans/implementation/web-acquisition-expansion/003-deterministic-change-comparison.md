# M003 — Deterministic Change Comparison

Status: proposed (blocked on a derived-state persistence ADR)
Date: 2026-11-24
Subsystem: web acquisition expansion
Milestone class: capability
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: a dedicated ADR for derived-state persistence (ADR-0026 when drafted); ADR-0011 (document addressing); ADR-0006 (deterministic retrieval)

References: `plans/003-planning-process.md` §2.4; `src/fetch/cache.rs`; `src/core/identity.rs` (content-derived FNV-1a IDs); `[knowledge_delta.index]` as the existing bundled-storage precedent.

## Objective

Answer "did this document change since last time" with a deterministic,
bounded comparison, so an agent can act on change instead of re-reading a page.
No scheduler, no notification channel, no model.

## Why this is proposed, not ready

`src/fetch/cache.rs` is an in-memory HTTP-freshness cache: it stores responses
for reuse under HTTP semantics, not durable per-URL content snapshots. A change
comparison that survives a process restart needs a new storage surface, which
the ADR threshold treats as a durable protocol decision. That ADR must be
accepted, and its enablement, retention, and privacy rules fixed, before work
package 2 starts.

## Current implementation evidence

- `ExtractionCacheKey` and `renderer_version` (5) define derived-cache identity;
  nothing is content-addressed by document version.
- The knowledge-delta index is the precedent for optional bundled storage:
  bundled `rusqlite`, disabled by default, with explicit
  `not_opened`/`disabled`/`open`/`degraded`/`offline` states.
- Stable IDs are content-derived FNV-1a; a snapshot key must not introduce
  random identifiers.

## Contract

New optional field on web items:

```text
snapshot: {
  compare: "none" | "line",     default none
  key: string?,                 caller label, max 128 bytes
  max_hunks: integer?,          default 20, max 100
  max_diff_chars: integer?      default 8192, max 32768
}
```

Response addition, only when `compare` is `line`:

```text
change: {
  status: "new" | "same" | "changed" | "removed",
  previous_fetched_at: string?,   RFC 3339 UTC
  current_fetched_at: string,     RFC 3339 UTC
  chars_added: integer,
  chars_removed: integer,
  hunks: [{ old_start, old_lines, new_start, new_lines, lines: [string] }],
  truncated: boolean
}
```

- `new`: no stored snapshot for the key.
- `same`: identical sanitised extracted text after the documented normalisation.
- `changed`: differing text with a bounded diff.
- `removed`: the origin returned 404 or 410 for a key that has a snapshot; the
  snapshot is retained and marked last-seen.
- A snapshot key is (normalised URL, extract mode, scoping fingerprint,
  representation class). A caller label scopes independent comparisons of the
  same URL, never across hosts.
- Diff is line-based with three lines of context, deterministic, and independent
  of hash iteration order.

## Storage and privacy

- Separate opt-in store: `[fetch.snapshots]` with `enabled = false` by
  default, a bounded path, `max_entries`, `max_bytes`, and `retention_days`.
- Only sanitised extracted text and metadata are stored: never raw HTML, never
  request headers, never credentials, never browser profile data.
- Content-addressed with SHA-256 for equality and an FNV-1a identifier for
  addressing, consistent with `src/core/identity.rs`.
- Pruning is deterministic: oldest `last_seen` first, then lexicographic key.
- A disabled store degrades explicitly: `snapshot_store_disabled` warning, and
  `caller compares to nothing` is visible rather than silently implying "new".

## Bounds

At most one stored snapshot per key; 1 MiB stored text per snapshot; diff capped
by `max_hunks` and `max_diff_chars`; when the line matrix would exceed the cost
bound, the response returns counts with `truncated: true` and no hunks rather
than an unbounded diff.

## Ordered work packages

1. ADR for derived-state persistence: enablement, retention, privacy, and the
   degradation vocabulary.
2. Store implementation behind the opt-in configuration with prune and bounds.
3. Normalisation rules and equality: whitespace, line endings, and sanitisation
   applied identically on write and read.
4. Bounded deterministic line diff with the cost guard.
5. Tool wiring, response shape, and docs.

## Verification

Focused: golden diff fixtures (insert, delete, modify, reorder, whitespace-only,
Unicode), status transitions across three writes, prune and retention tests,
disabled-store degradation, key isolation between scopes and modes, and a proof
that no raw body is stored. Broad: `make check`; `make docs-check`.

## Acceptance and stop conditions

- Identical content yields `same` with no hunks.
- A one-line change yields exactly one hunk with context and correct counts.
- Re-running the same comparison twice produces byte-identical output.
- A disabled store never reports `same` or `new` as if a comparison happened.
- **Stop:** the ADR selects a storage surface that cannot be bounded or
  disabled, or the diff cannot be made deterministic within the cost bound.

## Non-goals

- Scheduling, polling, webhooks, email, or Slack.
- Cross-host or cross-device snapshot sharing.
- Semantic or model-based summarisation of change.
- Storing raw HTML, screenshots, or credentials.

## Closure evidence

ADR reference, fixture results, status-transition table, prune evidence,
storage-content inspection showing sanitised text only, bounds tests, docs diff,
and limitations.
