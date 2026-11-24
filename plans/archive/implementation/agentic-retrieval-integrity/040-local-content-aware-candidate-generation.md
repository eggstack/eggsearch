# M040 — Local Content-Aware Candidate Generation and Symbol Recall

Status: closed
Date: 2026-11-19
Subsystem: agentic retrieval integrity
Milestone class: capability (corrective to M037)
Decision authority: existing contracts — ADR-0006 keyless retrieval, M006 routing, and the local
workspace boundary in `architecture/local-workspace.md`. No new ADR.

References: `plans/000-long-term-specification.md` §2 (stable MCP tool contract) and §4 (trust
and safety bounds); `architecture/local-workspace.md`; `architecture/meta.md`.

## Objective

Make a content or symbol query search the workspace rather than a role-ranked sample. When the
first path-ranked pass admits fewer matches than requested, continue deterministically through
the remaining inventory inside a time and byte bound, accelerate with a bounded git search when
the workspace is a git checkout, and report truncation only when the scan was cut short. A
symbol hint must be able to find a definition without a path hint.

## Evidence

Measured on 2026-11-19 against candidate `76c1156a2c5418d4b9dab4493eddd9232bdd7b8a`, with the
default local configuration (`max_indexed_files` 50,000, `max_file_bytes` 1 MiB), rooted at this
checkout (983 tracked files, 388 Rust files):

1. `select_inventory_candidates` scores path features and source role, then truncates to
   `max_results * 2`. The adapter passes
   `local_result_budget(effective_max_results) = effective_max_results / 2`, so a default
   request (effective max 10) reads **10 files**.
2. When no query token appears in a path, every implementation file ties at role score 10 and
   the tie breaks by path, so the window is the alphabetically first ten implementation files.
3. `repo_search` `query: "has_relevance"` (defined in `src/meta/local_inventory_cache.rs`)
   returned **0 cards** with `local_search_completed_no_matches`, though the file exists.
4. `query: "run_bounded_command"` (defined at `src/process.rs:12`) returned **0 cards**;
   `symbol: "run_bounded_command"` also returned **0 cards**; adding
   `path: "src/process.rs"` returned the correct card.
5. `git grep -l -F run_bounded_command` over the same tree completes in **0.05 s**.
6. Symbol hints do not participate in candidate selection; they only boost files that already
   survived the path cut.
7. The no-match warning reports "completed without matching files" although the scan read 10 of
   983 files; the warning is honest about the window but reads as absence.

## Scope

In scope:

- second-pass candidate generation for content and symbol queries;
- symbol-hint participation in candidate selection;
- bounded git-grep acceleration where available;
- truncation and no-match semantics for a multi-pass scan;
- tests and documentation for those contracts.

Out of scope:

- a persistent full-text index or a new dependency;
- remote provider selection, routing, and failure handling;
- `repo_map`, the local symbol parser's extraction rules, and trust/root authorization;
- changing `include_local`, the unscoped default, or any shipped default;
- a new tool or argument (a new configuration key is also out of scope; use internal bounds).

## Invariants

- The first pass and its path/role ranking are unchanged, so a path-rich query keeps its
  current latency and ordering.
- The second pass is deterministic, reads only files inside the configured roots through the
  existing safe-open/validation path, and stops at the requested result count, a byte bound, or
  a time bound, whichever comes first.
- `local_search_truncated` is set only when a bound cut the scan short or the returned matches
  were dropped; a completed scan with no match emits
  `local_search_completed_no_matches`.
- A git search runs only through `run_bounded_command` (no shell, no network) and only as a
  candidate hint; every candidate is still validated and read locally.
- Trust labels, root-relative paths, stable IDs, and deterministic ordering are unchanged.
- No new tool, argument, schema property, or shipped default; no workspace code execution.
- No new dependency; `cargo fmt` clean; ordinary files under 1,600 lines / 80 KB.

## Expected production changes

- `src/meta/local_backend.rs`: multi-pass orchestration, scan state, and admission accounting.
- `src/meta/local_inventory_cache.rs`: a bounded content-scan helper and/or the git-grep
  candidate source beside the existing bounded git runner.
- `src/core/local.rs`: internal scan-bound constants only; no new configuration key.
- `tests/local_workspace_integration.rs`, `tests/repo_workflow.rs`, and
  `tests/structured_local_code_intelligence.rs`: regression coverage.
- `architecture/local-workspace.md`, `docs/features.md`, `docs/tool-matrix.md`,
  `docs/test-inventory.md`, and `CHANGELOG.md`.

## Compatibility and migration

