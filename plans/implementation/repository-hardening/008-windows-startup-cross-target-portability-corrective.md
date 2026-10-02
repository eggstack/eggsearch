# Plan 008 — Windows Startup and Cross-Target Release Portability Corrective

Status: closed

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Milestone: M008 Windows startup/process-token portability and cross-target CI corrective

Primary class: invariant + infrastructure corrective

Planning baseline: `a5f1b0dcd89d90d8ff4fb3bc264bdfcace7acb19`

Relevant prior milestones and evidence:

- M004 process/unsafe boundary:
  `plans/implementation/repository-hardening/004-process-and-unsafe-boundary-hardening.md`
- M004 historical closure:
  `plans/closure/repository-hardening/004-status.md`
- M007 corrective process/unsafe ratchet:
  `plans/implementation/repository-hardening/007-corrective-closure-evidence-and-process-ratchet.md`
- M007 closure:
  `plans/closure/repository-hardening/007-status.md`
- CodeGG parity M003 release handoff:
  `plans/closure/codegg-legacy-search-parity/003-status.md`

Relevant long-term requirements:

- `plans/000-long-term-specification.md#6` — distribution and supported-platform operations.
- `plans/000-long-term-specification.md#9` — verification gates.
- `plans/003-planning-process.md#7` — corrective-pass requirements.

Applicable ADRs:

- `plans/adrs/ADR-0003-outcome-b-optional-outbound-routing.md` only as a
  preserved egress/release compatibility boundary.

Hard dependencies: none.

Release dependency created by this finding:

- the next tagged release, including the intended CodeGG parity release
  `v0.4.0`, MUST NOT be published until M008 closes on an exact candidate
  with green Windows and full release-target qualification.

## 1. Objective

Restore supported Windows compilation after a platform-cfg regression in
startup PID-record handling, clean the directly exposed Windows cfg warnings,
and add a durable ordinary-CI portability gate so a Linux-only green CI run
cannot again coexist with a known Windows compile failure.

The pass must preserve the existing startup/process ownership established by
M004/M007. It is a portability correction, not a new process architecture.

## 2. Current evidence

Current `main` is `a5f1b0dcd89d90d8ff4fb3bc264bdfcace7acb19`.

Routine CI on this exact SHA is green:

- workflow: `CI`
- run: `36979619175`
- event: `push`
- conclusion: success.

Cross-target qualification on the same SHA is red:

- workflow: `Egress feature qualification`
- run: `36979619135`
- event: `push`
- conclusion: failure.

Matrix result on that run:

- Linux x86_64: pass;
- Linux aarch64: pass;
- Linux armv7: pass;
- macOS x86_64: pass;
- macOS aarch64: pass;
- Windows x86_64 MSVC: **fail**;
- Windows aarch64 MSVC: **fail**;
- MSRV all-features 1.89: pass.

Both Windows jobs fail while compiling ordinary eggsearch library code, not an
egress implementation module:

```text
error[E0425]: cannot find function `process_start_token` in this scope
  --> src/startup.rs:1076:9
```

The failing call is in `process_record_contents()`, which calls
`process_start_token(std::process::id())` unconditionally. The available
implementations are cfg-gated to Linux and macOS only.

The regression was introduced by
`d16a6c6adb04a5b6e2ccb8fec68987e80a452875`
(`fix: address reported correctness bugs`), which correctly split
`process_matches` and process-token lookup by OS but left PID-record
serialization compiled against an OS-specific helper on Windows.

The same Windows job also exposes three portability warnings worth correcting
inside this bounded pass:

1. `src/meta/safe_open.rs` imports `CString` unconditionally although its
   use is Unix-only;
2. `platform_info()` has a `#[cfg(not(unix))]` fallback that also compiles on
   Windows after the Windows branch already returns, producing unreachable-code
   diagnostics;
3. `src/update.rs::set_executable(path)` does not use `path` on non-Unix
   targets.

These warnings are not the release blocker by themselves, but they are direct
cfg-hygiene defects exposed by the same supported target and should be removed
without expanding scope.

