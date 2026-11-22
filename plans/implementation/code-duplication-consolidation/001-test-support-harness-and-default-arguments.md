# M001 — Test Support Harness and Default-Constructed Tool Arguments

Status: closed
Closure: `plans/closure/code-duplication-consolidation/001-status.md`
Date: 2026-11-22
Subsystem: code duplication consolidation
Milestone class: infrastructure (test-only; one additive derive on production argument types)
Baseline: `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on `feat/auto-20261117-044c4885`
Decision authority: no new ADR. This milestone changes no contract, no schema, and no behaviour.

References: `plans/subsystems/code-duplication-consolidation-roadmap.md` §4, §7;
`architecture/testing.md` § Placement; `docs/test-inventory.md`; `tests/static_guards.rs`.

Dependencies: none.

## Objective

Replace eight byte-identical copies of twelve integration-test helpers with one
`tests/support/mod.rs`, and give the three tool argument structs that tests build field by field a
`Default`, so a new argument stops requiring edits in every suite.

## Current evidence

Measured 2026-11-22 on the baseline. Twelve helpers are byte-identical across these suites
(`tests/evidence_contract.rs`, `tests/mcp_tools.rs`, `tests/provider_routing.rs`,
`tests/repo_workflow.rs`, `tests/research_workflow.rs`, `tests/security_workflow.rs`,
`tests/web_fetch_integration.rs`, `tests/web_search_integration.rs`; `tests/corpus_runner.rs` also
carries `git_cmd`):

| Helper | Lines |
|---|---|
| `setup_git_repo_with_remote` | 32 |
| `state_with_local_backend_sanitize` | 21 |
| `state_with_local_backend` | 17 |
| `state_with_local_backend_mode_off` | 17 |
| `state_with_local_backend_mode_off_for_repo_map` | 12 |
| `run_git_checked` | 10 |
| `state_with_engines_sanitize` | 9 |
| `test_cfg` | 9 |
| `git_cmd` | 9 |
| `state_with_engines` | 7 |
| `state_with_arc_engines` | 7 |
| `repo_fetch_state` | 6 |

That is 173 lines per copy and roughly 1,211 redundant lines across the eight suites. There is no
`tests/common` or `mod support` in the repository today.

The same suites construct argument structs field by field: 116 `WebFetchArgs {` literals and 87
`RepoFetchArgs {` literals. The four largest clone blocks in the whole repository are the
all-`None` runs inside those literals (81, 79, 79, and 73 occurrences).

`RepoSearchArgs` already derives `Default` (`src/mcp/tools/repo_search.rs:18`). The three that do
not are:

| Struct | Location |
|---|---|
| `WebFetchArgs` | `src/mcp/tools/web_fetch.rs:126` |
| `BatchFetchArgs` | `src/mcp/tools/batch_fetch.rs:10` |
| `RepoFetchArgs` | `src/mcp/tools/repo_fetch.rs:24` |

## Invariants that cannot regress

- No production behaviour changes. `Default` is additive and unused by production code today.
- No advertised schema changes. `schemars` derivation is unaffected by `Default`.
- Helper bodies move verbatim; no assertion, timeout, or fixture is weakened.
- Every suite keeps its current feature requirements and its current test count.
- No test gains a network dependency.

## Work packages

1. **Create `tests/support/mod.rs`.** Move the twelve helpers verbatim, with the module-level
   `#![allow(dead_code)]` the suites already carry individually (`tests/mcp_tools.rs:1` and its
   siblings). Keep the file a directory module: Cargo compiles `tests/*.rs` and
   `tests/*/main.rs` as test targets, so `tests/support/mod.rs` is not a target, while
   `tests/support.rs` would become one.
2. **Add `mod support;` to each suite** and delete the local copies. Resolve name collisions by
   importing explicitly rather than glob-importing if a suite defines a same-named local helper
   for a different purpose; the detector found none, but the compiler must prove it.
3. **Gate feature-dependent helpers.** `state_with_engines`, `state_with_arc_engines`, and
   `state_with_local_backend` reach the `mock` harness. Add `#[cfg(feature = "mock")]` to those
   helpers and verify with `cargo check --locked --no-default-features --tests`; the suites
   themselves are exercised with `--features mock` in `make check`.
4. **Derive `Default`** on `WebFetchArgs`, `BatchFetchArgs`, and `RepoFetchArgs`. Do not add
   `Default` to types whose zero value would be semantically wrong and used by production code;
   these three are wire argument structs with `Option`/`Vec` fields only.
5. **Collapse the literals.** Replace the all-`None` runs with `Struct { field, ..Default::default() }`,
   highest-count files first: `tests/web_fetch_integration.rs` (88 `WebFetchArgs` sites),
   `tests/research_workflow.rs` (5), `tests/mcp_tools.rs` (4), `tests/batch_fetch_retrieval.rs` (3).
   Do not churn sites that already set every field meaningfully.
6. **Add a single-definition guard** to `tests/static_guards.rs`: for each of the twelve helper
   names, assert it appears as a definition once in `tests/support/mod.rs` and in no other
   `tests/*.rs`. This is the regression that prevents the copies from returning.

## Expected production changes

| File | Change |
|---|---|
| `tests/support/mod.rs` | New shared harness (twelve helpers) |
| Eight suites plus `corpus_runner.rs` | `mod support;`, local copies deleted |
| `src/mcp/tools/web_fetch.rs`, `batch_fetch.rs`, `repo_fetch.rs` | Add `Default` to the derive list |
| `tests/static_guards.rs` | Single-definition guard |
| `architecture/testing.md` | One sentence naming the shared harness |

No production logic changes.

## Non-goals

- Merging the eight suites into one file or renaming them.
- Replacing `httpmock` with a new harness.
- Adding `Default` to request types, response types, or config sections.
- Changing any helper's semantics, timeouts, or fixture contents.

## Compatibility and migration

None. The change is test-only plus an additive derive. A downstream consumer that constructs
`WebFetchArgs` without `..Default::default()` is unaffected.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo check --locked --no-default-features --tests
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test mcp_tools
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test web_fetch_integration
RUSTUP_TOOLCHAIN=1.89.0 make check
```

Failing-before evidence: add a thirteenth helper definition to a second suite and show the new
static guard fails; remove it. Record the guard output.

## Acceptance and stop conditions

- Twelve helpers are defined once and imported by every suite that used them.
- The literal count for `WebFetchArgs` and `RepoFetchArgs` drops; the remaining sites set fields
  that matter.
- Every suite's test count is unchanged; `make check` passes.
- **Stop:** if a helper body proves not to be identical once moved (a suite-local variant), keep
  that variant local, name it distinctly, and record it. Do not merge helpers that only look
  alike, and do not silence a `dead_code` warning by deleting a helper another suite needs.

## Required closure evidence

The closure record carries the before/after helper-definition grep, the before/after literal
counts, the static-guard failing-before run, the full suite counts, and a statement that no
production behaviour or schema changed.
