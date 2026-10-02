# Repository Hardening M008 — Windows Startup and Cross-Target Release Portability Corrective

Status: closed

Recommendation: closed.

Implementation candidate: `44f38abc765cf7fc2edebc37ab0ac1bf3949cc42`
Closure candidate: `44f38abc765cf7fc2edebc37ab0ac1bf3949cc42` (code-qualified candidate; closure records commit is docs/plans-only and does not change production code)

Original milestone and plan under correction:

- M008: `plans/implementation/repository-hardening/008-windows-startup-cross-target-portability-corrective.md`
- Planning baseline: `a5f1b0dcd89d90d8ff4fb3bc264bdfcace7acb19`
- Prior ownership: M004 `plans/closure/repository-hardening/004-status.md`, M007 `plans/closure/repository-hardening/007-status.md`

Historical M004/M007 and CodeGG M003 closure records remain unchanged. This record is the M008 control point.

## Root cause

Commit `d16a6c6adb04a5b6e2ccb8fec68987e80a452875` correctly split `process_matches` and process-token lookup by OS but left `process_record_contents()` calling `process_start_token(std::process::id())` unconditionally while only Linux and macOS implementations existed. Both Windows release targets therefore failed with `error[E0425]: cannot find function process_start_token in this scope` at `src/startup.rs:1076`. Routine `CI` runs only on Ubuntu, and the seven-target `egress-feature-qualify.yml` push/PR path filters did not include `src/startup.rs`, so Linux-green CI coexisted with a known Windows compile failure until an unrelated change triggered the seven-target matrix (run `36979619135`).

## Before/after cfg ownership

Before:

- `process_start_token`: `#[cfg(target_os = "linux")]` and `#[cfg(target_os = "macos")]` only; no unsupported-platform owner.
- `process_record_contents()`: unconditional call to `process_start_token`, uncompilable on Windows.
- `process_matches`: explicit Linux, macOS, and `not(any(linux, macos)) -> false` branches (already total).
- `platform_info()`: `#[cfg(windows)]`, `macos`, `linux`, `all(unix, not macos, not linux)`, plus bare `#[cfg(not(unix))]` fallback that also compiles on Windows after the Windows return.
- `stop_owned_process`: `#[cfg(unix)]` terminate and `#[cfg(windows)]` unsupported branches only.

After (`44f38ab`):

- `process_start_token`: Linux `/proc` start-time token, macOS bounded `ps/lstart` token, plus `#[cfg(not(any(target_os = "linux", target_os = "macos")))] fn process_start_token(_pid: u32) -> Option<String> { None }`. Unsupported platforms never fabricate an identity token.
- `process_record_contents()` unchanged at the call site; now compiles everywhere because the seam is total.
- `process_matches` unchanged: Linux and macOS require non-empty token plus executable plus token equality; unsupported returns `false`, so an empty token can never authorize ownership.
- `platform_info()` fallback narrowed to `#[cfg(not(any(unix, windows)))]`, making all five branches mutually exclusive.
- `stop_owned_process` keeps Unix terminate and Windows unsupported behavior and adds an explicit `not(any(unix, windows))` unsupported branch returning `Unsupported`, so the function is total beyond the seven supported targets.
- No PID-file format change; no new process architecture; cron/process control remains explicitly unsupported on Windows.

## Windows cfg warning disposition

All three directly exposed warnings in the bounded pass are resolved:

1. `src/meta/safe_open.rs` `CString` import is now `#[cfg(unix)]`; Unix-only FFI ownership no longer emits Windows unused-import diagnostics.
2. `platform_info()` cfg branches are mutually exclusive; Windows no longer compiles the generic non-Unix fallback.
3. `src/update.rs::set_executable` has an explicit `#[cfg(not(unix))] { let _ = path; }` branch; the non-Unix argument is intentional, not accidental.

No repository-wide warning cleanup was performed. `launchd_domain` retains its legitimate `#[cfg(not(unix))]` Unix/non-Unix pair, and `safe_open` retains its legitimate `#[cfg(not(unix))]` contained-open fallback; the static guard scopes the Windows-overlap prohibition to `platform_info()`.

## Verification

Focused tests on the exact candidate:

- `cargo test --locked --all-features --test bounded_command`: 31 passed.
- `cargo test --locked --all-features --test static_guards`: 58 passed, including new `windows_startup_portability_stays_total`.
- `cargo test --locked --all-features --lib startup::tests`: 15 passed on the macOS host, including `platform_info_matches_compile_target`, `pid_record_empty_token_never_matches`, `pid_record_mismatched_token_never_matches`, and `macos_process_token_preserves_match_semantics`; Linux `linux_process_token_is_non_empty_and_round_trips` compiles for Linux CI and the unsupported `unsupported_process_token_is_explicitly_none` compiles for non-Linux/macOS targets.
- `cargo test --locked --all-features --test egress_qualify_contract`: 7 passed.
- `make check`: pass (fmt + clippy `-D warnings` + no-default check + all-features tests + hygiene + dependency-policy + packaging-check).
- `make docs-check`: pass.
- `make release-check`: check + docs-check + release-build + `cargo publish --dry-run --locked` pass on the clean candidate (dry-run aborts as designed; no dirty-tree error).
- `bash packaging/check-egress-qualify-contract.sh`: agrees with `packaging/release-targets.txt` (seven targets).
- `python3 packaging/check-workflow-pins.py`: ok (4 workflow files).
- `python3 packaging/check-planning-consistency.py`: ok.

