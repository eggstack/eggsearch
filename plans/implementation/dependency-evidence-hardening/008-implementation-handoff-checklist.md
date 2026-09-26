# Plan 008 — Dependency Evidence Hardening Handoff Checklist

Status: handoff checklist

Scope: coordinates M001-M007 under
`plans/subsystems/dependency-evidence-hardening-roadmap.md`.

## Before implementation

- Confirm the current branch SHA; rebase each plan's current-state evidence if
  the parser/applicability code has materially changed from
  `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`.
- Read ADR-0004, the subsystem roadmap, `architecture/security.md`,
  `architecture/hardening.md`, `architecture/testing.md`, and
  `architecture/maintenance.md`.
- Preserve the existing dependency-file root/canonicalization/safe-open/1 MiB
  boundary.
- Do not run package managers or repository code as part of parsing.

## Gate A — M001 first

M001 must close before any format plan begins.

Required evidence:

- typed resolved versus requirement/reference/integrity fields;
- legacy `version` compatibility;
- confidence composition;
- finding provenance/relation propagated into applicability;
- parse-report seam;
- regression proving a requirement-shaped version cannot be treated as an
  installed resolved version.

## Gate B — Format milestones

After M001 closes, M002-M006 may be implemented independently.

### M002

Require Cargo source/alias/workspace tests, Go `go.sum` demotion +
replacement/vendor tests, Python PEP 508/name-normalization tests, and
Windows/Unix dispatch parity.

### M003

Require real NuGet lock graph fixtures and XML structural tests proving Maven
exclusions/dependency-management cannot become direct findings. Qualify
`quick-xml` footprint if added.

### M004

Require Bundler GEM/GIT/PATH fixtures and regression for nested child
constraints. Preserve Composer source/dist provenance.

### M005

Require npm v1-v3, Yarn Classic/current supported modern shape, and pnpm v6/v9
fixtures. Unknown future lock versions must be explicit unsupported/partial
states. Record the YAML parser decision.

### M006

Require action SHA/tag/branch/local/docker cases and Docker
tag/digest/platform/stage/registry-port/interpolation cases. No ref resolution
over the network.

## Gate C — M007 aggregate closure

Do not start final qualification until the intended format milestones are
closed.

Required evidence:

- file/finding budgets with boundary tests;
- parse diagnostics surfaced through stable warning codes;
- property-test suite;
- dependency parser fuzz target;
- realistic multi-ecosystem corpus;
- dependency graph and release-binary size comparison;
- `make fuzz-smoke`, `make bench-check`, and `make check` on the exact
  closure candidate.

## Review questions for every implementation agent

Before marking a plan ready for closure, answer:

1. Does every emitted exact version come from evidence that actually resolves a
   version?
2. Could a source/reference/workspace/path distinction change whether a public
   advisory applies?
3. Is unsupported syntax visible rather than silently empty?
4. Can malformed input panic, explode memory, or cause unbounded work?
5. Is ordering deterministic at normal and truncated boundaries?
6. Did compatibility preserve existing clients without preserving unsafe
   security semantics?
7. Were current upstream-generated fixtures used, not only handcrafted happy
   paths?

## Workstream complete

The handoff is complete when M001-M007 each have closure records, the subsystem
roadmap and registry agree on status, and no supported parser can promote a
requirement, checksum, mutable reference, or unsupported partial parse into
High-confidence exact dependency applicability.


## Post-M007 corrective handoff

Gate C was satisfied for the original M007 candidate, but it is no longer the
terminal workstream gate. A later audit found correctness conditions outside
the original closure coverage.

Before treating this workstream as complete:

1. execute
   `009-corrective-applicability-provenance-and-parse-status.md`;
2. require accepted `closure/dependency-evidence-hardening/008-status.md`;
3. execute
   `010-final-closure-and-registry-reconciliation.md`;
4. require accepted `009-status.md` and matching closed roadmap/registry
   status.

Do not rewrite M001-M007 closure records. If M008 uncovers another material
correctness defect, create another corrective plan and keep M009 blocked.

M008 closed at implementation `3fd3076c4ad9680490b35a96759cab3f5022e6bc`;
accepted corrective evidence is `plans/closure/dependency-evidence-hardening/008-status.md`.
Hosted CI run `36217887427` passed on exact closure candidate
`12d8f9bf52217cef30e53006ee412dac25b79f03`. M009 then passed exact-candidate
qualification and closed the subsystem in `plans/closure/dependency-evidence-hardening/009-status.md`.
