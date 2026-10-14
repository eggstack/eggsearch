# M002 — PyO3 Asyncio Execution and Lifecycle

Status: active
Primary class: capability
Source: `plans/subsystems/python-native-library-roadmap.md` M002
Planning baseline: M001 SDK candidate `5f686d02e15daa2c5db7ad6d889c06eaff3ceeee`; M001 closure evidence is `plans/closure/python-native-library/001-status.md`.
ADRs: `ADR-0001`, `ADR-0005`; long-term sections 1-4, 6-9 of `plans/000-long-term-specification.md`.

## Objective, readiness, non-goals

Build a native PyO3 extension exposing `AsyncEggsearch` and async `execute(tool, arguments)` across the full ten-tool facade, usable inside a Python asyncio event loop with no MCP process, HTTP listener, or external Eggsearch executable. This milestone is NOT ready until `plans/closure/python-native-library/001-status.md` proves closed M001 parity.

Non-goals: public wheel publishing, PyPI release, optional LangChain/CrewAI/etc integration, synchronous API, direct engine access, browser bundling, framework-specific models, TypeScript/C ABI, claiming unsupported Python interpreters.

## Existing evidence and layout

Rust engine and handler state already exist; `bindings/python/` does not. Use an **independent non-workspace** `bindings/python/Cargo.toml` `cdylib` with a path+version dependency on root Eggsearch and an isolated `pyproject.toml` mixed source layout (`bindings/python/python/eggsearch/`). Configure `tool.maturin.module-name` for `eggsearch._native` and define a deliberate CPython-facing package API. Do not modify root `crate-type`/default features or pull `pyo3` into the root crate. Confirm root `include`/`exclude` manifest and maturin sdist/lockfile constraints before selecting the final layout.

## Ordered work packages

**WP1 — Pin supported compatible versions.** Verify PyO3, `pyo3-async-runtimes`, and maturin against Rust 1.89, CPython 3.11-3.14, and repo's dependency graph. Modern reference: `https://docs.rs/pyo3-async-runtimes/latest/pyo3_async_runtimes/tokio/fn.future_into_py.html`; `https://www.maturin.rs/`. Pin bounded semver constraints and a reproducible lock/toolchain strategy. Do not assume Rust extension wheel features and free-threaded Python have identical ABI. Keep default initial build without `browser`, `egress` or PDF feature unless explicitly qualified.

**WP2 — Native bridge.** Implement extension-owned opaque client storing the Rust SDK facade, config and close-state. Expose constructor with defaults/config path and an `execute(name: str, arguments: dict[str, Any]) -> Awaitable[dict[str, Any]]` pathway. Marshal args at boundary using a bounded JSON-compatible conversion (reject non-string mapping keys, cyclic containers, nonfinite numbers, unsupported Python objects, invalid strings, and oversized payloads with deterministic errors); never convert source text into executable Python. Serialize canonical tool JSON to Python containers preserving key names, `null`, Unicode and numeric semantics. Add structured exceptions with `code`, `message`, `repair` and optional `data`, mapping internal panic/faults to sanitized internal errors only.

**WP3 — Async bridge ownership.** Use `pyo3-async-runtimes` Tokio integration (or rigorously tested equivalent) to produce awaitables. Reuse a bounded shared Rust runtime and stable Python-event-loop association; never create a Tokio runtime/thread per operation or block asyncio. Validate that PyO3's Python references are not held across `await` in invalid ways, locks are not held while calling Python, and all Rust futures satisfy bridge `Send` requirements. Include concurrency budgeting and no hidden background worker orphaning.

**WP4 — Cancellation, close, failure.** `aclose()` is idempotent; `async with` is supported through Python wrapper; no new calls after closing (stable error). Test Python task cancellation and Rust future drop/abort, prompt termination of queued work and bounded completion for unavoidable in-flight transport, timeouts and partial provider failures; do not promise instantaneous socket termination until proven. Test repeated creation/destruction, Python loop changes, a second client sharing state, interpreter exit, invalid config, missing credentials, no network on import, exception mapping and no panic crossing FFI. If per-runtime executor teardown is unsafe, document process-scoped runtime and explicit close semantics and test them.

**WP5 — Native consumer smoke and guard hygiene.** Demonstrate `asyncio.gather` of multiple `execute("web_search", {...})` and fetch/evidence calls against no-network fixtures. Add package-local pytest integration and offline Rust parity assertions. Check no child PID/server socket created and that import itself doesn't dial. Include negative Mode-Off `repo_search` with counters supplied by Rust fixture; do not rely on actual Internet success.

## Storage, protocol, security and compatibility

No new persistence or MCP protocol; Python acts as an in-process client of the same bounded tool semantics. Config path and resolved credentials adhere to `AppConfig`. Reject malformed requests before allocating unbounded buffers. Preserve external-untrusted labels, injection markers, provider skips, `response_detail` and errors. Uncaught panics and Python interpreter shutdown cannot corrupt other Rust clients. Rust root may retain `#![deny(unsafe_code)]`; all extension FFI constructs isolated in the new crate.

## Verification

- `make check` at root; `cargo test --locked --all-features`.
- From `bindings/python` after selecting tested toolchain: `maturin develop --manifest-path Cargo.toml` in venv; `python -m pytest -q` for unit and fixture smoke; `python -m compileall python/eggsearch`.
- Dedicated tests for 10/10 operation names, invalid input classes, concurrent futures, cancellation/timeout, loop lifecycle, multi-client/close and import safety. Record actual commands and Rust/Python/tool versions; update `docs/test-inventory.md` and package test metadata.

Acceptance: one CPython environment can import and use an asyncio native client calling all ten tools directly with correct JSON/errors; concurrent calls remain responsive; cancellation and close have verified bounded behavior; existing Rust/MCP gates green. STOP if bridge versions force a root MSRV upgrade or bypass policy, or if future cancellation leaves unbounded background work; register scoped corrective.

Closure: `plans/closure/python-native-library/002-status.md` with runnable demo, exact SHA, all ten parity cases, bounded cancellation/shutdown results, security negatives, test evidence and known platform limitations. Update roadmap+registry M002 closed, M003 ready atomically.
