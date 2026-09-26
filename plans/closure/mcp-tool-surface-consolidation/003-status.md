# Tool-Surface Consolidation M003 — MCP 2026 Protocol Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/mcp-tool-surface-consolidation/003-mcp-2026-protocol-and-error-contract.md`

Source subsystem roadmap:

- `plans/subsystems/tool-surface-consolidation-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

Repository baseline reviewed: `4b7e725` (M001 contract; proceeds in parallel with M002).

Soft dependency: M001; satisfied.

Implementation commit:

- `c102ac608ae89d7f3515858ec9a4447f352418a9` — structured results, output schemas, repairable error contract, dual-era transport

Closure candidate: `aab2e776b5177c3451713db4c737f1da8650318b` (no
protocol/error-mapping change since `c102ac6`; M004 projection consumes
the structured results, M006/M007 add only harness and guards).

## 1. Executive finding

M003 is complete. Typed calls produce native `structuredContent` with
text fallback on the modern path; all ten tools carry generated compact
output schemas; semantic failures are repairable tool errors
(`isError: true` with stable codes and bounded repair hints) while only
true shape failures become JSON-RPC `invalid_params`; error/result
mapping is centralized; modern 2026-07-28 and legacy paths both pass;
`tools/list` is deterministic and fingerprinted.

## 2. Requirement-to-evidence matrix

| Requirement (plan acceptance) | Evidence | Result |
|---|---|---|
| Native structured content on modern path | `success_maps_to_native_structured_content_with_text_fallback` | pass |
| Output-schema coverage or documented exception | `all_stable_tools_have_output_schemas` (10/10 object envelopes) + `typed_output_schemas_validate_representative_payloads` | pass |
| Semantic failures no longer universal invalid_params | `semantic_validation_becomes_repairable_tool_error_not_invalid_params`, `canonical_goal_conflict_is_repairable_with_code` | pass |
| Stable codes + bounded repair where deterministic | `execution_errors_carry_stable_codes_and_bounded_repair`, `repair_hints_are_bounded` (accepted ≤20), `canonical_invalid_goal_carries_accepted_values` | pass |
| Centralized conversion, not ten handler matches | `map_tool_result()` seam in `src/mcp/tools/` consumed by all handlers | pass |
| Modern + legacy paths covered | `tests/mcp_http.rs` dual-era fixtures (11 tests: discovery/negotiation, stateless modern, initialize legacy, headers, structured success, repairable error, invalid-params, deterministic list) | pass |
| Deterministic cacheable tool list | `tools_list_is_deterministic_and_fingerprinted` + `output_schemas_are_stable_across_calls` (name-sorted, 16-hex FNV-1a fingerprint) | pass |
| CodeGG integration tests pass | `provider_workstream_regression`, `mcp_tools`, `codegg_evidence_contract` green | pass |

## 3. Production implementation evidence

- `src/mcp/output_schema.rs`: per-tool compact stable envelopes
  (each ≤1200 bytes; fully-expanded typed schemas measured ~285k and
  rejected in favor of compact envelopes). Total tool-definition
  budget 70k → 72k for this coverage.
- `src/mcp/tools/` error taxonomy: `InvalidRequest` → JSON-RPC
  `invalid_params`; legacy `Validation` and new
  `Execution{code,message,data,repair}` → `isError` tool errors;
  `Internal` → `internal_error` without stack traces. Stable codes
  (`invalid_semantic_value`, `conflicting_arguments`,
  `capability_unavailable`, `provider_unavailable`, `policy_denied`,
  `budget_invalid`, `locator_invalid`, `manual_interaction_required`,
  `upstream_failed`); canonical translators emit
  `InvalidSemanticValue`/`ConflictingArguments` with `RepairHint`.
- Transport: `rmcp 3.2.0` (already covering MCP 2026-07-28:
  `server/discover`, stateless metadata) with legacy
  initialize/session retained; no `rmcp` upgrade required.
- `tests/mcp_2026_protocol.rs`: 13 tests; `tests/mcp_http.rs`: 11
  tests including modern/legacy lifecycle and deterministic list with
  output schemas.

## 4. Verification executed

Against candidate `aab2e77`:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test mcp_2026_protocol
cargo test --locked --all-features --test mcp_http
cargo test --locked --all-features --test mcp_tools
cargo test --locked --features mock --test provider_request_contract
cargo test --locked --all-features
make docs-check
```

Outcomes:

- `mcp_2026_protocol`: 13 passed, 0 failed
- `mcp_http`: 11 passed, 0 failed
- `mcp_tools`: 133 passed, 0 failed
- `provider_request_contract` (mock): 21 passed
- `cargo test --locked --all-features`: 81 suites ok, 0 failed
- `make docs-check`: pass

Negotiated versions: `rmcp 3.2.0`; MCP 2026-07-28 modern path plus
legacy initialize path (dual-era, legacy removal explicitly deferred
until CodeGG no longer requires it).

## 5. Invariant review

Error handling remains typed internally; no diagnostics or stack
traces leak to callers; output schemas validate stable envelopes while
open-ended metadata stays permissive.

## 6. Failure and recovery review

Repairable: unknown enum values, canonical/legacy conflicts,
unsupported/disabled providers, missing-workspace `dependency_files`,
bad ranges/caps, unavailable browser/PDF/forge capabilities (all
`isError` with code + bounded repair). Protocol-level: absent required
fields, wrong JSON types, undecodable inputs (all `invalid_params`).

## 7. Migration and compatibility review

Dual-era negotiation preserves older CodeGG/current clients; tool
names and request/response meaning stable across eras; CodeGG handoff
notes recorded (`structuredContent` preferred, `outputSchema`
validation when practical, `isError` vs transport failure
distinguished, fingerprint-based caching).

## 8. Security review

No error path exposes internals; repair hints are bounded
machine-readable objects, never hidden reasoning.

## 9. Documentation and operations

`architecture/mcp.md`, `architecture/codegg-contract.md`,
`docs/tool-matrix.md`, `docs/codegg-integration.md` (2026 handoff),
README, AGENTS.md, and skills record the structured/output-schema/
error contract with the exact `rmcp` version and dual-era support.

## 10. Residual findings

| Severity | Finding |
|---|---|
| Low, accepted | Deeply nested response fields use compact envelopes rather than exhaustive schemas by design (avoids brittle duplication). |
| None high/medium | No protocol or compat regression identified. |

## 11. Roadmap disposition

M003 moves to closed with this record as controlling evidence.
M004 consumes structured capture; M006/M007 already closed.

## 12. Registry updates

Covered in the same closure commit: M003 marked closed with closure
record `plans/closure/mcp-tool-surface-consolidation/003-status.md`.