## 3. Why existing verification did not catch it

Routine `.github/workflows/ci.yml` currently runs only on Ubuntu. The
Windows-specific cfg surface therefore does not compile in the ordinary
PR/main gate.

The repository already has a seven-target compile qualification workflow,
`.github/workflows/egress-feature-qualify.yml`, but its push/PR path filter is
oriented around egress-owned files and does not include `src/startup.rs`.
Recent unrelated provider/egress-sensitive changes happened to trigger that
workflow and exposed the startup regression.

This means the current green routine CI is insufficient evidence for supported
platform portability. The defect is not that the seven-target workflow failed;
the workflow correctly detected the problem. The defect is that Windows
compilation is not a durable ordinary-CI invariant for changes anywhere in
production Rust code.

## 4. Invariants that must not regress

- Rust MSRV remains 1.89.
- Seven release targets remain unchanged.
- Windows x86_64 and Windows aarch64 remain supported release targets.
- Ten MCP tools, schemas, CLI arguments, provider IDs, CodeGG contract, browser,
  PDF, and egress behavior remain unchanged.
- `src/process.rs` remains the shared bounded process/session owner.
- M007's crate-wide `deny(unsafe_code)` and explicit unsafe inventory remain
  enforced.
- No shell-mediated process execution is introduced.
- Cron/process control remains unsupported on Windows unless separately
  designed; this corrective pass must not invent partial Windows cron
  supervision merely to satisfy compilation.
- Linux PID reuse protection and macOS process identity semantics must remain
  intact.
- M001/M007 scheduled-security evidence condition, M002 release-provenance
  condition, and M006 chromiumoxide blocker remain unchanged.
- The intended `v0.4.0` release is not cut from a candidate with a red
  supported-target matrix.

## 5. Non-goals

- Designing Windows cron/process supervision.
- Replacing the PID-file format.
- Replacing the bounded process runner.
- Broad startup/service-manager refactoring.
- Broad warning cleanup unrelated to cfg/platform ownership.
- Changing the egress routing architecture.
- Changing release targets or dropping Windows support.
- Publishing `v0.4.0`.
- Closing M001/M002/M006 operational/external conditions.
- Altering CodeGG provider parity semantics.

## 6. Ordered corrective work

### Work package A — make process-token ownership total across compile targets

Refactor the startup process-token seam so every supported compilation target
has a well-defined implementation boundary.

Preferred contract:

```text
process_start_token(pid) -> Option<String>

Linux  -> /proc start-time token
macOS  -> bounded ps/lstart token
other  -> explicit unsupported/None implementation
```

An equivalent cfg structure is acceptable if it keeps unsupported-platform
semantics explicit and avoids compiling calls to nonexistent functions.

Requirements:

- `process_record_contents()` compiles on every supported target;
- Windows must not fabricate a process identity token it cannot later validate;
- Linux and macOS token generation/matching semantics remain unchanged;
- unsupported-target behavior must be deterministic and documented by tests;
- do not weaken `process_matches()` so an empty token can authorize ownership.

If inspection shows PID-record creation is genuinely unreachable and should be
cfg-gated away on Windows, that is also acceptable, but the implementation must
prove the public startup paths still compile and preserve the documented
unsupported Windows cron behavior. Prefer one total private abstraction over
scattered caller cfgs unless the ownership model clearly favors gating the
entire feature.

### Work package B — clean the directly exposed Windows cfg warnings

Within the same candidate:

- cfg-gate `CString` imports/use so Windows does not compile unused Unix-only
  imports;
- make `platform_info()` cfg branches mutually exclusive so Windows does not
  compile the generic non-Unix fallback after its Windows return;
- make `set_executable`'s non-Unix branch explicit so its argument is not an
  accidental unused binding.

Do not turn this into repository-wide warning cleanup.

Run Windows clippy/check evidence where practical. If an existing warning is
unrelated to these touched boundaries, record it rather than broadening M008.

### Work package C — add focused portability regression tests

