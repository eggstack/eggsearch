# Plan 004 — Agent Result Projection and Context-Budgeted Responses

Status: implementation plan
Scope: eggsearch result projection, response detail, and CodeGG consumption
Depends on: Plans 001 and 003

## Objective

Reduce context pollution from eggsearch tool results without discarding diagnostic, provenance, trust, or retrieval-state information. Introduce an explicit projection layer that separates the compact evidence an agent needs for its next decision from detailed telemetry useful to hosts, debugging, observability, or specialist analysis.

## Problem statement

Eggsearch has accumulated high-value structured diagnostics: routing decisions, provider failures, capability-enforcement telemetry, retrieval summaries, workflow coverage, conflict metadata, evidence-role summaries, trust markers, warnings, cache state, fetch telemetry, and suggested next actions. These improve correctness and observability, but returning every diagnostic field at full fidelity on every tool call can consume substantial context and distract an agent from the evidence it must act on.

The right response is not to remove these fields. The response contract should distinguish:

1. evidence needed for the immediate decision;
2. compact machine-readable status needed to avoid false certainty;
3. detailed diagnostics that remain retrievable without being injected into every agent turn.

## Non-goals

- Do not remove stable IDs, source provenance, trust semantics, warnings, or explicit partial/failure state.
- Do not hide provider failures in a way that can make an empty result look like evidence absence.
- Do not summarize source content with an LLM inside eggsearch.
- Do not create incompatible response shapes per model vendor.
- Do not move host observability into prompts.

## Design

Introduce a deterministic response projection policy. Suggested public concept:

```text
response_detail = compact | standard | diagnostic
```

Names may change, but there must be a clear default and deterministic mapping.

### Compact

Optimized for ordinary agent operation. Preserve:

- query/target identity;
- source cards or fetched content needed for the task;
- stable IDs;
- title/URL/repository locator;
- trust classification and critical trust markers;
- evidence role or source type when available;
- bounded excerpts/snippets/focus spans;
- essential warnings;
- explicit partial/failure/absence state;
- top suggested fetches / `next_actions`;
- conflict indicator when conflicts exist.

Compress or omit detailed routing/provider/capability telemetry unless it affects interpretation.

### Standard

Default for specialist research/security workflows. Include compact fields plus:

- retrieval summary;
- conflict metadata;
- workflow coverage summary;
- selected provider status/failures at bounded detail;
- capability-enforcement summary;
- useful fetch/cache metadata.

### Diagnostic

Preserve the current full diagnostic surface for operator tools, testing, troubleshooting, and explicit requests.

`provider_status` should remain diagnostic by nature and need not implement all three projections if that creates no value.

## Projection ownership

Add a dedicated projection module rather than scattering `if response_detail` logic throughout tool implementations. Candidate location:

- `src/core/projection.rs`, or
- `src/mcp/projection.rs` if projection is strictly MCP-facing.

Prefer projecting typed response structs before serialization. Avoid round-tripping large payloads through ad-hoc `serde_json::Value` mutation except at compatibility boundaries.

Define a small trait or explicit per-response projector only if it reduces duplication. Do not force heterogeneous response types into an overly generic abstraction.

## Tool-by-tool requirements

### `web_search`

Compact output should emphasize source cards, provider-failure interpretation, warnings that change trust/coverage, and `next_actions`. Full `routing_decision` and capability-enforcement detail should normally be summarized.

### `repo_search`

Keep group identity, repository/file/issue/release metadata, local-vs-remote trust, symbol evidence, suggested fetches, and workflow gaps. Detailed planner/retrieval telemetry can be standard/diagnostic.

### `research_search`

Standard should remain the default because evidence gaps, counterpoints, and multi-source coverage are part of the tool's value. Compact may reduce planner internals while preserving claims/gaps/grouped sources and next actions.

### `security_search`

Never omit applicability status, confidence, identifier/version metadata, KEV/exploitation relevance when requested, or warnings that constrain interpretation. Provider-routing details may be projected.

### `web_fetch` / `repo_fetch`

Prefer deterministic focus/range projection over returning oversized documents. Keep content truncation metadata and stable document/chunk/span IDs.

### `batch_fetch`

Preserve per-item success/failure and aggregate budget exhaustion. Compact telemetry can retain counters rather than every cache/provider detail.

### `build_evidence_bundle`

Preserve deterministic handoff content. Projection should not make the bundle semantically incomplete. If a compact model-facing rendering is needed, keep the canonical bundle available in structured content and project only the text/display representation.

## Host/CodeGG interaction

CodeGG should retain full structured results internally when available, but only inject the selected projection into the model-visible conversation. The following data should not be lost merely because display/context is compacted:

- stable IDs;
- trust markers;
- full structured warnings;
- retrieval summaries;
- provenance;
- raw next-action templates;
- canonical evidence bundle data.

This may require CodeGG to distinguish "stored structured result" from "model-visible projection" more explicitly. Its existing structured tool result path is the preferred seam.

## Context budget integration

Add deterministic budget controls around projected results. The budget system should prefer, in order:

1. fewer low-ranked result cards;
2. shorter excerpts/snippets;
3. fewer diagnostic details;
4. focused fetch spans;
5. explicit truncation markers.

Do not truncate JSON arbitrarily before structured data is captured. Structured canonical values must remain intact for host storage even when model-visible text is bounded.

## Testing

Add tests proving that compact projection does not erase interpretation-critical information.

Required cases:

- all providers fail versus no matching evidence;
- one provider fails but useful evidence remains;
- prompt-injection marker present;
- conflicting sources present;
- security applicability affected/not-affected/unknown;
- fetch truncation and focus projection;
- batch partial failure and aggregate budget exhaustion;
- local trusted versus external untrusted repo result;
- evidence bundle identity unchanged across projection modes.

Add byte-size snapshots/metrics for representative outputs and set regression thresholds for compact mode.

## Acceptance criteria

- A deterministic response-detail policy exists and is implemented through a central projection boundary.
- Compact mode materially reduces representative result bytes while preserving trust, coverage, failure/absence distinction, stable IDs, and next actions.
- Standard mode preserves specialist workflow semantics.
- Diagnostic mode retains current observability detail.
- Canonical structured results are captured before display truncation/projection.
- CodeGG can store full structured content while showing a bounded projection to the model.
- Tests prove that compact projection cannot turn retrieval failure into apparent negative evidence.

## Verification

```bash
cargo test --all-features
cargo test --features mock dispatch_fault_injection
cargo test --features mock batch_fetch_retrieval
cargo test --all-features evidence
make bench-check
```

Closure documentation must report before/after representative response sizes for web, repo, research, security, and batch workflows.