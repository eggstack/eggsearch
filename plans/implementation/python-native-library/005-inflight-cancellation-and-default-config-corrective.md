# M005 — Native In-Flight Cancellation and Default-Config Corrective

Status: closed
Primary class: invariant (lifecycle correctness and explicit configuration authority)
Source: `plans/subsystems/python-native-library-roadmap.md` M005
Closure: `plans/closure/python-native-library/005-status.md`; candidate `baf00cfc8e72f5975010f62e40d5d56985f77c78`.
Baseline: `main` `1434c1e55ddd92947042cff9918ffc0fe130ed01` (2026-10-14; Python 0.4.3)
Predecessors: M002 `plans/closure/python-native-library/002-status.md` (closed, candidate `2392e72178aa249445a64bd629caac8219353499`); M003 `plans/closure/python-native-library/003-status.md` (closed, candidate `ca4d0d6`). M004 distribution qualification remains active and must be repeated after this corrective changes the candidate.
Architecture: `plans/adrs/ADR-0005-python-native-library-boundary.md`, `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`; normative `plans/000-long-term-specification.md` sections 1–4, 7, 9 and `plans/003-planning-process.md` sections 2.4, 7.

## Objective and evidence gap

Requalify the safety and ergonomics of an **already-implemented** in-process Python asyncio SDK before distribution closure. Two gaps remain after M002/M003:

1. **In-flight cancellation not proven.** `bindings/python/tests/test_native.py::test_native_futures_cancel_and_client_close_is_bounded` creates a `provider_status` future, cancels immediately, and awaits its Python cancellation. It does not establish that a Rust handler entered before cancellation, prove that the Rust future was dropped or aborted, measure the release of the shared concurrency permit, or test `aclose()` racing active work. `bindings/python/src/lib.rs` uses `pyo3_async_runtimes::tokio::future_into_py`, an `ActiveCall` RAII guard, a process-scoped 32-permit semaphore and a per-client `CancellationToken` in `aclose`. The implementation looks designed for bounded cancellation, but the existing fixture is insufficient to assert the upstream Rust work stops.
2. **Configuration-selection asymmetry not resolved for agents.** `AsyncEggsearch(config_path=None)` constructs `AppConfig::default()`; `src/config.rs` (CLI) resolves `default_config_path()` and calls `AppConfig::load`. M003 documented the behavior, but did not provide a discoverable API for an agent to opt into the standard installed Eggsearch configuration without reimplementing OS-specific lookup. The difference is not necessarily a bug; silently changing the existing constructor's behavior would itself be a compatibility/security regression.

The original closures were appropriate for initial native capability, not exhaustive interrupted-operation or CLI-config-selection tests. This corrective adds missing adversarial regression evidence **without amending their historical closure records**.

## Readiness, invariants and non-goals

M002 and M003 are closed. No hard dependency remains; M005 is ready. M004 can continue collecting non-code packaging evidence, but its final exact-SHA wheel/sdist qualification must wait for M005 changes.

Do not change the ten tools, validated request/output shapes, Mode-Off no-remote-dispatch, SSRF/pinning, fetch bounds, trust/stable IDs, CLI config semantics, root default Cargo graph, Rust 1.89 MSRV, or Eggpack assets. Keep `#![deny(unsafe_code)]` for root crate; PyO3 remains in the separate binding package.

Non-goals: rearchitecting `src/mcp/tools/`, exposing a network MCP fallback, building a new cancellation scheduler/runtime, new provider endpoints, monkey-patching Python event loops, full general-purpose mutable config overrides, forced PyPI publication, broad typing rewrite, or changing prior closed milestone evidence to claim more than was observed.

## Ordered work packages

### WP1 — Reproduce and instrument true in-flight work

Build deterministic no-external-network test controls that guarantee the **Rust future has entered** a pending operation before Python cancels it (e.g. a counted, barrier-controlled injected handler/provider/fetch test double). Avoid timing-dependent sleeps or live endpoints. Test-only hooks may be feature-gated in `bindings/python`/test harness; they must not appear in release builds or bypass policy. If a restricted loopback fixture is used, require explicit authorization and count actual requests; prefer fully in-process fixtures.

Measure and assert: handler started, handler dropped/abort observed (or explicitly bounded unavoidable completion), no orphan provider requests, shared permit released, client active count drained. Use explicit bounded `asyncio.wait_for` timeouts as a hang guard, never as proof the transport was canceled. Include Python `Future.cancel()` before start as the existing fast path and **after** positive started barrier as the new critical path. Do not confuse Python Task cancellation with Rust future cancellation: trace both ends.

### WP2 — Concurrent close, cancellation and resource correctness

