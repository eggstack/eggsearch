# Planning Registry

Updated: 2026-09-08
Current maintenance/CodeGG-quality baseline: `4a713ff82cec701534e285bbe3d330ae121f352c`
Current baseline audited for deployment work: `f595683b8ebdec0afb13363ec9e8ad7654f9824b` (`eggsearch` 0.3.8)
Previous search-workstream baseline: `e645a3fe42090fb7b7e1ce8639681fe69878f57b` (`eggsearch` 0.3.7)

## Completed workstream — Search capability expansion

| Workstream | Status | Depends on | Plan |
|---|---|---|---|
| Provider capability realization and Brave completion | complete | none | `phase-1-provider-request-contract-and-brave-realization.md` |
| Extractive evidence and fetch/cache controls | complete | phase 1 | `phase-2-extractive-evidence-and-fetch-control.md` |
| Firecrawl Developer Index | implemented | phases 1-2 | `phase-3-firecrawl-developer-index.md` |
| Exa semantic search provider | implemented | phases 1-2 | `phase-4-exa-semantic-search-provider.md` |
| Tavily search provider and closure pass | implemented | phases 1-2 | `phase-5-tavily-provider-and-closure.md` |

The governing rationale and cross-phase invariants for this workstream are in `roadmap.md`.

## Completed workstream — Binary distribution and deployment

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 6 | Release binaries and bootstrap installers | implemented | none | `phase-6-release-binaries-and-bootstrap-installers.md` |
| 7 | Binary-first self-update | implemented | phase 6 asset contract | `phase-7-binary-first-self-update.md` |
| 8 | Persistent Streamable HTTP MCP | implemented | none; coordinated with phase 6 release smoke | `phase-8-persistent-streamable-http-mcp.md` |
| 9 | Startup supervision, croncheck, and restart | implemented | phases 7-8 | `phase-9-startup-supervision-croncheck-and-restart.md` |
| 10 | Agent/IDE integration and deployment closure | implemented | phases 6, 8, 9 | `phase-10-agent-ide-integration-and-deployment-closure.md` |

The governing rationale, target matrix, installer/update contract, lifecycle split, and cross-phase invariants are in `deployment-roadmap.md`.

## Active workstream — Maintenance and CodeGG retrieval quality

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 11 | Architecture and workflow consolidation | planned | none | `phase-11-architecture-and-workflow-consolidation.md` |
| 12 | Provider probe and diagnostics closure | implemented | phase 11 preferred | `phase-12-provider-probe-and-diagnostics-closure.md` |
| 13 | Structured local code intelligence and repo-map enrichment | planned | phase 11 | `phase-13-structured-local-code-intelligence-and-repo-map.md` |
| 14 | Retrieval ergonomics and focused batch evidence | planned | phase 11; phase 13 preferred | `phase-14-retrieval-ergonomics-and-focused-batch-evidence.md` |
| 15 | Public API, docs, tests, and repository-hygiene closure | planned | phases 11-14 | `phase-15-api-docs-tests-and-repository-hygiene-closure.md` |

The governing rationale, scope boundaries, cross-phase invariants, and stop conditions are in `maintenance-codegg-quality-roadmap.md`.

### Intended implementation order

```text
phase 11 -> phase 12
    |
    +-----> phase 13 -> phase 14 -> phase 15
```

Phase 12 may proceed in parallel once phase 11's execution seams are stable. Phase 15 is the closure pass.

### Workstream stop conditions

Do not mark this workstream complete until:

- the MCP and metasearch coordination layers are decomposed without replacing them with new monoliths;
- common repo/research/security workflow mechanics are shared while domain policy remains typed;
- the historical integration mega-suite is partitioned by behavioral contract;
- MCP provider probing is real, bounded, and uses the same core service as CLI diagnostics;
- provider capability documentation and descriptors agree;
- local search has a bounded structured symbol backend with regex fallback;
- `repo_map` exposes useful deterministic package/module/symbol/test/build structure;
- `batch_fetch` supports per-item focused evidence with an aggregate response budget;
- the accidental root `typescript` transcript and similar artifacts are removed and guarded against;
- the intended Rust public API boundary is explicit;
- CodeGG contracts and the routine verification gates pass on the exact closure candidate.

## Deferred by design

### Search/research extensions

- recursive crawling or autonomous browser interaction;
- provider-generated answers, summaries, deep-research agents, or schema-generation layers;
- a new general-purpose `site_map` MCP tool unless a future evidence-based plan promotes it;
- Firecrawl Research Index passage/citation-graph operations unless separately planned;
- new general web providers that duplicate existing evidence classes;
- mandatory vector/embedding local indexing;
- mandatory LSP/rust-analyzer local-search dependency;
- full PDF layout/OCR work unless separately planned after the maintenance closure.

### Distribution/deployment extensions

- apt/RPM/Homebrew/Winget/Chocolatey/Scoop/MSI/PKG package pipelines;
- containers as the primary install mechanism;
- unattended/background auto-update scheduling;
- non-loopback/LAN/public MCP exposure and authentication;
- multiple feature-specific binary SKUs;
- board-specific CPU-tuned or branded Raspberry Pi/Le Potato assets;
- MCPB as a required distribution mechanism until architecture-selection behavior is sufficient.

## Closure rule

A phase is `implemented` only when its own acceptance criteria have been exercised against the exact candidate and its status/registry entry are updated in the same closure commit. If implementation discovers a correctness/security/portability blocker, mark the phase `blocked` and write a scoped corrective plan rather than weakening acceptance criteria silently.
