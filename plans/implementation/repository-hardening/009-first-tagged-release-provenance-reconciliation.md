# Plan 009 — First Tagged-Release Provenance Reconciliation

Status: closing

Source roadmap: `plans/subsystems/repository-hardening-roadmap.md`
Milestone: M009 corrective reconciliation for M002
Primary class: infrastructure
Hard dependencies: none; M002 implementation and first tagged-release evidence are recorded

## Objective

Reconcile Repository Hardening M002's conditional status after the first
tagged release satisfied its remaining provenance and immutability condition.
Preserve the original M002 closure record as historical evidence and record the
later status decision in a hardening-owned closure record.

## Evidence boundary

Use the immutable `v0.4.1` first-tagged-release evidence already recorded in
`plans/closure/codegg-legacy-search-parity/005-status.md` §2 and
`plans/closure/repository-hardening/002-status.md` § First-release evidence
note. Confirm that the exact tag, release run, attestation verifier output,
artifact subjects, published immutability, and repository immutability setting
are linked and consistent. Do not mutate the published release or rerun a
release workflow against an immutable historical tag.

## Required work

1. Audit the two cited closure records and the repository's current M002 status
   references against the recorded first-release evidence.
2. Update the M002 closure recommendation, repository-hardening roadmap,
   sequencing overview, and active registry to `closed`, retaining the
   implementation and release evidence links.
3. Update `packaging/check-planning-consistency.py` and its Rust static guard
   fixture so they accept M002 as closed while keeping M001/M007 conditional
   and M006 blocked. The guard must continue rejecting closed-range shorthand.
4. Write `plans/closure/repository-hardening/009-status.md` recording this
   reconciliation, evidence sources, exact check results, preserved historical
   records, and remaining Repository Hardening blockers.

## Verification

Run `python3 packaging/check-planning-consistency.py`,
`cargo test --locked --test static_guards planning_status_consistency_no_closed_range_shorthand -- --exact`,
`make check`, and `git diff --check` on the reconciliation candidate. No
release, dependency, runtime, or external application mutation is in scope.

## Acceptance and stop conditions

Close only if the immutable release and attestation evidence satisfies the
remaining M002 condition and all source plan, closure, roadmap, overview,
registry, and guard status agree. If any provenance subject, source identity,
or immutability claim is not supported by the cited records, retain M002 as
conditionally closed and record the evidence gap instead.

Closure record: `plans/closure/repository-hardening/009-status.md`.
