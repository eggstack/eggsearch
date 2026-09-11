# AGENTS.md

eggsearch is a lightweight MCP search/fetch server for AI agents (live metasearch with RRF dedup, bounded fetch, deterministic evidence bundles). Transports: client-owned `mcp stdio` or explicit loopback-only `mcp serve`.

Single library + binary crate, not a workspace. Stable contract is MCP tools (10) plus CLI; the Rust module tree is an implementation detail with no semver library guarantee (see `src/lib.rs`). Dependency flow: `core <- meta <- mcp <- commands`; `fetch` is independent of `meta`. Start with `src/lib.rs` (module map) and `architecture/overview.md` (component index); operator docs live in `docs/`.

## Build & verification

From project root. CI pins Rust 1.88 (`rust-version` in `Cargo.toml`); edition 2021.

```bash
make check  # canonical gate: fmt + clippy + no-default check + all-features tests + hygiene + packaging-check
make release-check  # check + docs + release build + publish dry-run

cargo clippy --locked --all-targets --all-features -- -D warnings  # zero warnings required
cargo check --locked --no-default-features
cargo test --locked --all-features
make hygiene packaging-check bench-check  # hygiene/contract/bench-compile; fuzz: make fuzz-smoke
```

Feature flags: `mock` (test-only engine harness — **required for integration/corpus tests**; plain `cargo test` misses most of them), `pdf`, `browser`, `live-smoke` (implies `mock`, ignored by default, network). Tests must pass keyless: CI blanks all credential env vars, so missing credentials are provider-scoped skips, never global failures. Tests must not require network.

```bash
cargo test --locked --features mock --test web_search_integration  # single suite
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test tool_surface_live -- --ignored  # opt-in, needs EGGSEARCH_EVAL_MODEL
```

## Conventions

- **No comments** unless explicitly requested. `cargo fmt` required (CI fails on `cargo fmt --check`).
- **Stable IDs are content-derived FNV-1a** (`src/core/identity.rs`). Never random UUIDs; never change ID semantics (breaks corpus regression + cross-tool dedup).
- **Sanitize all untrusted text** through `src/core/sanitize.rs` / `sanitize_field()`.
- **Bound all untrusted I/O**: forge responses only via `read_bounded_body()` (never bare `.text()`/`.bytes()`); bounded git execution via `run_bounded_command()` (process-group kill on timeout/cap breach).
- `commit_sha` comes from `resolved_ref`, not the entry object SHA.
- `CacheScope::Profile` uses the opaque profile ID, never the display name. Invalid explicit browser path is `ExplicitPathInvalid` — do not fall back to auto-discovery.
- `integrate` prints by default; mutate only with `--apply` (atomic, backed up, `eggsearch` entry only). Never register `target/debug` binaries — require an installed executable or explicit `--executable`.

## Where things go (enforced by `tests/static_guards.rs`)

- MCP tools call the `MetadataSearchAdapter`, never engines directly. New tools: dedicated module under `src/mcp/tools/` (shared validation in `common.rs`, goal/workflow resolution in `canonical.rs`), register in `src/mcp/server.rs` — exactly 10 tools unless tool-matrix, docs contract tests, and CodeGG docs move in the same change.
- New engines implement `SearchEngine::search(&EngineSearchRequest)`; unsupported capabilities stay explicit (dispatch emits capability-skip attempts, never silent omissions). New providers also declare 24-flag `ProviderCapabilities`, add the ID to `KNOWN_PROVIDER_IDS`, document native-vs-local enforcement in `docs/provider-setup.md`, extend `tests/provider_capability_contract.rs`.
- Domain workflows (`repo`/`research`/`security`) share `WorkflowExecution`/`RetrievalAttemptSet`/`FetchCandidateBuilder` primitives but keep typed planners/grouping/builders — do not flatten into one generic workflow.
- Ordinary files must stay under 1,600 lines / 80 KB (named exceptions in `architecture/maintenance.md` only).
- New tests extend behavioral suites (`mcp_tools`, `web_search`/`web_fetch` integration, `provider_routing`, `provider_probe_conformance`, `repo`/`research`/`security` workflow, `evidence_contract`); multi-step regressions go in `corpus_runner.rs`; pure functions get `proptest` files; provider failures go in `dispatch_fault_injection.rs`. Never add `phase<N>_*` suite names.
- Keep in sync or guards fail: `packaging/release-targets.txt` + release workflow + installers + install docs (`make packaging-check`); `docs/test-inventory.md` + `architecture/testing.md` + `skills/eggsearch-dev/SKILL.md` when adding/renaming suites. `CHANGELOG.md` entries are append-only history.

## Skills

Canonical sources in `skills/` (symlinked to `.opencode/skills/`): `eggsearch-architecture` (crate layout, provider model, adapter), `eggsearch-dev` (commands, suites, pitfalls), `eggsearch-mcp` (tool selection, workflows, evidence), `eggsearch-release` (release process with `docs/release.md`).
