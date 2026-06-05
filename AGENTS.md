# AGENTS.md

This file contains information for AI coding agents working on the eggsearch codebase.

## Project Overview

eggsearch is a lightweight MCP (Model Context Protocol) metasearch server for AI agents. It queries upstream search providers (DuckDuckGo, Brave, Startpage, Yahoo), deduplicates results with reciprocal rank fusion, and returns compact source cards via MCP over stdio.

## Build & Test Commands

All commands are run from `crates/eggsearch-cli/`.

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

# Dry-run publish check
cargo publish --dry-run
```

## Project Structure

The eggsearch crate is a single library + binary. Submodules live under `src/`:

```
crates/eggsearch-cli/
  src/
    main.rs              # binary entry point (clap, tokio main)
    lib.rs               # library root, re-exports core/meta/mcp
    config.rs            # CLI config loader
    commands/            # subcommands: doctor, search, providers, mcp
    core/                # SourceCard, AppConfig, error, query types
    meta/                # MetadataSearchAdapter + vendored engines
    mcp/                 # MCP server (rmcp): web_search + provider_status
  tests/integration.rs   # end-to-end tool tests with mock engines
```

## Key Conventions

### Feature Flags
- `mock` (opt-in): enables the test-only mock engine harness in `meta::mock`
- The previous `metasearch` feature is gone; the metasearch code is always compiled
- Integration tests use `#[cfg(feature = "mock")]` and are run via `cargo test --features mock`

### Error Handling
- `core` defines `CoreError` and `CoreResult<T>` using `thiserror`
- `meta` adapter returns `WebSearchResponse` (never errors; partial failures are soft)
- `mcp` tools return `Result<serde_json::Value, String>` for MCP error mapping

### Testing
- Unit tests live in `#[cfg(test)] mod tests` at the bottom of each source file
- Integration tests live in `crates/eggsearch-cli/tests/integration.rs`
- Mock engines are in `src/meta/mock.rs` (feature-gated behind `mock`)
- The `MockEngine` struct supports success, failure, and hang (timeout) scenarios
- Vendored engine tests (HTML parsing) are in `src/meta/engines/`
- Tests must not require network access — all use mock engines

### MCP Protocol
- Server uses `rmcp` crate with `tool_router` proc macros
- Tools: `web_search` (live metasearch) and `provider_status` (diagnostic)
- Transport: stdio only (no HTTP/SSE)
- Server instructions are in `EGGSEARCH_INSTRUCTIONS` constant in `mcp/server.rs`

### Configuration
- Config file: `$XDG_CONFIG_HOME/eggsearch/config.toml`
- `AppConfig` is the root type, contains `SearchSection`
- `Mode` enum: `Live` or `Off`
- `ServerState` holds `Arc<AppConfig>` + `Arc<MetadataSearchAdapter>`

### Source Cards
- `SourceCard` is the primary output type returned by `web_search`
- Each card has a UUID-based `id` (`src_<uuid>`) unique per response
- Trust level is always `external_untrusted` for live web results
- Deduplication happens via URL normalization in the vendored `aggregate_rrf()` function

## Vendored Search Engines

The HTML scraping engines in `src/meta/engines/` are vendored from
[`metadata-search-engine-rs`](https://crates.io/crates/metadata-search-engine-rs)
(original source: [MikeLuu99/searxng-rust](https://github.com/MikeLuu99/searxng-rust)).

The vendored code includes:
- `engines/duckduckgo.rs` — DuckDuckGo HTML scraper
- `engines/brave.rs` — Brave Search HTML scraper
- `engines/startpage.rs` — Startpage HTML scraper
- `engines/yahoo.rs` — Yahoo Search HTML scraper
- `engines/normalizer.rs` — URL normalization for deduplication
- `engines/models.rs` — `SearchResult`, `AggregatedResult`
- `engines/error.rs` — `EngineError` enum
- `engines/mod.rs` — `SearchEngine` trait, `build_http_client()`, engine construction

When updating engines, check the upstream repo for HTML selector changes.
The `scraper` crate is used for HTML parsing.

## Publishing to crates.io

eggsearch is published as a single crate. Before publishing:

- `cargo clippy --all-features -- -D warnings` is clean
- `cargo test --all-features` passes (114 tests)
- `cargo publish --dry-run` succeeds
- The version in `crates/eggsearch-cli/Cargo.toml` is bumped
- `CHANGELOG.md` is updated

The crates.io package includes the README, LICENSE files, and CHANGELOG via
the `include` field in `Cargo.toml`.
