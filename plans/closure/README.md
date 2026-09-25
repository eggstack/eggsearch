# Closure and Verification Records

This directory contains evidence-based completion records for implementation
milestones. A closure record is the gate determining whether the corresponding
subsystem milestone can be marked complete.

## Layout and naming

```text
closure/<subsystem>/NNN-status.md
```

Use the same milestone number as the source implementation plan. One
workstream-level closure file may cover several historical milestones when the
underlying phase evidence already exists in `plans/archive/`; in that case
the record MUST map each historical milestone to its archived evidence.

## Required structure

Follow `plans/003-planning-process.md` §2.5: source plan and roadmap,
repository baseline reviewed, implementation commits, executive finding,
requirement-to-evidence matrix, production implementation evidence,
verification executed (exact commands plus pass/fail/skip counts without
concealing partial execution), invariant review, failure/recovery review,
migration/compatibility review, security review, documentation and operations,
unresolved findings by severity
(critical/high/medium/low), roadmap disposition, and registry updates.

## Closure rules

A milestone MUST NOT be marked closed when only compilation or formatting was
verified, required tests were not run without justified substitute evidence,
a user-visible capability has only internal infrastructure, a security or
migration requirement is unimplemented, or a known high-severity defect
remains. A milestone MAY be conditionally closed when production
implementation is complete but named external or operational evidence cannot
be obtained in the current environment, with the condition, risk, and exact
future evidence explicit.
