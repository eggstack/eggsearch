# M005 Agent Context Efficiency and CodeGG Handoff — Closure Status

Status: closed (Eggsearch contract and handoff; consumer implementation separately owned)

Source plan: `plans/implementation/deterministic-retrieval-quality/005-agent-context-efficiency-and-codegg-handoff.md`

Roadmap: `plans/subsystems/deterministic-retrieval-quality-roadmap.md`

Implementation candidate: `5c1f0d2c846456d5cf8bdb9afb015123ca479034`

Reports: `docs/retrieval-quality-context-efficiency.md`, `docs/retrieval-quality-codegg-handoff.md`

## Measurements

- Ten serialized tool definitions: 72,939 bytes (about 18,235 tokens at 4 bytes/token).
- Core palette (`web_search`, `web_fetch`, `repo_search`): 14,008 bytes (about 3,502 estimated tokens), an 80.8% schema-byte reduction.
- Compact discovery result: 769 bytes total for the observed 43-case run; 260 bytes maximum per query.
- M001 15-task serialized trace: diagnostic 21,221 bytes, standard 21,539 bytes, compact 21,068 bytes. All projections retain the same 14 relevant result documents present in the fixture result lists; one of 15 judged relevant documents was not retrieved by the fixture, matching M001 recall of 14/15 on ranking tasks. Failure and absence remain distinct. Estimated token counts are 5,306 / 5,385 / 5,267.
- Deterministic host-call simulation: 42 total calls if each task searches once, fetches each fixture-retrieved relevant source once, and builds one evidence bundle for non-empty result tasks. These are MCP calls, not observed model turns or real host round trips.
- Representative `web_fetch` payload: 4,292 diagnostic bytes versus 2,278 compact bytes, a 46.9% reduction. The M001 synthetic result cards were already short, so aggregate compact output shrinks only 0.7%; `standard` is 1.5% larger than diagnostic in the trace.
- Tool choice: 43/43 top-1, 43/43 recall@3, MRR 1.000.

Token counts are estimates and no real model, model tokenizer, or CodeGG runtime was used. The exact current tool-surface fingerprint and consumer recommendations are recorded in the handoff.

## Contract and compatibility

No MCP tool names, schemas, SDK parity, diagnostic default, feature flags or structured content behavior changed. `response_detail` remains diagnostic by default. Compact mode retains stable IDs, trust/provenance, truncation/focus and critical failure/absence fields. Full `structuredContent` remains the authoritative host-retained payload. No duplicate fetches or cross-tool schema behavior changed in production.

The CodeGG-side discovery, cache, hydration and turn-count acceptance tests are handed off but are not claimed as completed by Eggsearch. The CodeGG repository owns and must register those consumer changes independently.

## Verification

- `cargo test --locked --all-features --test tool_surface_evaluation --test mcp_projection --test mcp_schema_slimming --test mcp_tool_contract --test codegg_evidence_contract --test mcp_2026_protocol --test evidence_bundle_handoff --test mcp_tools`: 104 passed.
- `make check`, `make bench-check`, and `make docs-check` pass on the final Eggsearch candidate.

## Disposition

Close M005 for the Eggsearch contract, reproducible measurements and bounded CodeGG handoff. Do not infer model-quality improvement from byte reduction alone. Downstream CodeGG implementation and real host turn/latency measurements remain consumer-owned.
