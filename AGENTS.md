# AGENTS.md

This file contains information for AI coding agents working on the eggsearch codebase.

## Project Overview

eggsearch is a lightweight MCP (Model Context Protocol) metasearch server for AI agents. It queries upstream search providers (DuckDuckGo, Brave, Startpage, Yahoo), deduplicates results with reciprocal rank fusion, and returns compact source cards via MCP over stdio.

## Build & Test Commands

```bash
# Build (debug)
cargo build

# Build (release, optimized)
cargo build --release

# Run all tests (unit + integration)
cargo test --workspace --all-features

# Run tests for a specific crate
cargo test -p eggsearch-core
cargo test -p eggsearch-meta
cargo test -p eggsearch-mcp --all-features

# Clippy (must pass before committing)
cargo clippy --workspace --all-features

# Check compilation only
cargo check --all-features

# Dry-run publish check
cargo publish --dry-run -p eggsearch-core
```

## Workspace Structure

```
crates/
  eggsearch-core/     Core types, config, URL normalization, error types
  eggsearch-meta/     MetadataSearchAdapter with vendored search engines
  eggsearch-mcp/      MCP server (rmcp): web_search + provider_status tools
  eggsearch-cli/      CLI binary: doctor, search, providers, mcp stdio
```

## Key Conventions

### Feature Flags
- `metasearch` (default): enables the real search engine backend
- `mock`: enables mock engines for testing without network access
- All integration tests in `eggsearch-mcp/tests/` use `#[cfg(feature = "mock")]` for test-only mock engines

### Error Handling
- `eggsearch-core` defines `CoreError` and `CoreResult<T>` using `thiserror`
- `eggsearch-meta` adapter returns `WebSearchResponse` (never errors; partial failures are soft)
- `eggsearch-mcp` tools return `Result<serde_json::Value, String>` for MCP error mapping

### Testing
- Unit tests live in `#[cfg(test)] mod tests` at the bottom of each source file
- Integration tests live in `crates/eggsearch-mcp/tests/integration.rs`
- Mock engines are in `crates/eggsearch-meta/src/mock.rs` (feature-gated behind `mock`)
- The `MockEngine` struct supports success, failure, and hang (timeout) scenarios
- Vendored engine tests (HTML parsing) are in `crates/eggsearch-meta/src/engines/`
- Tests must not require network access — all use mock engines

### MCP Protocol
- Server uses `rmcp` crate with `tool_router` proc macros
- Tools: `web_search` (live metasearch) and `provider_status` (diagnostic)
- Transport: stdio only (no HTTP/SSE)
- Server instructions are in `EGGSEARCH_INSTRUCTIONS` constant in `server.rs`

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

The HTML scraping engines in `eggsearch-meta/src/engines/` are vendored from
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

Each sub-crate can be published independently. The publish order is:
1. `eggsearch-core` (no internal dependencies)
2. `eggsearch-meta` (depends on eggsearch-core)
3. `eggsearch-mcp` (depends on eggsearch-core + eggsearch-meta)
4. `eggsearch` (the CLI binary, depends on all three)

Before publishing, ensure:
- `cargo clippy --workspace --all-features` is clean
- `cargo test --workspace --all-features` passes
- Version numbers in workspace Cargo.toml are bumped
- CHANGELOG is updated (if one exists)
