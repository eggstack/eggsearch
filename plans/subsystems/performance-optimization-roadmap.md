# Performance Optimization Roadmap

Status: closed

Long-term references:

- `plans/000-long-term-specification.md#7`
- `plans/001-terminology-and-domain-model.md#1`
- `plans/002-long-term-roadmap.md`

Related ADRs:

- `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`

## 1. Purpose and ownership boundary

Optimize hot paths and qualify footprint as optimization-with-equivalence:
no change to tool surface, ranking weights, trust/SSRF, cache policy, batch
budgets, compatibility, or transport boundaries for benchmark numbers. Owns
local-search inventory sharing, candidate scoring, fetch/cache sharing,
MCP projection/focus, and Tokio/rmcp feature qualification.

## 2. Work classification

### Invariants

- Deterministic tie ordering; equal/shorter timeout overrides keep the shared
  client; longer overrides build one widened client; batch setup one
  adjustment per call.

### Capabilities

- None user-visible; performance and footprint qualification.

### Infrastructure

- Shared cached inventory snapshots; single-score candidate selection;
  derived-cache hit without full copy; projection without clone round trips.

### Polish

- Benchmark evidence; binary-size deltas; feature-graph narrowing.

## 3. Non-goals

Capability or maintainability trade-offs for speculative micro-optimizations;
narrowing rmcp client transports without proven equivalent behavior;
changing the phase-18 compression posture for numbers.

## 4. Current state

Closed on corrective candidate `0af540b8c4f7ec27678d83a74ba82aad45556f00`
with qualification run `35542118569` (seven targets plus exact 16-file
assembly). `main` since is a documentation-only descendant of that candidate.

## 5. Target architecture

Achieved: shared immutable ownership on hot paths, single client-reuse
policy, cached static metadata where proven, and qualified explicit Tokio
feature set.

## 6. Dependency graph

```text
M001 baseline + local-search hot paths
    |
    +--> M002 fetch/cache sharing + connection reuse (soft on M001 conventions)
    |
    +--> M003 MCP shaping + discovery caching (soft on M001 conventions)
              |
              `--> M004 footprint qualification + closure (hard on M001-M003)
                        |
                        `--> M005 timeout semantics + requalification (corrective, hard)
```

## 7. Milestones

### M001 — Performance baseline and local-search hot paths

Historical plan:
`plans/archive/phase-19-performance-baseline-and-local-search-hot-paths.md`.

### M002 — Fetch/cache sharing and timeout connection reuse

Historical plan:
`plans/archive/phase-20-fetch-cache-and-connection-reuse.md`.

### M003 — MCP response shaping, focus projection, discovery caching

Historical plan:
`plans/archive/phase-21-mcp-response-shaping-and-discovery-caching.md`.

### M004 — Dependency-footprint qualification and performance closure

Historical plan:
`plans/archive/phase-22-dependency-footprint-and-performance-closure.md`.

### M005 — Timeout override semantics and performance evidence requalification

Historical plan:
`plans/archive/phase-23-timeout-semantics-and-performance-requalification.md`.
Corrective closure restoring widened-timeout connector semantics and missing
benchmark evidence.

## 8. Cross-cutting requirements

Production-shaped benchmarks only; before/after evidence against comparable
environments; normal correctness/packaging/release gates on the exact
candidate.

## 9. Verification strategy

Criterion harness compile (`make bench-check`), targeted before/after
measurements, plus `make check`, `make packaging-check`, and the clean-tree
release gate.

## 10. Risks and decision points

None open. Later production/dependency changes invalidating the exact
candidate require separate qualification.

## 11. Completion definition

Closed with hybrid timeout policy under eggfetch resolved-route semantics and
the recorded size baseline (18,744,624-byte default release binary on
`x86_64-apple-darwin` against the 18,727,936-byte baseline).

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 | closed | `plans/archive/phase-19-performance-baseline-and-local-search-hot-paths.md` | `plans/closure/performance-optimization/001-status.md` | — |
| M002 | closed | `plans/archive/phase-20-fetch-cache-and-connection-reuse.md` | `plans/closure/performance-optimization/001-status.md` | — |
| M003 | closed | `plans/archive/phase-21-mcp-response-shaping-and-discovery-caching.md` | `plans/closure/performance-optimization/001-status.md` | — |
| M004 | closed | `plans/archive/phase-22-dependency-footprint-and-performance-closure.md` | `plans/closure/performance-optimization/001-status.md` | — |
| M005 | closed | `plans/archive/phase-23-timeout-semantics-and-performance-requalification.md` | `plans/closure/performance-optimization/001-status.md` | — |
