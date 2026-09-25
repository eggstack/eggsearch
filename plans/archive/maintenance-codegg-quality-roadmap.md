# Maintenance and CodeGG Quality Roadmap

Status: implemented (phases 11-15 closed; see `registry.md`)
Baseline audited: `4a713ff82cec701534e285bbe3d330ae121f352c` (`main`, 2026-09-05)
Primary downstream consumer: `dbowm91/codegg`

## Objective

Consolidate eggsearch after the search-capability and deployment workstreams, reduce architectural concentration and duplicated workflow machinery, complete provider diagnostics, and improve the quality of local/repository evidence delivered to CodeGG without expanding the stable MCP tool surface.

This workstream is deliberately consolidation-first. The repository already has broad provider coverage, strong tests, hardened fetch behavior, persistent MCP deployment, installers, self-update, and multi-client integration. The principal risk is now maintenance cost and duplicated orchestration rather than missing baseline capability.

## Current evidence

The audited tree has several large concentration points: `src/mcp/tools.rs` (~264 KB), `src/meta/adapter.rs` (~246 KB), `src/core/security.rs` (~101 KB), `src/core/provider.rs` (~100 KB), `src/core/config.rs` (~95 KB), `src/meta/dispatch.rs` (~96 KB), `src/meta/forge_adapter.rs` (~97 KB), `src/core/retrieval_status.rs` (~88 KB), `src/fetch/client.rs` (~85 KB), and `src/meta/security_search.rs` (~84 KB). `tests/integration.rs` is ~716 KB.

The high-level discovery tools intentionally differ semantically but repeat the same broad pipeline: planning, dispatch, normalization to `SourceCard`, grouping, coverage/retrieval accounting, fetch-candidate construction, ranking/diversification, and next-action generation. Shared ranking machinery exists, but repo/research/security workflow layers still duplicate substantial glue.

Provider health tracking and CLI diagnostics are mature, while MCP `provider_status(probe=true)` remains explicitly deferred. This prevents an MCP-only caller such as CodeGG from asking eggsearch to verify provider liveness through the same transport it already uses.

Local workspace search is operationally strong: bounded inventory, git-aware enumeration, ignore handling, safe file opening, source classification, and deterministic scoring. Its semantic model is intentionally lightweight and primarily regex-based for symbol discovery. For CodeGG, this is now a larger quality ceiling than general web-provider breadth.

The repository root also contains an accidental terminal transcript named `typescript`, demonstrating a gap in repository-hygiene enforcement despite strong Rust/test gates.

## Workstream principles

1. Preserve the ten stable MCP tools and their existing request/response compatibility unless a phase explicitly documents an additive schema extension.
2. Refactor behavior before adding breadth. No new general-purpose search provider belongs in this workstream unless it contributes a materially new evidence class that existing providers cannot supply.
3. Consolidate shared orchestration without flattening domain semantics. Repo, security, and research responses remain typed and domain-specific.
4. Keep ordinary CI deterministic and network-free. Live-provider probing stays explicit or request-driven and must not become required PR CI.
5. Prefer deterministic code intelligence over embeddings or model-dependent indexing for CodeGG-facing improvements.
6. Preserve keyless operation, bounded resource use, trust markers, SSRF protections, retrieval-attempt semantics, and stable identity behavior.
7. Do not weaken acceptance criteria to make refactors land. If a phase exposes an incompatible hidden contract, write a corrective plan.
8. Keep optional dependencies optional. Structured parsing/LSP integrations must not silently become mandatory for the default binary unless separately justified.

## Phases

| Phase | Workstream | Depends on |
|---|---|---|
| 11 | Architecture and workflow consolidation | none |
| 12 | Provider probe and diagnostics closure | phase 11 preferred, not strictly required |
| 13 | Structured local code intelligence and enriched repo mapping | phase 11 |
| 14 | Retrieval ergonomics and focused batch evidence | phases 11 and 13 where structured locators are reused |
| 15 | Public API, documentation, test, and repository-hygiene closure | phases 11-14 |

## Intended implementation order

```text
phase 11 -> phase 12
    |
    +-----> phase 13 -> phase 14 -> phase 15
```

Phase 12 may proceed in parallel after phase 11 establishes the final execution seams. Phase 15 is the closure pass and should not start until the architectural shape and additive retrieval features have stabilized.

## Cross-phase invariants

### MCP contract

The stable tool names remain:

`web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, `build_evidence_bundle`.

No phase should introduce a second CodeGG-specific protocol. CodeGG continues to consume eggsearch through MCP and existing configuration/bootstrap mechanisms.

### Workflow execution

Common execution abstractions may be introduced for planning lanes, retrieval attempts, source-card normalization, grouping, coverage, fetch-candidate ranking, and next actions. Domain-specific policy must remain typed; avoid a stringly-typed mega-framework that merely relocates complexity.

### Diagnostics

Provider probe state must be advisory. An explicit provider request remains authoritative even if health telemetry would otherwise suppress a default/profile-selected provider, unless current policy already rejects it for hard configuration/safety reasons.

### Local code intelligence

Regex symbol discovery remains a supported fallback. Structured parsing is additive and must fail closed to the existing fallback without making ordinary local search unavailable. No background daemon or heavyweight index is required for baseline operation.

### Resource bounds

Any richer local parsing, repo-map enrichment, or focused batch retrieval must have explicit file/count/byte/time limits and aggregate response caps. Existing safe-open and local-root containment rules remain authoritative.

### Public Rust API

The workstream must decide whether eggsearch intentionally supports downstream Rust-library consumers. It must not continue accidentally exporting broad implementation internals without documenting that commitment.

## Stop conditions

Do not mark this workstream complete until all of the following are true:

- `mcp/tools.rs` and `meta/adapter.rs` no longer serve as monolithic homes for unrelated tool/workflow responsibilities;
- common workflow primitives remove duplicated repo/research/security orchestration without changing stable MCP semantics;
- integration tests are partitioned into behavior-oriented suites with no single historical mega-suite acting as the default dumping ground;
- `provider_status(probe=true)` performs a bounded real probe through the same core engine used by CLI diagnostics;
- provider capability/constraint behavior has conformance coverage and documentation no longer contradicts implementation;
- local search supports at least one structured symbol backend in addition to the regex fallback, with deterministic bounded behavior;
- `repo_map` can expose useful package/module/symbol/test/build structure without requiring a model or persistent external index;
- `batch_fetch` can perform bounded focused extraction where representable, with an aggregate response budget;
- accidental generated/log artifacts such as the root `typescript` transcript are removed and guarded against;
- the intentional public Rust API boundary is documented and enforced;
- phase-oriented historical tests/docs are renamed or clearly mapped to stable behavioral contracts where practical;
- `make check` passes on the exact closure candidate, with targeted live/probe/structured-search smoke tests passing where applicable.

## Deferred by design

The following remain out of scope unless a later evidence-based plan promotes them:

- vector databases or embedding-based local indexing as a required dependency;
- a mandatory rust-analyzer/LSP daemon for local search;
- autonomous crawling or browser interaction beyond existing bounded rendering;
- new general web-search providers whose output class duplicates current title/URL/snippet search;
- model-generated summaries/answers inside eggsearch;
- broad remote/LAN MCP authentication work;
- full PDF layout/OCR implementation unless separately planned after this closure workstream.
