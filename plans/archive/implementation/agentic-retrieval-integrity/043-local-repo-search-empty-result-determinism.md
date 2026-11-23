# M043 — Local Repo-Search Empty-Result Determinism and Incomplete-Scan Diagnostics

Status: closed
Date: 2026-11-20
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective to M037 and M040, local repo-search admission and bounded second pass)
Baseline: `58446efb11e036b6edb81086f2f320eafa9382b5` on `feat/auto-20261117-044c4885`
Decision authority: existing contracts — accepted ADR-0006 (keyless deterministic retrieval) and
the local workspace boundary in `architecture/local-workspace.md`. No new ADR.

References: `plans/000-long-term-specification.md` §2; `plans/003-planning-process.md` §7;
`plans/subsystems/agentic-retrieval-integrity-roadmap.md` M037, M040, M041;
original plans `plans/archive/implementation/agentic-retrieval-integrity/037-local-repo-search-relevance-and-scope.md`
and `040-local-content-aware-candidate-generation.md`; closures
`plans/closure/agentic-retrieval-integrity/037-status.md` and `040-status.md`;
`architecture/local-workspace.md`; `docs/test-inventory.md`.

## Objective

Make a local `repo_search` that could not complete its content pass distinguishable from a
true no-match, and make the positive fixture assert that contract instead of a load-sensitive
happy path.

The user-visible contract M037 and M040 established is that a local result needs a query, path,
file, or symbol match, and that an incomplete scan must not be reported as "no match". This
milestone closes the remaining case: a content-only match whose bounded pass did not complete.

## Non-goals and compatibility

- No new tool, argument, schema property, configuration key, provider, or dependency.
- No change to local ranking, admission rules, result ordering, or global RRF behavior; no
  unbounded scan and no new host or remote dispatch.
- No amendment to the M037 or M040 closure records, and no reopening of their closed scope.
- Any added signal is response-only and additive; a consumer that ignores warnings is
  unaffected. No storage or migration change.

## Code pointers

Verified at the baseline, for the implementer:

- `src/meta/local/scan.rs:20` — `should_run_second_pass`, the gate that can suppress the
  content pass.
- `src/meta/local_backend.rs:1217` — the call site that decides whether to run the second pass.
- `src/meta/local_backend.rs:1276` — `run_second_pass`, bounded by `SECOND_PASS_MAX_FILES`,
  `SECOND_PASS_MAX_BYTES`, and `SECOND_PASS_TIME_BUDGET` from `src/meta/local/scan.rs`.
- `src/meta/local_backend.rs:944` and following — the inventory build, its
  `total_entries == 0` fallback walk, and the telemetry fields this milestone should read.
- `src/meta/local_inventory_cache.rs` — `INVENTORY_BUILD_TIMEOUT` (5 s),
  `FRESHNESS_PROBE_INTERVAL` (30 s), `INVENTORY_REBUILD_TTL` (300 s).
- `tests/local_workspace_integration.rs:772` — the fixture; the failing assertion is at
  `:793`, and `:155` is `mode_off_counting_state`.

## Evidence

Observed on 2026-11-20 during the M005–M007 hosted qualification, candidate
`e817dfde35372aa307f5580a144d7ededcb55574`, closure commit `bc52970`.

1. CI run `44812224641`, attempt 1: the `macOS canonical verification and local filesystem
   portability` job failed at `tests/local_workspace_integration.rs:793` in
   `local_repo_search_finds_source_files`. The asserted response contained no group with a
   non-empty `results` array.
2. Attempt 2 (`gh run rerun --failed`): the same test failed at the same assertion.
3. Attempt 3: all three CI jobs passed, including that job, with no source change between
   attempts.
4. The failing test binary hashes `3ace87ef457959f8`, the same binary that passed this test in
   the `a9fc806` macOS job (`44797522795`). The test therefore carries no source change from
   the tree that passed, and the failure is environment-sensitive rather than a regression
   introduced by the knowledge-delta work.
5. Locally the exact test passes 8 of 8 consecutive runs, and the full `make check` passes on
   the same tree.

The fixture is content-only by construction: it writes `lib.rs` containing
`pub fn helper() -> i32 { 42 }` and `main.rs` containing `fn main()`, commits both, then
searches for `helper`. Neither file name contains the query, so the match can only come from
the bounded content pass added by M040. The response in the failing attempts did not say which
of the following happened, and that is the defect this milestone owns:

- the inventory built empty and the bounded fallback walk did not complete;
- `scan::should_run_second_pass` returned false (for example `timed_out`,
  `matches >= max_results`, or `files_scanned >= max_indexed_files`);
- the second pass ran but was clipped by `SECOND_PASS_MAX_FILES`, `SECOND_PASS_MAX_BYTES`, or
  `SECOND_PASS_TIME_BUDGET`;
