# Web Acquisition M003 — Deterministic Change Comparison — Status

Status: closed
Date: 2026-11-25
Milestone: M003 — Deterministic Change Comparison
Decision authority: ADR-0026 Decision 1; ADR-0006 (deterministic retrieval); ADR-0011 (document addressing)
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`

## Candidate and changes

Local candidate: the baseline plus this change set, uncommitted in the worktree
on `docs/user-facing-polish-20261123`. The advertised tool count, aliases, and
default feature set are unchanged; the snapshot store is opt-in and closed by
default.

- `src/core/config/snapshot.rs` (70 lines): `FetchSnapshotSection`
  (`enabled=false`, `path`, `max_entries=10000`, `max_bytes=134217728`,
  `retention_days=30`) with validation.
- `src/core/config.rs` (2,043 lines): re-export and the `[fetch.snapshots]`
  field only. The module ratchet moved from 2,035 to 2,043 lines / 80,160 bytes
  with the next-slice note updated.
- `src/fetch/snapshot/store.rs` (614 lines): SQLite schema, `user_version`
  gating, permission hardening, put/get/prune, byte accounting, and typed errors
  (`SnapshotUnavailable`, `SchemaUnsupported`, `Corrupt`, `ContentLimit`).
- `src/fetch/snapshot/diff.rs` (335 lines): normalisation, SHA-256 equality,
  `similar` Patience diff with three context lines, hunk projection, and the
  size guard.
- `src/fetch/snapshot/mod.rs` (83 lines): key resolution, the comparison
  driver, and the `removed` payload.
- `src/mcp/tools/web_fetch_snapshot.rs` (71 lines): handler-side projection, so
  `src/mcp/tools/web_fetch.rs` stays at 1,596 lines against its 1,600 ceiling.
- Request wiring: `snapshot` on `WebFetchRequest`/`WebFetchArgs` and
  `BatchFetchItem::Web`, threaded through `fetch_locator`. 74
  `snapshot: None` initialisers were added across the request shapes and their
  tests.
- `bindings/python/python/eggsearch/types.py`: `FetchSnapshotRequest`,
  `FetchChange`, `FetchChangeHunk`, and `SnapshotCompare`.
- `tests/snapshot_change.rs` (6 cases).

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Storage, scope, retention, and diff guard per ADR-0026 Decision 1 | `src/fetch/snapshot/store.rs`, `diff.rs` | pass |
| `new` to `same` to `changed` across three fetches | `compares_extracted_content_and_never_stores_html` | pass |
| Identical content yields `same` with no hunks | same test, plus `same_text_has_no_hunks_after_normalization` | pass |
| One-line change yields one hunk with context and correct counts | `changed_line_reports_stable_hunk_and_character_counts` | pass |
| Insert, delete, reorder, and Unicode | `insertion_deletion_reorder_and_unicode_are_deterministic` | pass |
| Whitespace-only and CRLF normalisation | `normalization_preserves_unicode_and_removes_only_trailing_line_whitespace` | pass |
| Size guard returns counts with no hunks | `size_guard_returns_counts_without_hunks` | pass |
| `chars_added` and `chars_removed` agree with the emitted hunk lines | `changed_line_reports_stable_hunk_and_character_counts` (3/3) and the Unicode replacement case (2/1) | pass |
| `max_hunks` and `max_diff_chars` truncate in order | `hunk_and_character_limits_truncate_in_order` | pass |
| Re-running the same comparison is byte-identical | `serde_json::to_vec` equality in `changed_line_reports_stable_hunk_and_character_counts` | pass |
| `removed` on 404 with the snapshot retained | `removed_origin_reports_removed_and_retains_the_snapshot` | pass |
| Pruning: expired, then oldest `last_seen`, then key | `pruning_removes_oldest_then_lexicographic_key_under_caps` | pass |
| Scope isolation between anonymous and two profiles | `scope_isolation_uses_opaque_profile_ids` | pass |
| Newer schema refused; corrupt file refused without reset | `rejects_newer_schema_and_corrupt_database_without_resetting`; fetch-level `corrupt_store_degrades_without_failing_the_fetch` | pass |
| Disabled store never reports `new` or `same` | `disabled_store_warns_without_opening_a_file`; `default_compare_never_opens_the_store` | pass |
| No raw body, header, or credential stored | store inspection in `compares_extracted_content_and_never_stores_html` and `stores_only_normalized_text_and_reopens_durably` | pass |
| Caller key bounded and validated | `oversized_caller_key_is_rejected_before_any_fetch` | pass |

## Deviations and limits

- The `snapshot` option is attached to web items only: `fetch`, the
  `web_fetch` alias, and `batch_fetch` web items. Repo and site items are not
  web representations and do not carry it.
- Brief projection preserves the typed `change` object intact. Without that the
  contract would be invisible in the mode most agents use; the bounded hunk and
  character caps still bound the payload.
- A browser-rendered response returns `snapshot_browser_excluded` and no
  comparison. A truncated or over-limit extraction returns
  `snapshot_content_limit_exceeded` rather than storing a partial document.
- Retention is day-granular.

## Verification on this candidate

| Command | Outcome |
|---|---|
| `RUSTUP_TOOLCHAIN=1.89.0 cargo clippy --locked --all-targets --all-features -- -D warnings` | pass |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test snapshot_change` | pass: 6 passed |
| `cargo test --locked --all-features --lib fetch::snapshot` | pass: 12 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 make check` | pass |
| `git diff --check` | pass |

The macOS test runner serves `tempfile::tempdir()` as an existing `0755`
directory, so tests that need an explicit snapshot path create a private
`0700` child directory. The store's permission refusal was kept and the tests
were moved, not relaxed.

## Limitations

- No scheduling, polling, webhooks, or notification channel.
- No cross-host or cross-device sharing.
- Browser-rendered responses are not compared in this version.
- The store is local to the configured data directory.
