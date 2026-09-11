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
- Tool/probe inventories are code-derived (`tests/docs_tool_names.rs` from `src/mcp/server.rs`, `tests/docs_provider_inventory.rs` from `KNOWN_PROVIDER_IDS`): never invent tool-like or provider names in docs; keep prose in agreement with the code.

## Where things go

Guard-enforced by `tests/static_guards.rs` (fail-closed; see `architecture/maintenance.md` for the ownership table):

- MCP tools call the `MetadataSearchAdapter`, never engines directly. New tools: dedicated module under `src/mcp/tools/` (shared validation in `common.rs`, goal/workflow resolution in `canonical.rs`), register in `src/mcp/server.rs` — exactly 10 tools unless tool-matrix, docs contract tests, and CodeGG docs move in the same change.
- New engines implement `SearchEngine::search(&EngineSearchRequest)`; unsupported capabilities stay explicit (dispatch emits capability-skip attempts, never silent omissions). New providers also declare 24-flag `ProviderCapabilities`, add the ID to `KNOWN_PROVIDER_IDS`, document native-vs-local enforcement in `docs/provider-setup.md`, extend `tests/provider_capability_contract.rs`. Native enforcement: `brave_api` natively enforces safe-search, freshness/date-range, language, region, news; `exa` natively enforces freshness/date-range, domain filters, result timestamps; `tavily` natively enforces safe-search, freshness/date-range, language, region, domain filters, news. Domain filters are natively enforced only by providers advertising `supports_domain_filters` (currently `exa`, `tavily`); all other domain filtering is local approximation.
- Domain workflows (`repo`/`research`/`security`) share `WorkflowExecution`/`RetrievalAttemptSet`/`FetchCandidateBuilder` primitives but keep typed planners/grouping/builders — do not flatten into one generic workflow.
- Ordinary files must stay under 1,600 lines / 80 KB; larger modules carry explicit ratchet ceilings and next-slice notes in `architecture/maintenance.md`.

Conventions (kept in sync by discipline, not guards; see `architecture/maintenance.md`):

- Keep in sync: `packaging/release-targets.txt` + `packaging/release-inputs.txt` + release workflow + installers + updater + install docs (`make packaging-check`); `docs/test-inventory.md` + `architecture/testing.md` + `skills/eggsearch-dev/SKILL.md` when adding/renaming suites. `CHANGELOG.md` entries are append-only history.
- New tests extend behavioral suites (`mcp_tools`, `web_search`/`web_fetch` integration, `provider_routing`, `provider_probe_conformance`, `repo`/`research`/`security` workflow, `evidence_contract`); packaging behavior belongs under `packaging/test-install.sh` and `packaging/test-install.ps1`; multi-step regressions go in `corpus_runner.rs`; pure functions get `proptest` files; provider failures go in `dispatch_fault_injection.rs`. Historical phase-suite names are retired; use behavioral suite names.

## Architecture index

Contributor deep dives live in `architecture/`; start at `overview.md`, then jump by task:

| Task | Read first |
|------|-----------|
| Add/change an MCP tool or response shape | `mcp.md`, `codegg-contract.md`, `evidence-workflow.md` |
| Add/change a provider or engine | `engines.md`, `meta.md`, `config.md` |
| Touch search orchestration, dispatch, RRF, grouping | `meta.md`, `evidence-workflow.md`, `research.md`, `security.md` |
| Touch fetch, cache, browser, or safety bounds | `fetch.md`, `hardening.md` |
| Touch config, CLI, integrations, service, update | `config.md`, `commands.md`, `integrations.md`, `startup.md`, `packaging.md` |
| Touch local workspace or repo map | `local-workspace.md`, `core.md` |
| Touch tests, fuzz, or benchmarks | `testing.md`, `hardening.md`, `build.md` |
| Extension rules, ownership, hygiene | `maintenance.md` |

## Skills

Canonical sources in `skills/` (mirrored via `.opencode/skills/` and `.agents/skills/`): `eggsearch-architecture` (crate layout, provider model, adapter), `eggsearch-dev` (commands, suites, pitfalls), `eggsearch-mcp` (tool selection, workflows, evidence), `eggsearch-release` (release process with `docs/release.md`). Edit `skills/` only; never edit the mirror symlinks directly.

## Plans

`plans/` is a closed historical record: all 15 phases are implemented (see `plans/registry.md`). Do not open new work there; plan new work from the architecture docs above and the extension rules in `architecture/maintenance.md`.
