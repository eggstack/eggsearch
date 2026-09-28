# Repository Hardening M004 — Bounded Process and Unsafe Boundary

Status: closed

Recommendation: closed

Implementation commits: `cb4048180e29cdeaffb77f2cc86eaec2a8e33b7b`, `85322ac127033bdc4058177ea09cff67417308d0`

## Requirement evidence

| Requirement | Evidence | Result |
|---|---|---|
| One private captured-process owner | `src/process.rs` owns concurrent stdout/stderr drainage, typed outcomes, caps, deadlines, and process-group termination; local workspace re-exports compatibility types | pass |
| One documented Unix session-creation unsafe seam | `src/process.rs` alone configures `pre_exec(setsid)`; startup and inventory-cache code call the owner | pass |
| Bound startup captures and probes | crontab reads, startup checks, integration CLI commands, and browser version/path probes use explicit bounded runner policies | pass |
| Keep argument-vector execution | Call sites continue building `Command` arguments directly; no shell-command wrapper was added | pass |
| Guard process ownership | Static checks reject raw captured output in migrated production paths and duplicate session setup | pass |

## Verification

- `cargo test --locked --all-features --test bounded_command --test local_workspace_integration --test property_local_fs --test property_local_fs_extended`: included in all-features `make check`; pass.
- Startup and update tests: included in all-features `make check`; pass.
- `cargo test --locked --all-features --test static_guards`: 47 passed.
- `make check`, `make docs-check`: pass.
- `make release-check`: pass, including release build and publish dry-run.
- Exact-candidate GitHub CI passed: run [`36467672672`](https://github.com/eggstack/eggsearch/actions/runs/36467672672), including Windows and macOS compilation.
- Exact-candidate egress and process cross-target qualification passed on all targets, including Windows and macOS x86_64: run [`36469130010`](https://github.com/eggstack/eggsearch/actions/runs/36469130010).

No CLI or process API was added. Existing startup manager and local inventory
fallback semantics remain in their domain call sites; Windows process-tree
termination is bounded to one second.
