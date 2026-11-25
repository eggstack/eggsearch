# M003 — Deterministic Change Comparison

Status: closed
Date: 2026-11-24
Subsystem: web acquisition expansion
Milestone class: capability
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: ADR-0026 Decision 1 (accepted); ADR-0011 (document addressing); ADR-0006 (deterministic retrieval)

References: `plans/003-planning-process.md` §2.4; `src/fetch/cache.rs`;
`src/core/identity.rs`; `src/meta/knowledge_delta/index.rs` (storage hardening
precedent); `similar 2.7.0` (line diff).

## Objective

Answer "did this document change since last time" with a deterministic, bounded
comparison that survives a process restart, so an agent can act on change
instead of re-reading a page. No scheduler, no notification channel, no model.

## Decisions taken (ADR-0026 Decision 1)

- **Store:** SQLite through the existing bundled `rusqlite`, behind an opt-in
  `[fetch.snapshots]` section, disabled by default.
- **Scope:** the snapshot key includes `CacheScope` (anonymous or the opaque
  profile ID). The earlier draft omitted scope; without it, profile-scoped
  content would leak across profiles.
- **Identity:** SHA-256 over normalised text answers equality; the existing
  FNV-1a rules answer addressing. No random identifiers.
- **Diff:** `similar 2.7.0` with the Patience algorithm and three lines of
  context. The cost guard is **size-based**, never wall-clock, so repeated runs
  are byte-identical.
- **Content policy:** sanitised extracted text and metadata only; no raw HTML,
  headers, credentials, TLS material, or browser profile data. Browser-rendered
  responses are excluded in v1.
- **Removal:** 404 or 410 is observed before error conversion so a keyed
  snapshot can report `removed`.

## Current implementation evidence

- `src/fetch/cache.rs:342` is an in-memory LRU with raw and derived byte caps;
  it has no durable path and no content-version identity.
- `ExtractionCacheKey` and `renderer_version` (5) define derived-cache
  identity; scoping joined that key in web M001.
- `src/meta/knowledge_delta/index.rs:157` opens a bounded SQLite store with
  `secure_delete`, `journal_mode = DELETE`, a page-count cap, `0700`
  directory and `0600` file permissions, and typed degradation. This is the
  pattern to follow.
- `sha2` is already a normal dependency; `similar 2.7.0` is already in
  `Cargo.lock` as an `httpmock` dev-transitive (Apache-2.0, MSRV 1.60).

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
- `same`: identical normalised text.
- `changed`: differing text with a bounded diff.
- `removed`: the origin returned 404 or 410 for a key that has a snapshot;
  the snapshot is retained and its `last_seen` updated.
- The key is (scope, normalised URL, extract mode, scoping fingerprint,
  representation class, caller key). A caller label scopes independent
  comparisons of the same URL and never crosses hosts.
- Line diff has three context lines and is independent of hash iteration,
  parser ordering, and wall-clock time.

Diff hunks are ordered by `old_start`, then `new_start`. `chars_added` and
`chars_removed` count characters in hunk lines only, so they agree with the
bounded output even when `truncated` is true.

## Storage and privacy

### Configuration

```toml
[fetch.snapshots]
enabled = false
path = "/optional/override/snapshots.sqlite3"
max_entries = 10000
max_bytes = 134217728
retention_days = 30
```

- Define `FetchSnapshotSection` in a new `src/core/config/snapshot.rs` module
  and re-export it from `FetchSection`. `src/core/config.rs` is at its
  2,035-line ratchet ceiling, so the section must not be inlined there. The root
  still gains the field and its default (about five lines); raise that ratchet
  by the delta with a next-slice note, or move the `[fetch]` types into a
  submodule first.
- Default path `<data_local_dir>/eggsearch/snapshots.sqlite3`; create the
  directory `0700` and the file `0600`.
- `user_version` is the schema version. A newer schema is refused with
  `snapshot_store_schema_unsupported`; a corrupt file is
  `snapshot_store_corrupt`. Neither silently resets the store.
