# MCP Tool Surface Consolidation — Overview and Sequencing

Status: implementation plan index
Target repository: `eggstack/eggsearch`
Primary downstream integration: `dbowm91/codegg`

## Goal

Preserve every current eggsearch capability while making the tool surface easier for agents to discover, cheaper to place in context, less ambiguous to select, and more maintainable to evolve.

The implementation should not reduce the ten stable MCP tools to one mega-tool. Instead, it should separate backend capability from model-visible disclosure, reduce schema and result noise, use structured MCP semantics correctly, and let CodeGG's existing progressive-disclosure machinery expose only the capabilities justified by the current task.

## Research basis

The plans in this directory were prepared against the current eggsearch and CodeGG mainline implementations and current MCP/tool-use guidance as of September 2026.

Relevant current practices reflected in these plans:

- MCP 2026-07-28 supports full JSON Schema 2020-12 for tool input/output schemas and broadens structured tool output semantics.
- MCP 2026-07-28 removes the mandatory initialization/session lifecycle for the modern path, adds `server/discover`, and makes list results cacheable; migration should retain compatibility with older clients.
- Recoverable tool execution/semantic failures should be represented as tool errors rather than indiscriminately converted into JSON-RPC invalid-params errors.
- Large tool libraries perform better when high-frequency tools remain immediately visible and specialist tools are loaded on demand.
- Tool discovery should return compact selection metadata and hydrate only a few complete definitions; dumping many complete schemas into a discovery result defeats the purpose.
- Tool responses should be bounded/projected to preserve high-signal evidence while keeping full structured data available to hosts.
- Tool-boundary and naming decisions should be validated empirically across model families rather than assumed from schema aesthetics alone.

Primary references for implementers:

- Model Context Protocol specification and 2026-07-28 release notes/blog
- `rmcp` documentation for the exact version selected during implementation
- Anthropic engineering guidance on advanced tool use/tool search and effective agent tools
- current `architecture/maintenance.md` and `docs/tool-matrix.md` in eggsearch
- current `architecture/mcp.md`, `architecture/search_backend.md`, and `architecture/agent-tool-surface.md` in CodeGG

Do not copy vendor-specific beta semantics into the MCP contract unless they map cleanly to generic host behavior.

## Current-state summary

### What should remain

Eggsearch's ten stable tools represent real backend capabilities:

- `web_search`
- `web_fetch`
- `batch_fetch`
- `provider_status`
- `repo_search`
- `repo_fetch`
- `repo_map`
- `security_search`
- `research_search`
- `build_evidence_bundle`

The repository also has strong correctness infrastructure: all-feature testing, property tests, fault injection, adversarial corpora, fuzzing, schema/documentation contracts, packaging checks, and explicit architectural ownership rules.

### What should change

The largest remaining problems are agent-facing and maintenance-oriented:

- overlapping semantic controls across web/repo/research/security search;
- `repo_search` schema density (`profile`, `mode`, `workflow`, source booleans, provider/debug controls, package controls in one schema);
- partially duplicated workflow vocabularies across domain tools;
- repeated/long MCP descriptions and server instructions;
- CodeGG `tool_search` returning full schemas for multiple matches;
- useful `next_actions` not yet treated as a progressive-disclosure graph;
- structured MCP results/error semantics that can be modernized;
- rich result telemetry being injected more broadly than agents usually need;
- several large orchestration/local-code files where change locality is poor despite good test coverage.

## Plan set

### 001 — Canonical MCP Tool Contract and Disclosure Model

Create one typed semantic registry for tool purpose, use/not-for guidance, disclosure hints, annotations, discovery keywords, and related/next-tool relationships. Shorten tool descriptions and global server instructions while keeping all ten names stable.

This plan establishes vocabulary and should land first.

### 002 — Agent-Facing Schema Slimming Without Capability Loss

Separate compact canonical agent schemas from the compatibility/operator superset. Normalize repo/research/security task vocabulary, replace boolean forests with compact selectors where appropriate, hide infrastructure/debug knobs from ordinary schemas, and preserve legacy request compatibility.

Depends on 001.

### 003 — MCP 2026 Protocol, Structured Results, and Repairable Error Contract

Adopt native `structuredContent`, generated output schemas, standardized annotations, centralized error/result mapping, repairable tool errors, deterministic tool lists, and dual-era 2026-07-28/legacy protocol support.

Can begin after 001 and proceed in parallel with much of 002.

