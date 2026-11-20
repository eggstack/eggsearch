# Eggsearch Long-Term Roadmap

Status: normative long-term roadmap

This document provides macro-level ordering for eggsearch workstreams. It
states workstream sequence and dependencies, not implementation mechanics.
Subsystem roadmaps refine it; milestone plans execute against a repository
baseline. Amendments follow `plans/003-planning-process.md` §2.1.

Related: `plans/000-long-term-specification.md`,
`plans/001-terminology-and-domain-model.md`, `plans/003-planning-process.md`.

## Workstream order

```text
search capability expansion (closed)
    |
    +--> binary distribution and deployment (closed)
    |         |
    |         +--> first binary release hardening (closed)
    |
    +--> maintenance and CodeGG retrieval quality (closed)
    |
    +--> HTTP transport consolidation (closed)
    |         |
    |         +--> transport qualification closure (closed)
    |                   |
    |                   +--> performance optimization (closed)
    |                             |
    |                             +--> eggfetch 0.2.0 adoption (closed)
    |                                       |
    |                                       +--> optional outbound routing (closed)
    |
    `--> tool-surface consolidation (closed)
              |
              `--> repository security / supply-chain hardening (active)
```

## Workstreams

### 1. Search capability expansion — closed

Provider capability realization (Brave completion), extractive evidence and
fetch/cache controls, Firecrawl Developer Index, Exa semantic provider, Tavily
provider and closure. Roadmap: `plans/subsystems/search-capability-roadmap.md`.

### 2. Binary distribution and deployment — closed

Release binaries and bootstrap installers, binary-first self-update,
persistent Streamable HTTP MCP, startup supervision/croncheck/restart,
agent/IDE integration and deployment closure, first-binary-release hardening.
Roadmap: `plans/subsystems/binary-distribution-deployment-roadmap.md`.

### 3. Maintenance and CodeGG retrieval quality — closed

Architecture/workflow consolidation, provider probe/diagnostics closure,
structured local code intelligence and repo-map enrichment, retrieval
ergonomics and focused batch evidence, public API/docs/tests/hygiene closure.
Roadmap: `plans/subsystems/maintenance-codegg-quality-roadmap.md`.

### 4. HTTP transport consolidation — closed

eggfetch 0.1.7 consolidation, transport migration qualification and upstream
compression closure, eggfetch 0.2.0 adoption and compression-workaround
retirement. Roadmap: `plans/subsystems/transport-consolidation-roadmap.md`.

### 5. Performance optimization — closed

Baseline and local-search hot paths, fetch/cache sharing and connection
reuse, MCP response shaping and discovery caching, dependency-footprint
qualification, timeout-override semantics and evidence requalification.
Roadmap: `plans/subsystems/performance-optimization-roadmap.md`.

### 6. Optional outbound routing — closed

Eggress 1.0.8 optional proxy-chain integration (Outcome B), corrective
closure and qualification, maintenance closure and qualification-contract
hardening, documentation and test-hygiene cleanup. No egress-enabled binary
is published. Roadmap: `plans/subsystems/optional-outbound-routing-roadmap.md`.

### 7. Tool-surface consolidation — closed

Contract and disclosure model, agent-facing schema slimming, MCP 2026
protocol and error contract, agent result projection and context budget,
CodeGG progressive-disclosure integration, agentic tool-surface evaluation,
maintenance decomposition and overlap ratchet. Roadmap:
`plans/subsystems/tool-surface-consolidation-roadmap.md`.

### 8. Repository security, supply-chain, and maintenance hardening — active

Reachable dependency remediation and policy gates, immutable Action references,
release provenance, remaining reqwest elimination behind eggfetch ownership,
bounded process/unsafe consolidation, and security-sensitive forge maintenance.
Roadmap: `plans/subsystems/repository-hardening-roadmap.md`.

### 9. Knowledge delta — M001–M004 closed; M005–M007 proposed

Cutoff-aware `knowledge_brief`, the bundled rusqlite local evidence index, and
source-driven topic discovery closed on exact candidate
`c0f71909cfe2e6016512423e55b9d00fdb7220c5`. Closure evidence: M001
`plans/closure/knowledge-delta/001-status.md`, M002
`plans/closure/knowledge-delta/002-status.md`, and M003
`plans/closure/knowledge-delta/003-status.md`. M004 is a corrective pass to
M001 for brief delta production and honest diagnostics, registered from a live
default-configuration review on 2026-11-20 and closed on exact candidate
`380a55cd7ea7c0b37dc6eff53851926e96e894d0`; its plan is
`plans/implementation/knowledge-delta/004-brief-delta-production-and-honest-diagnostics.md`
and its closure record is `plans/closure/knowledge-delta/004-status.md`.
Three further corrective proposals remain `proposed` in
`plans/subsystems/knowledge-delta-corrective-addendum.md`: M005 verified
model-cutoff coverage, M006 opt-in index onboarding and seeding, and M007 live
discovery quality qualification. None is active.
Roadmap: `plans/subsystems/knowledge-delta-roadmap.md`.

## Dependency notes

- Distribution hardening depends on the closed distribution implementation.
- Transport consolidation depends on release hardening.
- Performance work depends on transport qualification.
- eggfetch 0.2.0 adoption depends on performance closure.
- Optional outbound routing depends on eggfetch 0.2.0 adoption.
- Tool-surface consolidation is independent of the egress workstream and is closed.
- Repository hardening proceeds against the post-tool-surface mainline. Its M005 depends on process-boundary M004; M006 additionally depended on an upstream chromiumoxide release with a no-reqwest launch-only feature graph, and ADR-0014 supersedes that objective: the edge is accepted and guard-bounded in both the default and all-features graphs.

## Deferred by design

Per `plans/000-long-term-specification.md` §8: recursive crawling,
provider-generated answers, `site_map` tool, mandatory vector indexing,
mandatory LSP dependency, full PDF layout/OCR, OS package pipelines,
container-primary install, unattended auto-update, non-loopback MCP exposure,
multiple SKUs, board-tuned assets, mandatory MCPB.
