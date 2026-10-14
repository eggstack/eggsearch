# Python Native Library M005 — In-Flight Cancellation and Default-Config Corrective Closure

Status: closed
Candidate: `baf00cfc8e72f5975010f62e40d5d56985f77c78`
Implementation plan: `plans/implementation/python-native-library/005-inflight-cancellation-and-default-config-corrective.md`
Predecessors: `plans/closure/python-native-library/002-status.md` (M002), `plans/closure/python-native-library/003-status.md` (M003)
Roadmap: `plans/subsystems/python-native-library-roadmap.md` M005

## Delivered

- Preserved `AsyncEggsearch()` no-arg `AppConfig::default()` semantics and explicit-path `AppConfig::load` behavior.
- Added Rust-owned opt-in `AsyncEggsearch.from_default_config()` and `default_config_path()` using `eggsearch::core::config::default_config_path()` plus `AppConfig::load`, matching CLI missing-file fallback and deterministic invalid-TOML errors without credential leakage.
- Added a counted barrier-controlled blocking engine to `src/sdk.rs` proving that dropping an SDK `execute` future aborts provider work.
- Added a debug-only bridge gate (`_test_blocking_execute`, `_test_gate_state`, `_test_gate_reset`, `_test_gate_release`, `_test_active_count`) sharing the production `ActiveCall`, 32-permit, and close-token path; compiled out with `debug_assertions` off and absent from release wheels.
- Extended `bindings/python/tests/test_native.py` from 17 to 28 tests: started-barrier cancellation with Rust drop/permit recovery, concurrent close with `client_closed` taxonomy, 32-call saturation and recovery, conversion-failure leak checks, sequential-loop recovery, and six default-config selection cases.
- Updated `docs/python-library.md` (config selection, cancellation/close semantics), `bindings/python/PYTHON_README.md`, `architecture/python-library.md`, `architecture/testing.md`, `docs/test-inventory.md`, and `skills/eggsearch-dev/SKILL.md`.
- Left M002/M003 closure records untouched; this record is the corrective evidence.

## Gap-to-regression matrix

| Original gap (M005 plan § Objective) | Regression case | Proof |
|---|---|---|
| Immediate `provider_status` cancel never proved Rust entered, stopped, or released permits | `src/sdk.rs::dropping_sdk_execute_aborts_blocked_engine` | Entered==1 via `Notify`, task abort, dropped==1, no hang (5 s timeouts) |
| `aclose()` racing active work untested | `src/sdk.rs::close_token_select_drops_blocked_engine` | Close-token `select!` wins, `client_closed`, dropped==1 |
| Python cancel vs Rust cancel conflated | `test_native_gate_cancel_after_started_barrier_drops_rust_future` | Entered==1 before `Future.cancel()`, `CancelledError` for awaiter, polled dropped==1, active 0, permits 32, next `provider_status` green |
| Close taxonomy, idempotence, post-close rejection, independent clients untested | `test_native_gate_close_cancels_inflight_with_client_closed` | Two confirmed in-flight blockers, `aclose()` bounded, both get `client_closed`, repeated close green, post-close rejected, fresh client green |
| 32-call saturation recovery untested | `test_native_gate_concurrency_cap_saturates_and_recovers` | 32 entered, permits 0, 33rd and other-client calls get `budget_invalid`, release recovers all 32, both clients green |
| Conversion/interrupted work leak risk | `test_native_conversion_failures_do_not_leak_permits` | Invalid/cyclic inputs keep active 0 and permits 32, next call green |
| Multi-loop progress after cancel/close untested | `test_native_runtime_recovers_on_next_loop_after_cancel_close` | Cancel/close in one `asyncio.run`, clean `provider_status` in the next |
| No-arg vs installed-config asymmetry | `test_no_arg_defaults_ignore_platform_config`, `test_from_default_config_loads_platform_config`, `test_explicit_path_overrides_default_config` | No-arg ignores platform file (`mojeek` stays false); factory loads it (`mojeek` true plus `local_trusted` workspace hit); explicit path wins over default path on both provider flag and workspace content |
| Missing/invalid config, network/token safety | `test_from_default_config_missing_file_falls_back_to_defaults`, `test_from_default_config_invalid_toml_reports_deterministic_error`, `test_config_construction_hides_tokens` | Missing file falls back to defaults; malformed TOML raises deterministic `ValueError`; secret env value absent from status JSON and error fields |

## Verification on the exact candidate

Working tree at commit was clean; all commands ran against `baf00cf` bytes unless noted.

