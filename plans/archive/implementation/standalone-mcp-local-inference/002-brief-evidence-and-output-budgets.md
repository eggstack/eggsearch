# M002 — Brief Evidence Projection and Deterministic Output Budgets

Status: closed (candidate `02a8af6fe4f87971c7daca7d1fca20c087b328ec`; closure `plans/closure/standalone-mcp-local-inference/002-status.md`)
Primary class: capability
Planning baseline: eggstack/eggsearch main `a666517c39adf28f32fad04198c74cb70d935c5b` (reconfirm after M001)
Roadmap: `plans/subsystems/standalone-mcp-local-inference-roadmap.md`
ADRs: ADR-0001, ADR-0006. Related closed predecessor: tool-surface consolidation M004 `plans/closure/mcp-tool-surface-consolidation/004-status.md`.

## Objective

Provide a genuinely short, deterministic MCP model-visible output mode for direct local-inference hosts. Preserve useful source evidence while suppressing redundant telemetry and repeated metadata; make it the **opt-in compact schema profile's** omitted-parameter default without changing the current `full`/`diagnostic` default or the existing `compact`/`standard` semantics.

## Current evidence

`src/mcp/projection.rs` exposes `ResponseDetail::{Compact,Standard,Diagnostic}` with `Diagnostic` default. Compact trims excerpts (normally one per card) and some top-level telemetry, but often retains most nested `SourceCard` fields: provider arrays, score, quality, metadata, trust and duplicate source fields. `src/core/source_card.rs` describes the canonical source-card identity, snippet/excerpt provenance, trust markers, result quality and domain-specific metadata. `src/mcp/tools/web_search.rs` returns both string and structured warnings plus routing, summaries and suggested next actions. `src/mcp/tools/web_fetch.rs` may return `text` plus a full structured `document` and `focus`. `src/mcp/tools/common.rs::map_tool_result` sends a structured payload with a text fallback: smaller returned payload helps clients that consume either channel.

The context-efficiency corpus reports only 0.7% compact savings on short result cards; a representative fetch is 46.9% smaller. Compact isn't sufficient as a low-token default for LM Studio.

## Ownership, non-goals, invariants

The `core` and adapter own canonical evidence, identity and ranking. The new layer only projects MCP responses after successful canonical retrieval; do not change upstream requests, number of internally retrieved candidates, RRF scores, provider selection, security advisory applicability, local workspace trust, caching, PDF/browser extraction, or compatibility views. No LLM summarization, embeddings, heuristic new ranking, hidden evidence fabrication, stateful opaque result handles, automatic additional requests or rewriting source provenance.

Do not assume a direct MCP host retains diagnostic structured data. When `brief` is requested, both native structured content and the compatibility text rendering may be brief; explicit `diagnostic` must retrieve the existing complete bounded response. Document this distinction rather than claiming full evidence is secretly stored. Avoid a second result-specific persistent cache.

## Response contract

Introduce `response_detail = "brief"` as an additive value for search/fetch tools that already accept detail modes; preserve the existing `compact`, `standard` and `diagnostic` values and their outputs. The config chosen by M001 controls the omitted-argument default: `full` => diagnostic; `compact` => brief. Explicit per-call detail ALWAYS wins. Diagnostic-only `provider_status` and identity-preserving `build_evidence_bundle` remain semantically complete; do not call an identity-trimmed evidence bundle "canonical".

Brief must be a structured, deterministic result envelope using the existing tool names. Suggested fields:
- Search: query/goal; status (`complete`/`partial`/`failed`/`unknown`) backed by actual retrieval signals; ranked source cards with stable IDs, titles, canonical URLs/locators, source kind/group, one short snippet or relevant excerpt (not both by default), trust and necessary provenance; compact warning codes and result-count/omission counters; at most the few useful next-action hints.
- Repository: path/owner/ref/local workspace provenance, checkout state/dirty state where applicable, symbol or line-span locator and evidence-role context. Preserve source/issue/release distinctions.
- Security: advisory ID, affected/unaffected/unknown applicability, package/version/source, severity where present, relevant exploitation/KEV status, confidence, conflicts and limitations. Never infer safety from missing providers.
- Fetch: known URL/redirect identity, content type, bounded `text` or deterministic focused chunks, retained span/chunk/page coordinates for citations and PDFs, truncation flag, trust markers. Avoid returning both full text and overlapping focus-chunk text.
- Batch: per-item identity, success/failure and one-item content budget; aggregate omissions/budget exhaust. Never suppress a failed member.
- Research: multi-source groups and evidence gaps/counterpoints as available. Keep enough coverage to distinguish support from independent evidence.