- the second pass ran and admission (`has_relevance`) rejected the match;
- the git-backed inventory failed or timed out (`GIT_OP_TIMEOUT`,
  `INVENTORY_BUILD_TIMEOUT`) and nothing was walked.

## Why earlier verification missed it

- M040's tests exercise the second pass on an unloaded machine and assert the found case. The
  forced-skip path has no test at all, so nothing fails when the diagnostic is absent.
- The suite already accepts an empty local result as valid:
  `non_empty_workspace_without_query_match_returns_no_cards_without_false_truncation` proves a
  real no-match may return no cards. A content-only fixture returning no cards is therefore
  indistinguishable from a correct answer without telemetry.
- The suite is parallel (23 tests in one binary, each creating a temporary git repository), so
  the load profile on a hosted runner differs from the single-test runs used during review.
- No assertion ties the positive fixture to the *reason* for its outcome, so a failure is
  unattributable and a pass cannot be trusted either.

## Invariants that must not regress

- M037 admission: a local result still requires a query, path, file, or symbol match, and a
  real no-match still returns an empty result without a false truncation claim.
- M040 bounds: the second pass keeps its file, byte, and time caps, and an incomplete pass is
  never reported as a complete no-match.
- M041 identity: linked-worktree remote resolution and the stated local-skip reason are
  unchanged.
- The ten-tool surface and every `repo_search` argument are unchanged; any new signal is
  response-only.

## Ordered work packages

1. **Classify before fixing.** Reproduce the failing fixture under saturation (e.g.
   `cargo test --test local_workspace_integration -- --test-threads=23` while the machine is
   loaded) and record the existing telemetry fields the backend already tracks
   (`used_inventory`, `fallback_walk`, `cold_build`, `inventory_build_time_ms`,
   `inventory_entries`, `files_scanned`, `timed_out`, `truncated`, and the second-pass
   outcome). Assign the observed failure to exactly one cause from the list in Evidence. If the
   fixture cannot be reproduced locally, instrument a temporary diagnostic build (not committed)
   to capture the same fields from the hosted job.
2. **Diagnostic surface, only if step 1 finds a silent skip.** When a content pass is skipped
   or clipped, return a bounded warning from the existing prefix table (the vocabulary already
   has `local_search_truncated` for a clipped second pass) and keep `truncated` truthful. A
   true no-match must not gain that warning. No new argument, tool, or schema property beyond
   what M040 already returns.
3. **Deterministic regression.** Add a test that forces each skip condition (timeout already
   latched, `max_results` reached, file cap reached, byte cap reached) and asserts the
   diagnostic rather than an empty result. Prefer a deterministic trigger over a sleep-based
   one; if a clock is needed, use the existing injection point rather than wall-clock waits.
4. **Fixture contract.** Make `local_repo_search_finds_source_files` assert the contract:
   either the content match is returned, or the response states why it could not be completed.
   Once step 2 lands, tighten it to require the match on an unloaded run and the stated reason
   on a forced skip. Do not weaken it to "some response".
5. **Load determinism.** Run the suite 20 times under the saturation profile from step 1 and
   record pass counts. A single green run is not evidence.
6. **Docs.** Update `architecture/local-workspace.md` with the incomplete-scan signal and
   `docs/test-inventory.md` with the suite's contract if it changes. No user-facing page change
   is expected unless the warning vocabulary grows.

## Verification

Focused, exact-candidate commands:

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test local_workspace_integration
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test repo_workflow
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

Plus the 20-run saturation profile from work package 5 and a hosted macOS run on the exact
candidate. The hosted run is the gate that failed twice; a local pass alone does not close this
milestone.

## Acceptance and stop conditions

- The failing outcome is classified with captured telemetry, not inferred from the assertion.
- If a silent skip exists, a forced-skip test fails before the fix and passes after it, and a
  true no-match still returns no warning.
- `local_repo_search_finds_source_files` passes 20 of 20 saturated runs, and the hosted macOS
  job is green on the exact candidate.
- M037, M040, and M041 regression suites pass unchanged; no tool, argument, ranking, or global
  RRF behavior changes.
- Exact-candidate `make check` and `make docs-check` pass.

**Stop:** the only available fix weakens the positive assertion without adding a diagnostic;
the cause cannot be classified after a genuine reproduction attempt; or the correction would
need a new tool, a new argument, a ranking change, or an unbounded scan. In the last case,
propose a separate decision instead of widening this milestone.

## Required closure evidence

Write `plans/closure/agentic-retrieval-integrity/043-status.md` with the classification and the
telemetry that supports it, the before/after response for the fixture, the 20-run saturation
counts, the hosted macOS run id and result, regression results for M037/M040/M041, the
exact-candidate gates, and residual risk. Update the roadmap and registry together; do not
amend the M037 or M040 closure records.
