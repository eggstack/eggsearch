# Plan 003 — MCP 2026 Protocol, Structured Results, and Repairable Error Contract

Status: implementation plan
Scope: eggsearch MCP server transport/result semantics; CodeGG compatibility notes
Depends on: Plan 001

## Objective

Bring eggsearch's MCP behavior in line with the current 2026-07-28 specification while preserving compatibility with older clients. In particular, use native structured results and output schemas where practical, separate protocol errors from recoverable tool execution errors, add standardized annotations, and prepare the server/client boundary for modern MCP version negotiation.

## Current-state observations

Eggsearch currently returns successful JSON through `ContentBlock::json(...)` and maps every `ToolError::Validation` to JSON-RPC `invalid_params`. This is simple but conflates malformed/protocol requests with recoverable semantic tool errors such as an unsupported enum value, contradictory workflow choice, unavailable provider, or out-of-range request field.

The current MCP specification makes `structuredContent` and `outputSchema` first-class and, as of the 2026-07-28 revision, supports full JSON Schema 2020-12 for tool schemas. The 2026 transport model also removes the mandatory initialization/session lifecycle, adds `server/discover`, and makes list responses cacheable. Migration should therefore be compatibility-first rather than an abrupt cutover.

## Non-goals

- Do not remove support for the current/legacy MCP path used by CodeGG until CodeGG has negotiated support.
- Do not make error handling less typed internally.
- Do not expose internal diagnostics or stack traces to tool callers.
- Do not require output schemas for every deeply nested field in the first implementation pass if doing so creates brittle duplication.

## Workstream A — Structured tool results

### 1. Use native `structuredContent`

Replace the current success helper with a result builder that populates MCP `structuredContent` natively where supported by `rmcp`.

Maintain a textual/content fallback for older clients if the negotiated protocol/library requires it. Do not make CodeGG parse truncated text to recover stable IDs or structured evidence.

### 2. Define output schemas for stable envelopes

Add `outputSchema` for tools whose top-level result shape is stable and already represented by typed Rust structures. Prioritize:

1. `web_search`
2. `web_fetch`
3. `repo_search`
4. `repo_fetch`
5. `repo_map`
6. `security_search`
7. `research_search`
8. `batch_fetch`
9. `provider_status`
10. `build_evidence_bundle`

Use generated schemas from the existing response types where possible rather than hand-maintaining parallel JSON.

If some response contains intentionally open-ended metadata, keep that portion permissive while validating the stable envelope.

### 3. Add output validation tests

For every declared `outputSchema`, serialize representative success/partial/degraded responses and validate that `structuredContent` conforms.

Include old-client compatibility fixtures if the library wraps non-object structured output differently by protocol version.

## Workstream B — Error taxonomy

### 1. Split protocol/input-shape errors from tool execution errors

Refactor `ToolError` into categories that reflect what the caller can do next. A suggested shape is:

```rust
pub enum ToolError {
    InvalidRequest(String),      // malformed argument shape / protocol-level issue
    Execution {
        code: ToolErrorCode,
        message: String,
        data: Option<Value>,
        repair: Option<RepairHint>,
    },
    Internal { ... },
}
```

Exact names are flexible, but semantics are not.

Use JSON-RPC `invalid_params` only when the request cannot be interpreted as a valid invocation shape for the tool. Return ordinary MCP tool errors (`isError: true`) for semantic/actionable failures that an agent can repair and retry.

Examples that should normally be repairable tool errors:

- unknown semantic enum value after successful JSON decoding;
- canonical/legacy workflow conflict;
- unsupported provider requested;
- provider configured but disabled;
- `dependency_files` requested without an enabled local workspace;
- invalid range/cap combinations;
- unavailable optional capability such as browser/PDF/native forge behavior.

Examples that may remain protocol invalid-params:

- required top-level field absent when the input schema requires it;
- field has a fundamentally wrong JSON type;
- input cannot be decoded against the advertised schema.

### 2. Add stable error codes

Introduce compact machine-readable codes, e.g.:

