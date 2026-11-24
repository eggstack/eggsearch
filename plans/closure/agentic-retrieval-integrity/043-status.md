# M043 Local Repo-Search Empty-Result Determinism and Incomplete-Scan Diagnostics — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/043-local-repo-search-empty-result-determinism.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M043

Corrective to: M037, closure `plans/closure/agentic-retrieval-integrity/037-status.md`; and M040,
closure `plans/closure/agentic-retrieval-integrity/040-status.md`.

Decision authority: accepted ADR-0006 (keyless deterministic retrieval) and the local workspace
boundary in `architecture/local-workspace.md`. No new ADR, tool, argument, schema property,
configuration key, or dependency.

## Symptom

CI run `44812224641` on candidate `e817dfde35372aa307f5580a144d7ededcb55574` failed the
`macOS canonical verification and local filesystem portability` job twice in
`tests/local_workspace_integration.rs::local_repo_search_finds_source_files` at the assertion
that some group held a non-empty `results` array, then passed on the third attempt with no
source change. The test binary was `3ace87ef457959f8`, the same binary that had passed the same
test in an earlier macOS job, so the fixture was environment-sensitive rather than a code
regression.

The fixture writes `lib.rs` containing `pub fn helper() -> i32 { 42 }` and `main.rs`
containing `fn main()`, commits both, and searches for `helper`. Neither file name contains the
query, so only the bounded content pass added by M040 can admit the match.

## Exact code candidate

Implementation candidate `5a218a4a91572f486244cf9881450aff7654e04f` (parent
`d5f17a5`), on `feat/auto-20261117-044c4885`. Every command, probe, and hosted job below ran
against that tree. This closure commit carries this record, the plan status, the roadmap status,
and the registry rows together.

## Classification

Cause: the bounded content pass never ran because the request deadline had already expired during
the cold inventory build. This is the pre-scan cause in the plan's Evidence list, not a content
pass, admission, git-hint, or ordering defect.

Telemetry captured on the exact tree, idle 14-core macOS host:

- The derived default request budget is `(max_indexed_files / 100).max(250)` = **500 ms** for
  the configured 50 000-file cap (`src/meta/local_backend.rs:411`). The fixture passes no
  `timeout_ms`, so it ran with 500 ms.
- `inventory_build_time_ms` for 13 fresh backends on the fixture: 174, 176, 185, 186, 190, 191,
  191, 202, 208, 212, 214, 275, 305 ms. The cold build alone consumed **35–61%** of the budget
  before a single candidate was examined, on an idle machine.
- The three git operations the search performs for this fixture (`git ls-files`,
  `git status --porcelain`, `git grep -l -z -i -F`) measured 120, 125, 134, 208, 219 ms across
  five runs on the same idle host.
- Deterministic reproduction through the existing `start` injection point of
  `LocalWorkspaceBackend::search_sync` with a 500 ms budget and the same fixture:
  `start` = now → 1 match, 2 files scanned, `timed_out = false`; `start` = now − 400 ms →
  0 matches, 0 files scanned, `timed_out = true`; `start` = now − 600 ms → 0 matches, 0 files
  scanned, `timed_out = true`.
- An empty result with `timed_out = true` is mapped to `local_search_timeout`
  (`src/meta/adapter/repo.rs:269`), and `local_search_completed_no_matches`
  (`:263`) requires `!timed_out && !truncated`.

Elimination: with room to run, the same fixture returns the content match. The pre-fix probe found
it with a full 500 ms budget (`start` = now → 1 match, `timed_out = false`); after the fix,
10 of 10 cold searches with the 500 ms default on an idle host and 15 of 15 focused repeats (the
fixture plus both reason tests) under 14-process CPU saturation did too. The scan logic, admission
rule, git hint, and second pass are therefore not the defect. A zero-card response can only come from a
latched deadline or a cut-short scan, and both are disclosed by existing warnings.

