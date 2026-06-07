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

# Dry-run publish check
cargo publish --dry-run
```

## Project Structure

The eggsearch crate is a single library + binary. Submodules live under `src/`:

```
eggsearch/
  src/
    main.rs              # binary entry point (clap, tokio main)
    lib.rs               # library root, re-exports core/meta/fetch/mcp
    config.rs            # CLI config loader
    commands/            # subcommands: doctor, search, providers, mcp, fetch
    core/                # SourceCard, AppConfig, error, query, fetch types
    meta/                # MetadataSearchAdapter + vendored engines
    fetch/               # bounded HTTP(S) URL fetch + HTML extraction
    mcp/                 # MCP server (rmcp): web_search, web_fetch, provider_status
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
- Integration tests live in `tests/integration.rs`
- Mock engines are in `src/meta/mock.rs` (feature-gated behind `mock`)
- The `MockEngine` struct supports success, failure, and hang (timeout) scenarios
- Vendored engine tests (HTML parsing) are in `src/meta/engines/`
- Tests must not require network access — all use mock engines

### MCP Protocol
- Server uses `rmcp` crate with `tool_router` proc macros
- Tools: `web_search` (live metasearch), `web_fetch` (bounded URL fetch), and `provider_status` (diagnostic)
- Transport: stdio only (no HTTP/SSE)
- Server instructions are in `EGGSEARCH_INSTRUCTIONS` constant in `mcp/server.rs`

### Configuration
- Config file: `$XDG_CONFIG_HOME/eggsearch/config.toml`
- `AppConfig` is the root type, contains `SearchSection`
- `FetchSection` is the `[fetch]` section: enables/disables `web_fetch` and configures fetch limits (timeout_ms, max_bytes, max_chars_default, max_chars_cap, redirect_limit, allow_private_network, allow_localhost, include_links_default, user_agent)
- `SearxngConfig` is the `[search].searxng` section: enables the optional `searxng` provider (`enabled`, `base_url`)
- `ApiProviderConfig` is the `[search.api.<id>]` section: API-key provider config (`enabled`, `api_key_env`, `base_url`)
- `Mode` enum: `Live` or `Off`
- `ServerState` holds `Arc<AppConfig>` + `Arc<MetadataSearchAdapter>`

### Provider Model
- `ProviderKind` enum: `HtmlScrape`, `JsonApi`, `ApiKey`
- `ProviderCapabilities` struct: 7 boolean flags for search option support
- `ProviderDescriptor` struct: full provider metadata (id, display_name, kind, enabled, default, requires_api_key, configured, capabilities)
- `built_in_provider_descriptor()` returns descriptors for all known providers
- `MetadataSearchAdapter::provider_status()` returns `Vec<ProviderDescriptor>`
- `resolve_providers()` validates explicit provider lists with distinct errors for disabled vs unknown providers
- API providers use env-var indirection for secrets (`api_key_env` field)

### Source Cards
- `SourceCard` is the primary output type returned by `web_search`
- Each card has a UUID-based `id` (`src_<uuid>`) unique per response
- Trust level is always `external_untrusted` for live web results
- Deduplication happens via URL normalization in the vendored `aggregate_rrf()` function
- `WebFetchResponse` is the output type returned by `web_fetch`; trust is always `external_untrusted` for live web content

### Prompt-injection Hardening
- Untrusted text from search and fetch flows through three tiers of
  defense, defined in `src/core/sanitize.rs`:
  1. **Tier 1** (always on): `strip_control_chars` removes NUL, CR,
     ASCII controls, bidi controls, and zero-width chars;
     `bound_text` clamps titles to 200 chars and snippets to 500.
  2. **Tier 2** (gated by `sanitize_output`): `frame` wraps the
     bounded text with `<<<EXTERNAL_UNTRUSTED field=... id=...>>>` /
     `<<<END>>>` delimiters.
  3. **Tier 3** (gated by `sanitize_output`): `scan_injection_markers`
     looks for an allowlisted set of prompt-injection patterns
     (`ignore_previous`, `disregard_all`, `system_colon`,
     `assistant_colon`, `im_start`, `im_end`, `chatml_tag`).
- The `TrustMarkers` struct is the canonical record of what was done
  to untrusted text in a call (`text_sanitized`, `text_truncated`,
  `text_framed`, `control_chars_removed`, `injection_hits`). It is
  per-card on `SourceCard`, per-response on `WebFetchResponse` and
  `WebSearchResponse`, and rolled up into a top-level `trust_markers`
  field on every MCP response.
- All untrusted text from upstream engines **must** flow through
  `convert_aggregated` (for search, in `src/meta/adapter.rs`) or the
  `sanitize_field` helper (for fetch, in `src/fetch/client.rs`). Future
  engines or output fields must respect this — never emit
  attacker-controlled text directly into a response without routing
  it through the same sanitization pipeline.
- `MetadataSearchAdapter::from_engines` defaults `sanitize_output`
  to `false`. This is intentional, to keep pre-sanitization
  integration-test assertions stable. Production code paths via
  `ServerState::build` use `AppConfig.search.sanitize_output`, which
  defaults to `true`. The `mock` feature exposes
  `MetadataSearchAdapter::from_engines_with_sanitize(engines, timeout,
  sanitize_output)` for tests that need to flip the flag explicitly.

## Vendored Search Engines

The HTML scraping engines in `src/meta/engines/` are vendored from
[`metadata-search-engine-rs`](https://crates.io/crates/metadata-search-engine-rs)
(original source: [MikeLuu99/searxng-rust](https://github.com/MikeLuu99/searxng-rust)).

The vendored code includes:
- `engines/duckduckgo.rs` — DuckDuckGo HTML scraper
- `engines/brave.rs` — Brave Search HTML scraper
- `engines/brave_api.rs` — Brave Search API provider (API-key, JSON)
- `engines/startpage.rs` — Startpage HTML scraper
- `engines/yahoo.rs` — Yahoo Search HTML scraper
- `engines/mojeek.rs` — Mojeek HTML scraper (added in 0.2.0)
- `engines/searxng.rs` — SearXNG JSON client for a self-hosted
  SearXNG instance (added in 0.2.0)
- `engines/normalizer.rs` — URL normalization for deduplication
- `engines/models.rs` — `SearchResult`, `AggregatedResult`
- `engines/error.rs` — `EngineError` enum
- `engines/mod.rs` — `SearchEngine` trait, `build_http_client()`, engine construction

When updating engines, check the upstream repo for HTML selector changes.
The `scraper` crate is used for HTML parsing.

The `searxng` provider is a JSON client, not an HTML scraper: it sends a
GET to `{base_url}/search?format=json` and deserializes the response
into `SearchResult` values. The base URL is operator-supplied via
`[search].searxng.base_url` and the provider is built only when
`[search].searxng.enabled = true`. This provider is the recommended
path for operators who want Qwant, Bing, or any other upstream that
SearXNG can aggregate.

## Publishing to crates.io

eggsearch is published as a single crate. Before publishing:

- `cargo clippy --all-features -- -D warnings` is clean
- `cargo test --all-features` passes (260 tests)
- `cargo publish --dry-run` succeeds
- The version in `Cargo.toml` is bumped
- `CHANGELOG.md` is updated

The crates.io package includes the README, LICENSE files, and CHANGELOG via
the `include` field in `Cargo.toml`.