### 004 — Agent Result Projection and Context-Budgeted Responses

Introduce compact/standard/diagnostic result projections. Preserve stable IDs, trust, failure-versus-absence semantics, evidence roles, and next actions while avoiding routine injection of full provider/routing telemetry.

Depends on structured result capture from 003.

### 005 — CodeGG Progressive Disclosure Integration

Downstream implementation handoff. Reuse CodeGG's existing catalog/BM25/deferred-tool surface, remove full schemas from `tool_search` results, hydrate only selected definitions, and use eggsearch `next_actions` to expose likely follow-up tools without another discovery round trip.

Can begin after 001; complete against 002–004 final contracts.

### 006 — Agentic Tool-Surface Evaluation and Regression Gate

Create deterministic labeled discovery fixtures, optional live-model evaluation, schema/result byte accounting, first-tool accuracy, redundant-call/malformed-argument metrics, and CI regression gates.

Build the deterministic baseline early, then use it throughout 001–005 rather than waiting until the end.

### 007 — Maintenance Decomposition and Overlap Ratchet

Perform behavior-preserving decomposition of remaining high-density orchestration/forge/local/dependency/evidence modules and expand static guards so the repository does not reconcentrate complexity after the tool-surface work.

Begin after 002 establishes stable canonicalization seams.

## Recommended execution order

Use this milestone order:

### Milestone A — Measurement and contract foundation

1. Land the deterministic portion of Plan 006 so current schema sizes and discovery behavior are baselined.
2. Implement Plan 001.
3. Re-run baseline metrics and freeze canonical tool-contract fingerprints.

### Milestone B — Input-side context reduction

4. Implement Plan 002 schema slimming and compatibility translators.
5. Start Plan 005 compact CodeGG `tool_search` and selective hydration.
6. Re-run tool-selection and context-size evaluation.

### Milestone C — Protocol/result correctness

7. Implement Plan 003 structured output, output schemas, annotations, and repairable error handling.
8. Implement Plan 004 result projection.
9. Complete CodeGG structured-result/next-action integration from Plan 005.

### Milestone D — Maintenance closure

10. Implement Plan 007 in narrow behavior-preserving slices.
11. Run full deterministic evaluation and optional live-model comparisons.
12. Write closure/status documents with before/after metrics.

## Cross-plan invariants

Every implementation plan must preserve these invariants:

1. All ten current MCP capabilities remain callable.
2. Existing tool names remain stable through this workstream.
3. Progressive disclosure never widens execution authority.
4. Raw eggsearch MCP tools remain hidden by default in CodeGG.
5. Local workspace evidence remains bounded and root-contained.
6. Remote/fetched content remains untrusted data, never instructions.
7. Compact projections must not erase the distinction between retrieval failure and evidence absence.
8. Legacy request fields are not removed without a documented compatibility/deprecation window.
9. Provider routing, search safety, and fetch safety stay with their current architectural owners.
10. Context reduction must be measured end-to-end; moving schema bytes from initial definitions into `tool_search` output is not considered success.

## Success metrics

The closure for the full workstream should report at least:

- serialized bytes and estimated tokens for all eggsearch tool definitions before/after;
- serialized bytes for ordinary CodeGG initial search-tool surface before/after;
- `tool_search` result bytes before/after;
- representative web/repo/research/security result bytes by projection mode;
- deterministic discovery top-1 and recall@3;
- optional live-model first-tool accuracy and completion rate;
- malformed argument and recovery rate;
- number of immediately advertised versus deferred/hydrated tools;
- before/after size/responsibility table for decomposed large modules.

A useful target is a substantial reduction in ordinary model-visible eggsearch schema/result context with no statistically meaningful loss in task completion. Plan 002 proposes a minimum 30% reduction in the four search schemas as an initial engineering target; the evaluation results may justify a stronger target.

## Definition of done

This workstream is complete when:

- the stable tool contract has one authoritative semantic registry;
- ordinary schemas are compact and domain-discriminative;
- legacy capabilities remain accessible;
- MCP structured content/output schemas/error semantics are modernized;
- CodeGG discovers and hydrates specialist capabilities without catalog/schema dumping;
- result projections keep agent context bounded without hiding epistemically important status;
- deterministic evaluation prevents future tool-surface regressions;
- large maintenance hotspots are decomposed or explicitly justified;
- full eggsearch and CodeGG integration test suites pass;
- closure documents contain measured before/after evidence rather than qualitative claims only.
