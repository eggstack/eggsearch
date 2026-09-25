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
    `--> tool-surface consolidation (active)
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

### 7. Tool-surface consolidation — active

Contract and disclosure model, agent-facing schema slimming, MCP 2026
protocol and error contract, agent result projection and context budget,
CodeGG progressive-disclosure integration, agentic tool-surface evaluation,
maintenance decomposition and overlap ratchet. Roadmap:
`plans/subsystems/tool-surface-consolidation-roadmap.md`.

## Dependency notes

- Distribution hardening depends on the closed distribution implementation.
- Transport consolidation depends on release hardening.
- Performance work depends on transport qualification.
- eggfetch 0.2.0 adoption depends on performance closure.
- Optional outbound routing depends on eggfetch 0.2.0 adoption.
- Tool-surface consolidation is independent of the egress workstream and
  proceeds against current `main`.

## Deferred by design

Per `plans/000-long-term-specification.md` §8: recursive crawling,
provider-generated answers, `site_map` tool, mandatory vector indexing,
mandatory LSP dependency, full PDF layout/OCR, OS package pipelines,
container-primary install, unattended auto-update, non-loopback MCP exposure,
multiple SKUs, board-tuned assets, mandatory MCPB.