Run `aclose()` concurrently with at least one confirmed in-flight operation and a second pending/queued (or rejected by the documented concurrency cap) call. Verify the intended cancellation/error taxonomy (`asyncio.CancelledError` for user cancellation; structured `client_closed` for close-triggered operation), idempotent repeated close, no new calls after close, permit availability for a distinct client, and progress on a subsequent asyncio loop. Test exceptions and interrupted conversions without leaking `ActiveCall`/permit or holding the GIL during async work. Inspect `pyo3-async-runtimes` versioned cancellation semantics and `future_into_py` behavior; if default cancellation does not abort Rust tasks promptly, add an explicit bounded cancellation mechanism with minimal bridge-local changes and a fail-closed no-orphan test.

Document which in-flight network operations can finish after a cancellation boundary, what deadlines contain them, and whether Python cancellation cancels queued work, running Rust work, or only awaiters. Do **not** claim instantaneous socket termination without a counted transport fixture. A burst stress test should exercise saturation of the global 32-call limit and recovery, independent clients, and cancellation/close races, without flakiness.

### WP3 — Explicit opt-in default configuration resolution

**Preserve** `AsyncEggsearch()` existing no-config defaults as documented. Add a clearly named, typed, publicly documented opt-in, recommended `AsyncEggsearch.from_default_config()`, which uses the Rust-owned `eggsearch::core::config::default_config_path()` and `AppConfig::load` policy (or a comparably direct helper) instead of reimplementing platform selection in Python. Preserve `AsyncEggsearch(path)` behavior. Include API/type stub support; expose resolved path only if safe, without leaking credentials or raw config.

Tests must demonstrate all three cases: no-arg defaults unaffected by a present local config; opt-in factory loading a temporary platform config and enabling an observable safe setting; explicit path override taking precedence. Verify missing standard config matches CLI fallback-to-defaults, invalid TOML / unreadable config reports a deterministic error and never silently weakens security, no network on Python import or configuration construction, and no token contents in exceptions/logs. Use platform-aware fixture isolation and avoid tests modifying users' real config directories.

If implementation evidence shows preserving no-arg semantics is unsafe or violates a previously documented contract, STOP and record a compatibility decision rather than silently reversing constructor behavior.

### WP4 — Review, documentation and tests

Update `bindings/python/tests/test_native.py` (or suitably split behavioural suites), `bindings/python/python/eggsearch/__init__.py`, `_native.pyi`, `bindings/python/python/eggsearch/types.py` only as required, `docs/python-library.md`, `bindings/python/PYTHON_README.md`, `architecture/python-library.md` and test inventories/skills according to repo conventions. Keep Python public errors and Rust `SdkError` codes consistent, preserve type-checking coverage and the ten-operation parity/mode-off fixtures. Do not change M004 wheel inventory or version unless justified by a separate artifact-level compatibility finding.

## Verification commands and acceptance evidence

Inspect exact package/test commands before execution, then run and record:

- `cargo test --locked --all-features --lib sdk`
- `cargo test --locked --all-features --test static_guards`
- `cargo test --locked --all-features --test evidence_contract`
- `make check`
- `rustup run 1.89.0 cargo check --locked --manifest-path bindings/python/Cargo.toml`
- `maturin develop --manifest-path bindings/python/Cargo.toml --locked` inside an isolated venv
- `python -m pytest -q bindings/python/tests` including started-barrier cancel, concurrent close, concurrency-cap recovery and default-config fixtures
- `python -m compileall -q bindings/python/python/eggsearch bindings/python/examples`
- `pyright bindings/python/python/eggsearch bindings/python/examples`

Do not claim a pass for commands not executed; rerun and record intermittent failures. Root CI, root dependency-policy guards, native Python qualification and packaging will be rerun on the final candidate in M006.

PASS requires deterministic proof that cancellation after actual Rust work begins has the documented bounded stop behavior; `aclose()` under active work returns without orphan tasks/permits; 32-call saturation recovers; optional default-config loading works using Rust path semantics, preserving no-arg and explicit-path compatibility; existing ten-tool and Mode-Off security parity remains green.

STOP if the test shows unbounded detached Rust work, swallowed partial failures, unsafe default-config loading, broken CLI/MCP semantics, or a need to bypass policy. Record the failed path and write a narrower follow-up if needed. An immediate `provider_status` cancellation alone cannot close this plan.

## Required closure record and handoff

Write `plans/closure/python-native-library/005-status.md` with exact candidate SHA; original-gap-to-regression-case matrix; proof of barrier start and Rust stop/drain/permit release; concurrent close and multi-loop results; config-selection test outcomes; full command results; security/ABI/backward compatibility evidence; limitations and severity-rated residuals. Update roadmap and registry status **in the same closure commit**. M006 becomes ready only after this evidence closes M005. Do not mark M004 closed as a side effect of M005; qualification of **new** bytes is required.
