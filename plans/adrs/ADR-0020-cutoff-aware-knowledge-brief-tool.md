# ADR-0020: Cutoff-Aware Knowledge Brief as a Canonical MCP Tool

Status: accepted
Date: 2026-11-19
Accepted: 2026-11-20
Revised: 2026-11-20 — accepted. This record does not implement the tool.
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
A cutoff-aware operation can use the adapter, attempt ledger, and fetch
selector to surface candidate changes since a model's published knowledge
cutoff, without claiming to inspect the model's internal knowledge or
synthesizing answers in an internal LLM.

The canonical specification currently fixes **nine** advertised MCP tools,
with `fetch` canonical and `web_fetch` / `batch_fetch` callable but
unadvertised. ADR-0016 superseded ADR-0001's exact-ten inventory. An
additional advertised tool is therefore prohibited until a new accepted ADR
and a coordinated specification, contract, client, and documentation update.
`AGENTS.md` still says "exactly 10 tools". That sentence is already stale
and is not authority for this decision.

`Freshness` is a recency hint, not a cutoff judgment. `SearchDateRange` is
an inclusive day filter. `parse_result_timestamp` accepts only RFC 3339 and
`YYYY-MM-DD` and folds both to midnight UTC. None of those types may be
silently reused as the comparison rule below.

## Alternatives considered

1. Add only a `research_search` mode and no canonical tool. Preserves the
   nine-tool inventory but widens an existing request/response contract and
   weakens the agent-visible distinction between general research and
   model-relative knowledge-gap discovery. Also conflicts with keeping
   domain planners typed.
2. Create a separate local MCP server. Isolates experimental behavior but
   duplicates metasearch, bounded fetch, trust, diagnostics, caching, and
   client registration; also increases agent MCP discovery overhead.
3. Add one optional, domain-neutral `knowledge_brief` canonical tool backed
   by the existing adapter, subject to explicit change control. **Proposed.**
4. Add a model-in-the-loop research or crawling agent. Rejected: cost,
   non-determinism, and conflict with §8 non-goals.

## Proposed decision (requires acceptance)

- Permit exactly one new canonical advertised tool, `knowledge_brief`;
  the advertised inventory becomes **ten** only when the implementing
  milestone lands. Preserve all nine existing canonical tool names and both
  fetch compatibility aliases. Do not treat older ten-tool documentation,
  including the `AGENTS.md` sentence, as authority.
- The new tool is evidence discovery and deterministic packaging, not a
  summarizer, fact checker, model-introspection API, or web-search
  replacement. The caller may synthesize from the returned evidence.
- Its request is a focused `topic`, optional `model_id`, optional
  `cutoff_override` (`date`, `precision`, `source`, `reason`), and
  bounded `max_items` / `max_chars`. There is no `scope` field. M001
  accepts omitted `refresh` or `refresh: "live"` only. Do not infer the
  invoking model from MCP transport, user agent, or router. Operator
  overrides live in `[knowledge_delta.cutoffs]`. A catalog import command
  is out of scope; the bundled snapshot is the registry.
- Full and Research presets advertise the tool. Code and Minimal do not.
  A hidden tool remains callable. The static `next_tools` contract
  and the response `next_actions` field may name `fetch` and
  `research_search` only.
- Its result contains cutoff provenance and precision, candidate
  developments with source IDs and supporting excerpts, date basis, trust
  markers, retrieval coverage, truncation, warnings, and next-action hints.
  It MUST NOT claim a post-cutoff fact is unknown to a given model. Unknown
  cutoff stays unknown. An unrecognized `model_id` resolves to `unknown`;
  it is not an error.
- **Comparison rule.** Each date has precision `day`, `month`, `year`,
  or `unknown`, and basis `event`, `publication`, `page_update`,
  `first_observed`, or `unknown`. Compare at the coarser precision, and
  only when the relationship holds for every calendar date that precision
  allows. `after_cutoff` requires every evidence date to be strictly after
  every cutoff date. `on_or_before_cutoff` requires every evidence date to
  be on or before every cutoff date, so a day equal to a day-precision
  cutoff is on or before. Every other case is `indeterminate`, including
  unknown precision or basis, a day inside a month-precision cutoff, and
  equal month or year periods. Never promote a month or year to a day.
  Never map a cutoff onto `Freshness`. Send `SearchDateRange` only for
  a day-precision cutoff. A coarser cutoff is classified locally and
  reported as not applied, not approximated.
