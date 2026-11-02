# M006 — Compact Direct-Fetch Tool-Choice Corrective

Status: closed
Primary class: corrective
Source roadmap: `plans/subsystems/standalone-mcp-local-inference-roadmap.md`
Corrects: M001 compact tool presentation, as evidenced by M003 native API qualification in `plans/closure/standalone-mcp-local-inference/003-status.md`
Related ADRs: ADR-0001 and ADR-0006
Baseline implementation: `de69f1ddd7a7e4acc067fb9892c438f6165cfe83`

## Objective

Correct the compact `web_fetch` disclosure if the native LM Studio paired
failure is caused by omitted workflow guidance. Preserve the small tool
palette while allowing a model to reliably fetch an explicit URL supplied by
the user and produce an evidence-grounded answer.

## Evidence and why earlier qualification missed it

On LM Studio 0.4.26+4, `lfm3-2b-omni-abliter8` received the same native
`/api/v1/chat` prompt, `mcp/eggsearch` integration, `allowed_tools=["web_fetch"]`,
and 24,000-token context under full and compact configs. Full called
`web_fetch` once for the exact URL and answered from its result (1,627 input,
97 output tokens). Compact made no tool call and answered from unverified
knowledge (854 input, 39 output tokens). A repeated compact turn made the same
no-call response with the same usage. The original config was restored
byte-for-byte after each swap.

Earlier offline checks verified the ten names, schema budgets, and tool-call
contracts. The M003 direct-inference harness produced a successful compact
fetch, but it did not reproduce this configured-server API turn. The native
paired evidence therefore exposed a tool-selection gap that byte and schema
checks did not detect.

## Scope and invariants

- Keep all ten canonical tool names and runtime behavior unchanged.
- Keep `full` descriptions and defaults unchanged.
- Preserve compact schema size and all validation semantics.
- Change compact disclosure only when a concrete regression test and the
  repeated native pair support the change.
- Do not claim model-token savings as a quality win when required retrieval
  is skipped.
- No configuration migration, provider behavior change, new tool, transport,
  or network listener is in scope.

## Expected changes

- Production: compact `web_fetch` tool description in `src/mcp/server.rs`, if
  supported by the schema comparison.
- Regression guard: compact description contract in
  `tests/mcp_schema_slimming.rs`.
- Operator docs: update `docs/lm-studio.md` only if the corrective changes
  recommended profile guidance.
- Planning evidence: update the M003 operational record and add this plan's
  M006 closure record.

## Work packages

1. Compare the serialized full and compact `web_fetch` definitions. Identify
   which compacted description details tell a model that a user-provided URL
   is a valid target and that the tool returns bounded page text.
2. If the evidence supports it, preserve those workflow details in the compact
   description. Add a contract assertion in `tests/mcp_schema_slimming.rs` for
   explicit user URL support and the no-search/no-crawl boundary.
3. Rebuild the exact candidate and repeat the native API pair with the same
   prompt, model, allowed tool, context, temperature, and URL. Run one repeated
   compact turn. Record tool calls, arguments, result identity, answer evidence,
   token usage, latency, and config restoration.
4. Run the M003 focused and canonical verification commands. Requalify the
   exact pushed candidate on hosted macOS, Linux, and Windows CI.
5. Update M003's closure record and this plan's closure record. Keep M003 open
   if tool selection or evidence quality still fails, and register any further
   corrective scope instead of changing M001/M002 history.

## Verification

```bash
cargo fmt --check
cargo test --locked --all-features --test mcp_schema_slimming
cargo build --locked --release --all-features
python3 scripts/lm_studio_stdio_smoke.py target/release/eggsearch
make check
make docs-check
make bench-check
```

Native LM Studio qualification is operational and must not become a routine
network-dependent test. No new MCP server or public listener is required.

## Acceptance and stop conditions

- Full and compact each call `web_fetch` exactly once for the same explicit
  URL in the paired native API task.
- The compact answer cites evidence from the returned document and does not
  substitute prior knowledge for a skipped tool call.
- The compact ten-tool presentation remains below its existing byte budget;
  all ten names remain callable.
- LM Studio's original `mcp.json` is restored byte-for-byte and its
  authentication and permission settings are unchanged.
- If repeated compact turns still skip the tool or produce materially weaker
  evidence, record the failure and keep M003 open; do not claim acceptance.

Closure evidence belongs in
`plans/closure/standalone-mcp-local-inference/006-status.md` and must include
the exact candidate SHA, native paired turns, usage and evidence findings,
tests, hosted jobs, and restored-config hash.
