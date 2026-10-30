# M004 — Compact Schema Discoverability Corrective

Status: blocked (schema correction, focused LM Studio requalification, and installed-child smoke pass; canonical `make check` stalls in the existing macOS `bounded_command` suite; see `plans/closure/standalone-mcp-local-inference/004-status.md`)
Primary class: capability
Source roadmap: `plans/subsystems/standalone-mcp-local-inference-roadmap.md`
Corrects: M003 evidence in `plans/closure/standalone-mcp-local-inference/003-status.md`
ADRs: ADR-0001 and ADR-0006
Baseline: `5daec8065f676a107a0539a4e7781f6949aa57e2`

## Objective

Make the compact MCP input schema disclose the accepted values for bounded
research classification fields, then verify that the actual advertised
contract retains those values without weakening runtime validation or
materially eroding compact size. Re-run targeted LM Studio tasks to learn
whether the schema correction reduces argument retries. This corrective does
not claim to solve general small-model reasoning or evidence extraction.

## Finding and why M001/M002 missed it

In M003 compact-profile LM Studio trials, Gemma E2B submitted invalid
`research_domain` and `goal` strings; the server's repair responses provided
the accepted values. `ResearchSearchArgs` currently models these as strings,
and compact projection removes property descriptions. Runtime parsing already
has finite accepted sets, but those constraints are not necessarily visible in
the compact tool schema. M001 qualified schema size and names, while M002
qualified output semantics; neither asserted these input-value constraints.

The M003 direct-fetch trial also returned an incomplete two-change answer, and
the research trial overran context and misclassified a secondary source. These
are qualification findings, not presumed code defects; this plan records them
as live recheck cases and will not loosen trust or retrieval policy to improve
a benchmark score.

## Scope and non-goals

- Add compact-profile JSON Schema enums for `research_search.goal` and
  `research_search.research_domain`, including the accepted aliases.
- Add regression assertions that compact `tools/list` advertises values that
  runtime parsing accepts.
- Requalify ten-tool count, deterministic fingerprint, and serialized compact
  size against the exact candidate.
- Repeat focused Gemma E2B research-search calls and the official-page fetch
  task, recording retries, context and evidence quality.
- Do not change tool handlers, accepted runtime inputs, providers, ranking,
  fetch behavior, brief response semantics, MCP permissions, or API auth.
- Do not mark M003 closed from this corrective alone; its larger-model,
  tokenizer, structured-content, refresh, and protected API criteria remain.

## Ordered work packages

1. Inspect the generated full/compact schemas and attach compact enum
   constraints from the canonical runtime accepted-value lists.
2. Add schema contract tests for `goal` and `research_domain` values, all ten
   names, stable compact fingerprint, and the existing size ceiling.
3. Run focused MCP tool-contract tests and `make docs-check`; then run the
   canonical `make check` gate if production code changes.
4. Rebuild the release candidate and installed-child smoke. Repeat the two
   focused live tasks in a fresh LM Studio session using Gemma E2B. Record
   per-call argument validity, retries, context meter, returned evidence and
   any remaining failure. Keep `mcp.json` rollback verified.
5. Update this plan and M003 status with exact candidate and test evidence;
   leave M003 blocked for remaining operational acceptance items.

## Invariants

- All ten tool names remain advertised; compact remains an opt-in presentation
  profile and is not an execution or authorization boundary.
- Every newly advertised value is accepted by the existing runtime parser.
- Full profile and runtime validation remain unchanged.
- Failure, applicability, trust, and truncation signals remain visible in
  tool results.
- No external network dependency is introduced to routine CI.

## Acceptance and stop conditions

- Compact `research_search` schema advertises all accepted research goal and
  domain values, including supported aliases; contract tests prove these
  values are accepted by runtime parsers.
- Ten canonical tools remain present and compact schema stays below the
  current 36,000-byte ceiling with deterministic fingerprinting.
- Focused offline tests, the canonical gate, and installed-child smoke pass
  on the exact candidate.
- Live results report whether the targeted retries were eliminated; no
  generalized quality claim is made from two tasks.
- If JSON Schema or client behavior cannot represent these constraints
  compatibly, stop and record the limitation rather than changing runtime
  semantics.

## Closure evidence

Record exact code candidate, schema excerpts/measurements, test commands and
outcomes, live Gemma task traces, config rollback, and remaining M003 blockers
in `plans/closure/standalone-mcp-local-inference/004-status.md`.
