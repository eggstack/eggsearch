# M005 — Agent Context Efficiency and CodeGG Consumer Handoff

Status: blocked on M001 baseline contract
Source: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` M005
Primary class: capability/infrastructure (Eggsearch contract; CodeGG adoption consumer-owned)
Baseline: `7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b`
References: `plans/000-long-term-specification.md` ``1-4, 7-9; ADR-0001, ADR-0005 and ADR-0006; `docs/tool-matrix.md`; `architecture/codegg-contract.md`; `docs/codegg-integration.md`.
Dependencies: hard M001; interface: closed MCP tool-surface consolidation M001-M007 and established CodeGG progressive-disclosure/tool-hydration protocol (confirm current downstream implementation before proposing consumer changes). Soft M004 for relevance improvements.

## Objective

Measure and improve the amount of *useful, attributable evidence per agent-visible token/byte and tool round trip* without adding inference to Eggsearch. Complete Eggsearch-side contracts and produce an exact, bounded CodeGG handoff rather than modifying the CodeGG repository from this milestone.

## Current evidence

Ten MCP contracts expose discovery hints `Core`/`Deferred`/`Diagnostic` and known-tool `next_actions`. `response_detail` currently defaults to `diagnostic` for backward compatibility. `compact` is opt-in; source cards and minimal failure/absence signals survive. The tool-surface M006 fixture set has 43 tool-choice cases, with ~73 KB full schema baseline; it does **not** measure end-to-end retrieval quality or host turn consumption. `src/mcp/projection.rs`, `src/mcp/tool_contract.rs`, `src/mcp/tools/*` and `src/sdk.rs` already hold the relevant owner seams. CodeGG frontends/daemon decide which tools to advertise/hydrate and retain full structured results.

## Work packages

1. **Measure a canonical task trace** against M001 gold tasks: full ten-tool advertised palette vs documented core-only (`web_search`, `web_fetch`, `repo_search`), deferred tool discovery, single/few schema hydration and `next_actions`; full/diagnostic, standard and compact projections; individual vs `batch_fetch` reads; query-focused extraction. Record total *actual serialized* tool definitions, input, output, evidence bytes, estimated tokens and turns; separately record relevant evidence retained, failure/absence signals preserved and latency.
2. **Audit the default choice boundary.** Keep `diagnostic` default and all stable schemas unchanged for standalone consumers unless a separately approved compatibility plan changes them. Prefer host-side explicit `response_detail=compact` and storing unprojected `structuredContent` outside the model window; define exact fallback to standard/diagnostic for uncertainty and troubleshooting.
3. **Test multi-step traces.** A selected source should be fetched once, retained through source ID/document/chunk and evidence bundle with consistent trust, no duplicate fetches solely from disclosure mechanics, and no loss of useful passages due to global context clipping. Count failed retries and distinguish no results from provider outage. Use deterministic host-simulator fixtures; real model performance is out of scope for Eggsearch CI.
4. **Make only measured, additive Eggsearch improvements** (for example improve omission/size projections or discovery metadata), preserving structured content and no capability hiding from callable MCP/SDK. Update `tests/mcp_projection.rs`, `tests/mcp_schema_slimming.rs`, `tests/tool_surface_evaluation.rs`, `tests/codegg_evidence_contract.rs` and Python contract tests if touched. Never let compact output remove critical provenance, trust markers, truncation or failure state.
5. **Write a consumer handoff** describing exact version/contract fingerprint, recommended disclosure and projection calls, schema/cache invalidation, error fallback, host memory ownership and CodeGG acceptance tests. The CodeGG team must register and implement any separate daemon/TUI changes in the CodeGG repository; no hidden cross-repo dependency is declared closed by an Eggsearch-only test.
6. **Cost-vs-quality selection.** Any compact or batch proposal must preserve benchmark relevant passage recall/coverage and critical failure semantics; report per-task reductions, latency and false negatives. If current client path already meets goals, document no-change and do not create a second host orchestration layer in Eggsearch.

## Constraints, migration, security and recovery

No new public tool, no breaking schema, no LLM in Eggsearch, no mandatory token estimator dependency, no changes to API key gating or local path permissions. Browser and PDF results are valid comparison inputs and remain bounded and externally untrusted. MCP `structuredContent` is the full authoritative payload, not truncated model prose. On unsupported host schema/projection mode, preserve existing error/repair contracts. Keep direct SDK/Python ten-tool parity, streamable HTTP loopback-only and independent Mode-Off zero-remote behavior.

## Verification

- M001 benchmark metrics on at least generic web, repo, security and comparative research tasks with full/compact projections.
- `cargo test --locked --all-features --test tool_surface_evaluation --test mcp_projection --test mcp_schema_slimming --test mcp_tool_contract --test codegg_evidence_contract --test mcp_2026_protocol`.
- `cargo test --locked --all-features --test evidence_bundle_handoff --test mcp_tools`.
- `cargo test --locked --features mock --test provider_routing`; Python `bindings/python/tests/test_native.py` if SDK result semantics touched.
- `make check`; `make bench-check`; `make docs-check`, plus downstream CodeGG contract tests as a separately recorded consumer action.

## Acceptance and closure

Provide an exact-SHA before/after per-task artifact containing context/round-trip/quality metrics, preserved trust and failure states, and a clear CodeGG integration contract. Existing default backwards compatibility and optional features remain intact. A CodeGG consumer implementation cannot be marked complete solely from Eggsearch work; record its status separately. Stop if proposal requires a model in Eggsearch, exposes secrets, makes auth/scope weaker, hides callable tools, or suppresses important evidence in compact mode. Closure `plans/closure/deterministic-retrieval-quality/005-status.md` includes measurements, changed/unchanged contract fingerprint and consumer handoff.