Add deterministic tests for the process-token/PID-record contract that can run
on the host platforms where the behavior exists, plus compile-time/static
evidence for unsupported platforms.

At minimum cover:

- Linux process records receive a non-empty token and reject mismatched tokens;
- macOS token path remains bounded and preserves current process-match
  semantics;
- unsupported-platform token acquisition is explicit and cannot cause a missing
  symbol;
- empty/unsupported tokens cannot make `process_matches` succeed;
- cfg ownership remains mutually exclusive.

Do not add tests that claim Windows cron runtime support.

### Work package D — make Windows compile a routine PR/main invariant

The repository needs a guard that would have caught this commit before merge.

Add an ordinary CI Windows compile job, preferably
`x86_64-pc-windows-msvc`, on every PR/main push that can change Rust
production behavior. It MUST compile the same broad feature surface expected of
normal development, preferably:

```bash
cargo check --locked --all-features --target x86_64-pc-windows-msvc
```

Use the pinned Rust 1.89 toolchain policy already established by the repo.

The goal is not to duplicate the full seven-target release matrix in routine
CI. One native Windows x86_64 compile gate is the fast portability sentinel.
The existing seven-target egress/release qualification remains the exact
release-target gate.

If all-features Windows check exposes a pre-existing unsupported optional
feature, stop and classify that incompatibility instead of silently reducing
the feature set until the check turns green.

### Work package E — preserve and strengthen full release-target qualification

Do not rename the egress workflow merely for this corrective pass, but update
its trigger/contract if needed so startup/process/platform-owned changes can
invoke it intentionally.

At minimum, a handoff agent must be able to dispatch
`egress-feature-qualify.yml` against an exact SHA and obtain all seven target
results.

Before M008 closure, dispatch the seven-target workflow on the exact corrective
candidate and require all jobs green.

Also dispatch `release-binaries.yml` in `qualify` mode against the exact
candidate and require all seven release builds/smokes to pass. Qualification
must not publish or mutate a GitHub Release.

This second gate matters because the failing source is ordinary release code:
the egress workflow is the detector, but release qualification is the actual
publication contract.

### Work package F — reconcile release handoff state

Update current planning state so the next release cannot be interpreted as
ready while M008 is open.

Required status while M008 is ready/active:

- repository hardening M008: ready/active;
- CodeGG parity M003: remains conditionally closed on publication of `v0.4.0`,
  but that publication is operationally gated on M008 closure;
- M002: remains conditionally closed until the first tagged release produces
  its provenance/immutability evidence;
- M006: remains blocked on chromiumoxide;
- M001/M007: remain conditional pending the scheduled dependency-security run.

Do not rewrite historical CodeGG M003 or M004/M007 closure records. Update
roadmaps/registry/current handoff text only.

## 7. Failure and recovery semantics

- A Windows compile failure is a release blocker.
- A Linux-only green `make check` is not sufficient M008 closure evidence.
- Do not disable Windows jobs, remove Windows release targets, or cfg-out
  supported functionality merely to obtain green CI.
- Unsupported Windows cron/process-control behavior must remain an explicit
  error/unsupported state, never silent success.
- If fixing the token abstraction changes Linux/macOS process ownership
  semantics, stop and treat it as a material process-boundary change requiring
  focused M004-style requalification.
- If the new routine Windows all-features job discovers unrelated failures,
  classify them by ownership/severity before deciding whether they belong in
  M008.
- Release qualification failures remain failures even if ordinary CI is green.

## 8. Compatibility and migration

No storage, wire, MCP, CLI, configuration, provider, or CodeGG migration is
expected.

PID-file serialization should remain compatible on Linux/macOS. Unsupported
platforms may continue writing an empty token only if that path is already
reachable and the token can never authorize process ownership; otherwise cfg
the unsupported path out explicitly.

No release version bump is part of this plan.

## 9. Required verification

Focused local/host tests:

```bash
cargo test --locked --all-features --test bounded_command
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features --test startup
make check
make docs-check
make release-check
```

