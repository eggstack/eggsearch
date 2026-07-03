# AGENTS.md

This file contains information for AI coding agents working on the eggsearch codebase.

## Project Overview

eggsearch is a lightweight MCP (Model Context Protocol) search/fetch server for AI agents. It queries upstream search providers (DuckDuckGo, Brave, Startpage, Yahoo, Mojeek), deduplicates results with reciprocal rank fusion, returns compact source cards, and also fetches one explicit HTTP(S) URL on demand with bounded text extraction. Transport is MCP over stdio.

## Build & Test Commands

All commands are run from the project root.

```bash
# Build (debug)
cargo build

# Build (release, optimized)
cargo build --release

# Run all tests (unit + integration)
cargo test --all-features

# Clippy (must pass before committing)
cargo clippy --all-features -- -D warnings

# Check compilation only
cargo check --all-features

# Format check (must pass in CI)
cargo fmt --check

# Format (auto-fix)
cargo fmt

# Dry-run publish check
cargo publish --dry-run

# Run tests without optional features
cargo test --no-default-features

# Run benchmarks
cargo bench

# Check no-default-features compilation
cargo check --no-default-features
```

## Project Structure

The eggsearch crate is a single library + binary (not a workspace). Submodules live under `src/`:

```
eggsearch/
  src/
    main.rs              # binary entry point (clap, tokio main)
    lib.rs               # library root, re-exports core/meta/fetch/mcp
    config.rs            # CLI config loader (thin wrapper around core::config)
    commands/            # subcommands: doctor, search, providers, mcp, fetch
    core/                # core types and logic, batch fetch types, code evidence metadata
    meta/                # MetadataSearchAdapter + vendored engines
    fetch/               # HTTP fetch client, HTML structural rendering, and extraction
    mcp/                 # MCP server (rmcp)
  tests/integration.rs   # end-to-end tool tests with mock engines
```

For a full module map, read `src/lib.rs` to see re-exports, then explore submodules as needed.

## CI Pipeline

CI runs 6 checks on every push/PR:

| Job | What it runs |
|-----|-------------|
| **check** | `cargo check` × 4 feature combos: `--all-features`, `--no-default-features`, `--features mock`, `--features pdf` |
| **test** | `cargo test` × 4 feature combos: `--all-features`, `--no-default-features`, `--features mock`, `--features pdf` |
| **clippy** | `cargo clippy --all-features -- -D warnings` |
| **schema-corpus** | 6 regression test binaries: `schema_identity_registry`, `fetch_safety`, `security_applicability_corpus`, `research_evidence_corpus`, `recipes_next_actions`, `evidence_bundle_handoff` |
| **fmt** | `cargo fmt --check` |
| **release-build** | `cargo build --release` |

**All 6 must pass before merging.** Run `make check` locally to replicate the full CI suite.

## Feature Flags

| Flag | Purpose | When to use |
|------|---------|-------------|
| `mock` | Enables test-only mock engine harness (`src/meta/mock.rs`) | Required for integration/corpus tests |
| `pdf` | Enables PDF text extraction via `lopdf` crate; MSRV 1.85 | When testing PDF fetch behavior |
| `live-smoke` | Enables live network smoke tests (implies `mock`) | Manual end-to-end validation |

Tests MUST NOT require network access — all use mock engines. Live smoke tests are ignored by default and run via `cargo test --features live-smoke --test corpus_runner -- --ignored`.

## Testing Patterns

- Unit tests live in `#[cfg(test)] mod tests` at the bottom of each source file
- Integration tests live in `tests/integration.rs`
- Mock engines are in `src/meta/mock.rs` (feature-gated behind `mock`)
- The `MockEngine` struct supports success, failure, and hang (timeout) scenarios
- Regression corpus suite: `tests/corpus_runner.rs` with JSON scenario files under `tests/corpus/`
- Total test count: ~3053 passed, 5 ignored (all-features)

### Test File Organization

