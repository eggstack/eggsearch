# Plan 008 — Implementation Handoff Checklist

Status: handoff checklist
Scope: coordinates Plans 001–007 and the downstream CodeGG work

## Purpose

Provide a concise execution checklist so an implementation agent can carry the work forward without re-deriving dependencies, compatibility constraints, or closure evidence from the individual plans.

## Before implementation

- Read `000-overview-and-sequencing.md` and all dependent plans before changing production code.
- Record the current eggsearch and CodeGG commit SHAs used as baselines.
- Record current serialized MCP tool-definition bytes and current CodeGG model-visible search-tool bytes.
- Run the deterministic baseline portion of Plan 006 before changing descriptions or schemas.
- Confirm the current `rmcp` version's exact support for MCP 2026-07-28 before designing transport/version code.
- Confirm current CodeGG MCP protocol/version support before changing eggsearch's compatibility behavior.

## Milestone A — Contract foundation

Implement Plan 001 first.

Required closure evidence:

- canonical registry lists exactly the ten stable tools;
- tool-name parity tests pass;
- concise-description size metrics;
- reduced `EGGSEARCH_INSTRUCTIONS` size;
- documentation no longer recommends routine `provider_status` calls before normal research;
- unchanged CodeGG upstream tool-name compatibility.

## Milestone B — Input surface

Implement Plan 002 next.

Required closure evidence:

- before/after schema bytes per search tool;
- legacy fixture compatibility;
- canonical/legacy conflict behavior tests;
- one documented mapping table for repo/research/security semantic vocabulary;
- provider/debug controls remain accessible through a supported compatibility/operator path.

Do not remove legacy fields merely because they disappear from the ordinary advertised schema.

## Milestone C — CodeGG discovery

Implement the independent portion of Plan 005 once Plan 001 metadata is stable.

Required closure evidence:

- `tool_search` default output contains no full multi-tool schemas;
- top-k discovery result size;
- selective hydration test;
- policy monotonicity tests;
- raw eggsearch MCP tools remain hidden;
- initial model-visible tool-definition bytes reduced versus baseline.

Do not claim success if context bytes merely move from initial tool definitions into `tool_search` results.

## Milestone D — Protocol correctness

Implement Plan 003 with dual-era compatibility.

Required closure evidence:

- exact `rmcp` version and supported protocol revisions;
- structured-content tests;
- output-schema validation tests;
- one repairable semantic validation failure represented as a tool error;
- one true protocol/shape failure represented as JSON-RPC invalid params;
- deterministic tool-list fingerprint;
- legacy CodeGG compatibility remains green.

Do not remove the old initialize/session path until CodeGG and other supported clients no longer require it.

## Milestone E — Output context

Implement Plan 004 after structured result capture is reliable.

Required closure evidence:

- representative compact/standard/diagnostic payload sizes;
- retrieval failure versus evidence absence preserved in compact mode;
- trust and prompt-injection warnings preserved;
- security applicability semantics preserved;
- full structured result retained before display projection/truncation.

## Milestone F — Next-action guided disclosure

Complete Plan 005's next-action integration after eggsearch result contracts are stable.

Required closure evidence:

- valid `next_actions` can hydrate allowed follow-up tools;
- invalid/unknown targets are ignored;
- no next action bypasses plan/deny/model/backend/parent-ceiling policy;
- repeated discovery calls decrease on representative multi-step workflows.

## Milestone G — Maintenance decomposition

Implement Plan 007 in narrow slices.

Required closure evidence:

- before/after source-file size and responsibility table;
- no behavior/public-contract change from extraction commits;
- static guards extended to remaining high-density orchestration files;
- local workspace and forge security invariants remain covered;
- compatibility/dead-code inventory includes explicit retention/removal rationale.

## Final evaluation

Complete Plan 006 after the new surface is integrated.

Report:

- deterministic discovery top-1, recall@3, and MRR;
- initial schema bytes/tokens before/after;
- hydrated schema bytes/tokens;
- `tool_search` bytes/tokens;
- representative result bytes by detail mode;
- malformed tool-call rate and repair rate;
- redundant tool calls;
- optional multi-model completion/first-tool metrics;
- all-capability discoverability coverage.

## Full verification

At final closure run the repository's canonical gates, including:

```bash
make check
make docs-check
make bench-check
```

Also run focused MCP, provider, workflow, fetch, security, local filesystem, forge, and CodeGG integration tests touched by the implementation.

## Stop conditions

Pause the workstream and write a corrective plan rather than pushing through if any of these occur:

- preserving old requests requires silently changing their semantics;
- schema projection cannot preserve advanced capability with the current `rmcp` API;
- MCP 2026 migration requires dropping current CodeGG support;
- compact projection makes retrieval failure indistinguishable from negative evidence;
- next-action hydration widens authority;
- a maintenance decomposition requires broad behavior changes to succeed;
- measured tool-selection accuracy materially regresses despite lower context usage.

## Definition of handoff complete

The planning phase is complete when an implementation agent can pick up Plan 001, follow the milestone order, and produce closure evidence without requiring additional architectural decisions about tool naming, capability retention, discovery ownership, MCP compatibility, or evaluation criteria.