# Python Native Library M002 — PyO3 Asyncio Execution and Lifecycle Closure

Status: closed
Candidate: `2392e72178aa249445a64bd629caac8219353499`
Implementation plan: `plans/implementation/python-native-library/002-pyo3-async-execution-and-lifecycle.md`
Predecessor: `plans/closure/python-native-library/001-status.md`

## Delivered

- Added an independent, unpublished `bindings/python` PyO3 cdylib package with a mixed maturin layout. Root crate defaults and release artifact inventory remain unchanged.
- Bound `AsyncEggsearch.execute` to the shared Rust SDK facade and canonical ten-tool inventory. Imports do not initialize the runtime or execute work; the runtime starts lazily on the first operation.
- Reused one process-scoped Tokio runtime, with a shared 32-operation concurrency cap. Per-client close rejects new operations, cancels active Rust futures, drains them, and is idempotent. Python task cancellation drops the operation future.
- Bounded Python-to-JSON conversion to 1 MiB, depth 64, and 10,000 values; rejected non-dict roots, non-string keys, cycles, unsupported objects, non-finite values, out-of-range integers, invalid Unicode, and oversized values.
- Preserved structured SDK error fields and sanitized panics at the native boundary. No Python reference is held across the asynchronous Rust operation.
- Added package-local tests and synchronized `docs/test-inventory.md`, `architecture/testing.md`, `architecture/python-library.md`, and `skills/eggsearch-dev/SKILL.md`.

## Verification on the exact candidate

| Check | Result |
|---|---|
| Rust 1.89 locked extension check: `rustup run 1.89.0 cargo check --locked --manifest-path bindings/python/Cargo.toml` | Passed |
| CPython 3.14.2 editable native build with maturin 1.9.5 | Passed |
| Python package suite: `python -m pytest -q bindings/python/tests` | 12 passed |
| Python source compilation: `python -m compileall -q bindings/python/python/eggsearch` | Passed |
| Root `make check` | Passed, including formatting, Clippy, no-default check, all-feature tests, hygiene, dependency policy, and packaging contract |
| Rust SDK/MCP parity and Mode-Off zero-dispatch counter | Inherited exact-candidate evidence from M001 closure; Python Mode-Off consumer calls also passed without remote providers |

The first root gate attempt had one failure in the existing
`meta::local_backend::tests::multiple_roots_share_global_cap` assertion. That
test passed alone, and the complete `make check` rerun passed. No source change
was made for that transient result.

## Compatibility and remaining limits

The extension was built and exercised locally on CPython 3.14.2, macOS x86_64,
Rust 1.89.0, PyO3 0.29.3, `pyo3-async-runtimes` 0.29.0, and maturin 1.9.5.
CPython 3.11–3.13 and other wheel platforms are not yet runtime-qualified;
M004 owns that matrix. The distribution name is a temporary unpublished
`eggsearch-native` pending M004's PyPI name and metadata preflight. No wheel or
sdist was published. The process-scoped Tokio runtime intentionally remains
alive until process exit.

## Security and behavior evidence

The fixture suite covers all ten tool names in concurrent asyncio calls,
invalid and oversized Python inputs, cyclic containers, task cancellation,
idempotent close, rejection after close, and sequential Python event loops.
The Rust bridge applies the SDK's existing request validation and policy. The
root all-feature suite and dependency-policy checks passed on this candidate.

## Handoff

M002 is closed. M003 may add Python async context management, named typed
methods, `TypedDict` contracts, examples, and semantic parity coverage while
continuing to delegate every operation through `execute` and the Rust SDK.
M004 remains gated by M003 closure and platform/distribution qualification.
