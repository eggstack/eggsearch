# Planning Registry

Updated: 2026-09-03
Baseline audited: `e645a3fe42090fb7b7e1ce8639681fe69878f57b` (`eggsearch` 0.3.7)

## Active workstream

| Workstream | Status | Depends on | Plan |
|---|---|---|---|
| Provider capability realization and Brave completion | complete | none | `phase-1-provider-request-contract-and-brave-realization.md` |
| Extractive evidence and fetch/cache controls | complete | phase 1 | `phase-2-extractive-evidence-and-fetch-control.md` |
| Firecrawl Developer Index | implemented | phases 1-2 | `phase-3-firecrawl-developer-index.md` |
| Exa semantic search provider | planned | phases 1-2 | `phase-4-exa-semantic-search-provider.md` |
| Tavily search provider and closure pass | planned | phases 1-2; may run after phase 4 or in parallel with it | `phase-5-tavily-provider-and-closure.md` |

The governing rationale and cross-phase invariants are in `roadmap.md`.

## Deferred by design

The following capabilities were researched but are not implementation commitments in this workstream:

- recursive crawling or autonomous browser interaction;
- provider-generated answers, summaries, deep-research agents, or schema-generation layers;
- a new general-purpose `site_map` MCP tool;
- Firecrawl Research Index passage/citation-graph operations.

A bounded same-origin site-discovery tool and Firecrawl Research Index integration remain plausible follow-on work. Phase 5 must record whether evidence gathered during implementation changes their priority. If either is promoted, create a separate plan rather than expanding an existing provider phase.

## Closure rule

The workstream is complete when phases 1-5 satisfy their acceptance criteria, provider/status/docs inventories agree with the implementation, CodeGG compatibility remains additive, `make check` passes on the final candidate, and phase 5 records the deferred-extension decision. Provider credentials are optional enhancements: the keyless default installation must still start and all ten existing MCP tools must remain usable without API keys.