- A publication date is not an event date. Emit an event relationship only
  when a provider field is already an event date. A publication-only
  candidate may be `after_cutoff` on publication basis and must say so.
  Conflicting values on one basis stay conflicted. Do not parse dates from
  snippet prose, and do not fold coarser precision to midnight UTC.
- Every search uses `MetadataSearchAdapter`. Every fetch uses the existing
  authorized fetch path. The planner is new and typed. It records attempts
  on `RetrievalAttemptSet` and selects fetches with
  `FetchCandidateBuilder`. It does not add a `ResearchWorkflow` variant
  or call `build_research_search_plan`. No new HTTP client, credential
  scope, or provider-generated answer path.
- New delta IDs use FNV-1a in `src/core/identity.rs`, the
  `eggsearch-id-v1` prefix, and a `kdelta_` tag. Hash the canonical URL,
  basis, precision, and normalized date. Do not hash excerpt text. Existing
  source, fetch, bundle, and provider identities remain unchanged.
- `max_chars` bounds serialized delta objects as M001 specifies, using the
  existing distinction that `truncated` means text was clipped. Omitting
  a delta is budget exhaustion, not truncation.
- A `live` call has no persistent side effects. A later index must not
  write on that call unless the caller opts in under ADR-0021.

## Spec and implementation gate

Accepted 2026-11-20. The rejection path, a bounded `research_search`
extension or a separate experimental server, was not chosen. This
acceptance supersedes ADR-0016 only where it fixes the canonical count
at nine, and ADR-0001 only to permit the reviewed addition. It does not
reopen ADR-0016's `fetch` unification, alias behavior, or tool ownership.
It does not amend `plans/002-long-term-roadmap.md`; the implementation
change does that.

Upon acceptance, the **implementation change**, not this ADR, must amend
`plans/000-long-term-specification.md` §§1–2 and §7, add the subsystem
pointer in `plans/002-long-term-roadmap.md`, and update the tool names and
counts in `architecture/mcp.md`, `architecture/codegg-contract.md`,
`architecture/maintenance.md`, `architecture/testing.md`,
`docs/tool-matrix.md`, `docs/agent-workflows.md`,
`docs/codegg-integration.md`, the SDK and Python contract descriptions,
and the stale "exactly 10 tools" sentence in `AGENTS.md`.
Update `src/mcp/server.rs`, `tool_contract.rs`, `advertisement.rs`,
`projection.rs`, `output_schema.rs`, `src/sdk.rs`, the source-derived
inventories, `tests/docs_tool_names.rs`, `tests/mcp_tool_contract.rs`,
`tests/static_guards.rs`, and downstream client tests together.

## Security, cost, and compatibility

Requests are bounded before provider I/O. Cutoff metadata and candidate web
text are untrusted data, never instructions. Follow existing SSRF, origin
control, sanitization, timeout, profile isolation, and privacy rules.
Provider failures and a filter that was not applied stay visible in the
retrieval ledger. No results are not evidence of no changes.

The new tool is additive to existing callers, but `tools/list` and
full/compact schema payloads change for Full and Research. M001 must
quantify that cost for CodeGG, eggui, SDK, Python, and the presets.
Code and Minimal lists do not grow.

## Acceptance criteria for this ADR

A maintainer either accepts this proposed ten-tool contract, including the
comparison rule and preset membership above, or rejects it and chooses an
alternative. No milestone may mark itself ready while the status remains
proposed.

## Supersession note

ADR-0022, accepted 2026-11-22, supersedes only this record's clause fixing the canonical
advertised count at ten. `knowledge_brief` remains canonical; the retirement concerns
`repo_fetch` alone.
