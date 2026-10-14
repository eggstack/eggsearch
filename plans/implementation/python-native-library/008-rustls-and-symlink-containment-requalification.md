# Python Native Library M008 — Advisory Remediation and Symlink-Containment Re-qualification

Status: active
Roadmap: `plans/subsystems/python-native-library-roadmap.md` M008
Class: invariant
Related: ADR-0005 (`plans/adrs/ADR-0005-python-native-library-boundary.md`);
closures `plans/closure/python-native-library/004-status.md`,
`006-status.md`, `007-status.md` (retained as history, never amended).
Baseline: `8f632b3` (M004/M006/M007 closed; `dependency-policy` red on
upstream rustls advisories; Windows symlink-following gap contained only
in prose).

## Objective

Resolve the two residuals named in the M004/M006/M007 closures, then
re-qualify the Python distribution on the exact repair candidate so the
closure records reconcile to fully green evidence:

1. Remediate upstream rustls advisories RUSTSEC-2026-0346/0347/0348/0349
   against locked rustls 0.23.45 (lockfile bump to the fixed 0.23 line,
   no manifest change, no waiver).
2. Close the deterministic Windows filesystem hole M007 left open: the
   non-unix walk in `src/meta/safe_open.rs` follows symlinks
   (`std::fs::metadata` at every component) while the unix branch rejects
   them, so `follow_symlinks=false` is not enforced on Windows. Reject
   symlinks present at walk time with the existing `SymlinkDetected`
   taxonomy, and explicitly contain the residual check-to-open race
   (Windows std offers no atomic no-follow open) in the closure record.
3. Rerun the hosted Python native qualification on the exact repair
   candidate and reconcile M004/M006 documentation to the resulting
   green evidence.

## Non-goals

- Publication to PyPI (still separately authorized); no tag, no release
  mutation, no Eggpack producer change.
- Chromiumoxide reqwest removal (repository-hardening M006 owns that
  blocker); no new Windows-only dependencies (no `windows-sys` atomic
  open — the residual race is contained, not engineered away).
- New `SafeOpenError` variants, taxonomy changes, unix-branch behavior
  changes, or root-symlink policy changes (unix follows root symlinks;
  Windows keeps parity).
- Free-threaded Python, ARMv7, Windows arm64, or any target-matrix change.
- Amending `001-status.md`–`007-status.md`; reconciliation lives in the
  new `008-status.md` with a supersession table.

## Current implementation evidence

- `cargo deny check advisories` on the baseline fails exactly on
  RUSTSEC-2026-0346–0349, each with solution `Upgrade to >=0.23.46`.
  Reverse dependents of rustls 0.23.45: `eggfetch-core 0.2.2`,
  `eggress-* 1.0.12`, `hyper-rustls 0.27.9`, `tokio-rustls 0.26.4`,
  plus the direct `rustls = "0.23"` requirement in `Cargo.toml`.
- `src/meta/safe_open.rs` `#[cfg(not(unix))]` branch uses
  `std::fs::metadata` (follows links) for root, intermediate, and final
  components; `File::open` follows a swapped-in link. All local content
  reads (`src/meta/local_backend.rs`, `src/meta/security_search.rs:1538`)
  flow through this path.
- CI `windows-portability` runs `cargo check --locked --all-features
  --target x86_64-pc-windows-msvc` only: no Rust test executes on
  Windows. The Python smoke matrix (5 targets x CPython 3.11-3.14) is the
  only Windows execution gate.

## Invariants that cannot regress

- Ten MCP tools, schemas, CLI behavior, deterministic FNV-1a IDs,
  sanitization tiers, bounded-I/O helpers, `follow_symlinks=false`
  default, `SafeSymlinkFollowingUnsupported` for `follow_symlinks=true`
  on non-Linux, root `#![deny(unsafe_code)]` with the three-file allow
  inventory unchanged, Rust 1.89 MSRV, lockfile-only dependency change
  (manifests, versions, registries untouched).
- Static guards: `no_path_based_reads_in_safe_open`,
  `crate_roots_deny_unsafe_code`,
  `unsafe_allow_inventory_is_explicit_and_minimal`,
  `planning_status_consistency_no_closed_range_shorthand`.

## Expected production changes

- `Cargo.lock`: rustls 0.23.45 -> 0.23.46 (checksum update only).
- `docs/dependency-security.md`: remediation line follows the lockfile.
- `src/meta/safe_open.rs` non-unix branch only: `symlink_metadata` at
  each component, `SymlinkDetected` on links (message parity with the
  unix branch), plus `cfg(any(unix, windows))` regression tests for
  final and intermediate link rejection.
- `bindings/python/tests/test_native.py`: one Windows-only behavioral
  smoke test (symlinked file content never surfaces; real file snippet
  intact; skip with reason when symlink privilege is unavailable).
- `architecture/local-workspace.md`, `architecture/hardening.md`: scope
  the race-free/TOCTOU claims to the atomic primitives and state the
  Windows containment explicitly.

## Ordered work packages

- WP1 rustls remediation: bump, `cargo deny check`, policy script,
  MSRV build proof, deny/policy evidence recorded.
- WP2 symlink containment: fix, Rust tests
  (`safe_open` lib suite green on Linux), Windows-target check compile,
  Python smoke test green locally on macOS/Linux (skipped) plus review.
- WP3 local gates: `make check` fully green including
  `dependency-policy`; `make packaging-check`; Python develop suite
  (28 passed) and `compileall`; `cargo package --list` /
  `cargo publish --dry-run --locked` unchanged shape.
- WP4 hosted exact-commit qualification: push repair candidate,
  dispatch the Python native qualification workflow, require 5/5 wheel
  builds, 20/20 installed-wheel smokes (4/4 Windows incl. the new
  symlink test), extracted-sdist offline rebuild with clean-venv
  consumer run, and a green audit job; harvest digests and job IDs.
- WP5 closure reconciliation: `008-status.md` with evidence matrix and
  the supersession table (new candidate/run supersede `faa1da2` /
  `38617598636` as the qualification basis; old records stand as
  history); roadmap + registry + architecture handoff to M008 closed;
  commit and push on the workstream branch.

## Verification commands

Focused: `cargo deny check`, `cargo test --locked --all-features
--lib safe_open`, `cargo check --locked --all-features --target
x86_64-pc-windows-msvc` (when the target is installed),
`python -m pytest -q bindings/python/tests` (post-`maturin develop`).
Broad: `make check`, `make packaging-check`,
`./packaging/check-contract.sh`.

## Acceptance and stop conditions

M008 closes when `make check` passes end to end (including a green
`dependency-policy`), the hosted qualification on the exact repair
candidate is green in all jobs including audit, and `008-status.md`
plus roadmap/registry/architecture updates land in one closure commit.
Stop and escalate rather than waive: if the rustls bump breaks the
MSRV build, route a waiver through the repository dependency process
instead of adding a `deny.toml` ignore here; if the Windows smoke
symlink test cannot execute (privilege), record the skip counts and let
the Rust tests plus review stand as the containment evidence.

## Required closure evidence

Candidate SHA, qualifying run id with per-job outcome table, wheel/sdist
digests, `cargo deny check` output, `make check` / `make
packaging-check` results, consumer-run counts per target, residual list
(check-to-open race containment, any skipped Windows symlink execution),
and the M004/M006/M007 supersession table.
