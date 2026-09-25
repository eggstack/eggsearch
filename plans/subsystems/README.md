# Subsystem Roadmaps

Subsystem roadmaps translate the canonical eggsearch direction into coherent,
dependency-aware workstreams. They are not direct coding-agent checklists.

Each roadmap should remain useful across several implementation milestones
and repository revisions. Commit-specific mechanics belong in
`plans/implementation/`.

## Naming

```text
<subsystem>-roadmap.md
```

Current subsystems:

```text
search-capability-roadmap.md
binary-distribution-deployment-roadmap.md
maintenance-codegg-quality-roadmap.md
transport-consolidation-roadmap.md
performance-optimization-roadmap.md
optional-outbound-routing-roadmap.md
tool-surface-consolidation-roadmap.md
dependency-evidence-hardening-roadmap.md
```

## Required roadmap structure

Adapt `plans/003-planning-process.md` §2.3 to eggsearch: purpose and
ownership boundary, work classification (invariant/capability/infrastructure/
polish), non-goals, current state, target architecture, dependency graph with
hard/interface/soft/operational classification, milestones with class,
objective, dependencies, deliverable boundary, user value, exit conditions,
and deferred work, cross-cutting requirements (storage/migration, protocol/
compatibility, security/authorization, failure/timeout/recovery,
observability, performance/resources, documentation/operations), verification
strategy, risks and decision points, completion definition, and milestone
status table.

## Roadmap rules

A subsystem roadmap MUST link to canonical long-term requirements rather than
duplicating them, define ownership boundaries before milestones, distinguish
infrastructure from completed capability, expose dependencies and decision
points, preserve completed milestone history, link each active milestone to
one implementation plan and later one closure record, state non-goals, and
remain at the subsystem level rather than becoming a file-by-file checklist.
