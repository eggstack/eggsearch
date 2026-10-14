# Python Native Library

The Python library is an additive native consumer of the ten existing tool
operations. Its ownership boundary is registered in
`plans/adrs/ADR-0005-python-native-library-boundary.md` and sequenced by
`plans/subsystems/python-native-library-roadmap.md`.

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
adaptation; all execution delegates through `src/sdk.rs`.

Milestone status and exact-candidate evidence live in
`plans/registry.md` and `plans/closure/python-native-library/`. A compiled
extension alone is not evidence of a supported Python consumer. Wheel support
is limited to targets with native install and runtime qualification, and
publication requires separate authorization.
