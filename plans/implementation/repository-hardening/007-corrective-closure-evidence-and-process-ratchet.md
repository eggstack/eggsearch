# Plan 007 — Corrective Closure Evidence and Repository-Wide Process Ratchet

Status: ready

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Milestone: M007 corrective closure evidence and process/unsafe ratchet

Primary class: invariant + infrastructure corrective

Planning baseline: `79ac110e4be53daaf8fb6c45f8276de3f2b5f86b`

Original milestones and closure records under correction:

- M001: `plans/implementation/repository-hardening/001-dependency-security-and-policy-gate.md`
- M001 closure: `plans/closure/repository-hardening/001-status.md`
- M004: `plans/implementation/repository-hardening/004-process-and-unsafe-boundary-hardening.md`
- M004 closure: `plans/closure/repository-hardening/004-status.md`

Related state that MUST remain unchanged unless new evidence exists:

- M002 remains conditionally closed under
  `plans/closure/repository-hardening/002-status.md` until a tagged release
  generates and verifies provenance and demonstrates immutable-release behavior.
- M006 remains blocked under
  `plans/closure/repository-hardening/006-status.md` until an upstream
  chromiumoxide release provides a capability-preserving no-reqwest build path.

Relevant long-term requirements:

- `plans/000-long-term-specification.md#4`
- `plans/000-long-term-specification.md#6`
- `plans/000-long-term-specification.md#9`
- `plans/003-planning-process.md#7`

Applicable ADRs:

- `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`
- `plans/adrs/ADR-0003-outcome-b-optional-outbound-routing.md`

Hard dependencies: none.

Operational dependency for full closure:

- one successful GitHub Actions run whose event is actually `schedule` for
  `.github/workflows/dependency-security.yml`.

A manual `workflow_dispatch` run MAY validate workflow wiring immediately,
but MUST NOT be represented as satisfying the scheduled-run acceptance
criterion.

## 1. Objective

Correct the three closure defects discovered after M001-M005 implementation
without reopening already proven transport, forge, dependency-remediation, or
release functionality:

1. reconcile M001's dependency-security closure with its unexercised scheduled
   workflow acceptance criterion;
2. strengthen M004's process/unsafe regression ratchet from a fixed list of
   migrated files to a repository-wide production-source invariant;
3. reconcile roadmap/registry milestone status so compact summaries cannot claim
   stronger closure than the detailed evidence supports.

The pass is intentionally small. It should add regression enforcement and
honest closure evidence, not redesign dependency policy or process execution.

## 2. Readiness and current implementation evidence

The hardening implementation landed in:

- `cb4048180e29cdeaffb77f2cc86eaec2a8e33b7b` —
  `security: harden repository boundaries and supply chain`;
- `85322ac127033bdc4058177ea09cff67417308d0` —
  color-independent reqwest dependency-shape correction;
- `79ac110e4be53daaf8fb6c45f8276de3f2b5f86b` — planning/closure records.

The corrected implementation candidate `85322ac...` has:

- normal CI pass: run `36467672672`;
- release qualification pass: run `36467682962`;
- egress/cross-target qualification pass: run `36469130010`.

Current `main` at `79ac110e4be53daaf8fb6c45f8276de3f2b5f86b` also has a green CI run `36470129630`.

The production implementation itself is materially intact:

- quick-xml resolves 0.41.0;
- rustls resolves 0.23.45;
- the default dependency graph is reqwest-free;
- all-features reqwest is chromiumoxide-only;
- rmcp HTTP verification is eggfetch-backed and bounded;
- `src/process.rs` owns bounded captured process execution and Unix
  session/process-group primitives;
- forge safety policy is decomposed behind one shared policy/budget ownership
  boundary.

This corrective pass MUST preserve those facts.

## 3. Discovered closure defects

### Finding C1 — M001 scheduled dependency-policy evidence was never produced

Original requirement:

- `001-dependency-security-and-policy-gate.md` requires both PR/main CI and a
  scheduled dependency-security run to exercise the policy.
- The focused verification section also says to run the scheduled/PR
  dependency-security workflow on the exact candidate.

What landed:

- `.github/workflows/dependency-security.yml` exists;
- it has a Monday 09:00 UTC schedule and `workflow_dispatch`;
- `make check` runs the same pinned cargo-deny policy on ordinary CI;
- candidate CI proves the policy itself passes.

What is missing:

- `plans/closure/repository-hardening/001-status.md` cites no
  `Dependency security policy` workflow run ID;
- Actions history reviewed on 2026-09-28 contains no completed run of that
  scheduled workflow.

Why original verification missed it:

