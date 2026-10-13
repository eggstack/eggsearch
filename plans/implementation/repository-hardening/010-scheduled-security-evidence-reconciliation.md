# Plan 010 — Scheduled Dependency-Security Evidence Reconciliation

Status: closing

Source roadmap: `plans/subsystems/repository-hardening-roadmap.md`
Milestone: M010 corrective reconciliation for M001/M007
Primary class: infrastructure + security invariant
Hard dependencies: none; the first scheduled workflow run is available

## Objective

Reconcile Repository Hardening M001 and M007 after the scheduled
dependency-security workflow completed successfully for the first time. Keep
the original implementation and corrective closure records intact and append
the event-specific operational evidence.

## Evidence boundary

Verify the GitHub Actions run itself: workflow identity, `schedule` event,
head SHA, completion status, job outcome, and cargo-deny summary. A
`workflow_dispatch` run does not satisfy this plan. Do not change the cron or
manufacture a scheduled event.

## Required work

1. Audit the first schedule-event run and confirm the cargo-deny step passed
   with advisory, ban, license, and source checks successful.
2. Update M001/M007 plans, closure records, repository-hardening roadmap,
   sequencing overview, and registry to reflect closure. Preserve the original
   conditional decisions as historical context.
3. Update `packaging/check-planning-consistency.py` and its Rust static guard
   fixture so they require M001/M007 closed, M002 closed by M009, and M006
   blocked. Keep the closed-range shorthand guard.
4. Write `plans/closure/repository-hardening/010-status.md` with run ID,
   event, head SHA, job outcome, exact evidence, status changes, checks, and
   remaining blockers.

## Verification

Run `python3 packaging/check-planning-consistency.py`,
`cargo test --locked --test static_guards planning_status_consistency_no_closed_range_shorthand -- --exact`,
`make check`, and `git diff --check` on the reconciliation candidate.

## Acceptance and stop conditions

Close only if the first `schedule`-event run completed successfully and
reported the expected cargo-deny checks as passing. If the event or run
outcome differs, keep M001/M007 conditionally closed and record the mismatch.

Closure record: `plans/closure/repository-hardening/010-status.md`.