| Check | Result |
|---|---|
| `cargo test --locked --all-features --lib sdk` | 6 passed (2 new barrier tests green) |
| `cargo test --locked --all-features --test static_guards` | 60 passed |
| `cargo test --locked --all-features --test evidence_contract` | 12 passed |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `cargo check --locked --no-default-features` | Passed |
| `cargo test --locked --all-features` (full) | First run: 3477 passed, 3 failed (`multiple_roots_share_global_cap` known transient per M002 closure; 2 `update::tests` candidate-timing flakes under load); all 3 pass in isolation and `update::tests` passes as a suite on retry; second full `make check` test phase passed and reached dependency-policy |
| `./packaging/check-repo-hygiene.sh` | Passed |
| `./packaging/check-contract.sh` | Passed (workflow pins, planning consistency, Eggpack qualifier, egress contract) |
| `./packaging/check-dependency-policy.sh` | Failed on pre-existing upstream rustls advisories RUSTSEC-2026-0346/0347/0348 against pinned rustls 0.23.45; `Cargo.lock` untouched by M005; recorded below as Medium residual |
| `rustup run 1.89.0 cargo check --locked --manifest-path bindings/python/Cargo.toml` (rustc 1.89.0) | Passed (debug); release `--release` check also passed, confirming debug-only gate compiles out |
| `maturin develop --manifest-path bindings/python/Cargo.toml --locked` (maturin 1.9.5, isolated arm64 venv, CPython 3.14.2) | Passed |
| `python -m pytest -q bindings/python/tests` (pytest 8.4.2) | 28 passed |
| `python -m compileall -q bindings/python/python/eggsearch bindings/python/examples` | Passed |
| `pyright bindings/python/python/eggsearch bindings/python/examples` (pyright 1.1.414) | 0 errors, 0 warnings |

`pyo3-async-runtimes` 0.29 inspection: `future_into_py` wraps the Rust future in `Cancellable` with a Python-cancel `oneshot`; a cancelled Python future completes the wrapper and drops the inner future, releasing `ActiveCall` and the semaphore permit by RAII. No extra bridge cancellation scheduler was needed; the existing close-token `select!` plus `Cancellable` covers user-cancel and close paths with the documented taxonomy. Socket teardown stays bounded by Tokio/`eggfetch-core` timeouts and is not claimed as instantaneous RST.

## Compatibility, security, ABI

- Ten tools, request/output shapes, Mode-Off no-remote-dispatch, SSRF/pinning, fetch bounds, trust/stable IDs, CLI config semantics, root default Cargo graph, Rust 1.89 MSRV, and Eggpack assets unchanged.
- Root `#![deny(unsafe_code)]` preserved; PyO3 remains in the separate binding package only.
- Gate hooks share production error codes (`client_closed`, `budget_invalid`, `internal`), never call SDK tools, and are absent from `--release` builds.
- `from_default_config` reuses CLI `AppConfig::load` fallback; missing file returns defaults, invalid TOML raises without silent weakening, and token values never appear in status JSON or error `message`/`data`/`repair`.
- Python `abi3-py311` tags, distribution name, and unpublished status unchanged; no publication performed.
- No new MCP tools, providers, network fallback, runtime, or mutable global config overrides.

## Limitations and residuals

- Medium: root dependency-policy fails on upstream rustls RUSTSEC-2026-0346/0347/0348 (Debug key-material disclosure, ECH data-ordering, Brotli large-window allocation) against locked rustls 0.23.45. `Cargo.lock` is untouched by M005. Owned by the repository dependency process; M006 must re-evaluate on its candidate, not silently waive.
- Low: first full all-features run showed 3 transient failures under load (`multiple_roots_share_global_cap` matches the M002-closure transient; 2 `update::tests` candidate-timing flakes). All pass in isolation and on retry with no source change.
- Low by design: cancellation evidence proves prompt Rust-future drop with permit/active recovery, not instantaneous socket termination; network operations remain bounded by existing Tokio/`eggfetch` deadlines as documented.
- Informational: Python evidence here is CPython 3.14.2 arm64 debug (plus release compile check); the 5-target x 4-interpreter matrix, extracted-sdist rebuild, and digests remain M004/M006 owned. Release wheels skip the 5 debug-only barrier tests by design (23 run); SDK barrier tests ship in all builds.

## Handoff

M005 is closed. M006 is ready to qualify the post-M005 candidate and must not reuse baseline run `38522548258` as evidence for changed bytes. M004 stays active until M006 records `004-status.md`/`006-status.md` with exact-SHA wheels, sdist rebuild, digests, and root compatibility. No publication authorized by this closure.