The closure process treated "workflow file exists + the same make target passes
inside routine CI" as equivalent to exercising the scheduled workflow. That
proves policy correctness but not schedule trigger/wiring correctness.

Corrective classification:

- no dependency vulnerability or runtime defect is reopened;
- M001's implementation is complete;
- M001's closure evidence is operationally incomplete until an actual
  `schedule` event succeeds.

### Finding C2 — M004 process/unsafe ratchet is not repository-wide

Original requirement:

`004-process-and-unsafe-boundary-hardening.md` requires static guards/hygiene
so production code cannot casually reintroduce:

- raw `.output()` capture;
- new `unsafe` outside the approved Unix process helper;
- shell-mediated command strings where argument vectors are available.

It explicitly calls for repository-wide production-source coverage.

What landed:

- `captured_production_processes_use_the_bounded_runner` checks only:
  `startup.rs`, `integrations/common.rs`, `fetch/browser/discover.rs`,
  and `meta/local_inventory_cache.rs`;
- `process_session_setup_has_one_owner` verifies `pre_exec/setsid` only in
  `src/process.rs` and two former owners;
- `src/process.rs` itself has `#![deny(unsafe_code)]` with narrow function
  allows, but the crate root does not enforce that invariant globally.

Why original verification missed it:

The closure matrix verified that all known migrated call sites were clean, then
treated that finite inventory as a future-proof ratchet. It did not test a new
production module outside the allowlist.

Corrective classification:

- no currently demonstrated unsafe/process runtime regression is assumed;
- the missing item is the acceptance guard that prevents future bypass.

### Finding C3 — control-document status is internally inconsistent

Current control surfaces contain stronger summaries than their detailed state:

- subsystem roadmap header says M001-M005 are closed while the milestone text
  correctly says M002 is conditionally closed;
- registry summary says M001-M005 are closed while its detailed M002 row is
  conditionally closed;
- the subsystem roadmap does not contain the explicit milestone-status table
  required by `plans/subsystems/README.md`.

Why original verification missed it:

The close-all bookkeeping update changed summary prose independently from the
per-milestone rows. No guard currently checks compact status summaries against
the authoritative milestone table.

## 4. Invariants that must not regress

- Ten stable MCP tools and their request/response schemas remain unchanged.
- CLI arguments and integration configuration formats remain unchanged.
- CodeGG compatibility remains unchanged.
- quick-xml and rustls patched floors remain enforced.
- cargo-deny source/license/advisory policy remains fail-closed.
- Default dependency graph remains reqwest-free.
- All-features reqwest remains chromiumoxide-only until M006.
- M003's eggfetch-backed MCP verification semantics and bounds do not change.
- M004's bounded runner/process-group behavior does not change merely to add
  guards.
- M005 forge policy/budget/host separation remains unchanged.
- Browser/PDF/egress features remain available.
- M002's first-tagged-release operational condition is not waived.
- M006 is not unblocked by a private chromiumoxide fork.
- Original closure records remain historical evidence; do not rewrite them to
  conceal the defect.

## 5. Non-goals

- Updating dependency versions beyond what current policy requires.
- Changing cargo-deny policy or waiver scope absent a newly discovered advisory.
- Rewriting `src/process.rs`.
- Replacing `libc` process/session primitives.
- Adding a subprocess sandbox.
- Reworking startup/service-manager behavior.
- Publishing a release solely to satisfy M002.
- Forking chromiumoxide or changing browser ownership.
- Reopening M003 or M005 without new evidence.
- Broad config/security module decomposition.

## 6. Ordered corrective work

### Work package A — make unsafe enforcement crate-wide

Add `#![deny(unsafe_code)]` at the library crate root and, if the binary crate
contains independent production code, at the binary crate root as well.

Retain only the narrowly documented `#[allow(unsafe_code)]` sites required by
the internal process owner. Do not broaden the allow to an entire module if
function-level scope remains practical.

Add a static guard that recursively inventories production Rust sources and
fails if:

- `#[allow(unsafe_code)]` appears outside the approved process owner;
- the approved owner gains an unreviewed additional unsafe allow/site;
- another production module introduces `pre_exec`, `setsid`, direct
  process-group kill primitives, or equivalent duplicated session setup.

The compiler lint is the primary unsafe-code enforcement. The static inventory
guards the exception boundary itself.

Acceptance evidence:

- existing process helper compiles with the narrow exception;
- a synthetic/fixture regression that introduces an unsafe allow outside the
  owner is rejected by the guard;
- the allowlist is explicit and minimal.

### Work package B — make captured-process and shell guards production-wide

Replace the fixed four-file captured-process scan with a deterministic recursive
walk of `src/**/*.rs` production code.

The guard MUST reject unapproved:

