# Standalone MCP Local-Inference M001 — Compact Advertisement Closure

Status: closed

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/001-opt-in-compact-tool-advertisement.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Repository baseline reviewed: `a666517c39adf28f32fad04198c74cb70d935c5b`
(2026-10-30 main baseline).

Implementation candidate: `02a8af6fe4f87971c7daca7d1fca20c087b328ec`
(compact ten-tool advertisement plus budgeted brief output and the corrective
summary-budget pass).

## 1. Executive finding

M001 is closed. The opt-in `[mcp].schema_profile = "compact"` advertises all
ten canonical tools with required inputs intact, while leaving the default
profile as `full`. Real initialized stdio MCP enumeration measured 72,923 bytes
for full and 18,545 bytes for compact (74.6% less). Full and compact both
scored 43/43 top-1 and 43/43 recall@3 with MRR 1.000 in the deterministic
discovery corpus. Loopback HTTP and stdio use the same configured contracts.

The full contract includes the additive `response_detail = "brief"` enum
introduced by M002; omitted response detail and all other defaults retain the
full/diagnostic behavior. This additive option is recorded as a M002 contract
extension, not a schema-profile behavior change.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Opt-in process-level configuration | `src/core/config.rs`, `tests/config_validation.rs` | pass | `full` is default; invalid profiles reject |
| Ten canonical names remain advertised | `tests/mcp_tool_contract.rs`, `tests/mcp_http.rs`, installed-candidate stdio smoke | pass | Compact names and required keys retained |
| Schema-size reduction meets 36 KB limit | Initialized stdio child `tools/list`: full 72,923 bytes, compact 18,545 bytes | pass | Compact is 74.6% smaller; both profiles expose ten tools |
| Both MCP transports share the profile | `tests/mcp_http.rs`, `tests/mcp_tool_contract.rs` | pass | HTTP initialization and stdio contract paths exercised |
| Runtime behavior/default remain intact | Full and compact `provider_status(probe=false)` produce equal structured output; existing MCP/tool suites | pass | Hidden advanced inputs continue through existing handlers |
| Tool discovery remains above existing gate | `tests/tool_surface_evaluation.rs` | pass | Full and compact: 43/43 top-1, 43/43 recall@3, MRR 1.000 |
| Profile fingerprint reflects advertised contract | `tests/mcp_tool_contract.rs` | pass | Deterministic within profile and distinct between profiles |

## 3. Production implementation evidence

`McpSchemaProfile` selects an advertised-only projection in
`src/mcp/server.rs`; it does not replace handlers or runtime argument
validation. Compact retains every canonical tool name, required property and
accepted additional property behavior, and omits optional output schemas and
nonessential descriptions. Profile-specific fingerprints derive from the
serialized advertised contracts. Stdio and loopback HTTP share the same
configuration path.

An installed-candidate child-process smoke initialized MCP over stdio, listed
all tools and called `provider_status` with `probe=false`. It completed with
exit code 0 for both profiles and identical structured call results. The
recorded measurements are wire bytes from the child response; estimated
token costs are documented separately and are not LM Studio prompt-token
measurements.

## 4. Verification executed

- `rtk make check` — pass; formatting, Clippy, no-default-features check,
  all-features test gate (3,486 tests), docs, hygiene, dependency policy and
  packaging contract checks passed.
- `rtk make docs-check` — pass.
- `rtk make bench-check` — pass; optimized Criterion harness compiled with
  `--no-run` (11m32s on this host).
- `cargo test --locked --all-features --test mcp_tool_contract` — pass.
- `cargo test --locked --all-features --test mcp_http` — pass (12 tests).
- `cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture`
  — pass (5 tests; full and compact 43/43).
- `cargo test --locked --all-features --test mcp_2026_protocol` — pass
  (14 tests).
- `cargo test --locked --all-features --test mcp_schema_slimming` — pass
  (17 tests).
- `rtk git diff --check` — pass.

## 5. Invariant review

The ten-tool registry, handlers, tool names, dispatch/permission semantics,
provider routing, search budgets, stable IDs, CLI/Python surfaces and default
full profile remain in place. Compact is presentation metadata only; it does
not act as an authorization boundary. Runtime deserialization remains
authoritative for advanced fields omitted from compact schemas.

## 6. Failure and recovery review

The ordinary MCP and workflow suites pass. The separate LM Studio live-model
qualification remains M003 work; no host prompt behavior or model-quality
claim is inferred from the offline byte and discovery measurements. Operators
can return to `schema_profile = "full"` and restart the MCP process.

## 7. Migration and compatibility review

No existing config requires migration. Omitted `schema_profile` remains
`full`. Compact is explicit opt-in. An additive `brief` detail value is
available for callers choosing that response mode; existing detail values
and the full-profile diagnostic default remain available.

## 8. Security review

No trust, filesystem, SSRF, provider-egress, authorization, or tool-execution
policy changed. Compact schemas do not narrow runtime capabilities or imply
permission denial.

## 9. Documentation and operations

Configuration and behavior are documented in `docs/config.md`,
`architecture/config.md`, `architecture/mcp.md`, `docs/lm-studio.md`, and
`docs/retrieval-quality-context-efficiency.md`. The context-efficiency report
distinguishes actual child `tools/list` bytes from estimated token costs and
LM Studio prompt inclusion.

## 10. Unresolved findings

- Low, operational: exact schema injection and tokenizer costs inside LM
  Studio remain unmeasured; M003 owns that qualification.
- No M001 correctness, security, or compatibility finding remains open.

## 11. Roadmap disposition

M001 moves from active to closed. M002 is ready and may proceed on this
accepted M001 contract. M003 remains blocked on M002 and its separately
required real-client qualification.

## 12. Registry updates

In the same closure commit, M001 is marked closed, M002 ready, the roadmap
records M001 closed/M002 ready/M003 blocked, and `plans/registry.md` points to
this control record.
