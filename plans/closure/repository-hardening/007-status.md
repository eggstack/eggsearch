# Repository Hardening M007 — Corrective Closure Evidence and Process Ratchet

Status: conditionally closed

Recommendation: conditionally closed with one operational condition: the first successful GitHub Actions run of `.github/workflows/dependency-security.yml` with event `schedule`.

Implementation candidate: `15ae6e57693c5ca8b15a684298730ba70de09bb9`
Closure candidate: `15ae6e57693c5ca8b15a684298730ba70de09bb9` (code-qualified candidate; closure records commit is docs-only and does not change production code)

Original milestones and closure records under correction:

- M001: `plans/implementation/repository-hardening/001-dependency-security-and-policy-gate.md`
- M001 closure: `plans/closure/repository-hardening/001-status.md`
- M004: `plans/implementation/repository-hardening/004-process-and-unsafe-boundary-hardening.md`
- M004 closure: `plans/closure/repository-hardening/004-status.md`

M001/M004 historical records remain unchanged. This record is the corrective control point.

## Finding-to-correction matrix

| Finding | Correction | Evidence | Result |
|---|---|---|---|
| C1 M001 scheduled workflow never exercised | Dispatched standalone workflow via `workflow_dispatch` as wiring proof; retained scheduled-event condition for full closure | Dispatch run `36484159375` success, event `workflow_dispatch`; schedule runs `0` for `dependency-security.yml` as of 2026-09-28T21:09Z; workflow contains `schedule: cron '0 9 * * 1'` plus `workflow_dispatch` | pass wiring; conditional on first `schedule` success |
| C2 M004 ratchet covered only migrated files | Crate-wide `#![deny(unsafe_code)]` plus repository-wide production scan for captured output, shell wrappers, and session-setup duplication | `src/lib.rs`, `src/main.rs` deny; `production_rust_files()` walk of `src/**/*.rs`; `captured_production_processes_use_the_bounded_runner`, `shell_wrappers_rejected_in_production`, `process_session_setup_has_one_owner`, `crate_roots_deny_unsafe_code`, `unsafe_allow_inventory_is_explicit_and_minimal` | pass |
| C3 control-document status inconsistent | Reconciled roadmap/overview/registry to one milestone table; added `packaging/check-planning-consistency.py` wired into `packaging-check` plus `planning_status_consistency_no_closed_range_shorthand` | `python3 packaging/check-planning-consistency.py` ok; `cargo test --test static_guards planning_status_consistency` pass; no `M001-M005 closed` shorthand in hardening files | pass |

Why original verification missed C1/C2/C3 is documented in `007-corrective-closure-evidence-and-process-ratchet.md` and preserved here by reference without rewriting history.

## Crate-wide unsafe exception inventory

Crate roots enforce `#![deny(unsafe_code)]`: `src/lib.rs`, `src/main.rs`, plus existing `src/process.rs` file lint. Narrow `#[allow(unsafe_code)]` sites, all function-level:

- `src/process.rs` (5): `configure_new_session`, `terminate_process_group`, `terminate_process`, `is_privileged_user`, `real_user_id`. Approved process owner for Unix session/process-group behavior with no safe equivalent.
- `src/meta/safe_open.rs` (4): `openat_sys`, `close_fd`, `fstat_is_regular_and_size`, `safe_open_relative`. Race-resistant file containment via `openat`/`openat2`, `close`, `fstat`, `File::from_raw_fd`. No safe equivalent for TOCTOU containment. Classified explicitly per stop condition; second legitimate owner beyond the process helper.
- `src/fetch/browser/profiles.rs` (2): `ProfileLock::try_acquire`, `ProfileLock::drop`. Unix `flock` for profile locking. No safe std equivalent for non-blocking exclusive file lock. Classified explicitly; third legitimate owner.

Total 11. Guard `unsafe_allow_inventory_is_explicit_and_minimal` fails on any new file or count change. No module-level broad allow. Compiler lint is primary enforcement; static inventory guards the exception boundary.

## Production process-capture and shell scan scope

Scanner walks `src/**/*.rs` deterministically sorted, skips symlinks, never follows `target/` or external links. Cleaning strips line/block comments and string contents (preserving newlines and length), then removes `#[cfg(test)] mod` blocks by balanced-brace matching, so test-only fixtures never count as production.

