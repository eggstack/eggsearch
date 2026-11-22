# M001 Test Support Harness and Default-Constructed Tool Arguments — Closure Evidence

Status: closed

Candidate: the working tree on `feat/auto-20261117-044c4885` above baseline
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`, uncommitted because the request did not ask for a
commit. The canonical gate ran on exactly this tree; its output and the diff fingerprint are in
`plans/closure/code-duplication-consolidation/gate.md`.

Source plan: `plans/implementation/code-duplication-consolidation/001-test-support-harness-and-default-arguments.md`

Roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md` M001

Decision authority: no ADR. The milestone is test-only plus one additive derive.

## Change

- `tests/support/mod.rs` (218 lines) defines the twelve helpers once: `git_cmd`,
  `run_git_checked`, `setup_git_repo_with_remote`, `repo_fetch_state`,
  `state_with_engines`, `state_with_arc_engines`, `state_with_engines_sanitize`,
  `test_cfg`, `state_with_local_backend`, `state_with_local_backend_sanitize`,
  `state_with_local_backend_mode_off`, and
  `state_with_local_backend_mode_off_for_repo_map`. Bodies moved verbatim; mock-dependent
  helpers and imports are feature-gated.
- All nine suites that used them (`evidence_contract`, `mcp_tools`, `provider_routing`,
  `repo_workflow`, `research_workflow`, `security_workflow`, `web_fetch_integration`,
  `web_search_integration`, `corpus_runner`) declare `mod support;` and `use support::*;`.
- `Default` is additive on `WebFetchArgs`, `BatchFetchArgs`, and `RepoFetchArgs`.
- 96 of the 105 `WebFetchArgs { ... }` construction sites now set only the fields that matter
  and finish with `..Default::default()` (85 of them in `web_fetch_integration.rs`). Nine
  sites still set every field meaningfully and were not churned.
- `tests/static_guards.rs::integration_test_helpers_are_defined_once` asserts each of the
  twelve names is defined exactly once in the harness and nowhere else under `tests/`.

## Evidence

| Fact | Before | After |
|---|---:|---:|
| Twelve helper definitions across suites | 98 | 0 outside `tests/support` |
| Helper definitions in `tests/support/mod.rs` | 0 | 12 |
| `WebFetchArgs` literals with all-`None` runs | 96 | 0 |
| Suites importing the shared harness | 0 | 9 |

Failing-before guard evidence: adding `fn test_cfg() -> u8 { 0 }` to
`tests/evidence_contract.rs` produced an assertion failure naming `test_cfg` as a second
definition (left 1, right 0); the probe was removed and the guard passed again.

Suite counts after the change: `static_guards` 68, `mcp_tools` 29,
`batch_fetch_retrieval` 27, `web_fetch_integration` 128 passed / 1 ignored,
`web_search_integration` 78, `research_workflow` 141, `repo_workflow` 103,
`security_workflow` 20, `provider_routing` 99, `evidence_contract` 13. No test was added,
removed, or renamed.

## Caveat

`cargo check --locked --no-default-features --tests` fails on the baseline too, on the ungated
`ProgressSink::for_test` use in `tests/mcp_tools.rs` (`for_test` is
`#[cfg(any(test, feature = "mock"))]` and integration tests do not see the library's
`cfg(test)`). The library-only no-default check passes, and the integration suites are
exercised with `--features mock` as `make check` does.

## Verification

`cargo test --locked --features mock --test static_guards`, plus the nine suites above, passed.
The integrated result is in `gate.md`.
