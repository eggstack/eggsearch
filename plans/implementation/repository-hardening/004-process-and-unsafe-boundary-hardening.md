# Plan 004 — Bounded Process Execution and Unsafe-Boundary Hardening

Status: ready

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Milestone: M004 bounded process execution and unsafe-boundary consolidation

Primary class: infrastructure + security invariant

Planning baseline: `c49c600b76e690bb1bc52f641554cca6bf79f36f`

Hard dependencies: none.

Relevant long-term requirements:

- `plans/000-long-term-specification.md#4`
- `plans/000-long-term-specification.md#9`

## Objective

Make external process execution obey one explicit ownership model: bounded
captured output, explicit deadlines, deterministic process-group termination,
no shell interpolation, and one narrowly documented Unix unsafe seam for
creating owned sessions/process groups.

Preserve startup/service/update/local-workspace behavior and all platform
capabilities.

## Current evidence

`src/meta/local_inventory_cache.rs` already contains
`run_bounded_command()`, which:

- captures stdout/stderr concurrently;
- caps captured bytes;
- applies a timeout;
- owns process-group termination on timeout/cap breach.

The same module has a Unix `pre_exec(libc::setsid)` block so group
termination cannot kill the parent/session.

`src/startup.rs` separately uses:

- `Command::new("crontab").arg("-l").output()`;
- generic `capture_command(...).output()`;
- status-only `Command::status()` helpers without a shared timeout;
- a second Unix `pre_exec(libc::setsid)` block for detached server launch.

`src/update.rs` is already materially better bounded:

- cargo fallback has a 30-minute timeout, inherited stdout/stderr, and
  `kill_on_drop(true)`;
- downloaded candidate `--version` execution has bounded stdout/stderr and a
  short timeout.

Existing `tests/static_guards.rs` forbids unbounded `.output()` in
`local_inventory_cache.rs` but does not enforce the rule repository-wide.

## Invariants

- Local workspace discovery never executes repository code.
- Git commands remain argument-vector execution, never `sh -c` / `cmd /c`.
- Git stdout/stderr caps, timeout outcomes, and process-group kill semantics do
  not weaken.
- Startup method selection/systemd/launchd/cron/Windows SCM behavior remains
  equivalent.
- Detached croncheck/server processes remain properly detached and owned.
- `eggsearch update` keeps its existing long-running cargo fallback UX and
  candidate verification semantics.
- Process failures return bounded diagnostics without leaking unrelated
  environment/secrets.
- No public API or CLI surface is added for arbitrary command execution.

## Non-goals

- A general subprocess library for external consumers.
- Sandboxing third-party processes.
- Replacing service managers.
- Moving chromiumoxide's internal process management into eggsearch.
- Changing git commands or local inventory semantics for performance.
- Using async process execution everywhere merely for uniformity.

## Required work

### 1. Define one internal process-execution owner

Extract the reusable mechanics from `local_inventory_cache.rs` into a private
top-level/platform module with a small typed policy surface.

The internal API should distinguish at least:

- bounded captured command: timeout + stdout/stderr caps + exit status;
- bounded status command: timeout + no unbounded capture;
- process-session/group configuration and owned-group termination.

Do not expose arbitrary command execution through the public crate API.

Keep command construction at call sites so domain owners still decide the
program/arguments/current directory/environment.

### 2. Preserve and generalize concurrent draining

The shared captured runner must retain simultaneous stdout/stderr draining.
Sequential drain is prohibited because either pipe can fill and deadlock the
child.

Output cap semantics must specify:

- separate or aggregate caps;
- whether truncation kills the command or returns a truncated partial result;
- how timeout vs output-cap vs non-zero-exit are distinguished.

For git inventory, retain current fail-safe behavior exactly unless focused
tests demonstrate a defect.

### 3. Bound startup/service probes and crontab reads

Migrate captured startup operations such as `crontab -l`,
`systemctl is-system-running`, and `sc query` to the shared bounded runner.

Apply finite deadlines to service-manager write/status commands where hanging
would otherwise block eggsearch indefinitely. Choose conservative platform
budgets and keep user-facing errors actionable.

Crontab content still passes through the existing newline/NUL injection guard
before writeback.

Do not change service templates, executable quoting, or startup registration
formats in this milestone.

