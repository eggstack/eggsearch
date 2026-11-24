# B006 — Absent-Browser Degradation Contract — Closure

Status: closed
Recommendation: closed; all acceptance criteria are exercised on the exact candidate.
Implementation candidate: `fedf82c67a6d331fa1fab127b4ad5376cd27a84c`
Date: 2026-11-24
Plan: `plans/implementation/browser-artifact-availability/006-absent-browser-degradation-contract.md`
Roadmap: `plans/subsystems/browser-artifact-availability-roadmap.md`
ADR: `plans/adrs/ADR-0024-browser-enabled-wheels-and-absent-browser-degradation.md`
PR: [#20](https://github.com/eggstack/eggsearch/pull/20), open and unmerged at the time of closure.

## Candidate and implementation

The qualifying candidate is the exact PR head `fedf82c67a6d331fa1fab127b4ad5376cd27a84c`. M006 implementation and qualification commits are `e71b8f93f77159eca7914f82aa0bbb6e1ff0e3b0` (browser-absence and Python-wheel capability alignment), `2061e2636ec1b66757fac31de99c971764de9a26` (qualification workflow repair), `120a922cbf8a66a21cb812eb256b39dac0b722ff` (deterministic Chrome qualification), `af09fe5bdcb55137a4bcd456a40c6a235580539b` (gate the Chrome fixture qualification), and `fedf82c67a6d331fa1fab127b4ad5376cd27a84c` (bounded Chrome startup timeout in the test configuration only).

The final timeout correction changes only real-Chrome qualification settings: 45-second browser startup, 60-second fetch, and 75-second outer test timeouts. Production browser defaults and network protections are unchanged.

## Requirement-to-evidence matrix

| Requirement | Evidence | Outcome |
|---|---|---|
| No-browser search, ordinary HTTP fetch, and capability reporting remain available | Rust MCP absence tests in `src/mcp/tools/browser_availability_tests.rs`; Python `test_browser_absence_is_explicit_and_http_operations_still_work`; exact-head installed-wheel matrix in Python native qualification run [45363963916](https://github.com/eggstack/eggsearch/actions/runs/45363963916) | pass |
| Explicit browser requests return the precise error class; disabled and feature-absent states stay distinct | `src/mcp/tools/tests.rs`, `src/commands/fetch.rs`, `src/fetch/browser/discover.rs`, and Python absence tests; exact-head CI and qualification runs | pass |
| Automatic escalation preserves HTTP and warns only when rendering was warranted | `browser_absent_mcp_search_fetch_and_status_degrade` and `browser_launch_failure_is_named_and_does_not_repeat_http` in `src/mcp/tools/browser_availability_tests.rs`; Python no-Chrome and auto-discovery qualification | pass |
| Launch failure names its cause and does not repeat the HTTP fetch | `browser_launch_failure_is_named_and_does_not_repeat_http` asserts a named container-sandbox error, HTTP retention, and one browser attempt | pass |
| CLI/MCP/Python outcomes and the discovery/status taxonomy remain documented | `docs/features.md`, `docs/config.md`, `docs/python-library.md`, `architecture/fetch.md`, `architecture/python-library.md`; hosted CI run [45363963977](https://github.com/eggstack/eggsearch/actions/runs/45363963977) | pass |
| No-Chrome behavior is a hosted regression barrier | Python qualification run [45363963916](https://github.com/eggstack/eggsearch/actions/runs/45363963916): explicit absence tests on five native wheel families and automatic discovery in a browser-free container; every job succeeded on this head | pass |

## Verification

- Hosted CI [45363963977](https://github.com/eggstack/eggsearch/actions/runs/45363963977): success on head `fedf82c67a6d331fa1fab127b4ad5376cd27a84c`.
- Python native qualification [45363963916](https://github.com/eggstack/eggsearch/actions/runs/45363963916): success on the exact head, including the no-Chrome lanes and Chrome fixture/lifecycle lane.
- Release drift guard [45363963952](https://github.com/eggstack/eggsearch/actions/runs/45363963952): success on the exact head.
- Egress feature qualification [45363963974](https://github.com/eggstack/eggsearch/actions/runs/45363963974): success on the exact head.
- Local `RUSTUP_TOOLCHAIN=1.89.0 make check` and `make docs-check` passed on prior candidate `af09fe5bdcb55137a4bcd456a40c6a235580539b`; the exact-head hosted CI also passed after the test-only timeout change.

## Security, compatibility, and fixture disposition

Private-target protections are unchanged. The deterministic HTTPS fixture is not hosted publicly: the fixture URL and synthetic HTML are compiled only with `mock` in `src/fetch/browser/navigate.rs`, where CDP request interception validates the target and fulfills the fixture locally. The ordinary release wheel and binary do not contain the mock fixture. No public fixture service, credentials, or live provider is required.

No MCP tool inventory or schema change was made for M006. HTTP remains the safe fallback for `auto`; explicit browser requests remain structured errors when unavailable. Chrome remains runtime-only and is not bundled.

## Known limitations and findings

- Docker is unavailable on the local workstation, so the browser-free-container lane was not run locally; its hosted run passed.
- No release was published. The source version remains `0.4.5`; PR #20 remains open and unmerged.
- No unresolved M006 findings. M004 ARMv7 physical benchmarking remains a separate Python roadmap blocker and is not a browser-M006 condition.
