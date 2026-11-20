# M001 — Cutoff-Aware Knowledge Brief

Status: proposed
Date: 2026-11-19
Revised: 2026-11-20 — proposed contract. Not implementation authorization.
Subsystem: knowledge-delta
Milestone class: capability
Planning baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`
Roadmap: `plans/subsystems/knowledge-delta-roadmap.md` M001

References: `plans/000-long-term-specification.md` §§1–4, 7–8;
`plans/001-terminology-and-domain-model.md` §§1–3;
`plans/003-planning-process.md`; accepted ADR-0001, ADR-0006, ADR-0016;
proposed ADR-0020.
Design owners: `architecture/research.md`, `architecture/mcp.md`,
`architecture/codegg-contract.md`, `architecture/evidence-workflow.md`,
`architecture/fetch.md` (emitted-payload measure only).

## Objective and readiness

Expose one deterministic, model-cutoff-aware MCP `knowledge_brief` operation.
Given a topic and an explicit model identifier or cutoff, return a compact
ranked list of *candidate* developments whose dated evidence is after the
published cutoff, with source spans, date basis and precision, evidence
gaps, and retrieval accounting. There is no embedded LLM, index, crawler,
or prose date parser.

**Hard dependency:** ADR-0020 must be accepted, including authorization to
change the advertised inventory from nine to ten. If ADR-0020 is rejected
or altered, stop and revise this plan before touching code.
**Interface dependencies:** existing adapter, search, fetch, and evidence
contracts are stable on this baseline. CodeGG and eggui may pass
`model_id` against this request schema. They must not send `scope` or a
non-live `refresh`.

## Current implementation evidence

- `src/meta/research_planner.rs` plans typed subqueries for
  `research_search` only. `MAX_SUBQUERIES` is 8. That planner is a
  pattern, not a shared engine. `ResearchWorkflow` in
  `src/core/research.rs` is a closed goal enum. Do not add a variant.
- Shared mechanics are `WorkflowExecution`, `RetrievalAttemptSet`, and
  `FetchCandidateBuilder` (`src/meta/workflow.rs`,
  `src/meta/fetch_ranking.rs`). Spec §3 forbids flattening domain
  workflows into one planner.
- `Freshness` (`day`/`week`/`month`/`year`) is a recency hint. The
  adapter does not apply it locally. `SearchDateRange` is an inclusive
  `YYYY-MM-DD` provider filter and is mutually exclusive with a non-`any`
  freshness hint. Neither type is a cutoff judgment.
- `parse_result_timestamp` in `src/core/source_card.rs` accepts only
  RFC 3339 and `YYYY-MM-DD`, folds both to midnight UTC, and refuses
  snippet text. `SourceCard.published_at` is publication evidence.
  `src/fetch/` has no `datePublished`, JSON-LD, or `<time>` extractor.
- Stable IDs are FNV-1a in `src/core/identity.rs` (`eggsearch-id-v1`,
  `canonicalize_url`, `source_id`). Excerpts are not part of stable IDs.
- Advertised tools are the nine names in `src/mcp/advertisement.rs`.
  `web_fetch` and `batch_fetch` are callable aliases, not advertised.
  `src/sdk.rs` asserts facade names match that inventory.
  `AGENTS.md` still says "exactly 10 tools"; that sentence is already
  stale and is not authority. Do not edit it in this plan.
- `architecture/fetch.md` defines `payload_chars` as the Unicode scalar
  count of the serialized item response, measured before projection.
  `truncated` means text was clipped. Structural omission is not
  `truncated`.

## Invariants

- Do not alter incumbent canonical tool names, `fetch` semantics,
  compatibility aliases, source IDs, RRF tie-breakers, trust classes, or
  provider capability routing.
- Never state that a model lacks a fact. A temporal relationship describes
  dated evidence against a published cutoff, under the ADR-0020 rule.
- Unknown cutoff, unknown precision, or an unknown basis stays unknown.
  Never guess, and never promote month or year precision to a day.
- Publication, event, page-update, and first-observed dates are distinct.
  A publication date is not an event date. A recent page about an older
  event is not a new event.
- Follow existing SSRF, origin, sanitization, and credential policy. Do
  not add a private-network exception for this tool. Evidence stays
  external-untrusted data.
- No provider-produced summaries, internal answer generation, recursion,
  or uncontrolled source expansion.
- Return explicit budget, coverage, and provider failures. No matches
  never means no developments occurred.
- A `live` call has no persistent side effects. Later milestones must
  not write an index on this call unless the caller opts in under
  ADR-0021.

## Scope

**In:** bundled cutoff snapshot, operator cutoff overrides, one typed
planner, the ADR-0020 date rule, existing live search, a small authorized
fetch selection, one new MCP contract, diagnostics, tests, and docs.

**Out:** local persistent index, catalog import command, prose or
snippet date extraction, an open `scope` field, `refresh` values other
than omitted or `live`, feed crawler, auto-refresh daemon, semantic
embeddings, local LLM, new search engines, a `ResearchWorkflow` variant,
and unrelated repo or search refactors.

## Cutoff comparison rule

Implement the rule in ADR-0020. It is restated here so this handoff is
executable. Each date carries `precision` (`day`, `month`, `year`,
`unknown`) and `basis` (`event`, `publication`, `page_update`,
`first_observed`, `unknown`).

Compare only at the coarser precision. The relationship must hold for
every calendar date that precision allows:

- `after_cutoff` only when every evidence date is strictly after every
  cutoff date.
- `on_or_before_cutoff` only when every evidence date is on or before
  every cutoff date. A day equal to a day-precision cutoff is
  `on_or_before_cutoff`. This matches inclusive `SearchDateRange`.
- Otherwise `indeterminate`, including unknown precision or basis, a
  day inside a month-precision cutoff, and equal month or year periods.

Required fixtures: cutoff day 2024-06-01 with evidence day 2024-06-02 is
`after_cutoff`; the same day is `on_or_before_cutoff`; cutoff month
2024-06 with evidence day 2024-07-01 is `after_cutoff`; that cutoff with
evidence day 2024-06-15 or month 2024-06 is `indeterminate`; cutoff day
2024-06-15 with evidence month 2024-06 is `indeterminate`; cutoff year
2024 with evidence month 2024-06 is `indeterminate`.

Do not call `parse_result_timestamp` as the classifier. Use it only to
accept an already structured provider timestamp, and record precision
`day`. A value it rejects stays `unknown` unless a separate parser
preserves coarser precision explicitly. Do not fold a month or year to
midnight UTC. Do not parse dates out of snippets. `src/fetch/` does not
extract structured HTML dates; adding that extractor is not this
milestone.

`published_at` is publication basis. Emit an event relationship only
when a provider field is already an event date. Do not promote
publication to event. A publication-only delta may be `after_cutoff`
on publication basis and must say so. Conflicting values on the same
basis stay conflicted; do not pick a winner. The headline relationship
is `indeterminate` when the event basis is missing or conflicting and
the caller would otherwise read it as a new event.

Send `SearchDateRange` only when cutoff precision is `day` (start is
the next day, end is the call's UTC day). A coarser cutoff is classified
locally and reported as not applied, not approximated. Do not map a
cutoff onto `Freshness`. Pass a caller `language` hint through. Reuse
the existing prefixes in `src/core/warning/prefixes.rs`:
`language_unenforced`, `date_range_unenforced` when a sent day range
is not provider-enforced, and `freshness_unenforced` only if a caller
freshness hint was passed through. Do not invent a cutoff warning code.

## Proposed request/response contract

```json
{
  "topic": "free-threaded Python third-party compatibility",
  "model_id": "openai/gpt-6-luna",
  "max_items": 8,
  "max_chars": 6000
}
```

`topic` is required and is passed through unchanged. `model_id` is
optional. An unrecognized model resolves to `unknown`; that is not an
error, and the call remains usable without a temporal claim.
`cutoff_override`, when present, wins and must include `date`,
`precision`, `source`, and `reason`. It must not be labeled
provider-official. A malformed override, an invalid date, or an unknown
`refresh` value is a repairable error. Omitted `refresh` and
`refresh: "live"` are the only accepted forms. `prefer_live` and
`index` are rejected until ADR-0021. There is no `retain` field
and no `scope` field.

Operator overrides are `[knowledge_delta.cutoffs]` entries of model
alias to `{date, precision, source, verified_at}`. They are not index
settings. Invalid config fails closed at load. Precedence is caller
override, operator override, then the bundled snapshot
(provider-official, then Models.dev, then OpenRouter), then `unknown`.
The snapshot records source URL, verification date, and precision. No
runtime import, and no inference from model family. Proposed defaults
are `max_items` 8 and `max_chars` 6000; write the caps to
`docs/config.md` in the implementation change.

`max_chars` counts Unicode scalars of `serde_json::to_string` on each
delta object, before projection. Sum emitted deltas as
`total_payload_chars`. Topic, model resolution, retrieval summary, and
warnings are envelope overhead outside the sum, as in
`architecture/fetch.md`. `truncated` means excerpt text was clipped.
Omitting a delta sets `budget_exhausted` and does not set
`truncated`. New warning strings go through
`src/core/warning/prefixes.rs`.

Output contains `topic`, `model_resolution`, `deltas` (a `kdelta_`
ID, bounded excerpt, `source_ids`, URLs, spans, `date_basis`,
`precision`, `temporal_relationship`, conflict markers, and
`payload_chars`), `total_payload_chars`, `retrieval_summary`,
`coverage`, `warnings`, `truncated`, and bounded `next_actions`.
The static `next_tools` contract and the response `next_actions`
field may name `fetch` and `research_search` only. No confidence
percentage.

Delta IDs use FNV-1a in `src/core/identity.rs` with the
`eggsearch-id-v1` prefix and a new `kdelta_` tag. Hash
`canonicalize_url` plus basis, precision, and the normalized date. Do
not hash excerpt text. Source identity stays the existing `src_` ID.

## Ordered work packages

1. **Contract and catalog.** In `src/core/knowledge_delta.rs`, define
   the DTO and the precision, basis, and relationship vocabulary. In
   `src/meta/knowledge_delta/`, resolve aliases from the bundled
   snapshot and `[knowledge_delta.cutoffs]`. Do not add an import
   command.
2. **Query planning.** A new typed planner calls
   `MetadataSearchAdapter`, records attempts on
   `RetrievalAttemptSet`, and selects fetches with
   `FetchCandidateBuilder`. Do not call `build_research_search_plan`
   or add a `ResearchWorkflow` variant. Default 4 subqueries, hard
   cap 8. The topic string is one subquery, unchanged. Further
   subqueries may append a closed English hint list (release,
   compatibility, deprecation). Coverage says those hints are English
   and are not a translation.
3. **Evidence selection.** Deduplicate by the existing source ID.
   Fetch only a caller-capped subset through the authorized fetch path.
   Classify dates by the rule above. Given the same provider responses,
   packaging is deterministic across provider order and equivalent
   aliases. Live provider drift across calls is not a stability
   requirement.
4. **MCP and native facade.** Add `src/mcp/tools/knowledge_brief.rs`.
   Register it in `src/mcp/server.rs`, `tool_contract.rs`,
   `advertisement.rs`, `projection.rs`, `output_schema.rs`, and
   `src/sdk.rs`. The SDK assertion requires the facade name in the
   same change. Python bindings follow that facade
   (`architecture/python-library.md`); this is not optional. Match
   `research_search` read-only annotations. Set `next_tools` to
   `fetch` and `research_search` only. Full and Research presets
   advertise the tool. Code and Minimal do not. A hidden tool stays
   callable.
5. **Compatibility and docs.** On accepted ADR, the implementation
   change amends `plans/000-long-term-specification.md` §§1–2 and §7
   and adds the subsystem pointer in `plans/002-long-term-roadmap.md`.
   Do not amend those files in this plan. Update
   `architecture/mcp.md`, `architecture/maintenance.md`,
   `architecture/codegg-contract.md`, `architecture/research.md`,
   `architecture/testing.md`, `docs/tool-matrix.md`,
   `docs/agent-workflows.md`, `docs/codegg-integration.md`,
   `docs/config.md`, `docs/test-inventory.md`, and the stale
   "exactly 10 tools" sentence in `AGENTS.md`. Correct other stale
   ten-tool prose only where the code-derived inventory disagrees.
6. **Evaluation.** Property-test the comparison fixtures above in
   `tests/property_knowledge_delta.rs`, plus unknown cutoff and
   conflicting bases. Extend `tests/property_identity.rs` for the
   `kdelta_` ID. Failed-provider coverage belongs in the behavioral
   suite. An equal-budget comparison with `research_search` is a
   closure report, not a pass gate. Do not invent a recall threshold.

## Failure, timeout, and recovery

Each call has an aggregate deadline and max-query, max-fetch,
max-page-byte, and max-output bounds. Provider failure is soft when
any evidence remains. Total failure to retrieve is a structured
failure or an explicit partial state, not an empty success. Sanitize
and bound untrusted text. Catalog trouble falls back to the bundled
snapshot or `unknown`. Cancellation stops fan-out. No persistent
side effects.

## Verification (run against the exact implementation SHA)

- The comparison rule is a pure function. Put its fixtures in
  `tests/property_knowledge_delta.rs`. Extend
  `tests/property_identity.rs` for the `kdelta_` ID. Do not put
  either pure case in a behavioral suite.
- New behavioral suite `tests/knowledge_delta_brief.rs` for the MCP
  and planner path, including alias precedence, deterministic
  packaging, and the subquery cap. Do not put these cases in
  `tests/research_workflow.rs`. Extend `tests/mcp_tools.rs`,
  `tests/mcp_tool_contract.rs`, `tests/mcp_projection.rs`,
  `tests/docs_tool_names.rs`, `tests/static_guards.rs`, and client
  schema parity. Add both suite rows to `architecture/testing.md`
  and `docs/test-inventory.md`.
- Focused:
  `cargo test --locked --features mock --test knowledge_delta_brief`;
  `cargo test --locked --features mock --test mcp_tool_contract`;
  `cargo test --locked --features mock --test mcp_tools`;
  `cargo test --locked --features mock --test docs_tool_names`;
  `cargo test --locked --features mock --test static_guards`.
- Broad: `make check` and `make docs-check`, plus installed-wheel
  schema parity. Keyless offline fixtures. Record advertised tool
  count and full/compact tool-list bytes against the nine-tool
  baseline.

## Acceptance and stop conditions

M001 passes only if the tool is callable, the comparison fixtures
match the rule, evidence is source-linked and basis-labeled, and
existing tools, aliases, and budgets do not regress. An unknown
cutoff stays usable without a false temporal claim.

**Stop:** ADR not accepted; a date rule other than ADR-0020; snippet
dates treated as verified; a `ResearchWorkflow` variant; fetch
bypassing SSRF, sanitization, or the adapter; a tool-count change
without the specification, `AGENTS.md`, and guards; or a provider
failure hidden as empty coverage.

## Closure evidence required

Record the implementation SHA, accepted ADR status, requirement-to-test
matrix, focused and broad results, SDK, Python, and CodeGG contract
results, tool-inventory and schema-size delta, the date-fixture
results, the ungated `research_search` comparison if run, failure
cases, security review, known limitations, and decision
`closed | conditionally closed | corrective pass required | blocked`.
Write `plans/closure/knowledge-delta/001-status.md` and update the
registry and roadmap together. Nothing in this plan counts as closure.
