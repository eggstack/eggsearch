# Python Native Library

The Python library is an additive native consumer of the ten existing tool
operations. Its ownership boundary is registered in
`plans/adrs/ADR-0005-python-native-library-boundary.md` and sequenced by
`plans/subsystems/python-native-library-roadmap.md`.

Current handoff: M001's shared Rust facade and ten-operation parity are
closed at `plans/closure/python-native-library/001-status.md`; M002's native
asyncio bridge is closed at `plans/closure/python-native-library/002-status.md`;
M003's Python API, types, examples, and semantic parity work is closed at
`plans/closure/python-native-library/003-status.md`; M004 distribution
qualification is active.

## Rust operation facade

`src/sdk.rs` owns the narrow `EggsearchClient`, canonical `ToolName` mapping,
shared `ServerState` lifetime, and structured `SdkError` conversion. It
delegates every operation to the matching function under `src/mcp/tools/`.
Those handlers remain the only owner of argument semantics, provider and
fetch policy, sanitization, evidence identity, warnings, and result shape.
The facade returns tool JSON payloads without MCP envelopes. Cloned clients
share one state; the caller supplies the Tokio runtime.

The facade does not make the rest of the crate's module tree a stable general
Rust library. It introduces no Python dependency, runtime, process, listener,
or background task into the root crate. `ToolName` stays aligned with the
code-derived `tool_contract::ALL_CONTRACTS` inventory.

## Python binding and distribution boundary

The Python package lives in an independent `bindings/python` Cargo package.
PyO3 and Python packaging dependencies must not enter the root crate's
default dependency graph or change its binary artifact inventory. Python
import is side-effect-free. The binding owns Python types and lifecycle
adaptation; all execution delegates through `src/sdk.rs`. The PyO3 layer
initializes one process-scoped Tokio runtime on first operation, caps shared
in-flight calls at 32, and bounds JSON conversion to 1 MiB, 64 levels, and
10,000 values. `aclose()` rejects new work, cancels active operations, and
waits for their Rust futures to drop; Python task cancellation drops its
corresponding future. The runtime intentionally remains process-scoped.

Milestone status and exact-candidate evidence live in
`plans/registry.md` and `plans/closure/python-native-library/`. A compiled
extension alone is not evidence of a supported Python consumer. Wheel support
is limited to targets with native install and runtime qualification, and
publication requires separate authorization.

M004's candidate binding uses PyO3's `abi3-py311` stable ABI. Its release
matrix builds a manylinux2014 Linux x86_64/aarch64 wheel, macOS x86_64/arm64
wheels, and a Windows x86_64 wheel, then installs each exact artifact on
CPython 3.11–3.14 in separate consumer jobs. These platform claims remain
conditional on the matching exact-candidate matrix jobs passing. An sdist
rebuild uses the binding and root Cargo manifests from clean extraction; it can
build offline with locked crates already cached, otherwise Cargo registry
access is required. PyPy, free-threaded CPython, ARMv7, and Windows ARM64 are
outside the support matrix.

The public Python `AsyncEggsearch` wrapper adds `async with`, `aclose()`, and
the ten named keyword methods over the generic native `execute` call. Request
and response `TypedDict` definitions live in `bindings/python/python/eggsearch/types.py`;
they describe common JSON fields while runtime dictionaries retain additive
keys. The `py.typed` marker and native extension stub make the package
discoverable to type checkers. `tool_names()` comes from the Rust SDK's
code-derived canonical inventory. `tool_contracts()` returns input/output
schemas and the MCP contract fingerprint from the existing server definitions;
the Python test suite checks request type coverage against advertised schema
properties.
