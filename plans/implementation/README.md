# Milestone Implementation Plans

This directory contains bounded plans handed directly to implementation agents.

Implementation plans are operational documents tied to the current repository
state. They may be corrected, superseded, or archived without modifying the
canonical long-term documents.

## Layout and naming

```text
implementation/<subsystem>/NNN-short-title.md
```

Milestone numbering is local to the subsystem roadmap unless the roadmap
defines another stable identifier.

## Required structure

Follow `plans/003-planning-process.md` §2.4: source roadmap and milestone,
long-term requirements, applicable ADRs, primary class, objective, readiness
(closed hard dependencies, stable interface contracts), current implementation
evidence at the stated baseline, invariants that must not regress, in/out of
scope, required production changes, ordered work packages with acceptance
evidence, failure/timeout/recovery semantics, compatibility and migration,
required tests, exact verification commands, documentation updates, acceptance
criteria, stop conditions, closure evidence required, and handoff notes.

## Handoff rules

Before assigning a plan to an agent: confirm the repository baseline is
current, confirm all hard dependencies are closed, confirm unresolved
decisions have ADRs or are explicitly out of scope, ensure the milestone can
be completed in one coherent pass, ensure tests and closure evidence are
specific, and register the plan in `plans/registry.md`.

## Corrective plans

Corrective work receives a new plan in the same subsystem directory. The
corrective plan must reference the original plan and closure record,
enumerate unclosed findings, and add regression evidence that would have
caught them.
