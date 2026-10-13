# Mode-Off Repository Locality M001 Closure

Status: closed
Primary class: invariant / security and runtime correctness
Implementation candidate: `cc0c9c7b27e0706a3bab5f5d7cb137e64e9eaa3d`
Branch: `agent/mode-off-local-repo-zero-dispatch-20261013` (pushed)
Plan: `plans/implementation/mode-off-repository-locality-corrective/001-explicit-zero-remote-dispatch-boundary.md`
Roadmap: `plans/subsystems/mode-off-repository-locality-corrective-roadmap.md`

## Decision

M001 is closed. Mode-Off `repo_search` now passes a call-scoped `Denied`
authority to the repository adapter. The adapter sends no engines to dispatch,
skips package registry resolution and package advisory lookup, and retains
local workspace search. Provider-list semantics are unchanged for normal
remote-allowed calls. The MCP schema and ten-tool surface are unchanged.

## Implementation and requirement evidence

| Requirement | Evidence |
|---|---|
| Explicit authority instead of empty provider-list encoding | `RepoRemoteExecution::{Allowed, Denied}` at the MCP-to-adapter call. Mode-Off requests retain their provider hints while selecting `Denied`. The denied adapter branch supplies empty engine and queried-id vectors without calling `selected_engines(&[])`. |
| Local match and provenance | The direct MCP integration fixture returns `locality_fixture_marker` from `lib.rs` with `local_trusted` local-workspace provenance. `providers_queried` contains only `local_workspace`. |
| No provider/forge engine invocation | The MCP fixture configures two counting engines (`mock_a`, `mock_b`) and explicitly requests `mock_a`; both counters remain zero. The adapter-level regression independently counts its engine at zero and verifies every subquery reports no attempted providers. Forge adapters share the same `SearchEngine` dispatch boundary. |
| No package registry HTTP | Package resolution is an injectable adapter seam. The adapter-level test supplies a counting resolver and package coordinate; its counter remains zero and the response states that registry lookup was skipped by policy. The production HTTP resolver is reachable only through the remote-allowed resolver closure. |
| No package advisory lookup | The same adapter test requests security context and receives `package_security_skipped` for local-only policy. The denied branch returns before `query_advisories_by_package`. |
| Honest absence and limited coverage | An empty local workspace returns `local_search_completed_no_matches`; remote sources are separately reported as skipped by policy. Provider attempt telemetry remains empty. |
| Policy rejection paths | The MCP regression verifies `include_local=false` returns an error before adapter work. Existing `repo_workflow` cases verify Mode Off without a local backend is denied. |
| Specialized and failure/deadline routing | The empty-workspace MCP trajectory exercises `exact_error`; the denied authority is fixed before planning, dispatch, local search, and deadline handling, so no local result/error/timeout can select a remote fallback. No cancellation-specific backend injection was added; cancellation aborts the same call and there is no fallback branch. |
| Live-mode compatibility | Existing empty/default and explicit-subset provider-routing cases pass; empty providers remain auto-selection in remote-allowed mode. |
| Static regression guard | `repo_search_local_only_uses_explicit_remote_execution_authority` checks the explicit authority at the tool seam, the package skip, the empty engine branch, and the advisory skip. The counting MCP test would fail if the original empty-means-all path invoked the configured engines. |
| Local discovery | `ServerState::local_inventory` walks configured roots and reads local Git metadata with bounded local commands. It does not fetch remotes or perform HTTP. Suggested fetch URLs are only generated as inert response data; no fetch execution occurs in `repo_search`. |

## Outbound seam census

| Seam | Mode-Off behavior | Observer/evidence |
|---|---|---|
| `SearchEngine::search` (including remote forge search adapters) | No engine is selected or dispatched | Two MCP counting engines and one adapter counting engine: 0 calls each; attempt telemetry empty |
| Package registry resolver HTTP | Resolver closure is not called; no HTTP client is built for package resolution | Injected resolver counter: 0; skipped-policy result warning |
| Package advisory lookup | Skipped before the adapter advisory method | `package_security_skipped` warning and guarded branch |
| Local inventory and workspace scan | Filesystem and bounded local Git metadata only | Source path audit; fixture returns a local card |
| Suggested fetch generation | May return inert candidate URLs, never executes them | Adapter code path ends with response construction; no fetch client call |
| Background work | No jobs are created | Repository adapter path audit |

This is an application dispatch invariant, not an operating-system network
sandbox claim. No live provider, registry, forge, or fetch request was used for
these regressions.

## Verification on the implementation candidate

All commands below ran against the source tree at
`cc0c9c7b27e0706a3bab5f5d7cb137e64e9eaa3d` before the closure-record commit.

| Command | Result |
|---|---|
| `cargo test --locked --lib meta::adapter::repo::tests::remote_denial_skips_engine_and_package_resolution -- --exact` | 1 passed; resolver and engine counters 0 |
| `cargo test --locked --features mock --test local_workspace_integration mode_off_repo_search_uses_local_workspace_without_remote_dispatch -- --exact` | 1 passed; local positive, empty exact-error workspace, explicit provider, package/security, and local-opt-out cases |
| `cargo test --locked --test static_guards` | 60 passed |
| `cargo test --locked --lib meta::package_resolver::tests -- --test-threads=1` | 37 passed |
| `cargo test --locked --features mock --test provider_routing -- --test-threads=1` | 83 passed |
| `python3 packaging/check-planning-consistency.py` | Passed |
| `make check` | Passed: format, Clippy, no-default-features check, 3,474 all-features tests, hygiene, dependency policy, and packaging checks |
| `make docs-check` | Passed with `RUSTDOCFLAGS="-D warnings"` |
| `git diff --check` | Passed |

No platform or credential skips affected these gates. No schema, config,
storage, or migration change was made. The previously published `v0.4.1` and
`v0.4.2` release evidence remains untouched.

The original empty-provider dispatch path was reintroduced temporarily at
the MCP seam, with package coordinates omitted so the mutation could not
attempt registry HTTP. The focused Mode-Off integration test failed at its
remote-call counter (`left: 6`, `right: 0`) as expected. Both temporarily
mutated source/test files were restored to the implementation candidate before
this closure record was prepared.

## Findings and handoff

- No unresolved severity findings remain within M001 scope.
- Direct cancellation of an in-progress local backend scan was not separately
  forced in a test. The authority is selected before that scan and remains
  `Denied` for the whole adapter call, so cancellation cannot turn into a
  remote fallback.
- M002 is ready. Its remaining operational requirements are exact 0.4.3
  candidate qualification, immutable crate/tag/release provenance, and the
  release-specific CodeGG consumer handoff. CodeGG paired requalification and
  legacy backend retirement remain consumer-owned and are not claimed complete.