- No tool, argument, schema, or configuration change.
- Behaviour change: a content-only or symbol query can now return files that the previous
  ten-file window missed. Results remain capped by the existing local result budget.
- A scan that hits its bound reports `local_search_truncated`; the no-match warning now means
  the scanned set was exhausted. Both are documented.
- No index, storage, or dependency change. No migration.

## Dependencies

- Hard: M037 (closed), which established relevance admission and the no-match/truncation
  semantics.
- Interface: M006 routing (closed) for the hint semantics; `architecture/local-workspace.md`
  for the boundary.
- Soft: none.

## Work packages

1. **Paper first.** Update `architecture/local-workspace.md` to state the multi-pass candidate
   generation, the scan bounds, the symbol path, the git-acceleration rule, and the
   truncated-versus-no-match meanings. This lands before code.
2. **Scan state.** Distinguish `exhausted` (the scan completed) from `cut_short` (a bound
   stopped it). Preserve the first-pass result and ordering, and skip files already read.
3. **Second pass.** When the query or symbol hint is non-empty and the first pass admitted fewer
   than `max_results`, iterate the remaining inventory in deterministic path order, read and
   score through the existing helpers, and stop at `max_results`, the byte bound, the time
   bound, or exhaustion. Use the request timeout as the outer bound.
4. **Symbol recall.** When `symbol` is set, let the second pass search for the definition
   (structured parser first, regex fallback) rather than relying on a path hint. If a
   persistent symbol index is required for the desired precision, defer that sub-item and record
   it rather than adding an index silently.
5. **Git acceleration.** On a git workspace, derive candidate paths for a literal query with a
   bounded `git grep -l -F` through `run_bounded_command`, then validate and read each path
   locally. Fall back to the walk on any git failure, timeout, or cap breach. Respect the
   configured `respect_gitignore` semantics and record any tracked-versus-untracked difference.
6. **Truthful signalling.** Set `local_search_truncated` only for `cut_short` or dropped
   matches; emit `local_search_completed_no_matches` only for an exhausted zero. Keep the
   timeout warning distinct.
7. **Tests.** Extend the suites with:
   - a content-only query in a workspace larger than the first window finds a file outside it;
   - a symbol-only query finds a definition outside the first window, with provenance;
   - a true miss reports no-match only after an exhausted scan;
   - a scan cut by the time or byte bound reports truncation and not no-match;
   - the git-accelerated and walk paths return the same deterministic result;
   - a path-rich query keeps its first-pass result and ordering;
   - `include_local`, trust labels, and root-relative paths are unchanged.
8. **Documentation.** `architecture/local-workspace.md`, `docs/features.md`,
   `docs/tool-matrix.md` (the `repo_search` local note), `docs/test-inventory.md`, and
   `CHANGELOG.md`.

## Verification

```bash
cargo test --locked --all-features --test local_workspace_integration
cargo test --locked --all-features --test repo_workflow
cargo test --locked --all-features --test structured_local_code_intelligence
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test static_guards
make check
make docs-check
./packaging/check-repo-hygiene.sh
```

## Acceptance criteria

- A content-only query for a term in a file outside the first ten candidates returns that file
  within the scan bound.
- A `symbol` query finds a definition without a path or file hint, with the existing
  provenance fields.
- A true miss reports `local_search_completed_no_matches` only after the scan completed; a scan
  stopped by a bound reports `local_search_truncated` instead.
- A path-rich query keeps its first-pass result, ordering, and latency within the recorded
  tolerance.
- Git-accelerated and walk-only workspaces produce the same deterministic result; a git failure
  falls back without failing the search.
- No new tool, argument, schema property, configuration key, or shipped default; no new
  dependency.
- The broad gate passes with 0 failures.

## Stop conditions

Stop and record rather than working around any of these:

- the second pass cannot fit inside the request timeout for a large workspace; choose a smaller
  fixed bound and record the recall tradeoff;
- `git grep` semantics diverge from the inventory (untracked or ignored files) in a way that
  changes results; use it only as a hint and verify every candidate, or drop it and record the
  lost acceleration;
- symbol-only recall needs a persistent index to be useful; defer that sub-item and record it
  rather than adding an index silently;
- the fix requires a new configuration key or a default change (that needs its own measured
  acceptance).

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/040-status.md` with the exact candidate SHA, a
requirement-to-evidence matrix, the before/after responses for the content-only, symbol-only,
scoped, and no-match probes, the scan-bound measurements, the suite results, the broad-gate
counts, and residual risk (including the recall bound and any deferred symbol-index work).