Use existing `structured_warnings` severity/code grouping to deduplicate repeated global trust/framing warnings in agent display; do **not** silently strip critical injection or capability-enforcement warnings. Preserve untrusted framing on all retained excerpts/text. Keep full debug/error detail available via explicit diagnostic requests and operator tools.

## Budget semantics

Introduce an optional, bounded display budget at the MCP presentation boundary, e.g. server config `[mcp].brief_max_chars` with explicit per-call override only if it can be added consistently to all supported search/fetch types. Proposed initial **character** ceiling 8,000 for typical direct calls, subject to fixture evidence; character/UTF-8 bytes are not model tokens and must be labelled correctly.

Prioritize reserved space for status and critical warnings, then complete top-ranked cards and selected snippets, then optional next actions/telemetry. Emit `returned_count`, `omitted_count` and `output_truncated` where applicable; do not prefix-clip JSON or claim a page is complete when its content was clipped. If a single critical warning exceeds budget, use a bounded stable code with accessible diagnostic detail, not silence. Honor explicit fetch `max_chars` and server hard caps independently of presentation budget.

Implement through one central projection boundary (`src/mcp/projection.rs` or its owned submodules), with per-domain typed/field-aware reductions; keep conversions deterministic and bounded. Avoid repeated full-JSON cloning and serializer roundtrips on low-memory SBC devices when a typed projection is practical.

## Ordered work packages

1. Freeze realistic fixture payloads for web/repo/research/security/fetch/PDF/batch, partial provider failures, all-providers-failed, local/remote trust, prompt injection, stale/unknown applicability and missing credential. Measure full/compact/brief bytes and the number of complete source cards under budgets; no invented model-token claim.
2. Extend detail parsing/default resolution in `projection.rs` and all response-bearing argument structs; bind omitted-detail policy to the M001 profile, not a global breaking default. Keep the ten output schemas truthful to all supported result modes (some are already permissive).
3. Implement explicit field allowlists and reserved diagnostic/status slots, with stable source IDs and URL/locator cross-tool handoff. Prefer concise safety/status codes over duplicated prose, retaining threat indicators.
4. Implement display-budget enforcement over complete records, UTF-8 boundaries and bounded memory. Avoid dropping semantically essential security/advisory/version data, conflict indicators, source provenance or partial/failed signals to satisfy a numeric size target.
5. Test MCP `CallToolResult` structured and text compatibility paths against both local profile and full diagnostic profile. Test direct call override `response_detail="diagnostic"` with unchanged output, and plain `tools/call` behavior in stdio and HTTP.
6. Update docs, examples, response fixtures and context-efficiency benchmark output. No new automatically running LLM, no network provider dependency in CI.

## Verification

```bash
cargo fmt --check
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test mcp_2026_protocol
cargo test --locked --features mock --test dispatch_fault_injection
cargo test --locked --features mock --test batch_fetch_retrieval
cargo test --locked --all-features --test evidence_contract
cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture
make check
make docs-check
make bench-check
```

Extend existing projection, fetch and domain-workflow suites with status-first budget fixtures; record tokenizer-independent byte/character costs and a named model tokenizer where available in optional live measurements.

## Acceptance / stop

- Default full-profile results remain byte/structure compatible in frozen fixtures; legacy detail values preserve their prior meaning. Explicit diagnostic requests still expose prior diagnostic information.
- Every brief response (including 0 results, all providers fail, partial failure, injected snippet, security applicability unknown, local dirty checkout, PDF truncation) retains identity/status/trust, is parseable and declares omissions.
- Brief saves **at least 30% serialized bytes** on representative nontrivial web/repo/research/fetch fixtures relative to diagnostic, with per-domain results published. Do not disguise negative or zero savings in sparse fixtures; the 15-task corpus should report separately rather than enforcing an artificial threshold.
- No lower-level retrieval quality/capability change and no additional upstream requests introduced by projection. Bounded CPU/memory/UTF-8 behavior on large batch and PDF fixtures.
- Test results and documentation explain that returned `structuredContent` is also projected in brief mode, so callers may explicitly request diagnostic if they need full data.

Stop if any projection masks a material failure, vulnerability applicability, untrusted content marker, provider partial state or content truncation; or if budget fitting requires invalid JSON. Exact-SHA closure: `plans/closure/standalone-mcp-local-inference/002-status.md`. M003 becomes ready only on accepted M002 closure.