- `.output()`;
- `wait_with_output()`;
- explicit shell command launch through `sh`, `bash`, `cmd`,
  `powershell`, `pwsh`, or an equivalent shell wrapper when an argument
  vector can be used directly.

Tests and `#[cfg(test)]` code may use direct process capture for fixtures.
The scanner must exclude test-only code intentionally rather than maintaining a
growing production-file allowlist.

Do not create a fragile check that can be bypassed merely by adding a new
production source file. If a textual scanner is used, add parser/scanner
fixtures for comments, strings, multiline command construction, qualified
`std::process::Command`, and test modules so false positives/negatives are
known.

If the new repository-wide guard finds a legitimate production exception,
stop and document that exception's owner/rationale rather than silently adding
a wildcard allow.

Acceptance evidence:

- all current production sources pass;
- a synthetic new production module containing raw captured output fails;
- a synthetic shell-mediated command fails;
- test-only command fixtures remain permitted;
- no runtime process behavior changed unless the scan discovers a real bypass.

### Work package C — exercise dependency-security workflow honestly

Immediately after the corrective candidate is available:

1. run `make dependency-policy` locally/CI as part of the normal candidate
   gate;
2. dispatch `.github/workflows/dependency-security.yml` manually against the
   candidate/default branch to prove the standalone workflow is executable;
3. record the workflow-dispatch run ID and result.

The manual run is wiring evidence only.

For the original M001 scheduled-run requirement:

- if a real `schedule`-event run has already occurred by closure time, record
  its run ID, event type, head SHA, and pass/fail outcome;
- otherwise, M001 and M007 MUST remain **conditionally closed** with exactly one
  operational condition: the first scheduled dependency-security run must
  succeed.

Do not change the cron merely to manufacture a schedule event and do not label
a `workflow_dispatch` event as scheduled evidence.

When the first scheduled run succeeds, a closure-only reconciliation commit MAY
promote M001/M007 to closed. If it fails, M007 remains active/corrective until
the schedule/wiring defect is fixed and re-exercised.

### Work package D — reconcile planning/status control surfaces

Update the subsystem roadmap, overview, and registry so one authoritative
milestone-status table drives summary prose.

Required state after the corrective implementation, before the scheduled event
if it has not yet occurred:

- M001: conditionally closed via M007 operational evidence correction;
- M002: conditionally closed on first tagged release provenance/immutability
  evidence;
- M003: closed;
- M004: closed through M007 once repository-wide guards pass;
- M005: closed;
- M006: blocked on upstream chromiumoxide;
- M007: conditionally closed if scheduled evidence is pending, otherwise closed.

The roadmap MUST contain the milestone status table required by
`plans/subsystems/README.md`.

The registry MUST NOT use shorthand such as "M001-M005 closed" while any member
of that range is conditional.

Do not modify M002 or M006 closure recommendations without their own required
external evidence.

### Work package E — add status-consistency regression evidence

Add a lightweight deterministic planning consistency check, preferably to the
existing repository/packaging hygiene path, that verifies the repository
hardening roadmap and registry do not contradict their milestone status table.

Keep it narrow: this is not a Markdown framework or a generic planning parser.
A small machine-readable status block/table with a checker is acceptable if it
reduces duplicated prose.

At minimum, the guard should catch the exact regression seen here: a summary
claiming a milestone range is closed while one row is conditional or blocked.

If a robust automatic check would introduce more complexity than the duplicated
status it protects, remove/reduce the duplicated summary instead and test the
single-source form.

## 7. Failure, timeout, and recovery semantics

- Dependency-policy execution failure is not a clean audit.
- A manual workflow pass does not clear the scheduled-event condition.
- If the scheduled job fails because RustSec gained a new advisory, treat that
  advisory according to M001 policy; do not weaken or ignore it to close M007.
- Static-guard scanner errors fail closed rather than silently skipping files.
- Recursive production-source enumeration must be deterministic and must not
  follow build output, generated target trees, or external symlinks.
- If crate-wide `deny(unsafe_code)` exposes another legitimate unsafe site,
  stop and classify it explicitly before adding an allow.
- No corrective guard failure should trigger automatic code deletion or
  dependency mutation.

## 8. Compatibility and migration

There is no data, storage, protocol, configuration, CLI, MCP, or provider
migration.

Expected production behavior is unchanged. The only normal source-level change
is stronger compile/static enforcement around unsafe/process use.

If the repository-wide scan discovers a real production bypass and correcting it
changes behavior, record that as material corrective implementation evidence and
rerun the relevant platform qualification rather than treating it as guard-only
work.

## 9. Required focused tests

At minimum:

