# Eggsearch Planning and Agent-Handoff Process

Status: normative planning governance

This document defines how eggsearch's long-term direction is translated into
actionable work without allowing short-lived implementation details to
destabilize the canonical specification, terminology, or master roadmap. It is
adapted from the CodeGG planning convention to eggsearch's single-crate,
ten-tool, bounded-fetch reality.

The keywords MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY are normative.

## 1. Purpose

Eggsearch requires two distinct planning horizons:

1. **Long-term planning** defines product identity, domain boundaries,
   architectural ownership, invariants, capability dependencies, non-goals,
   and end-state acceptance criteria.
2. **Interim planning** defines bounded implementation work against a
   particular repository baseline and is intended for handoff to coding agents.

These horizons MUST remain separate. Interim plans may discover evidence that
warrants a long-term change, but they MUST NOT silently edit long-term
direction to match the easiest implementation.

## 2. Document classes

### 2.1 Canonical long-term documents

Canonical long-term documents are:

- `plans/000-long-term-specification.md`;
- `plans/001-terminology-and-domain-model.md`;
- `plans/002-long-term-roadmap.md`;
- this planning-governance document.

The first three MUST remain stable during ordinary feature implementation.
They MAY be amended only when product direction has intentionally changed, a
contradiction or material omission is identified, an accepted ADR requires the
canonical end state to change, or the user explicitly directs a revision. A
corrective implementation pass is not, by itself, justification for changing a
long-term requirement.

### 2.2 Architecture decision records

ADRs capture one architectural decision affecting several milestones,
subsystems, or durable contracts. An ADR MUST state context and forces,
considered alternatives, the selected decision, consequences and tradeoffs,
affected long-term sections and subsystems, migration or compatibility
implications, and status: proposed, accepted, rejected, deprecated, or
superseded. Accepted ADRs MUST NOT be rewritten to conceal history; a later
decision supersedes the prior ADR and links to it.

### 2.3 Subsystem roadmaps

A subsystem roadmap translates relevant long-term requirements into one
coherent workstream. It MUST define subsystem purpose and ownership boundary,
relevant specification and terminology references, invariants and non-goals,
current-state summary, dependency graph, ordered milestones, user-visible exit
conditions, cross-cutting security/migration/protocol/observability concerns,
and known risks and deferred work. It SHOULD avoid commit-specific file
lists, exact current line numbers, and mechanical implementation sequences.

### 2.4 Milestone implementation plans

A milestone implementation plan is the primary handoff artifact for a coding
agent. It MUST be independently executable, bounded, and tied to a repository
baseline. It MUST include source subsystem roadmap and milestone, relevant
ADRs and long-term requirements, objective and explicit non-goals, current
implementation evidence, invariants that cannot regress, expected
production-code changes, storage/protocol/migration/compatibility effects,
ordered work packages, focused and broad verification commands, static guards
and documentation updates, acceptance and stop conditions, and closure
evidence required. Material deviations MUST be recorded rather than hidden.

### 2.5 Closure records

A closure record determines whether a milestone is actually complete. It MUST
include implementation commits, requirement-to-evidence matrix, tests and
guards run with outcomes, migration and compatibility evidence, security
evidence where applicable, documentation and operational evidence, known
limitations, unresolved findings classified by severity, and recommendation:
closed, conditionally closed, corrective pass required, or blocked. A commit
message saying a plan is closed is not sufficient closure evidence.

### 2.6 Archive records

Completed, superseded, or abandoned interim plans SHOULD move under
`plans/archive/` once they are no longer active. Archive moves MUST preserve
traceability and SHOULD retain original filenames and subsystem grouping.
Canonical long-term documents and accepted ADRs MUST NOT be archived merely
because their initial implementation completed.

## 3. Work classification

Every planned item MUST be assigned one primary class: invariant (a property
that must remain true across releases), capability (user/developer/operator
visible behavior; completion requires end-to-end acceptance evidence),
infrastructure (internal machinery; MUST NOT be presented as completed
capability until a consumer path exists), or polish (ergonomics, diagnostics,
performance, cleanup, documentation that does not establish the principal
capability boundary; SHOULD normally follow correctness closure).