- `invalid_semantic_value`
- `conflicting_arguments`
- `capability_unavailable`
- `provider_unavailable`
- `policy_denied`
- `budget_invalid`
- `locator_invalid`
- `manual_interaction_required`
- `upstream_failed`

Reuse existing browser/manual-interaction data rather than creating a second error vocabulary.

### 3. Add repair hints

Where deterministic, include a bounded repair object such as:

```json
{
  "field": "workflow",
  "accepted": ["architecture", "debug", "migration"],
  "suggested_value": "debug"
}
```

Keep this machine-readable and bounded. Do not return hidden reasoning.

### 4. Centralize MCP error/result mapping

Remove the repeated per-handler `match` blocks in `src/mcp/server.rs`. Implement one conversion seam that maps internal `ToolError` to either:

- JSON-RPC error;
- `CallToolResult { isError: true, ... }`;
- successful structured result.

Every tool handler should delegate to that seam.

## Workstream C — MCP annotations and schema metadata

Consume Plan 001's tool contracts to expose standardized annotations where supported:

- `readOnlyHint`
- `openWorldHint`
- other applicable MCP tool annotations supported by the selected `rmcp` release.

Annotations remain hints and must not replace policy enforcement.

For advanced-only fields, investigate whether JSON Schema 2020-12 metadata can mark them clearly without inflating descriptions. Avoid non-standard metadata unless CodeGG explicitly consumes it.

## Workstream D — 2026-07-28 protocol compatibility

### 1. Audit `rmcp` support

Before modifying transport code, determine the minimum `rmcp` version that supports:

- MCP 2026-07-28;
- `server/discover`;
- stateless request handling;
- current standard headers;
- list caching semantics;
- full tool schema 2020-12 behavior.

Upgrade `rmcp` only if needed and record MSRV/dependency implications.

### 2. Support dual-era negotiation

Do not remove legacy initialize/session support immediately. Implement or configure negotiation so:

- modern clients can use `server/discover` and 2026-07-28 semantics;
- older CodeGG/current clients continue using the existing lifecycle;
- tool names and request/response meaning stay stable across eras.

### 3. Deterministic/cacheable tool lists

Ensure `tools/list` ordering is deterministic and changes only when the actual public contract changes. Expose appropriate cache metadata/version identity if supported by the library.

A deterministic hash/fingerprint of canonical tool contracts and advertised schemas should be available to tests and optionally diagnostics.

### 4. Transport tests

Extend `tests/mcp_http.rs` with modern and legacy fixtures. Cover:

- discovery/version negotiation;
- stateless modern call path;
- old initialize-based path;
- required protocol headers on modern HTTP;
- structured success results;
- repairable tool error result;
- true protocol invalid-params error;
- deterministic `tools/list` across repeated calls.

## CodeGG coordination

CodeGG currently documents an older initialize-based client. Add a handoff note for the CodeGG implementation plan:

- negotiate 2026-07-28 when supported;
- keep fallback for old MCP servers;
- prefer `structuredContent` over parsing JSON from text/content blocks;
- validate `outputSchema` when practical;
- distinguish tool-level `isError` from transport/protocol failure;
- cache deterministic tool definitions by content fingerprint rather than count.

Do not block eggsearch's internal error cleanup on CodeGG transport modernization if backward-compatible content fallback can preserve current operation.

## Acceptance criteria

- Successful typed tool calls produce native structured MCP content on the modern path.
- Stable output envelopes have generated `outputSchema` coverage or a documented exception.
- Semantic validation failures no longer universally become JSON-RPC `invalid_params`.
- Repairable failures contain a stable code and bounded repair information where deterministically available.
- MCP error/result conversion is centralized rather than duplicated across ten handlers.
- Modern 2026-07-28 and legacy client paths are both covered by tests.
- `tools/list` is deterministic and suitable for client caching.
- Existing CodeGG integration tests remain passing throughout the migration.

## Verification

```bash
cargo test --all-features mcp
cargo test --all-features --test mcp_http
cargo test --all-features --test mcp_tools
cargo test --features mock provider_request_contract
make check
make docs-check
```

Document the negotiated protocol versions and `rmcp` version in the closure report.