The skip was **not silent**, so work package 2 ("diagnostic surface, only if step 1 finds a
silent skip") was not implemented: a second warning for the same outcome would duplicate
`local_search_timeout`. That decision is recorded here rather than widened into a new warning
code.

## Change

1. **Scan deadline origin.** `search_sync` keeps `start` as the operation start and adds
   `scan_start`; a cold or stale inventory rebuild resets it to `Instant::now()` when the build
   returns (`src/meta/local_backend.rs:948`). Every scan bound — the first-pass candidate loops,
   the bounded fallback walk, the second pass, and the git-hint budget — now measures from
   `scan_start`. The build keeps its own `INVENTORY_BUILD_TIMEOUT`/`GIT_OP_TIMEOUT` bound, so
   the operation stays bounded (build cap plus request budget) and no scan becomes unbounded.
2. **Hint amortisation.** The second pass no longer spawns `git grep` when fewer than
   `GIT_HINT_MIN_CANDIDATES` (32) entries remain unscanned (`src/core/local.rs:19`,
   `src/meta/local/scan.rs:40`); it walks them instead, because one git spawn costs more than
   reading a small set. The hint remains a skip filter in the large case, and skipping it can only
   widen what is read, never change admission.
3. **Fixture contract.** `local_repo_search_finds_source_files` now requires the `lib.rs`
   match and asserts that no `local_search_timeout` warning is present, and prints the response
   on failure. It is not weakened to "any response".
4. **Regression tests.**
   - `meta::local_backend::tests::content_pass_budget_starts_after_the_inventory_build` passes a
     10 s-old `start` with a 500 ms budget and requires the match and a completed scan. It fails
     on the unfixed tree and passes on the fixed tree (below).
   - `meta::local::scan::tests::git_hint_is_skipped_below_the_candidate_threshold` pins the
     threshold boundary.
   - `local_repo_search_reports_timeout_instead_of_no_match` forces a 1 ms budget over 200 files
     whose only match sorts last, and requires `local_search_timeout` with no
     `local_search_completed_no_matches`.
   - `local_repo_search_reports_file_cap_truncation` caps `max_indexed_files` at 1 and requires
     `local_search_truncated` with no completed-no-match claim.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | The failing outcome is classified with captured telemetry, not inferred from the assertion | The cold-build `inventory_build_time_ms` distribution (174–305 ms against 500 ms), the aged-start reproduction producing 0 matches with `timed_out = true`, and the elimination run above. |
| 2 | A silent skip, if any, is closed with a forced-skip test that fails before and passes after | Classification found no silent skip; the disclosed skip is covered by `local_repo_search_reports_timeout_instead_of_no_match` and `local_repo_search_reports_file_cap_truncation`. The clock fix itself is covered by a test that **fails on the unfixed tree** and passes on the fixed tree (below). |
| 3 | A true no-match still returns no timeout or truncation warning | Unchanged `non_empty_workspace_without_query_match_returns_no_cards_without_false_truncation` passes; both new reason tests assert `local_search_completed_no_matches` is absent from the forced-incomplete outcomes. |
| 4 | `local_repo_search_finds_source_files` passes 20 of 20 saturated runs | 20 of 20 runs of the suite with `--test-threads=23` under 14-process CPU saturation, 25 tests each, 0 failed. |
| 5 | The hosted macOS job is green on the exact candidate | CI run `44829078087` on `5a218a4`: all three jobs succeeded on **attempt 1**. |
| 6 | M037, M040, and M041 regression suites pass unchanged | `local_workspace_integration` 25 passed, `repo_workflow` 103 passed, `static_guards` 67 passed; no tool, argument, schema, ranking, or global RRF change. |
| 7 | Exact-candidate `make check` and `make docs-check` pass | `make check` exit 0 (6 553 passed, 0 failed, 28 ignored); `make docs-check` exit 0; clippy `-D warnings` clean; `cargo fmt -- --check` clean; `git diff --check` clean. |
| 8 | No new tool, argument, schema property, configuration key, provider, or dependency | Ten-tool surface and `repo_search` schema unchanged; `static_guards` and the docs contract tests pass; `Cargo.toml` untouched. |

## Before and after

Without the `scan_start` reset the in-module regression fails:

```text
thread 'meta::local_backend::tests::content_pass_budget_starts_after_the_inventory_build' panicked at src/meta/local_backend_tests.rs:857:5:
pre-scan elapsed time must not consume the content-pass budget
test result: FAILED. 0 passed; 1 failed; 0 ignored
```

With the reset it passes: `test result: ok. 1 passed; 0 failed; 0 ignored`.

The aged-start reproduction gives the response shape for the same fixture and the same 500 ms
budget:

| `start` | matches | files_scanned | `timed_out` | Response |
|---|---|---|---|---|
| now − 400 ms, unfixed | 0 | 0 | true | `groups: []` plus `local_search_timeout` |
| now − 400 ms, fixed | 1 | 2 | false | one `source_files` group holding `lib.rs` |
| now, fixed | 1 | 2 | false | one `source_files` group holding `lib.rs` |

The incomplete response body for the same fixture and budget, captured through
`local_repo_search_reports_timeout_instead_of_no_match`:

```json
{
  "groups": [],
  "providers_queried": ["local_workspace"],
  "warnings": [
    {"message": "local_search_timeout: Local workspace search timed out", "provider_id": "local_workspace"},
    {"message": "local_search_truncated: Local workspace search results were truncated", "provider_id": "local_workspace"}
  ]
}
```

A timeout latches both `timed_out` and `truncated` because the second pass is also cut short;
neither suppresses the other, and `local_search_completed_no_matches` is absent. The fixed
response for the positive fixture is one `source_files` group holding
`{"title":"lib.rs","url":"workspace://<root>/lib.rs","snippet":"pub fn helper() -> i32 { 42 }"}`
with no `local_search_timeout` warning.

## Saturation profile

20 consecutive runs of `local_workspace_integration` with `--test-threads=23` while 14 CPU load
processes saturated a 14-core host: **20 of 20 passed**, 25 tests each, 0 failed.

The same profile already passed before the fix on this 14-core host, so it confirms the fixed tree
rather than discriminating the bug; the hosted 3-core macOS runner is the discriminating gate,
and it passed on attempt 1 there.

## Hosted qualification

On candidate `5a218a4`:

- CI run `44829078087`: `ci`, `windows portability`, and `macOS canonical verification and
  local filesystem portability` all succeeded on attempt 1. The macOS job is the one that failed
  twice on `e817dfd`.
- Egress feature qualification `44829078073`: success, attempt 1.
- Release drift guard `44829078125`: success, attempt 1.
- Python native qualification `44829078082`: failure in `Rebuild and smoke extracted sdist`.
  The same job failed on the three preceding docs-only tips (`950f219`, `58446ef`, `77c32aa`,
  `d5f17a5`). **Reconciled 2026-11-20:** that pre-existing attribution was wrong. The job was
  green on `57f5fed` and the six runs before it and red from `bc52970`, the first commit
  containing the M005 catalog, whose `catalog.json` was missing from `[package] include`. It
  remains outside M043's gate, but it is a regression owned by
  `plans/archive/implementation/python-pypi-distribution/005-packaged-artifact-asset-completeness.md`.

## Gates

- `cargo test --locked --all-features`: 6 553 passed, 0 failed, 28 ignored; 92 test binaries.
- `cargo test --locked --features mock`: 6 526 passed, 0 failed, 5 ignored.
- `cargo test --locked --all-features --test local_workspace_integration`: 25 passed.
- `cargo test --locked --all-features --test repo_workflow`: 103 passed.
- `cargo test --locked --all-features --test static_guards`: 67 passed.
- `make check` exit 0, `make docs-check` exit 0, clippy `-D warnings` clean, `cargo fmt --
  --check` clean, `git diff --check` clean.

## Residual risk

1. **A cold scan still has a bounded budget.** The default remains 500 ms for a 50 000-file cap.
   Removing the build from that budget restores the intended scan window, but a genuinely slow
   workspace can still time out; it is disclosed as `local_search_timeout`.
2. **The hint threshold is a fixed constant.** `GIT_HINT_MIN_CANDIDATES` (32) is chosen from the
   measured cost of one git spawn against small-file reads; it is not measured per host.
3. **The byte cap and the 512-file cap have no deterministic test.** Forcing
   `SECOND_PASS_MAX_BYTES` or `SECOND_PASS_MAX_FILES` needs more than 8 MiB of fixture data or a
   clock the second pass does not expose; the clipped outcome is covered through
   `max_indexed_files`, and both clipped branches share the same `CutShort` →
   `local_search_truncated` path.
4. **The original hosted failure has no captured response body.** The classification rests on the
   measured fixed cost, the deterministic aged-start reproduction, and the elimination above; a
   future recurrence now prints the response.
5. **Total worst case grows.** A cold search can now take the build bound plus the request budget
   (about 5.5 s at defaults) instead of `max(build, budget)`. The build was never cancellable by
   the request deadline, so the increase only applies to a search that can actually scan.

## Not done

No new warning code, tool, argument, schema property, configuration key, ranking rule, or
dependency. M037 admission, M040 second-pass bounds, and M041 identity resolution are unchanged,
and their closure records are not amended.
