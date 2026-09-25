# Search Capability Roadmap

Status: closed

Long-term references:

- `plans/000-long-term-specification.md#3`
- `plans/000-long-term-specification.md#5`
- `plans/001-terminology-and-domain-model.md#1`
- `plans/001-terminology-and-domain-model.md#2`
- `plans/002-long-term-roadmap.md`

Related ADRs:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

## 1. Purpose and ownership boundary

Deliver provider capability realization and evidence controls over the
`core <- meta <- mcp` flow without turning eggsearch into an autonomous
browsing agent. Owns `EngineSearchRequest` fidelity, RRF aggregation,
excerpt/evidence packaging, and the Brave/Firecrawl/Exa/Tavily provider
adapters. Does not own distribution, transport internals, or tool-surface
disclosure.

## 2. Work classification

### Invariants

- Keyless baseline preserved; missing credentials are provider-scoped skips.
- Content-derived FNV-1a IDs and sanitization pipeline unchanged.
- Unsupported capabilities emit capability-skip attempts, never silent omissions.

### Capabilities

- Native Brave constraint enforcement; Firecrawl Developer Index; Exa
  semantic retrieval; Tavily provider with domain/topic/time handling;
  excerpt controls and focused fetch reads.

### Infrastructure

- `EngineSearchRequest` migration across adapter and dispatch paths;
  `ProviderCapabilities` 24-flag model; highlight/excerpt plumbing.

### Polish

- Provider inventory docs and CodeGG wrapper compatibility notes.

## 3. Non-goals

Provider-generated answers, live-crawl/schema/agent features, Research Index
passage/citation-graph operations, `site_map` tool, duplicate-evidence
providers.

## 4. Current state

Closed on `eggsearch` 0.3.7 baseline `e645a3fe42090fb7b7e1ce8639681fe69878f57b`.
All five milestones landed with behavioral suites
(`provider_request_contract`, `extract_fetch_contract`,
`provider_workstream_regression`) and CodeGG compatibility preserved.

## 5. Target architecture

Achieved: discover/rank/select/bounded-fetch/evidence-handoff pipeline with
native-vs-local enforcement matrix and additive backward-compatible fields.

## 6. Dependency graph

```text
M001 provider request contract + Brave
    |
    +--> M002 extractive evidence and fetch controls
              |
              +--> M003 Firecrawl Developer Index
              |
              +--> M004 Exa semantic provider
              |
              +--> M005 Tavily provider and closure
```

M001 is hard for M002-M005. M003/M004/M005 are soft relative to each other.

## 7. Milestones

### M001 — Provider request contract and Brave realization

Class: capability + infrastructure. Objective: carry query constraints into
provider calls with Brave completion. Historical plan:
`plans/archive/phase-1-provider-request-contract-and-brave-realization.md`.
Exit: `provider_request_contract` fidelity green; keyless baseline intact.

### M002 — Extractive evidence and fetch/cache controls

Class: capability. Objective: excerpts, timestamps, focus reads, cache
controls. Historical plan:
`plans/archive/phase-2-extractive-evidence-and-fetch-control.md`.

### M003 — Firecrawl Developer Index

Class: capability. Objective: developer-corpus discovery with scoped
repository echo. Historical plan:
`plans/archive/phase-3-firecrawl-developer-index.md`.

### M004 — Exa semantic search provider

Class: capability. Objective: semantic retrieval with native date/domain/
highlight handling. Historical plan:
`plans/archive/phase-4-exa-semantic-search-provider.md`.

### M005 — Tavily provider and closure

Class: capability + polish. Objective: Tavily adapter plus workstream closure
pass and deferred-extension record. Historical plan:
`plans/archive/phase-5-tavily-provider-and-closure.md`.

## 8. Cross-cutting requirements

Storage: no migration. Protocol: additive MCP fields only. Security:
sanitization and bounded reads preserved. Observability: capability-skip and
health accounting. Docs: provider-setup native-vs-local matrix.

## 9. Verification strategy

Behavioral suites plus determinism/budget/trust/CodeGG-compatibility
coverage; network-free keyless runs.

## 10. Risks and decision points

None open. Follow-on Research Index work requires a new evidence-based plan.

## 11. Completion definition

Closed: all five milestones have accepted evidence and the enforcement matrix
in spec §5 holds.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 | closed | `plans/archive/phase-1-provider-request-contract-and-brave-realization.md` | `plans/closure/search-capability/001-status.md` | — |
| M002 | closed | `plans/archive/phase-2-extractive-evidence-and-fetch-control.md` | `plans/closure/search-capability/001-status.md` | — |
| M003 | closed | `plans/archive/phase-3-firecrawl-developer-index.md` | `plans/closure/search-capability/001-status.md` | — |
| M004 | closed | `plans/archive/phase-4-exa-semantic-search-provider.md` | `plans/closure/search-capability/001-status.md` | — |
| M005 | closed | `plans/archive/phase-5-tavily-provider-and-closure.md` | `plans/closure/search-capability/001-status.md` | — |
