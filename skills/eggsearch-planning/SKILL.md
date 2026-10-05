---
name: eggsearch-planning
description: Use when planning, registering, handing off, or closing eggsearch work. Covers the plans/ registry convention, document classes, status vocabulary, closure evidence, corrective passes, and keeping AGENTS.md plus architecture/ in sync.
---

# eggsearch Planning Skill

Use when planning, registering, handing off, or closing eggsearch work. Covers the `plans/` registry convention, document classes, status vocabulary, closure evidence, corrective passes, and keeping `AGENTS.md` plus `architecture/` in sync.

Normative governance lives in `plans/003-planning-process.md`. This skill is the
operational checklist; the registry (`plans/registry.md`) is the control surface.
The contributor-side ownership summary is `architecture/maintenance.md` § Plans
registry.

## Before You Plan

1. Read `plans/registry.md` first. It is the only place that states what is active.
2. Find the owning subsystem roadmap in `plans/subsystems/`. If no roadmap owns the work, you do not have an approved workstream yet.
3. Check for a conditional or blocked predecessor. `conditionally closed` and `blocked` are not `closed` — do not build on them without recording the assumption.
4. If the work corrects a landed milestone, it is a **new plan**, never an amendment to the old one.

Current state (verify against `plans/registry.md` before relying on it): every
subsystem roadmap is closed except **repository security, supply-chain, and
maintenance hardening**, which is `active` — M001 conditionally closed via M007,
M002 conditionally closed pending first-release provenance evidence, M006
`blocked` on an upstream chromiumoxide release, M007 conditionally closed pending
a scheduled `dependency-security` workflow run. `eggsearch` 0.4.1 is the current
published release; `v0.4.0` is crate + tag only with no binaries.

## Where A Document Goes

| Document | Location | Purpose |
|----------|----------|---------|
| Canonical long-term | `plans/000-`–`003-*.md` | Specification, terminology, roadmap, process. Stable; amend only for intentional direction change, a real contradiction, an accepted ADR, or an explicit user request. |
| Architecture decision | `plans/adrs/ADR-NNNN-*.md` | One decision affecting several milestones or durable contracts. Accepted ADRs are never rewritten; supersede and link. |
| Subsystem roadmap | `plans/subsystems/<subsystem>-roadmap.md` | One coherent workstream: ownership boundary, invariants, non-goals, dependency graph, ordered milestones, exit conditions, risks. |
| Milestone implementation plan | `plans/implementation/<subsystem>/NNN-<slug>.md` | The agent handoff artifact. Bounded, independently executable, tied to a baseline. |
| Closure record | `plans/closure/<subsystem>/NNN-status.md` | The evidence that decides whether a milestone is actually done. |
| Archive | `plans/archive/` | Completed/superseded/abandoned plans. Traceability preserved; original filenames retained. |
| Control surface | `plans/registry.md` | Compact: active roadmaps, dependency-ready plans, blockers, latest control points. Links, never duplicates. |

A milestone implementation plan is only handoff-ready when it states: source
roadmap and milestone, relevant ADRs, objective **and explicit non-goals**,
current implementation evidence, invariants that cannot regress, expected
production changes, migration/compatibility effects, ordered work packages,
focused and broad verification commands, static guards and docs to update,
acceptance and stop conditions, and required closure evidence.

## Status Vocabulary

Use exactly these terms, from `plans/registry.md`:

`proposed` · `ready` · `active` · `blocked` · `closing` · `closed` · `conditionally closed` · `superseded` · `archived`

- `closed` requires the acceptance criteria exercised **against the exact candidate** plus the closure record, roadmap status, and registry entry updated in the same closure commit.
- `conditionally closed` names the one outstanding condition explicitly. It is not a softer `closed`.
- A commit message saying a plan is closed is not closure evidence.
- A roadmap must not mark a capability complete solely because its infrastructure milestone landed.

Classify each item as exactly one of: `invariant`, `capability`, `infrastructure`, `polish`. Infrastructure is not a user-visible capability until a consumer path exists. Polish normally follows the correctness boundary.

Declare dependencies as `hard`, `interface`, `soft`, or `operational`. A milestone is dependency-ready only when every hard dependency is closed and every interface dependency has a stable written contract.

## Corrective Passes

A corrective pass is a new implementation plan that references the original
milestone and closure record. It must list each unclosed requirement or
discovered defect, explain why the original verification missed it, add
regression tests or guards preventing recurrence, and avoid reopening already
closed scope without evidence. Never silently amend a closed plan or an
archived phase.

Qualification is SHA-specific. Re-qualify a different eventual release
candidate before publication; a green run on another SHA is not evidence for
this one.

## Keeping The Rest In Sync

Planning status drifts out of date silently — a one-line summary in `AGENTS.md`
is the usual first casualty. When you close or open a milestone:

1. Update `plans/registry.md` (status, control point, blockers).
2. Update the owning `plans/subsystems/` roadmap.
3. Write the `plans/closure/<subsystem>/NNN-status.md` record.
4. If the change alters what a contributor must know, update the **`architecture/`** deep dive that owns the topic, then keep `AGENTS.md` as an index pointing at that section. `AGENTS.md` is deliberately thin; contributor detail belongs in `architecture/`.
5. If it changes a release fact (version, binary availability, target matrix), update `docs/release.md`, `docs/installation.md`, and `README.md` together.

`tests/static_guards.rs` enforces the planning-status consistency guard
(`planning_status_consistency_no_closed_range_shorthand`): repository-hardening
planning documents must state `M001`/`M002` conditionally, `M006` blocked, and
`M007` conditionally closed, and must never summarize them with a
`M001-M005 closed` range shorthand. Match that precision when you summarize
milestone ranges anywhere in planning docs.

## Anti-Patterns

- Creating work with no registry entry. Do not create unregistered work.
- Reopening a closed workstream, or extending anything under `plans/archive/`.
- Reopening phases 1–28 as active scope; they are closed historical evidence.
- Handing an agent a broad product goal instead of one bounded milestone contract.
- Equating compilation, or a passing build, with closure.
- Recording only successful evidence while omitting blocked or unrun verification.
- Repeating the same requirement in several files with no authoritative source.
- Creating polish phases before the capability correctness boundary closes.
- Duplicating contributor rules into `AGENTS.md` until they drift; index them instead.