- Open with `secure_delete = true`, `journal_mode = DELETE`,
  `busy_timeout = 5000`, and `max_page_count` derived from `max_bytes`.

### Pruning

After each write, prune in this order until both caps hold: expired entries by
`retention_days`, then oldest `last_seen`, then lexicographic key. A prune
failure is a warning, never a failed fetch.

### Degradation

- Disabled: the response carries `snapshot_store_disabled` and reports no
  `change` object. It must never report `new` as if a comparison happened.
- Store error: the fetch succeeds, the response carries a
  `snapshot_store_unavailable` warning, and `change` is omitted.
- A snapshot write failure never fails a successful fetch.

## Bounds

- 1 MiB stored text per snapshot; 4,096 UTF-8 boundary-checked lines.
- Diff guard: if `old_lines + new_lines > 20_000` or
  `old_lines * new_lines > 4_000_000`, return counts with
  `truncated: true` and no hunks.
- `max_hunks` and `max_diff_chars` cap returned hunks; the caller budget
  still applies on top.
- One snapshot per key; snapshot text counts toward the store byte cap, not the
  fetch response budget.

## Ordered work packages

1. **Types, config, and validation.** `FetchSnapshotSection`, request
   `snapshot` fields across the three web shapes, caps, and typed errors
   (`invalid_snapshot_key`, `snapshot_store_disabled`,
   `snapshot_store_unavailable`, `snapshot_store_schema_unsupported`,
   `snapshot_store_corrupt`).
2. **Store.** `src/fetch/snapshot/store.rs`: schema, open, permission
   hardening, put/get/prune, byte accounting, and typed errors. No handler code
   yet.
3. **Normalisation and diff.** `src/fetch/snapshot/diff.rs`: normalisation,
   SHA-256 equality, `similar` Patience diff, hunk projection, determinism
   and size-guard tests.
4. **Handler wiring.** `src/mcp/tools/web_fetch.rs` stays under 1,600 lines:
   call a small `src/fetch/snapshot/mod.rs` helper that resolves the key,
   reads/writes the store, and builds the `change` object. Extend the fetch
   outcome so 404/410 is visible for a keyed snapshot. Preserve the direct
   path when `compare = "none"`.
5. **Response projection, diagnostics, and docs.** Standard/brief/diagnostic
   projection, `docs/config.md`, `docs/features.md`,
   `architecture/fetch.md`, test inventories, and Python shape parity.

## Verification

Focused: a new `tests/snapshot_change.rs` with a deterministic mock origin and
a temporary store path: `new` → `same` → `changed` across three fetches;
insert, delete, modify, reorder, whitespace-only, and Unicode fixtures; prune
and retention; disabled and corrupt-store degradation; scope isolation between
anonymous and two profile IDs; a store inspection proving no raw body, header,
or credential is present; and a repeated-run byte-identical comparison. Unit
tests cover normalisation and the size guard. Broad: `make check`;
`make docs-check`.

## Acceptance and stop conditions

- Identical content yields `same` with no hunks.
- A one-line change yields exactly one hunk with context and correct counts.
- Re-running the same comparison twice produces byte-identical output.
- A disabled store never reports `same` or `new` as if a comparison
  happened.
- A snapshot created under one profile is invisible to another.
- No raw body is stored; removal of the store file rebuilds cleanly.
- **Stop:** the store cannot be bounded or disabled, determinism needs a
  wall-clock deadline, or the handler wiring would push
  `src/mcp/tools/web_fetch.rs` back over 1,600 lines.

## Non-goals

- Scheduling, polling, webhooks, email, or Slack.
- Cross-host or cross-device snapshot sharing.
- Semantic or model-based summarisation of change.
- Storing raw HTML, screenshots, credentials, or browser profile data.
- Snapshotting browser-rendered responses in v1.

## Closure evidence

Store schema and permission evidence, golden diff fixtures, the three-write
status table, prune and retention output, disabled/corrupt degradation,
scope-isolation result, storage-content inspection, determinism run, docs diff,
and limitations.
