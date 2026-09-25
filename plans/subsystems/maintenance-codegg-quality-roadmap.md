# Maintenance and CodeGG Quality Roadmap

Status: closed

Long-term references:

- `plans/000-long-term-specification.md#2`
- `plans/000-long-term-specification.md#3`
- `plans/000-long-term-specification.md#9`
- `plans/002-long-term-roadmap.md`

Related ADRs:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

## 1. Purpose and ownership boundary

Consolidate architecture and workflows, close provider probing, enrich local
code intelligence, improve retrieval ergonomics, and close public
API/docs/tests/hygiene while preserving the ten stable tools and
trust/safety/identity semantics. Owns `src/meta/adapter/`,
`src/meta/workflow.rs`, `src/meta/local/`, dispatch decomposition, and
behavioral suite partitioning. Does not add general-purpose providers without
a new evidence class.

## 2. Work classification

### Invariants

- Ten-tool surface and adapter-only call path preserved.
- Shared workflow primitives with typed domain semantics (no generic flattening).
- No `phase<N>_*` test names; behavioral suites only.

### Capabilities

- Real bounded provider probing over the shared core service; structured
  symbol backend with regex fallback; enriched `repo_map`; per-item focused
  batch evidence with aggregate budget.

### Infrastructure

- MCP/metasearch decomposition without new monoliths; mega-suite partitioning.

### Polish

- Public API boundary, docs accuracy, transcript/hygiene guards.

## 3. Non-goals

New general-purpose providers duplicating existing evidence classes;
second downstream-specific protocol (CodeGG uses the existing MCP contract).

## 4. Current state

Closed. Phases 11-15 landed the consolidation, probing, local intelligence,
ergonomics, and hygiene passes with routine gates green on the closure
candidate.

## 5. Target architecture

Achieved: decomposed coordination layers, shared retrieval primitives,
bounded structured local search, and explicit Rust API boundary.

## 6. Dependency graph

```text
M001 architecture and workflow consolidation
    |
    +--> M002 provider probe and diagnostics closure
    |
    +--> M003 structured local code intelligence --+
    |                                              |
    `----------------------------------------------+
                                                   v
                                          M004 retrieval ergonomics
                                                   |
                                                   `--> M005 API/docs/tests/hygiene closure
```

M001 is hard for M002-M004; M005 is hard on M001-M004.

## 7. Milestones

### M001 — Architecture and workflow consolidation

Historical plan:
`plans/archive/phase-11-architecture-and-workflow-consolidation.md`.

### M002 — Provider probe and diagnostics closure

Historical plan:
`plans/archive/phase-12-provider-probe-and-diagnostics-closure.md`.

### M003 — Structured local code intelligence and repo-map enrichment

Historical plan:
`plans/archive/phase-13-structured-local-code-intelligence-and-repo-map.md`.

### M004 — Retrieval ergonomics and focused batch evidence

Historical plan:
`plans/archive/phase-14-retrieval-ergonomics-and-focused-batch-evidence.md`.

### M005 — Public API, docs, tests, repository-hygiene closure

Historical plan:
`plans/archive/phase-15-api-docs-tests-and-repository-hygiene-closure.md`.

## 8. Cross-cutting requirements

Module-size ratchet and overlap guards in `static_guards.rs`; factual
inventories code-derived; hygiene rejects tracked transcripts and build
outputs.

## 9. Verification strategy

All-features, property, fault-injection, forge-safety, and parser suites plus
`make check` on the exact candidate.

## 10. Risks and decision points

None open. Later decomposition slices are tracked as next-slice notes in
`architecture/maintenance.md`.

## 11. Completion definition

Closed when the workstream stop conditions in the archived registry section
held on the exact candidate.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 | closed | `plans/archive/phase-11-architecture-and-workflow-consolidation.md` | `plans/closure/maintenance-codegg-quality/001-status.md` | — |
| M002 | closed | `plans/archive/phase-12-provider-probe-and-diagnostics-closure.md` | `plans/closure/maintenance-codegg-quality/001-status.md` | — |
| M003 | closed | `plans/archive/phase-13-structured-local-code-intelligence-and-repo-map.md` | `plans/closure/maintenance-codegg-quality/001-status.md` | — |
| M004 | closed | `plans/archive/phase-14-retrieval-ergonomics-and-focused-batch-evidence.md` | `plans/closure/maintenance-codegg-quality/001-status.md` | — |
| M005 | closed | `plans/archive/phase-15-api-docs-tests-and-repository-hygiene-closure.md` | `plans/closure/maintenance-codegg-quality/001-status.md` | — |
