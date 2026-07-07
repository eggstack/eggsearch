# AGENTS.md

## Project Overview

eggsearch is a lightweight MCP (Model Context Protocol) search/fetch server for AI agents. It queries upstream search providers, deduplicates results with reciprocal rank fusion, returns compact source cards, and fetches explicit HTTP(S) URLs on demand with bounded text extraction. The shipped generic default providers are DuckDuckGo, Startpage, and Yahoo; other built-in providers such as Brave, SearXNG, GitHub/GitLab/Gitea code/issues/releases, OSV, local workspace search, security advisory databases (GitHub Advisory, NVD, CISA KEV, RustSec), package registries (crates.io, PyPI, npm, Go Proxy, Maven Central, NuGet, RubyGems, Packagist), scholarly search (OpenAlex, Crossref, Semantic Scholar), and Sourcegraph code search are available when configured. Transport is MCP over stdio.

## Build & Verification

All commands from project root. **Run `make check` to replicate the full CI suite locally.**

```bash
# Full CI gate (fmt + clippy + tests + schema-corpus)
make check

# Individual targets
cargo fmt --check            # format check (CI fails on this)
cargo clippy --all-targets --all-features -- -D warnings  # zero warnings required
cargo test --all-features    # all tests
cargo test --no-default-features  # no-default compilation + tests
cargo build --release        # release build
cargo publish --dry-run      # pre-publish check
```

**Critical: Integration/corpus tests require `--features mock`.** Running `cargo test` without features misses most integration tests. The CI runs tests across 4 feature combos: `--all-features`, `--no-default-features`, `--features mock`, `--features pdf`.

## Project Structure

Single library + binary crate (not a workspace). Submodules under `src/`:

```
src/
  main.rs          # binary entry point (clap, tokio main)
  lib.rs           # library root, re-exports core/meta/fetch/mcp
  config.rs        # CLI config loader
  commands/        # subcommands: doctor, search, providers, mcp, fetch
  core/            # types, config, error, query, sanitize, identity, warning
  meta/            # MetadataSearchAdapter + vendored engines
  fetch/           # HTTP fetch client, HTML rendering, extraction, span selection
  mcp/             # MCP server (rmcp), tool definitions, server state
tests/             # integration, corpus, and contract tests
docs/
  architecture/    # codegg-contract.md — stable contract, IDs, warnings, trust model
  config.md         # config defaults, provider enablement, provider_status semantics
  safety.md        # trust model, fetch safety, metadata_only behavior
  agent-workflows.md # recommended tool call sequences and recipe catalog
  tool-matrix.md   # compact tool reference table
  release.md       # authoritative release process and pre-release command sequence
  release-checklist.md # short operational checklist (links to release.md)
.skills/           # agent skill files for development, MCP, release, architecture
plans/             # roadmap and phase documentation (historical)
```

Read `src/lib.rs` for the module map, then explore submodules as needed.

## CI Pipeline

| Job | What it runs |
|-----|-------------|
| **check** | `cargo check` × 4 feature combos |
| **test** | `cargo test` × 4 feature combos |
| **clippy** | `cargo clippy --all-targets --all-features -- -D warnings` |
| **schema-corpus** | 6 regression test binaries: `schema_identity_registry`, `fetch_safety`, `security_applicability_corpus`, `research_evidence_corpus`, `recipes_next_actions`, `evidence_bundle_handoff` |
| **docs-contract** | 3 documentation contract tests: `docs_config_snippets`, `docs_provider_inventory`, `docs_tool_names` |
| **fmt** | `cargo fmt --check` |
| **release-build** | `cargo build --release` |
| **publish-check** | `cargo publish --dry-run` |
| **docs** | `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` |

## Feature Flags

| Flag | Purpose |
|------|---------|
| `mock` | Test-only mock engine harness (`src/meta/mock.rs`) — **required for integration/corpus tests** |
| `pdf` | PDF text extraction via `lopdf` |
| `live-smoke` | Live network smoke tests (implies `mock`); ignored by default |

Tests MUST NOT require network access. Run live smoke tests via: `cargo test --features live-smoke --test corpus_runner -- --ignored`.

## Testing

### Test file locations

