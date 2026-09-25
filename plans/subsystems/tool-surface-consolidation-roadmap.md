# Tool Surface Consolidation Roadmap

Status: active

Long-term references:

- `plans/000-long-term-specification.md#2`
- `plans/000-long-term-specification.md#3`
- `plans/001-terminology-and-domain-model.md#1`
- `plans/001-terminology-and-domain-model.md#3`
- `plans/002-long-term-roadmap.md`

Related ADRs:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

## 1. Purpose and ownership boundary

Preserve every current capability while making the tool surface easier to
discover, cheaper in context, less ambiguous to select, and more maintainable
to evolve. Owns the contract/disclosure model, agent-facing schemas, MCP 2026
protocol and error contract, result projection and context budgets, CodeGG
progressive-disclosure integration, evaluation, and maintenance decomposition.
Does not reduce the ten tools to one mega-tool and does not copy
vendor-specific beta semantics into the generic MCP contract.

## 2. Work classification

### Invariants

- Ten stable tools with adapter ownership and canonical resolution preserved.
- Backend capability separated from model-visible disclosure.
- Legacy field compatibility retained through a supported operator path.

### Capabilities

- Discoverable tool selection with compact metadata and on-demand hydration;
  bounded/projected responses preserving high-signal evidence.

### Infrastructure

- Canonical contract registry; schema slimming; structured output semantics;
  maintenance decomposition and overlap ratchets.

### Polish

- Description/schema size metrics; evaluation across model families;
  documentation updates.

## 3. Non-goals

Mega-tool collapse; breaking CodeGG upstream tool-name compatibility;
removing provider/debug controls without a compatibility path; claiming wins
that merely move bytes from definitions into discovery results.

## 4. Current state

Planning complete (`000` overview through `008` handoff checklist against
September 2026 eggsearch/CodeGG mainlines and MCP 2026-07-28). Implementation
is dependency-ready starting at milestone M001. No production change has
landed under this roadmap yet.

## 5. Target architecture

Backend capability stays complete; model-visible disclosure becomes layered
(compact selection metadata, selective hydration, bounded projection) with
CodeGG progressive disclosure consuming only justified capabilities.

## 6. Dependency graph

```text
M001 contract + disclosure model (hard first)
    |
    +--> M002 agent-facing schema slimming (hard: M001 metadata stable)
    |
    +--> M003 MCP 2026 protocol + error contract (soft: M001)
    |
    +--> M004 result projection + context budget (soft: M001)
    |
    +--> M005 CodeGG progressive-disclosure integration (interface: M001 metadata)
              |
              `--> M006 agentic evaluation (operational: M001-M005)
                        |
                        `--> M007 maintenance decomposition + overlap ratchet (hard: M001)
```

## 7. Milestones

### M001 — Contract and disclosure model

Class: invariant + infrastructure. Objective: canonical registry, disclosure
hints, and usage guidance without breaking compatibility.
Implementation plan:
`plans/implementation/mcp-tool-surface-consolidation/001-contract-and-disclosure-model.md`.
Exit: registry lists exactly ten tools; parity tests pass; instruction bytes
reduced; CodeGG compatibility unchanged.

### M002 — Agent-facing schema slimming

Class: infrastructure + polish. Objective: smaller advertised schemas with
legacy compatibility.
Implementation plan:
`plans/implementation/mcp-tool-surface-consolidation/002-agent-facing-schema-slimming.md`.

### M003 — MCP 2026 protocol and error contract

Class: infrastructure. Objective: correct JSON Schema 2020-12 use, structured
output semantics, recoverable errors as tool errors, version compatibility.
Implementation plan:
`plans/implementation/mcp-tool-surface-consolidation/003-mcp-2026-protocol-and-error-contract.md`.

### M004 — Agent result projection and context budget

Class: capability. Objective: bounded/projected responses with full data
available to hosts.
Implementation plan:
`plans/implementation/mcp-tool-surface-consolidation/004-agent-result-projection-and-context-budget.md`.

### M005 — CodeGG progressive-disclosure integration

Class: capability. Objective: CodeGG exposes only justified capabilities;
raw eggsearch tools stay hidden.
Implementation plan:
`plans/implementation/mcp-tool-surface-consolidation/005-codegg-progressive-disclosure-integration.md`.

### M006 — Agentic tool-surface evaluation

Class: polish. Objective: empirical validation across model families, not
schema aesthetics.
Implementation plan:
`plans/implementation/mcp-tool-surface-consolidation/006-agentic-tool-surface-evaluation.md`.

### M007 — Maintenance decomposition and overlap ratchet

Class: infrastructure + polish. Objective: decomposed ownership with guards
against reconcentration.
Implementation plan:
`plans/implementation/mcp-tool-surface-consolidation/007-maintenance-decomposition-and-overlap-ratchet.md`.

Sequencing and preconditions:
`plans/implementation/mcp-tool-surface-consolidation/000-overview-and-sequencing.md`.
Handoff checklist:
`plans/implementation/mcp-tool-surface-consolidation/008-implementation-handoff-checklist.md`.

## 8. Cross-cutting requirements

Storage: no migration. Protocol: MCP 2026-07-28 compatibility with older
clients retained. Security: trust/sanitization/bounds unchanged. Docs:
tool-matrix and maintenance updated with code. Evaluation: deterministic
baseline before description/schema changes.

## 9. Verification strategy

Tool-name parity, schema-byte budgets, legacy fixture compatibility,
conflict-behavior tests, hydration/monotonicity tests, projection budgets,
plus `make check` and the maintenance guards.

## 10. Risks and decision points

No ADR open. If CodeGG protocol support or the selected `rmcp` version lacks
required MCP 2026-07-28 behavior, M003 stops for a compatibility decision
rather than inventing vendor-specific semantics.

## 11. Completion definition

Closed when M001-M007 each have accepted closure evidence proving preserved
capability, reduced model-visible bytes without hidden displacement,
backward compatibility, and passing routine gates.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 | closing | `plans/implementation/mcp-tool-surface-consolidation/001-contract-and-disclosure-model.md` | — | Implementation landed; closure record pending |
| M002 | closing | `plans/implementation/mcp-tool-surface-consolidation/002-agent-facing-schema-slimming.md` | — | Implementation landed; closure record pending |
| M003 | closing | `plans/implementation/mcp-tool-surface-consolidation/003-mcp-2026-protocol-and-error-contract.md` | — | Implementation landed; closure record pending |
| M004 | closing | `plans/implementation/mcp-tool-surface-consolidation/004-agent-result-projection-and-context-budget.md` | — | Implementation landed; closure record pending |
| M005 | closing | `plans/implementation/mcp-tool-surface-consolidation/005-codegg-progressive-disclosure-integration.md` | — | Implementation landed; closure record pending |
| M006 | ready | `plans/implementation/mcp-tool-surface-consolidation/006-agentic-tool-surface-evaluation.md` | — | M001-M005 baseline now exists |
| M007 | ready | `plans/implementation/mcp-tool-surface-consolidation/007-maintenance-decomposition-and-overlap-ratchet.md` | — | M001 |