```bash
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features --test bounded_command
cargo test --locked --all-features --test local_workspace_integration
cargo test --locked --all-features --test property_local_fs
cargo test --locked --all-features --test property_local_fs_extended
make dependency-policy
make packaging-check
make check
make docs-check
```

Run:

```bash
make release-check
```

before final closure because the crate-level unsafe lint and planning/package
guards affect release qualification.

Also run the standalone dependency-security workflow through
`workflow_dispatch` and record its run ID.

If production process code changes beyond lint/guard ownership, rerun the M004
cross-target qualification (Linux, macOS, Windows) and cite the new run. If
only guards/docs/planning change and normal all-target CI compiles those
platform-gated modules as currently designed, do not fabricate a new platform
claim; state exactly what was and was not rerun.

## 10. Regression tests that would have caught the original defects

The corrective candidate MUST include evidence for these negative cases:

1. **New-file process capture:** a new/fixture production source containing
   `Command::new(...).output()` is rejected without editing an allowlist.
2. **New-file unsafe:** unsafe code or `#[allow(unsafe_code)]` outside the
   process owner fails through compiler/static policy.
3. **Shell wrapper:** a production command routed through an interactive/script
   shell is rejected.
4. **Conditional status mismatch:** a roadmap/registry fixture with M002
   conditional but summary "M001-M005 closed" fails the status consistency
   check.
5. **Workflow event honesty:** closure logic/evidence explicitly distinguishes
   `workflow_dispatch` from `schedule`.

These cases are closure requirements, not optional polish.

## 11. Documentation updates

Update only documents whose factual state changes:

- `plans/subsystems/repository-hardening-roadmap.md`;
- `plans/implementation/repository-hardening/000-overview-and-sequencing.md`;
- `plans/registry.md`;
- `architecture/security.md` or `architecture/maintenance.md` only if the
  unsafe/process enforcement contract needs clarification;
- `docs/test-inventory.md` if a new static/planning guard is added.

Do not rewrite M001/M004 historical closure records. M007's closure record is
the corrective control point and should explicitly say which statements in the
older records were incomplete.

## 12. Acceptance criteria

M007 may be fully **closed** only when all of the following are true:

- crate-wide unsafe denial is active with only the approved narrow process-owner
  exceptions;
- repository-wide production scanning rejects new raw captured process output
  and shell-mediated command wrappers;
- negative regression fixtures/tests prove new files cannot bypass the ratchet;
- M004 runtime/process tests remain green;
- dependency policy remains green;
- standalone dependency-security workflow succeeds;
- at least one successful dependency-security workflow run has event
  `schedule`, satisfying M001's original scheduled-run criterion;
- roadmap, overview, and registry agree on every M001-M007 status;
- roadmap contains the required milestone-status table;
- status-consistency regression evidence would catch the original M002 summary
  contradiction;
- M002 remains conditional unless a real tagged-release attestation/immutability
  event occurred;
- M006 remains blocked unless a real upstream chromiumoxide release cleared it;
- full repository/release checks pass on the exact closure candidate.

If all code/guard/planning criteria pass but no real scheduled workflow event has
occurred yet, the required recommendation is **conditionally closed** with that
single named operational condition.

## 13. Stop conditions

Stop and split new work if:

- repository-wide scanning discovers a genuine unsafe/process behavior defect
  requiring architecture changes rather than a bounded call-site correction;
- another unsafe owner is legitimately required outside `src/process.rs`;
- the dependency-security workflow fails because of a new reachable
  vulnerability needing substantive remediation;
- planning reconciliation reveals a different milestone whose closure evidence
  is materially incorrect;
- satisfying M001 would require weakening the schedule/event requirement rather
  than producing the missing evidence;
- M002/M006 external conditions are accidentally pulled into this corrective
  implementation.

## 14. Closure evidence required

Create `plans/closure/repository-hardening/007-status.md`.

It MUST include:

- exact M007 implementation and closure candidate SHAs;
- references to original M001/M004 plans and closure records;
- finding-to-correction matrix for C1/C2/C3;
- crate-wide unsafe exception inventory;
- production process-capture/shell scan scope and negative fixtures;
- static-guard and bounded-command test outcomes;
- `make dependency-policy`, `make check`, `make docs-check`, and
  `make release-check` results;
- standalone dependency-security workflow-dispatch run ID;
- scheduled dependency-security run ID and `schedule` event proof, if it exists;
- explicit recommendation of either closed or conditionally closed according to
  the scheduled-run evidence;
- final M001-M007 status table;
- explicit statement that M002 and M006 were not opportunistically promoted;
- compatibility statement covering MCP/CLI/CodeGG/provider/browser/PDF/egress
  surfaces.

The closure commit must update this record, roadmap status table, overview, and
registry together.