| Location | Feature gate | Purpose |
|----------|-------------|---------|
| `src/*/mod.rs` `#[cfg(test)]` | Varies | Unit tests for internal logic |
| `tests/integration.rs` | `mock` | MCP tool contracts, error handling, provider behavior |
| `tests/corpus_runner.rs` | `mock` | Multi-step workflow regression |
| `tests/schema_identity_registry.rs` | None | Schema + deterministic ID fixtures |
| `tests/fetch_safety.rs` | None | Render/sanitization safety |
| `tests/security_applicability_corpus.rs` | `mock` | Security applicability pipeline |
| `tests/security_applicability_regression.rs` | None | Range evaluation boundary regressions |
| `tests/security_applicability_phase8.rs` | None | Defensive output verification |
| `tests/research_evidence_corpus.rs` | `mock` | Research evidence regression |
| `tests/recipes_next_actions.rs` | `mock` | Workflow hint generation |
| `tests/evidence_bundle_handoff.rs` | None | Evidence bundle packaging |
| `tests/docs_config_snippets.rs` | None | TOML snippet validation against AppConfig |
| `tests/docs_provider_inventory.rs` | None | Provider ID validation against KNOWN_PROVIDER_IDS |
| `tests/docs_tool_names.rs` | None | Tool name validation against MCP tools |
| `tests/config_validation.rs` | None | Config deserialization, validation, and provider resolution |

### Running specific suites

```bash
cargo test --features mock --test integration              # integration only
cargo test --features mock --test corpus_runner            # corpus regression
cargo test --all-features --test security_applicability_regression --test security_applicability_phase8  # standalone
make schema-corpus                                         # all contract tests
make docs-tests                                            # documentation contract tests
cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names
```

### Adding tests

- **New file** when testing a distinct subsystem or targeting a specific bug class
- **Extend `integration.rs`** for MCP tool input validation, provider failures, tool response shape
- **Extend `corpus_runner.rs`** for multi-step workflows
- **Unit tests** at bottom of source file for private functions
- Always run `cargo clippy --all-targets --all-features -- -D warnings` after adding

## Code Conventions

- **No comments** unless explicitly requested
- **Formatter:** `cargo fmt` (standard rustfmt). CI checks `cargo fmt --check`.
- **Linter:** `cargo clippy --all-targets --all-features -- -D warnings` — zero warnings.
- **Error handling:** `core` defines `CoreError`/`CoreResult<T>` via `thiserror`. Adapter returns `WebSearchResponse` (never errors; partial failures are soft). MCP tools return `Result<serde_json::Value, ToolError>`.
- **Deterministic IDs:** SourceCard IDs, suggested fetches, and grouping use content-derived FNV-1a hashes (`src/core/identity.rs`). Never use random IDs for stable output types.
- **Sanitization:** All untrusted text flows through `src/core/sanitize.rs` (3 tiers: control-char strip, framing, injection scan). Production defaults `sanitize_output = true`; tests default to `false`.

## Key Architecture

- **Single crate:** Library + binary in one package. `src/lib.rs` re-exports `core`, `fetch`, `mcp`, `meta`.
- **Adapter pattern:** `MetadataSearchAdapter` wraps all search engines, handles RRF aggregation, sanitization, and provider health. MCP tools call the adapter, never engines directly.
- **Provider model:** `ProviderKind` enum (`HtmlScrape`, `JsonApi`, `ApiKey`, `Local`). Capability flags are conservative — HTML scrapers report `ProviderCapabilities::none()`.
- **Profiles:** `SearchProfile` (`generic`, `coding`, `security`, `research`) influence provider selection. Profiles are advisory; unavailable providers are skipped with warnings, not errors.
- **Config:** `$XDG_CONFIG_HOME/eggsearch/config.toml`. Root type is `AppConfig` with `SearchSection`, `FetchSection`, and `LocalConfig`.
- **Transport:** MCP over stdio only. Server instructions are in `EGGSEARCH_INSTRUCTIONS` constant in `mcp/server.rs`.

## MCP Tools (10 total)

`web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, `build_evidence_bundle`.

Tools are defined in `src/mcp/tools.rs`. The MCP server uses `rmcp` crate with `tool_router` proc macros.

## Publishing

```bash
make publish-check  # runs cargo publish --dry-run
```

Pre-publish: clippy clean, tests pass, fmt clean, version bumped in Cargo.toml, CHANGELOG.md updated.

The authoritative pre-release command sequence, required CI checks, and
live-smoke policy live in [`docs/release.md`](docs/release.md). The short
operational checklist lives in [`docs/release-checklist.md`](docs/release-checklist.md).
The `Makefile`, `.github/workflows/ci.yml`, and `docs/release.md` are kept in
sync; if you change a flag or command in one, update the others in the same
commit.

## Pitfalls

- **Forgetting `--features mock`** — integration/corpus tests won't compile without it
- **Adding random UUIDs** to stable output types — use FNV-1a hashes via `src/core/identity.rs`
- **Bypassing sanitization** — all untrusted text must flow through `sanitize.rs` or `sanitize_field()`
- **Hardcoding provider lists** — use `resolve_providers()` which validates enabled/known status
- **Changing deterministic IDs** — breaks regression corpus tests and cross-tool deduplication
- **Missing `cargo fmt`** — CI will fail on `cargo fmt --check`