| Location | Purpose | Feature gate | When to use |
|----------|---------|--------------|-------------|
| `src/*/mod.rs` `#[cfg(test)] mod tests` | Unit tests for internal logic | Varies | Testing private functions, type conversions, edge cases |
| `tests/integration.rs` | MCP tool surface tests | `mock` | Testing tool input/output contracts, error handling, provider behavior |
| `tests/corpus_runner.rs` | Whole-workflow regression | `mock` | Quality regression for multi-step scenarios (repo_search, security_search, research_search) |
| `tests/schema_identity_registry.rs` | Schema + golden identity + warning codes | None | Phase 13 contract: MCP arg deserialization, deterministic ID fixtures, enum stability |
| `tests/fetch_safety.rs` | Render/sanitization safety | None | HTML fixture tests, span selection, local path validation |
| `tests/security_applicability_corpus.rs` | Security applicability pipeline | `mock` | Version/range evaluation, dependency relations, remediation, KEV |
| `tests/security_applicability_regression.rs` | Range evaluation boundary bugs | None | Directly exercises `assess_version_applicability` for inverted-comparison regressions |
| `tests/security_applicability_phase8.rs` | Phase 8 defensive output | None | Applicability status, remediation categories, no exploit instructions |
| `tests/research_evidence_corpus.rs` | Research evidence regression | `mock` | Claims, conflicts, quality signals, workflow scaffolding |
| `tests/recipes_next_actions.rs` | Task recipe regression | `mock` | Workflow hint generation, recipe detail, next-action suggestions |
| `tests/evidence_bundle_handoff.rs` | Evidence bundle packaging | None | Bundle ID computation, gap detection, multi-agent handoff shape |

### When to add a new test file vs extend existing ones

- **New standalone test file** when: testing a distinct subsystem (e.g., security applicability, evidence bundles) or when regression tests target a specific bug class (e.g., `security_applicability_regression.rs` for the inverted `>=` fix).
- **Extend `integration.rs`** when: testing MCP tool input validation, provider failure modes, or tool-level response shape.
- **Extend `corpus_runner.rs`** when: testing multi-step workflows that exercise multiple tools together (e.g., repo_search → repo_fetch pipeline).
- **Add unit tests** when: testing internal functions that don't cross module boundaries.

### Running specific test suites

```bash
# All tests
cargo test --all-features

# Just integration tests
cargo test --features mock --test integration

# Just corpus regression tests
cargo test --features mock --test corpus_runner

# Phase 13 contract tests (schema, fetch safety, security, research, recipes, evidence)
cargo test --features mock --test schema_identity_registry --test fetch_safety --test security_applicability_corpus --test research_evidence_corpus --test recipes_next_actions --test evidence_bundle_handoff

# Standalone security applicability tests (no mock feature needed)
cargo test --all-features --test security_applicability_regression --test security_applicability_phase8

# Or use the Makefile
make test-all
make schema-corpus
```

### Adding a new test

1. Unit tests: add `#[cfg(test)] mod tests` at bottom of the source file
2. Integration tests: add to `tests/integration.rs` (requires `#[cfg(feature = "mock")]` for mock engine tests)
3. Corpus tests: add JSON scenario to `tests/corpus/` and test in `tests/corpus_runner.rs`
4. Standalone regression tests: create `tests/<name>.rs` with a doc comment explaining what it guards against
5. Contract tests: add to the appropriate `tests/<name>.rs` file under the relevant workstream
6. Run `cargo clippy --all-features -- -D warnings` to check

## Code Style & Conventions

