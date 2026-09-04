# Planning Registry

Updated: 2026-09-04
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

## Active workstream — Binary distribution and deployment

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 6 | Release binaries and bootstrap installers | planned | none | `phase-6-release-binaries-and-bootstrap-installers.md` |
| 7 | Binary-first self-update | planned | phase 6 asset contract | `phase-7-binary-first-self-update.md` |
| 8 | Persistent Streamable HTTP MCP | planned | none; coordinate with phase 6 release smoke | `phase-8-persistent-streamable-http-mcp.md` |
| 9 | Startup supervision, croncheck, and restart | planned | phases 7-8 | `phase-9-startup-supervision-croncheck-and-restart.md` |
| 10 | Agent/IDE integration and deployment closure | planned | phases 6, 8, 9 | `phase-10-agent-ide-integration-and-deployment-closure.md` |

The governing rationale, target matrix, installer/update contract, lifecycle split, and cross-phase invariants are in `deployment-roadmap.md`.

### Intended implementation order

The default handoff order is:

```text
phase 6 -> phase 7
     \       \
      -> phase 8 -> phase 9 -> phase 10
```

Phase 8 can proceed in parallel with phases 6-7 once the CLI/release smoke interface is coordinated. Phase 9 must not start before a real persistent HTTP MCP transport exists. Phase 10 is the closure phase and must re-audit the exact current client configuration surfaces before implementing adapters.

### Deployment workstream stop conditions

Do not mark this workstream complete until:

- GitHub Release assets use one stable target/asset contract across workflow, Unix/PowerShell installers, updater, and docs;
- Linux x86-64/AArch64, macOS Intel/Apple Silicon, and Windows x86-64 release binaries are verified; ARMv7 and Windows ARM64 are either verified as planned or retain explicit technical blockers that keep the relevant phase non-complete;
- binary bootstrap verifies SHA-256 before candidate execution and only falls back to Cargo for unsupported/404 assets;
- crates.io is the stable-version authority for `eggsearch update`, and updater downloads the exact corresponding GitHub tag asset;
- existing `eggsearch mcp stdio` remains CodeGG-compatible;
- persistent MCP uses current Streamable HTTP semantics and is loopback-only by default;
- systemd/launchd/Windows/cron startup logic is idempotent, manager-exclusive, and never auto-elevates;
- `croncheck` starts only on definite absence and cannot race into duplicate servers;
- update restarts only a persistent service that was running before replacement;
- client integration preserves unrelated third-party configuration and defaults to stdio unless HTTP is explicitly selected;
- CodeGG default stdio bootstrap and explicit remote MCP path both work against the closure binary;
- README/docs distinguish ordinary binary-only install from fleet `--service` install;
- `make check` and release/deployment-specific smoke gates pass on the exact closure candidate.

## Deferred by design

The following capabilities were researched but are not implementation commitments in the current workstreams:

### Search extensions

- recursive crawling or autonomous browser interaction;
- provider-generated answers, summaries, deep-research agents, or schema-generation layers;
- a new general-purpose `site_map` MCP tool unless a future evidence-based plan promotes it;
- Firecrawl Research Index passage/citation-graph operations unless separately planned.

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