## 4. Dependency model

Each milestone MUST declare dependencies as hard (implementation cannot
correctly begin before the dependency closes), interface (work may proceed
against an agreed contract or test double), soft (parallel work possible, but
integration depends on the other milestone), or operational (implementation
can land, but deployment or release depends on external evidence). A
milestone is dependency-ready only when every hard dependency is closed and
every interface dependency has a stable written contract. The registry MUST
identify blocked milestones and their blockers.

## 5. Milestone sizing

A handoff milestone SHOULD be small enough that one implementation agent can
understand the affected ownership boundary, implement production changes, add
focused tests, run required verification, update documentation, and report
residual risks in one coherent pass. Prefer vertical slices establishing one
complete contract over broad horizontal refactors with no consumer. A
milestone is too large when it combines several independently releasable
capability boundaries or contains several unresolved architecture decisions.
A milestone is too small when it only renames one symbol without producing
meaningful closure evidence, unless it is a corrective action unblocking
another milestone.

## 6. Agent handoff contract

Default authority order: canonical specification and terminology, accepted
ADRs, subsystem roadmap, milestone implementation plan, current repository
evidence. When repository evidence conflicts with the plan, the agent SHOULD
preserve long-term invariants, record the discrepancy, and make the smallest
coherent adjustment necessary. The agent MUST inspect current code before
editing, preserve unrelated user changes, update tests and docs with code,
produce a closure-oriented status report, and identify anything not
completed. The agent MUST NOT invent a new architecture merely to finish the
checklist, weaken canonical invariants, or silently enlarge scope.

## 7. Corrective passes

A corrective pass is a new implementation plan, not an amendment pretending
the original milestone succeeded. Corrective plans MUST reference the
original milestone and closure record, list each unclosed requirement or
discovered defect, explain why original verification did not catch it,
include regression tests or guards preventing recurrence, and avoid reopening
already closed scope without evidence.

## 8. Updating subsystem roadmaps

Subsystem roadmaps MAY evolve as implementation reveals new dependencies or
better decomposition. Updates MUST preserve links to canonical requirements,
completed milestone history, reasons for reordering or splitting work, and
explicit status of removed or deferred items. A roadmap MUST NOT mark a
capability complete solely because its infrastructure milestone landed.

## 9. Registry requirements

`plans/registry.md` is the active planning control surface. It MUST remain
compact and SHOULD contain only active subsystem roadmaps, dependency-ready
implementation plans, active or recently completed implementation plans,
required closure passes, blocked work and blockers, and latest status or
closure record. The registry MUST link to source documents rather than
duplicate their detailed content.

## 10. Required planning review

Before handoff, review correct long-term references, unresolved architecture
decisions, dependency readiness, bounded scope and non-goals, explicit
ownership and invariants, migration and compatibility effects, failure and
timeout semantics, security effects, required test and static-guard evidence,
and unambiguous closure criteria. If these are not answerable, the work is
not ready for handoff.

## 11. Planning anti-patterns

Prohibited or strongly discouraged: adding transient TODO checklists to
canonical documents; one roadmap mixing all subsystems at file granularity;
handing an agent a broad product goal without a bounded milestone contract;
equating compilation with closure; changing terminology per subsystem;
allowing implementation plans to override accepted architecture silently;
retaining stale active plans after the repository materially changed;
repeating requirements in several files without one authoritative source;
recording only successful evidence while omitting blocked or unrun
verification; creating polish phases before the capability correctness
boundary is closed.

## 12. Eggsearch subsystem decomposition

The long-term roadmap produces subsystem roadmaps along these boundaries:

- search capability and provider evidence;
- binary distribution, install/update, and deployment;
- maintenance, architecture consolidation, and CodeGG retrieval quality;
- HTTP transport consolidation and compression behavior;
- performance optimization and footprint qualification;
- optional outbound routing (egress);
- MCP tool-surface consolidation and disclosure.

This is the fixed initial decomposition for the migration. A boundary MAY be
split later when ownership or dependency analysis warrants it, with reasons
recorded.
