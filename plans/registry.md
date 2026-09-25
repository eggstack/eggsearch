# Eggsearch Active Planning Registry

This file is the compact control surface for active interim planning.
Detailed requirements and completed history remain in source roadmaps,
implementation plans, `plans/closure/`, and Git history.

Canonical direction remains in:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Historical `phase-*.md` plans and pre-migration roadmaps are archived under
`plans/archive/` and MUST NOT be extended.

## Status vocabulary

- **proposed** — roadmap or plan exists but is not approved for execution.
- **ready** — dependencies and interfaces are satisfied; plan may be handed off.
- **active** — implementation or closure work is in progress.
- **blocked** — a named dependency or evidence requirement prevents progress.
- **closing** — implementation landed and closure evidence is being gathered.
- **closed** — closure record accepted.
- **conditionally closed** — substantial work landed, but a named correctness or operational evidence condition remains.
- **superseded** — replaced by another document.
- **archived** — no longer active and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| Search capability and provider evidence | closed | `plans/subsystems/search-capability-roadmap.md` | M001-M005 closed | None. Pre-migration baseline `e645a3fe` (`eggsearch` 0.3.7). |
| Binary distribution, install/update, deployment | closed | `plans/subsystems/binary-distribution-deployment-roadmap.md` | M001-M006 closed | None. First binary release `v0.3.9` at `0cbbeee7`; qualify `34653366561`, release `34655458760`. |
| Maintenance, consolidation, CodeGG quality | closed | `plans/subsystems/maintenance-codegg-quality-roadmap.md` | M001-M005 closed | None. Baseline `4a713ff8`. |
| HTTP transport consolidation | closed | `plans/subsystems/transport-consolidation-roadmap.md` | M001-M003 closed | None. `eggfetch-core 0.2.0` at `bac6f49f`; qualify `35692096012`. |
| Performance optimization and footprint | closed | `plans/subsystems/performance-optimization-roadmap.md` | M001-M005 closed | None. Corrective candidate `0af540b8`; qualify `35542118569`. |
| Optional outbound routing (egress) | closed | `plans/subsystems/optional-outbound-routing-roadmap.md` | M001-M004 closed | None. Terminal baseline `6414a72`; hardened qualify `35810222447`. No egress-enabled binary published. Outcome B remains the runtime baseline. |
| MCP tool-surface consolidation | active | `plans/subsystems/tool-surface-consolidation-roadmap.md` | M001 ready | No hard blockers. M003 handoff confirms `rmcp`/CodeGG MCP 2026-07-28 support; M006 blocked on M001-M005 baseline. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Tool-surface consolidation | M001 contract and disclosure model | ready | `plans/implementation/mcp-tool-surface-consolidation/001-contract-and-disclosure-model.md` | No hard blockers; entry point for the workstream. |
| Tool-surface consolidation | M002 agent-facing schema slimming | ready | `plans/implementation/mcp-tool-surface-consolidation/002-agent-facing-schema-slimming.md` | M001 metadata preferred. |
| Tool-surface consolidation | M003 MCP 2026 protocol and error contract | ready | `plans/implementation/mcp-tool-surface-consolidation/003-mcp-2026-protocol-and-error-contract.md` | Confirm `rmcp`/CodeGG 2026-07-28 support at handoff. |
| Tool-surface consolidation | M004 result projection and context budget | ready | `plans/implementation/mcp-tool-surface-consolidation/004-agent-result-projection-and-context-budget.md` | — |
| Tool-surface consolidation | M005 CodeGG progressive-disclosure integration | ready | `plans/implementation/mcp-tool-surface-consolidation/005-codegg-progressive-disclosure-integration.md` | Requires M001 metadata stable. |
| Tool-surface consolidation | M006 agentic evaluation | blocked | `plans/implementation/mcp-tool-surface-consolidation/006-agentic-tool-surface-evaluation.md` | Blocked on M001-M005 baseline. |
| Tool-surface consolidation | M007 maintenance decomposition and overlap ratchet | ready | `plans/implementation/mcp-tool-surface-consolidation/007-maintenance-decomposition-and-overlap-ratchet.md` | Requires M001. |

Sequencing overview: `plans/implementation/mcp-tool-surface-consolidation/000-overview-and-sequencing.md`.
Handoff checklist: `plans/implementation/mcp-tool-surface-consolidation/008-implementation-handoff-checklist.md`.

## Current execution order and dependency gates

**Tool-surface gate:** M001 is the entry point and is dependency-ready. M005
may start its independent portion once M001 metadata is stable. M006 stays
blocked until the M001-M005 baseline exists; do not run evaluation against
pre-consolidation bytes and present it as consolidation evidence.

**Closed-workstream gate:** phases 1-28 (now `plans/archive/phase-*.md`) are
closed historical evidence. Their closure records live in `plans/closure/`
per subsystem. Do not reopen them for new scope; register new milestones
under the owning subsystem roadmap instead.

## Blocked work

| Subsystem | Milestone | Blocker |
|---|---|---|
| Tool-surface consolidation | M006 agentic evaluation | M001-M005 implementation baseline not yet landed |

## Closure work and current control points

| Subsystem | Status | Controlling evidence |
|---|---|---|
| Search capability | closed | `plans/closure/search-capability/001-status.md`; archived phases 1-5 |
| Binary distribution and deployment | closed | `plans/closure/binary-distribution-deployment/001-status.md`; archived phases 6-10, 16 |
| Maintenance and CodeGG quality | closed | `plans/closure/maintenance-codegg-quality/001-status.md`; archived phases 11-15 |
| Transport consolidation | closed | `plans/closure/transport-consolidation/001-status.md`; archived phases 17-18, 24 |
| Performance optimization | closed | `plans/closure/performance-optimization/001-status.md`; archived phases 19-23 |
| Optional outbound routing | closed | `plans/closure/optional-outbound-routing/001-status.md`; archived phases 25-28 |

## Closure rule

A milestone is `closed` only when its own acceptance criteria have been
exercised against the exact candidate and its closure record, roadmap status,
and registry entry are updated in the same closure commit. Corrective work is
a new plan referencing the original milestone and closure record, never a
silent amendment. Qualification is SHA-specific; re-qualify a different
eventual release candidate before publication.