Regression evidence that would have caught the defect:

1. Missing target implementation: `windows_startup_portability_stays_total` pins all three `process_start_token` owners and fails if the unsupported owner disappears; ordinary CI `windows-portability` compiles the exact failure.
2. Windows cfg overlap: the same guard pins the five mutually exclusive `platform_info` branches and rejects a bare `not(unix)` fallback inside `platform_info()`.
3. Unix-only import: the guard requires `#[cfg(unix)]` on the `CString` import.
4. Non-Unix helper: the guard requires the explicit `not(unix)` plus `let _ = path;` branch.
5. Release-target proof: both Windows targets are green below.

## CI evidence on the exact candidate

Candidate SHA for all runs: `44f38abc765cf7fc2edebc37ab0ac1bf3949cc42`.

Ordinary `CI` push run `37036453017` (event `push`): success.

- `ci`: success (Ubuntu `make ci`).
- `windows portability (x86_64-pc-windows-msvc all-features check)`: success on `windows-latest` via `cargo check --locked --all-features --target x86_64-pc-windows-msvc`.

Seven-target egress qualification, manually dispatched run `37036467646` (event `workflow_dispatch`, ref `44f38ab`): success.

- Preflight: success.
- Linux `x86_64-unknown-linux-gnu`: success.
- Linux `aarch64-unknown-linux-gnu`: success.
- Linux `armv7-unknown-linux-gnueabihf`: success.
- macOS `x86_64-apple-darwin`: success.
- macOS `aarch64-apple-darwin`: success.
- Windows `x86_64-pc-windows-msvc`: success.
- Windows `aarch64-pc-windows-msvc`: success.
- MSRV all-features check (1.89): success.

The push-triggered egress run `37036453007` on the same SHA is also success across all jobs, proving the extended path filters now invoke the seven-target gate for startup/process/platform changes.

Release-binaries qualification run `37036489273` (event `workflow_dispatch`, `mode=qualify`, ref `44f38ab`): success, non-publishing.

- Preflight (qualify): success.
- Linux `x86_64-unknown-linux-gnu`: success.
- Linux `aarch64-unknown-linux-gnu`: success.
- Linux armv7 qualification: success.
- macOS `x86_64-apple-darwin`: success.
- macOS `aarch64-apple-darwin`: success.
- Windows `x86_64-pc-windows-msvc`: success.
- Windows `aarch64-pc-windows-msvc`: success.
- Assemble qualify release output: success.
- Qualification-only artifact: `qualification-0.3.9-44f38abc765cf7fc2edebc37ab0ac1bf3949cc42-complete`, plus per-target `qualify-0.3.9-44f38abc765c-*` artifacts. No GitHub Release was created or mutated.

## Compatibility

No MCP, CLI, provider, CodeGG, browser, PDF, or egress behavior change. Ten stable tools and schemas unchanged. MSRV remains 1.89 with seven unchanged release targets. Linux PID-reuse protection and macOS process-identity semantics are intact; `process_matches` still requires a non-empty token. Unsupported Windows cron/process-control behavior remains an explicit `Unsupported` error, never silent success. PID-file serialization is unchanged on Linux/macOS; unsupported platforms write an explicit empty token that cannot authorize ownership. `src/process.rs` remains the shared bounded process/session owner; M007 `deny(unsafe_code)` inventory is untouched.

## Planning-state reconciliation and unblocked work

M008 is closed. The next tagged release, including CodeGG parity `v0.4.0`, is no longer blocked by a red Windows/release matrix; its remaining condition is the normal release qualification of its own eventual candidate plus M002 first-release provenance evidence.

No milestone is opportunistically promoted by this closure:

- Repository hardening M001 remains conditionally closed via M007 pending the first `schedule`-event dependency-security run.
- M002 remains conditionally closed pending first tagged-release attestation and immutable-release evidence.
- M003 and M005 remain closed; M004 remains closed through M007.
- M006 remains blocked on an upstream chromiumoxide release making HTTP discovery optional.
- M007 remains conditionally closed pending the same scheduled dependency-security run.
- CodeGG parity M001/M002 remain closed; M003 remains conditionally closed with its single condition now actionable: publication of tagged release `v0.4.0` may proceed to exact-candidate qualification and publication. This closure does not publish `v0.4.0` and does not change CodeGG parity semantics.

Registry, subsystem roadmap, sequencing overview, implementation plan status, and the CodeGG addendum handoff text are updated together in the closure commit to reflect M008 closed and the lifted publication gate.

## Unresolved findings

- High: none.
- Medium: none.
- Low: local macOS-to-Windows-MSVC `cargo check --target` still requires a Windows runner or MSVC C toolchain because `ring` C compilation cannot run under the macOS `cc` shim; this is a local-toolchain limitation, not a repository defect. Ordinary CI now covers the exact check on `windows-latest`, and the static guard pins totality without a Windows compiler.