If there is no integration test target literally named `startup`, run the
existing startup/service-manager test targets discovered in the repository
rather than inventing a command solely to match this plan. Record the exact
commands in closure evidence.

Required CI evidence on the exact candidate:

1. ordinary `CI` including the new Windows x86_64 all-features compile job;
2. manually dispatched `Egress feature qualification` for the exact SHA, with
   all seven release targets and the MSRV job green;
3. manually dispatched `Release binaries` in `qualify` mode for the exact
   SHA, with all seven build/smoke jobs and assembly green.

Record run IDs, candidate SHA, each Windows target result, and the qualification
artifact name.

## 10. Regression evidence that would have caught the defect

The candidate MUST establish evidence for these cases:

1. **Missing target implementation:** a target-specific helper used from
   platform-neutral code cannot disappear on Windows without a compile failure
   in ordinary CI.
2. **Windows cfg overlap:** Windows-specific and generic non-Unix branches do
   not both compile in `platform_info()`.
3. **Unix-only import:** Unix-only FFI/import ownership does not emit Windows
   unused-import diagnostics.
4. **Non-Unix executable permission helper:** the Windows/non-Unix branch is
   explicit and warning-free.
5. **Release-target proof:** both Windows release targets compile in the
   seven-target qualification before closure.

## 11. Documentation updates

Update only factual current-state documents:

- `plans/subsystems/repository-hardening-roadmap.md`;
- `plans/implementation/repository-hardening/000-overview-and-sequencing.md`;
- `plans/registry.md`;
- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`;
- `docs/test-inventory.md` / `architecture/testing.md` if routine Windows CI
  coverage changes.

Do not rewrite historical M004, M007, or CodeGG M003 closure records.

## 12. Acceptance criteria

M008 is closed only when:

- `src/startup.rs` compiles on both Windows release targets;
- both Windows jobs from the seven-target qualification are green;
- Linux and both macOS target checks remain green;
- the directly exposed cfg warnings in the touched Windows boundary are
  resolved or explicitly classified;
- Linux/macOS process-token ownership semantics remain intact;
- unsupported Windows cron/process-control behavior remains explicit;
- ordinary PR/main CI contains a durable Windows x86_64 all-features compile
  gate;
- `make check`, `make docs-check`, and `make release-check` pass;
- exact-candidate egress qualification is entirely green;
- exact-candidate release-binary `qualify` run is entirely green across all
  seven release targets;
- registry/roadmaps identify M008 as the gate before `v0.4.0` publication;
- no MCP/CLI/provider/CodeGG/browser/PDF/egress contract regression is present.

## 13. Stop conditions

Stop and split further work if:

- Windows process supervision must become a supported runtime capability rather
  than remaining explicitly unsupported;
- fixing process identity requires changing the PID-file format;
- a second independent Windows release-blocking defect is found outside the
  bounded startup/cfg boundary;
- the routine Windows all-features gate exposes a systemic optional-feature
  portability problem;
- release qualification finds a medium-or-higher unrelated correctness issue;
- closing M008 would require dropping a release target or weakening M007's
  process/unsafe invariants.

## 14. Closure evidence required

Create `plans/closure/repository-hardening/008-status.md` containing:

- exact implementation and closure candidate SHAs;
- root-cause statement linking the regression to
  `d16a6c6adb04a5b6e2ccb8fec68987e80a452875`;
- before/after cfg ownership for `process_start_token` and
  `process_record_contents`;
- disposition of the three Windows cfg warnings;
- focused startup/process/static-guard test outcomes;
- ordinary CI run ID proving the Windows sentinel job passes;
- seven-target egress qualification run ID and per-target results;
- release-binaries qualification run ID, seven-target results, and artifact;
- compatibility statement for Linux/macOS process identity and unsupported
  Windows cron behavior;
- planning-state reconciliation showing `v0.4.0` publication is unblocked only
  after M008 closes;
- unresolved findings by severity;
- recommendation: closed, corrective pass required, or blocked.

The closure commit must update the M008 status in the subsystem roadmap,
sequencing overview, and registry together.
