# M007 — Windows Safe-Read Corrective for Local Content Access

Status: closed (closure `plans/closure/python-native-library/007-status.md` at candidate `faa1da2`, proven by run `38617598636` Windows smokes)
Primary class: invariant (local path-jail reads must behave identically on all supported platforms; fail-closed semantics preserved)
Source: `plans/subsystems/python-native-library-roadmap.md` M007; corrective to M004/M006 qualification, not a rewrite of closed M001–M003/M005 records.
Planning baseline: `plan/python-native-closure-correctives` `d5e4dcb`, 2026-10-14.
Dependencies: M001–M003 and M005 closed; M004 active; M006 active and awaiting this candidate for its final matrix. No new operational dependencies beyond the existing hosted qualification workflow.
ADR: `plans/adrs/ADR-0005-python-native-library-boundary.md`; normative `plans/003-planning-process.md` sections 2.4, 2.5, 7–10.

## Scope and observed evidence

M006 qualification of `c8bd030` (run `38608502185`) failed 3 config tests
on all Windows smoke jobs. Test-isolation repair (`748ae59`, per-test CWD
plus fallback filename tolerance) fixed 2 of 3; run `38611932990` left
exactly `test_explicit_path_overrides_default_config` failing on all four
Windows interpreters while 16/16 Linux/macOS smoke jobs passed.
Instrumented run `38614592873` (temporary diagnostics, since reverted)
showed the Windows client loading the correct config (`mojeek=true`,
`repo=default-workspace`, card present with `local_trusted` provenance)
but with no content snippet, so `"alpha" in snippet` failed.

Cause, confirmed by code inspection: `safe_open_relative()` in
`src/meta/safe_open.rs`, `#[cfg(not(unix))]` branch, begins with
`File::open(root)` where `root` is always a workspace directory. Opening a
directory with `File::open` fails on Windows (CreateFile without
`FILE_FLAG_BACKUP_SEMANTICS` returns access-denied), so **every**
`safe_read_file`/`safe_open_relative` call fails on Windows. Local search
enumeration still finds files (cards exist, trust intact), but all content
reads fail and every Windows local snippet is `None`. The same branch also
follows symlinked roots, where the unix branch rejects them (`O_NOFOLLOW`
semantics) — the repair aligns both. No Rust test executes on Windows in CI
(the Windows job is `cargo check` only), so the defect was invisible until
the Python smoke matrix asserted snippet content.

Blast radius on Windows: local search scoring degrades to path/role-only
and snippets are absent; `security_search.rs:1538` uses the same helper.
No crash, no jail escape, no cross-platform behavior change: the repair
keeps fail-closed semantics and the existing error taxonomy (no new error
variants, no API change).

## Non-goals and invariants

No change to the jail policy (relative-only, no traversal, no hidden files
unless configured, size caps, `follow_symlinks=false` default), no new
`SafeOpenError` variants, no MCP/CLI contract change, no provider,
transport, sanitization-bypass, or Eggpack change. Root Rust 1.89 and
`#![deny(unsafe_code)]` unchanged. M004 stays active until this fix is
re-qualified; no publication.

## Ordered work packages

### WP1 — Repair the non-unix root check

In `src/meta/safe_open.rs`, replace the non-unix `File::open(root)`
existence probe with a `std::fs::symlink_metadata(root)` check that
requires a directory and rejects symlinked roots when
`follow_symlinks=false` (unix parity). Keep the remainder of the
component walk byte-identical. Preserve every error variant currently
used on that path.

### WP2 — Regression tests

Add unit tests in `src/meta/safe_open.rs`: round-trip read of a file
through a directory root, and rejection of a file-as-root. Both pass on
Linux before and after (shared logic guard); on Windows they fail before
and pass after (stated, not executed here — Linux CI has no Windows test
execution). The hosted Windows smoke matrix (`test_explicit_path_overrides_default_config`
snippet asserts) is the Windows proof.

### WP3 — Re-qualify and close out

Run local gates (`cargo fmt --check`, clippy, affected Rust suites, Python
28-test suite), push, and record the full hosted `Python native
qualification` matrix on the exact repair candidate. M006 then finalizes
M004/M006 closure on those bytes; this plan closes with its own
`007-status.md` in the same commit.

## Verification commands and exit

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all-features safe_open local_backend sdk`
- `maturin develop --manifest-path bindings/python/Cargo.toml --locked` + `python -m pytest -q bindings/python/tests`
- Hosted `.github/workflows/python-native-qualification.yml` on the exact candidate: 5/5 builds, 20/20 smokes (in particular the 4 Windows jobs with snippet asserts), sdist rebuild green; audit job outcome recorded with the standing rustls residual.

PASS means the repair candidate shows Windows snippet asserts green with no
other matrix movement, local gates pass, and the deficiency record plus
this plan close together with M004/M006. Fail closed on any new red.

## Handoff and closure

M007 closes with `plans/closure/python-native-library/007-status.md`
recording the defect, the repair diff, test evidence on both platforms
(including the stated Windows-only observability limit), the exact repair
SHA, and the re-qualification run ID. M004 and M006 close on the same
repair candidate in the same commit; the M006 plan already anticipates this
routing ("issue another corrective rather than writing fictional green
closure").
