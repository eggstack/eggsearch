# M001 — Opt-in Compact Ten-Tool MCP Advertisement

Status: active
Primary class: capability
Planning baseline: eggstack/eggsearch main `a666517c39adf28f32fad04198c74cb70d935c5b` (2026-10-30)
Roadmap: `plans/subsystems/standalone-mcp-local-inference-roadmap.md`
ADRs: `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`; `plans/adrs/ADR-0006-keyless-deterministic-retrieval-and-optional-extraction.md`
Hard dependencies: none. Previously closed tool-surface M001-M007 are the retained baseline.

## Objective

Create a **process-configured, opt-in** compact MCP tool-definition profile for direct local-model hosts that advertise all ten stable canonical tool names but far fewer ordinary arguments and metadata bytes. Full/default `tools/list` and tool-call behavior must be unaffected. This milestone concerns advertised contract *size*, not search quality or response content.

## Current evidence and ownership

- `src/mcp/server.rs` assembles ten decorated tools once at construction and returns all from `list_tools()`. `contract_fingerprint()` hashes the current advertised name, descriptions, annotation and schemas. The handler router is separate from the advertised list.
- `src/mcp/output_schema.rs` attaches permissive per-tool output schemas; `src/mcp/tool_contract.rs` already has concise canonical selection descriptions, aliases and discovery metadata.
- `src/mcp/tools/{web_search,repo_search,research_search,security_search,web_fetch,repo_fetch,batch_fetch,repo_map,evidence_bundle,provider_status}.rs` deserialize the canonical runtime argument structs; older advanced fields are sometimes `#[schemars(skip)]` while remaining accepted.
- `src/mcp/mod.rs::build_server` is shared by client-owned stdio and loopback HTTP; `src/commands/mcp.rs` is transport startup. `src/core/config.rs` / `src/config.rs` own config parsing and validation.
- `docs/retrieval-quality-context-efficiency.md` records 72,950 full `tools/list` serialized bytes, 18,481 bytes for the compact ten-tool profile, and 14,008 bytes for a three-tool subset. Token figures are a four-bytes/token proxy, not a measured LM Studio prompt cost.

## Invariants / non-goals

Retain the ten canonical names and their exact tool-call dispatch/permission semantics, default full tool definitions, default diagnostic response behavior, existing protocol revisions, existing validation, hidden legacy argument acceptance, all provider/repo/PDF/browser capabilities, CodeGG wrapping, Python SDK and wire compatibility. No mega-tool, 11th discovery tool, new dynamic-client protocol, inferred model detection, output truncation or per-client identity-based behavior. Compact metadata is not a permission wall.

## Intended contract

Add a validated process-level configuration (illustrative spelling; pick one canonical name consistently in code/docs):

```toml
[mcp]
schema_profile = "full"  # full (default) | compact
```

`full` must serialize unchanged except for strictly necessary additive configuration behavior. `compact` advertises exactly the same ten names and minimum valid inputs: primarily `query`/`url` or current structured locator, task-specific discriminator (`goal`/`include`/`sources` only where necessary), useful budget (`max_results`/`max_chars`), and a response-detail override. Preserve required keys, JSON type, default, enum and nested-required semantics. Suppress nonessential schema descriptions, advanced provider knobs, nested schemas that are only diagnostics, and potentially `outputSchema` only after verifying compatibility. Do not produce a schema that falsely forbids accepted advanced arguments: use a documented compatible `additionalProperties` policy, or provide truthful constrained schema if the runtime cannot accept extra properties.

All inputs (including unadvertised advanced fields) must continue through the same existing Serde deserializer and validation path. Advanced capability access is through `full` presentation or explicit arguments from clients that know the contract. All ten tools must remain visible and selectable in compact, not moved into an uncallable deferred category.

## Ordered work packages

1. Re-run current locked baseline: enumerate `tools/list` with a real stdio MCP initialization and native loopback HTTP where feasible. Record exact bytes per tool split into name/description/input/output/annotations, instructions bytes, selected-client model-tokenizer counts, and repository SHA. Distinguish total wire bytes from actual LM Studio prompt inclusion.
2. Define the `[mcp]` config owner and validation with default `full`; wire the same immutable `McpSchemaProfile` into the shared `EggsearchServer` construction for both transports. Do not change server settings based on an incoming untrusted MCP request or client `User-Agent`.
3. Build compact metadata from the existing registered tool contracts. Prefer a pure deterministic schema projector applied only to the advertised copy. Preserve `ToolRouter` and actual argument types; avoid duplicating the handlers. Carefully handle required parameters and JSON Schema refs/defs; schema validity is tested using the exact supported protocol representation.
4. Establish profile-aware `tool_fingerprint` based on actual serialized definitions, instructions and schema profile as appropriate. Verify deterministic order, cross-process stability, profile separation, and no compatibility drift in full.
5. Add per-profile contract fixtures, per-tool byte budgets and a frozen accepted-input matrix covering advanced aliases, provider selection, repo/security/research constraints, PDF/browser configuration and exact errors. Run full-profile vs compact-profile identical dispatch inputs; assert same normalized outcomes on mock backends.
6. Update MCP/config/operator docs and code-derived tool-name inventory tests. Avoid hard-coding the local-client implementation into a generic protocol feature.

## Required verification

```bash
cargo fmt --check
cargo test --locked --features mock --test mcp_tool_contract
cargo test --locked --features mock --test mcp_schema_slimming
cargo test --locked --features mock --test tool_surface_evaluation -- --nocapture
cargo test --locked --features mock --test mcp_tools
cargo test --locked --all-features --test static_guards
make check
make docs-check
make bench-check
```

Add a focused `standalone_mcp_profiles` suite (or extend existing `mcp_tool_contract` and `mcp_2026_protocol` suites) exercising real MCP initialized `tools/list` and mock calls; update test inventory and architecture/testing ownership. Run both stdio and loopback HTTP negotiation in offline integration tests when supported.

## Acceptance / stop

- Full profile remains bit-for-bit equivalent for advertised definitions, stable names, per-call behavior and default response mode (excluding unavoidable version/build metadata). No existing consumer config requires migration.
- Compact profile advertises all 10 names, each input schema validates, all existing advanced requests still work, and protocol adapters do not lose access to any capability.
- Recorded pre/post serialized total target **<= 36,000 bytes (~50% of baseline)** for compact across ten tools, with stretch goal <= 24,000; do not silently downgrade semantics to meet the target. If 36 KB cannot be reached without invalid schema/host interoperability, document evidence, stop threshold claim, and propose a narrow follow-up rather than force a breaking shape.
- At least the repository's 43-case deterministic tool-selection/discovery suite remains above its existing accuracy gates; separately record compact-profile results and any tools becoming ambiguous.
- Both MCP transports, mock domain workflows, and CodeGG compatibility cases remain green.

Stop on a missing required field, JSON Schema incompatibility, material tool-selection regression, authorization bypass, or change to full-mode behavior. Record exact candidate SHA, serialized bytes, compatibility matrix, CI commands/outcomes and residual risk in `plans/closure/standalone-mcp-local-inference/001-status.md`. M002 becomes ready only on accepted M001 closure.
