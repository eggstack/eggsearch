# Plan 010 — Dependency Evidence Final Closure and Registry Reconciliation

Status: closed

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M009 final closure reconciliation

Primary class: polish + closure infrastructure

Baseline for planning: `a10f23ba92be05d4b42ba1f373c895895cbf88aa`

Hard dependency:

- M008 / Plan 009 must be closed by
  `plans/closure/dependency-evidence-hardening/008-status.md`.

Operational dependency:

- the exact corrected candidate must have successful repository CI evidence
  required by current release/qualification policy.

## Objective

Perform the final evidence and planning-state reconciliation for the dependency
evidence hardening workstream after the corrective M008 semantics are closed.
This milestone changes no dependency-parser or applicability behavior. It
exists to prevent the planning registry and roadmap from claiming closure on a
candidate different from the one that actually fixes the post-M007 findings.

## Why this is a separate milestone

At baseline `a10f23ba`, M001-M007 have closure records and the registry says
"implementation complete", but the subsystem roadmap and registry remain
`active`. A later audit identified correctness defects that require M008.
Simply flipping those status strings now would violate the repository's
SHA-specific closure rule.

M009 therefore runs only after M008 closes, verifies the corrected candidate,
writes the final closure record, and reconciles all planning control surfaces in
one closure commit.

## Invariants

- No production behavior changes in M009.
- Historical M001-M007 closure records remain immutable evidence of what was
  qualified at those candidates; they are not edited to imply they caught the
  later defects.
- M008 retains its own closure record and corrective provenance.
- The roadmap, registry, sequencing plan, handoff checklist, and final closure
  record must agree on the exact terminal candidate and status.
- If verification exposes another correctness defect, stop and create a new
  corrective implementation plan instead of closing the subsystem.

## Non-goals

- Parser refactoring.
- New ecosystem or format support.
- Test-suite cleanup unrelated to dependency evidence.
- Fixing the pre-existing `local_backend` load-sensitive tests unless they
  become reproducible failures attributable to this workstream.
- Publishing a release.

## Required work

### 1. Verify M008 closure is complete and internally consistent

Read `008-status.md` and verify it contains:

- exact implementation SHA;
- provenance compatibility evidence;
- ecosystem identity matrix evidence;
- malformed-versus-empty parser matrix;
- dedup regression evidence;
- focused/canonical test results;
- no unresolved High/Medium correctness finding.

If M008 is conditionally closed for a correctness issue, M009 is blocked.

### 2. Run final exact-candidate qualification

On the exact M008 closure candidate, run the repository's current canonical
gates:

```bash
make check
make bench-check
cargo test --locked --all-features
RUSTUP_TOOLCHAIN=nightly make fuzz-smoke
```

Also run the focused dependency evidence suites from Plan 009 if they are not
already part of `make check`.

Record hosted CI status for the same candidate (ordinary CI and any mandatory
qualification workflow applicable to the repository). A different SHA's green
CI is not closure evidence.

Do not rerun unrelated expensive optional live/network tests unless current
repository policy requires them.

### 3. Write the final M009 closure record

Create:

`plans/closure/dependency-evidence-hardening/009-status.md`

It must summarize:

- M001-M007 historical closures;
- the post-M007 audit findings;
- M008 corrective implementation/closure;
- exact final candidate SHA;
- final verification/hosted-CI evidence;
- remaining intentionally unsupported cases;
- final recommendation.

Do not copy large amounts of old closure prose; link the earlier records.

### 4. Reconcile the subsystem roadmap

Only in the same commit that accepts M009 closure:

- change roadmap `Status:` from active/corrective-active to `closed`;
- mark M008 and M009 closed in the milestone table;
- identify `009-status.md` as the terminal controlling closure record;
- retain the low-severity deferred format limitations as deferred work, not
  blockers;
- preserve the corrective-history note explaining that M008 followed the
  initial M007 qualification.

### 5. Reconcile the active planning registry

Update `plans/registry.md` so:

- Dependency evidence hardening is `closed`, not `active`;
- current milestone reads M001-M009 closed;
- M008/M009 implementation-plan rows show closed with closure links;
- the obsolete dependency-evidence blocked-work table is removed or replaced by
  a concise "none" state;
- the "Closure work and current control points" table includes dependency
  evidence hardening and points to `009-status.md` plus M008 corrective
  evidence where useful;
- no entry describes the M007 candidate as the terminal workstream candidate.

Keep the registry compact; historical detail belongs in closure records.

### 6. Reconcile sequencing/handoff documents

Update:

- `plans/implementation/dependency-evidence-hardening/000-overview-and-sequencing.md`;
- `plans/implementation/dependency-evidence-hardening/008-implementation-handoff-checklist.md`.

Preserve the original M001-M007 sequence, but add/resolve the post-M007
corrective closure addendum so future agents do not stop at Gate C and miss
M008/M009.

### 7. Archive only if current planning policy calls for it

Do not move implementation or closure records merely to reduce the active
directory. If the registry/process convention now calls for archiving fully
closed handoff plans, perform a traceability-preserving move in a separate
mechanical commit or defer it. Closure does not depend on archive cleanup.

## Verification

Because this plan should not alter production code, verify the final
reconciliation diff contains only planning/closure/document-control files.

Then run the documentation/planning checks included by the repository's
canonical gate. If there is no dedicated planning checker, `make check`
success on the exact candidate plus review of the registry/roadmap status is
required.

## Acceptance criteria

- M008 has an accepted closure record with no unresolved correctness blocker.
- Final canonical gates and required hosted CI are green on the same corrected
  candidate.
- `009-status.md` exists and identifies that exact candidate.
- Dependency evidence roadmap status is `closed`.
- Registry status is `closed`, M001-M009 are represented consistently, and
  the closure-control table links terminal evidence.
- Sequencing/handoff documents include the corrective closure history.
- M001-M007 historical closure records remain unchanged.
- M009 introduces no production-code behavior change.

## Stop conditions

Do not close the subsystem if:

- M008 is conditional/blocked on a correctness issue;
- final canonical verification fails;
- hosted CI for the corrected candidate fails;
- registry/roadmap reconciliation would require concealing or rewriting the
  post-M007 corrective history;
- another Medium/High dependency-evidence defect is discovered.

Create a new corrective plan instead.

## Closure evidence required

The M009 closure commit must contain:

- `plans/closure/dependency-evidence-hardening/009-status.md`;
- roadmap closed status;
- registry closed/control-point update;
- sequencing/handoff reconciliation;
- exact final candidate and CI/verification references;
- explicit statement that no production behavior changed in M009.
