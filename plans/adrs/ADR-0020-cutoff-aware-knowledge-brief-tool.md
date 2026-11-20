# ADR-0020: Cutoff-Aware Knowledge Brief as a Canonical MCP Tool

Status: proposed
Draft: yes — architectural decision pending; not implementation authorization
Date: 2026-11-19
Decision owners: project maintainers
Baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`

Related: ADR-0001 (adapter ownership), ADR-0006 (keyless deterministic retrieval), ADR-0016 (nine canonical tools and legacy fetch aliases).
Affected specification: `plans/000-long-term-specification.md` §§1–4, 7–8; `plans/001-terminology-and-domain-model.md` §§1–3.
Affected subsystem: `plans/subsystems/knowledge-delta-roadmap.md`.
Related implementations: `plans/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md` and subsequent milestones.

## Context and forces

A model often omits developments it does not know to search for. `research_search`
already provides bounded multi-provider evidence discovery, typed research
planning, source roles, retrieval attempts, and deterministic evidence gaps.
A cutoff-aware operation can reuse those seams to surface candidate changes
since a model's published knowledge cutoff, without claiming to inspect a
model's internal knowledge state or synthesizing answers in an internal LLM.

The canonical specification currently fixes **nine** advertised MCP tools,
with `fetch` canonical and `web_fetch` / `batch_fetch` callable but
unadvertised. ADR-0016 superseded ADR-0001's exact-ten inventory. An
additional advertised tool is therefore prohibited until a new accepted ADR
and a coordinated specification, contract, client, and documentation update.

## Alternatives considered

1. Add only a `research_search` mode and no canonical tool. Preserves the
   nine-tool inventory but widens an existing request/response contract and
   weakens the agent-visible distinction between general research and
   model-relative knowledge-gap discovery.
2. Create a separate local MCP server. Isolates experimental behavior but
   duplicates metasearch, bounded fetch, trust, diagnostics, caching, and
   client registration; also increases agent MCP discovery overhead.
3. Add one optional, domain-neutral `knowledge_brief` canonical tool backed
   by the existing adapter, subject to explicit change control. **Proposed.**
4. Add a model-in-the-loop research or crawling agent. Rejected: cost,
   non-determinism, and conflict with §8 non-goals.

## Proposed decision (requires acceptance)

- Permit exactly one new canonical advertised tool, `knowledge_brief`;
  inventory becomes **ten** only when the implementing milestone lands.
  Preserve all nine existing canonical tool names and both fetch compatibility
  aliases. Do not treat older ten-tool documentation as authority.
- The new tool is an evidence-discovery and deterministic packaging operation,
  not a summarizer, guaranteed fact checker, model-introspection API, or web
  search replacement. The caller may synthesize from the returned evidence.
- Its request is a focused `topic`, optional `model_id` or explicit cutoff
  override, scope, bounded `max_items` / `max_chars`, and refresh policy.
  Do not infer the invoking model from MCP transport, user agent, or router.
- Its result contains cutoff provenance/precision, candidate developments
  with source IDs and supporting excerpts, event/publication/observation date
  *basis*, trust markers, retrieval coverage, truncation, warnings, and
  next-action hints. It MUST NOT claim a post-cutoff fact is definitely
  unknown to a given model. Unknown cutoff stays unknown.
- Every search uses `MetadataSearchAdapter` and existing bounded provider
  dispatch. Every fetch uses existing authorized `FetchClient` / canonical
  fetch pathways. No new direct engine/HTTP client, credential scope, or
  provider-generated answer path.
- Stable new delta IDs are content-derived and deterministic. Existing
  `SourceCard`, fetch, bundle, and provider identities remain unchanged.
- User-supplied overrides and catalog records retain source and precision:
  day/month/unknown cutoffs are not silently made more exact. The default
  registry is bundled and usable without a network metadata API.
- Tool advertisement may use existing presets/disclosure controls; disabled
  knowledge-delta capability is represented explicitly. Do not remove
  existing tools to make room.

## Spec and implementation gate

This ADR is **proposed**, not accepted. M001 has a hard dependency on its
acceptance. If rejected, revise the roadmap and M001 to use a bounded
`research_search` extension or a separate experimental server; do not ship
an unapproved tenth tool.

Upon acceptance, the **implementation change**, not this draft planning
commit, must amend `plans/000-long-term-specification.md` §§1–2 and §7,
the tool names and counts in `architecture/mcp.md`,
`architecture/codegg-contract.md`, `architecture/maintenance.md`,
`docs/tool-matrix.md`, `docs/agent-workflows.md`,
`docs/codegg-integration.md`, and the SDK/Python contract descriptions.
Update `src/mcp/server.rs`, `tool_contract.rs`, `advertisement.rs`,
`projection.rs`, `output_schema.rs`, the source-derived inventories,
`tests/docs_tool_names.rs`, `tests/mcp_tool_contract.rs`,
`tests/static_guards.rs`, and downstream client tests together.
Review the actual code-derived inventory before changing references: some
contributor prose still describes an earlier ten-tool state.

This ADR supersedes ADR-0016 **only** where it fixes the canonical count
at nine and ADR-0001 **only** to permit the reviewed addition; it does not
reopen ADR-0016's `fetch` unification, alias behavior, or tool ownership.

## Security, cost, and compatibility

Requests are bounded before provider I/O; cutoff metadata and candidate web
text are untrusted data, never instructions. Follow existing SSRF, origin
control, sanitization, timeout, profile isolation, and privacy rules. Search
date-filter approximations and provider failures remain visible in the
retrieval ledger; no-results is not asserted as evidence of no changes.

The new tool is additive to existing callers, but tools/list and full/compact
schema payloads change; M001 must quantify the advertisement cost and assess
CodeGG, eggui, SDK, Python, and tool-presets explicitly.

## Acceptance criteria for this ADR

A maintainer either accepts this proposed ten-tool contract with the exact
change-control requirements above, or rejects it and chooses an alternative.
No milestone may mark itself ready while the status remains proposed.
