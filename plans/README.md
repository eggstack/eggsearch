# Eggsearch Planning System

This directory separates durable architectural direction from temporary
execution planning.

## Canonical long-term documents

The following files define the intended product and architecture and MUST NOT
be edited as part of ordinary implementation work:

- `000-long-term-specification.md` — normative end-state specification and invariants.
- `001-terminology-and-domain-model.md` — normative language and identity model.
- `002-long-term-roadmap.md` — dependency-ordered long-term capability roadmap.
- `003-planning-process.md` — rules for deriving and managing interim plans.

The first three documents are stable architectural references. Changes to
them require an explicit long-term architecture decision, not an
implementation convenience. Interim plans MUST reference them rather than
copying or silently revising their requirements.

## Planning hierarchy

```text
Long-term specification and terminology
        |
        v
Architecture decision records
        |
        v
Master long-term roadmap
        |
        v
Subsystem roadmaps
        |
        v
Milestone implementation plans
        |
        v
Implementation and verification
        |
        v
Closure records and archive
```

## Directory roles

- `adrs/` — durable architecture decisions. Accepted decisions are superseded, not rewritten.
- `subsystems/` — subsystem specifications and dependency-ordered roadmaps.
- `implementation/` — focused milestone plans handed to implementation agents.
- `closure/` — verification, evidence, residual-risk, and completion records.
- `archive/` — completed or superseded interim planning retained for traceability.
- `registry.md` — compact index of active subsystem roadmaps, implementation plans, closure work, and dependencies.

## Core rule

Long-term documents state **what eggsearch is becoming and what must remain
true**. Interim documents state **what an agent should implement next against
a specific repository baseline**.

Implementation agents MUST NOT add commit-specific steps, transient file
lists, current test counts, or short-lived corrective work to the canonical
long-term documents.

## Planning lifecycle

1. Identify the relevant long-term specification sections and invariants.
2. Record any unresolved architectural decision in `adrs/`.
3. Create or update a subsystem roadmap in `subsystems/`.
4. Select one dependency-ready milestone.
5. Write a bounded handoff plan under `implementation/`.
6. Implement and verify the milestone.
7. Write a closure record under `closure/`.
8. Update `registry.md` and the subsystem roadmap status.
9. Move completed or superseded interim documents to `archive/` when they no longer represent active work.

No milestone is complete merely because code landed. Completion requires the
closure evidence defined by its implementation plan and subsystem roadmap.

## Required classification

Every subsystem roadmap and implementation plan MUST distinguish:

- **Invariant** — a property that must always remain true.
- **Capability** — user- or operator-visible behavior.
- **Infrastructure** — internal machinery required by capabilities.
- **Polish** — ergonomics, diagnostics, performance tuning, cleanup, or documentation.

Infrastructure and polish MUST NOT be presented as completed user capability
unless the user-visible acceptance criteria are actually satisfied.

## Naming conventions

- ADR: `adrs/ADR-NNNN-short-title.md`
- Subsystem roadmap: `subsystems/<subsystem>-roadmap.md`
- Milestone implementation plan: `implementation/<subsystem>/NNN-short-title.md`
- Closure record: `closure/<subsystem>/NNN-status.md`
- Archived document: retain its original relative structure beneath `archive/`

Use stable subsystem names. Do not encode dates in filenames unless the
document is inherently time-bound.

## Starting a new workstream

Begin with `subsystems/README.md`, then use the templates and rules in:

- `adrs/README.md`
- `implementation/README.md`
- `closure/README.md`

Register active work in `registry.md` before handing implementation plans to agents.

## Historical note

The `phase-*.md` plans and the four pre-migration roadmaps
(`roadmap.md`, `deployment-roadmap.md`,
`maintenance-codegg-quality-roadmap.md`,
`performance-optimization-roadmap.md`) are retained under `archive/` for
traceability. They are superseded by the canonical documents and subsystem
roadmaps above and MUST NOT be extended.