- Rejects `.output()` with optional whitespace/newlines between receiver, parens.
- Rejects `wait_with_output`.
- Rejects `Command::new("sh"|"bash"|"dash"|"zsh"|"fish"|"cmd"|"powershell"|"pwsh")` including `tokio::process::Command`, multiline construction, qualified paths. Language-detection strings such as `"bash"` in `src/fetch/detect.rs` do not trigger because they are not inside `Command::new`.
- Rejects `pre_exec(`, `libc::setsid()`, `libc::kill` outside `src/process.rs`.

Negative fixtures prove the ratchet:

- New-file `Command::new(...).output()` rejected without allowlist edit.
- New-file unsafe or `allow(unsafe_code)` outside inventory fails.
- Production shell wrapper rejected; comment, string-only, non-shell `Command::new("git")`, and test-only shell remain permitted.
- Multiline `.output\n()` and qualified `std::process::Command` detected; `//` comment and `".output()"` string do not false-positive.
- Test-module `#[cfg(test)]` content excluded.

All current production sources pass. No runtime process behavior changed. `src/process.rs` taskkill path uses `spawn`, not captured output.

## Verification

- `cargo test --locked --all-features --test static_guards`: 54 passed.
- `cargo test --locked --all-features --test bounded_command`: 31 passed.
- `cargo test --locked --all-features --test local_workspace_integration`: 9 passed.
- `cargo test --locked --all-features --test property_local_fs`: 22 passed.
- `cargo test --locked --all-features --test property_local_fs_extended`: 31 passed.
- `make dependency-policy`: pass (`advisories ok, bans ok, licenses ok, sources ok`; `dependency-policy-check: ok`).
- `make packaging-check`: pass (`workflow-pin-check: ok`, `planning-consistency: ok`, egress contract agrees).
- `make check`: pass (fmt + clippy `-D warnings` + no-default check + all-features tests + hygiene + dependency-policy + packaging-check).
- `make docs-check`: pass (`cargo doc --all-features` no warnings).
- `cargo publish --dry-run --locked`: pass on clean candidate (warns already exists, no dirty-tree error).
- `make release-check`: check + docs-check + release-build + publish dry-run pass; full `test` phase passed on candidate except one transient live-provider flake (`duckduckgo 403`/`yahoo 500`) that passed on rerun and is unrelated to process/unsafe/planning changes.
- Standalone dependency-security `workflow_dispatch` run `36484159375`: success, event `workflow_dispatch`, head `b7d4fcdedc7dcff629d83b8679e897f8275b06e4`, 2026-09-28T21:09:12Z. Wiring evidence only; explicitly not claimed as scheduled evidence.
- Scheduled dependency-security runs for `dependency-security.yml`: 0 total runs as of 2026-09-28T21:09Z (`gh run list --workflow dependency-security.yml`). No `schedule`-event success exists, so full closure remains conditional.

No M004 cross-target requalification was fabricated: production process code changed only by lint ownership (plus two classified safe owners), not behavior. Normal all-target CI compiles platform-gated modules as designed.

## Final M001-M007 status table

| Milestone | Status |
|---|---|
| M001 dependency security/policy | conditionally closed via M007 pending first scheduled run |
| M002 release provenance | conditionally closed on first tagged release |
| M003 rmcp HTTP verification | closed |
| M004 process/unsafe boundary | closed through M007 |
| M005 forge safety decomposition | closed |
| M006 zero-reqwest closure | blocked on upstream chromiumoxide |
| M007 corrective evidence + ratchet | conditionally closed pending first scheduled run |

M002 and M006 were not opportunistically promoted. M002 still requires generated/verified attestation and immutable behavior on first tagged release. M006 still requires an upstream chromiumoxide release making HTTP discovery optional without regressing `Browser::launch`. No fork was introduced.

## Compatibility

No MCP, CLI, CodeGG, provider, browser, PDF, or egress behavior change. Ten stable tools and schemas unchanged. CLI arguments and integration formats unchanged. Default graph remains reqwest-free; all-features reqwest remains chromiumoxide-only. M003 eggfetch verification semantics unchanged. M005 forge policy/budget/host separation unchanged. quick-xml 0.41.0 and rustls 0.23.45 floors remain enforced. cargo-deny policy remains fail-closed.

## Operational condition to promote to closed

When the first `schedule`-event dependency-security run succeeds, a closure-only reconciliation commit may promote M001/M007 to closed citing run ID, event type `schedule`, head SHA, and outcome, without reworking implementation. If it fails on a new advisory, treat per M001 policy; do not weaken to close. `workflow_dispatch` success does not satisfy this condition.