### 4. Consolidate Unix setsid/pre_exec unsafe code

Move the two current `pre_exec + libc::setsid()` implementations behind one
Unix-only helper.

The helper must:

- contain the only production unsafe block needed for this behavior;
- do only async-signal-safe operations appropriate to the post-fork/pre-exec
  context (`setsid` and immediate errno reporting);
- capture/allocate nothing in the closure;
- carry a concise `SAFETY:` rationale. This safety note is an explicit
  exception to the repository's normal "no comments unless needed" guideline.

Apply `#![deny(unsafe_code)]` at the appropriate crate/module scope with the
smallest targeted `#[allow(unsafe_code)]` around this helper, if current
module layout permits it without affecting generated/upstream code.

If another legitimate production unsafe site is discovered, inventory it and
stop before broadening the allowlist silently.

### 5. Termination semantics across platforms

Retain Unix owned-session/process-group termination and Windows tree termination
for commands whose descendants must not survive a timeout.

Tests must prove:

- timeout kills the owned child/group;
- output-cap breach does not leak a live child;
- successful commands are reaped;
- a process PID/session mismatch cannot kill an unrelated process;
- Windows command-tree behavior remains guarded by platform-specific tests or
  CI qualification.

### 6. Keep updater exceptions explicit

Do not force the 30-minute `cargo install` fallback into a small captured
buffer. It intentionally inherits user-visible output and already has an outer
deadline/kill-on-drop.

Do use the shared process policy for updater operations only where it improves
ownership without changing UX. Document why the long-running cargo child is a
separate policy class.

### 7. Repository-wide static guards

Extend static guards/hygiene so production code cannot casually reintroduce:

- raw `.output()` for unbounded captured external commands;
- new `unsafe` blocks outside the approved Unix process helper;
- shell-mediated command strings where argument vectors are available.

Tests and narrowly documented platform fixtures may use raw process helpers as
needed.

Avoid purely textual guards that produce false positives in documentation/test
strings; scope the scan to production Rust sources.

### 8. Documentation

Update `architecture/security.md`, `architecture/maintenance.md`,
`architecture/local-workspace.md`, and startup docs with:

- process ownership/budget model;
- unsafe seam and its invariant;
- platform termination behavior;
- allowed exceptions (updater cargo fallback, browser upstream management).

## Failure and recovery semantics

- Timeout and output-cap outcomes must be distinguishable from non-zero exits.
- Failure to kill/reap an owned child is an error, not a silent success.
- Startup health/probe failures preserve current degraded/unavailable
  classification rather than crashing unrelated commands.
- Missing platform utilities preserve current supported fallback/error behavior.
- No process runner may return success before owned child cleanup is complete
  when cleanup is required.

## Focused verification

At minimum:

```bash
cargo test --locked --all-features --test bounded_command
cargo test --locked --all-features --test local_workspace_integration
cargo test --locked --all-features --test property_local_fs
cargo test --locked --all-features --test property_local_fs_extended
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features startup
cargo test --locked --all-features update
make check
make docs-check
```

Run platform CI/qualification required by the startup subsystem, especially
Windows and macOS paths, before closure.

## Acceptance criteria

- One shared internal owner implements bounded captured process execution.
- Production startup captures have explicit output caps and deadlines.
- Git/local inventory preserves its existing bounded behavior.
- Exactly one production Unix unsafe process-session helper owns
  `pre_exec/setsid`.
- Static guards reject new unapproved unsafe/unbounded process captures.
- Service/startup/update/local workspace behavior is compatible on all
  supported platforms.
- No API, tool, provider, browser/PDF/egress, or release-target regression.

## Stop conditions

Stop if:

- extracting the runner changes git inventory ordering/identity semantics;
- startup service behavior cannot be preserved with a finite timeout;
- safe consolidation would require a platform-specific process library with a
  materially larger dependency/security surface;
- another unsafe boundary is discovered that requires a separate design;
- Windows/macOS qualification exposes process-lifecycle regressions.

## Closure evidence required

Create `plans/closure/repository-hardening/004-status.md` containing:

- process-execution inventory before/after;
- shared runner ownership map;
- timeout/output-cap/group-kill tests;
- unsafe-site count and exact allowlist;
- platform qualification evidence;
- canonical CI and compatibility statement.
