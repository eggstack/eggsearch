# Python Native Library M007 — Windows Safe-Read Corrective Closure

Status: closed
Candidate: `faa1da2e5d5a8c38045001fec1ae1a230bc36bde`
Implementation plan: `plans/implementation/python-native-library/007-windows-safe-read-corrective.md`
Roadmap: `plans/subsystems/python-native-library-roadmap.md` M007

## Defect

`safe_open_relative()` in `src/meta/safe_open.rs`, `#[cfg(not(unix))]`
branch, probed the jail root with `File::open(root)` where `root` is
always a workspace directory. Opening a directory with `File::open` fails
on Windows, so every `safe_read_file`/`safe_open_relative` call failed on
Windows: local search enumeration still found files (cards exist, trust
markers intact), but all content reads failed and every Windows local
snippet was `None`. A downstream consumer in `security_search.rs:1538`
shares the helper. No crash, no jail escape — fail-closed to absent
content. The defect predates the Python workstream; M006 Windows smoke
(run `38611932990`, confirmed by instrumented run `38614592873`) exposed
it because no Rust test executes on Windows in CI (the Windows job is
`cargo check` only) and no prior smoke test asserted snippet content.

## Repair

`src/meta/safe_open.rs` (+20/−3 lines in the non-unix branch only):
replace the `File::open(root)` probe with a `std::fs::metadata(root)`
directory check. Missing roots and file-as-root both report the pre-existing
`RootOpenFailed` variant; symlinked roots keep unix behavior (followed at
open, components still jailed by the walk). No new error variants, no API
change, no policy change — the jail contract (relative-only, no traversal,
hidden-file handling, size caps, `follow_symlinks=false` default) is
byte-identical, and the unix fd-walking branch is untouched.

## Regression tests

- New `safe_open_root_must_be_directory`: file-as-root reports
  `RootOpenFailed` on every platform. Passes on Linux before and after;
  on Windows it fails before the repair (the probe opened the file and the
  walk died later with `NotFound`) and passes after.
- Existing `safe_open_normal_file` / `safe_read_file_works` already fail
  on Windows before the repair (directory probe) and pass after; they
  simply never ran on Windows until the Python smoke matrix.
- Windows proof is the hosted matrix, not unit execution: all four
  Windows smoke jobs on the repair candidate report `23 passed, 5
  skipped`, including `test_explicit_path_overrides_default_config` with
  content-snippet asserts that failed on every pre-fix run.

## Verification on the exact candidate

Working tree clean at `faa1da2`.

| Check | Result |
|---|---|
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `cargo test --locked --all-features --lib safe_open` | 18 passed, incl. the new test |
| `cargo test --locked --all-features --lib local_backend` | 34 passed |
| `cargo test --locked --all-features --lib sdk` | 6 passed |
| `make check` | fmt/clippy/no-default/tests/hygiene pass; stops at `dependency-policy` on upstream rustls RUSTSEC-2026-0346–0349 (owned by the dependency process, lockfiles untouched) |
| Python develop suite | 28 passed |
| Hosted `Python native qualification` run `38617598636` | 5/5 builds, 20/20 smokes (4/4 Windows), sdist rebuild green; audit job red only on the standing rustls residual |

## Residuals and observations

- The non-unix component walk follows per-component symlinks where the
  unix branch rejects them with `O_NOFOLLOW` (pre-existing, unchanged;
  noted for the repository-hardening workstream, not a qualifier for
  this closure).
- Linux CI still has no Windows test execution for Rust code; the Python
  smoke matrix remains the only Windows execution gate. Any future change
  to `safe_open.rs` should be accompanied by a Windows smoke run before
  the owning plan closes.

## Handoff

M007 is closed. M004 and M006 close on the same repair candidate
`faa1da2` in the same commit (`004-status.md`, `006-status.md`).