- **Formatter:** `cargo fmt` (standard rustfmt). CI checks `cargo fmt --check`.
- **Linter:** `cargo clippy --all-features -- -D warnings` — zero warnings required.
- **No comments:** Do not add code comments unless explicitly requested.
- **Error handling:** `core` defines `CoreError`/`CoreResult<T>` via `thiserror`. Adapter returns `WebSearchResponse` (never errors; partial failures are soft). MCP tools return `Result<serde_json::Value, String>`.
- **Feature gating:** Tests use `#[cfg(feature = "mock")]`. The `mock` feature is NOT default.
- **Determinism:** SourceCard IDs, suggested fetches, and grouping are all deterministic (content-derived FNV-1a hashes). Never introduce random IDs for stable output types.
- **Sanitization:** All untrusted text flows through `src/core/sanitize.rs` (3 tiers: control-char strip, framing, injection scan). Production defaults `sanitize_output = true`; tests default to `false` for assertion stability.

## Key Architecture Decisions

- **Single crate:** Library + binary in one package. `src/lib.rs` re-exports `core`, `fetch`, `mcp`, `meta`.
- **Adapter pattern:** `MetadataSearchAdapter` wraps all search engines, handles RRF aggregation, sanitization, and provider health. MCP tools call the adapter, never engines directly.
- **Provider model:** `ProviderKind` enum (`HtmlScrape`, `JsonApi`, `ApiKey`, `Local`). Capability flags are conservative — HTML scrapers report `ProviderCapabilities::none()`.
- **Profiles:** `SearchProfile` (`generic`, `coding`, `security`, `research`) influence provider selection. Profiles are advisory; unavailable providers are skipped with warnings, not errors.
- **Deterministic identity:** Every output type carries a `stable_id` (FNV-1a hash) alongside the random UUID `id`. IDs are versioned (`eggsearch-id-v1`) and use entity-specific prefixes (`src_`, `fetch_`, `loc_`, etc.).
- **Config:** `$XDG_CONFIG_HOME/eggsearch/config.toml`. Root type is `AppConfig` with `SearchSection`, `FetchSection`, and `LocalConfig`.
- **Transport:** MCP over stdio only (no HTTP/SSE). Server instructions are in `EGGSEARCH_INSTRUCTIONS` constant in `mcp/server.rs`.

## MCP Tools (10 total)

| Tool | Purpose |
|------|---------|
| `web_search` | Live metasearch with optional intent/freshness hints |
| `web_fetch` | Bounded URL fetch with structured document extraction |
| `batch_fetch` | Bounded batch fetch over explicit URLs/locators |
| `provider_status` | Diagnostic/host-facing capability discovery |
| `repo_search` | Structured repository evidence discovery with grouped bundles |
| `repo_fetch` | Fetch repository files by structured locator with line ranges |
| `repo_map` | Bounded repository structure discovery with important-file classification |
| `security_search` | Security-oriented retrieval with normalized vulnerability metadata |
| `research_search` | Research-oriented multi-source evidence discovery |
| `build_evidence_bundle` | Package evidence for multi-agent handoff (no search/fetch) |

Tools are defined in `src/mcp/tools.rs`. The MCP server uses `rmcp` crate with `tool_router` proc macros.

## Publishing

Before publishing to crates.io:

1. `cargo clippy --all-features -- -D warnings` is clean
2. `cargo test --all-features` passes
3. `cargo fmt --check` passes
4. `cargo publish --dry-run` succeeds
5. Version in `Cargo.toml` is bumped
6. `CHANGELOG.md` is updated

```bash
make publish-check  # runs the full pre-publish validation
```

## Common Pitfalls

- **Forgetting `--features mock`** when running integration tests — they won't compile without it
- **Running `cargo test` without features** misses most integration tests (they're behind `#[cfg(feature = "mock")]`)
- **Adding random UUIDs** to stable output types — use deterministic FNV-1a hashes via `src/core/identity.rs`
- **Bypassing sanitization** — all untrusted text must flow through `sanitize.rs` or `sanitize_field()`
- **Hardcoding provider lists** — use `resolve_providers()` which validates enabled/known status
- **Changing deterministic IDs** — this breaks regression corpus tests and cross-tool deduplication
- **Adding comments** — the project convention is no comments unless requested
- **Missing `cargo fmt`** — CI will fail on `cargo fmt --check`
