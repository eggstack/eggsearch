# AGENTS.md

This file contains information for AI coding agents working on the eggsearch codebase.

## Project Overview

eggsearch is a lightweight MCP (Model Context Protocol) search/fetch server for AI agents. It queries upstream search providers (DuckDuckGo, Brave, Startpage, Yahoo, Mojeek), deduplicates results with reciprocal rank fusion, returns compact source cards, and also fetches one explicit HTTP(S) URL on demand with bounded text extraction. Transport is MCP over stdio.

As of the agent-tool-surface-simplification, `web_search` also accepts
optional `intent` and `freshness` retrieval hints and returns
deterministic `SourceCard` metadata (`source_kind`, `domain`,
`rank_reasons`) to help agents choose which result to inspect first.
Intent-aware post-RRF reranking applies bounded domain priors.

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
    config.rs            # CLI config loader (thin wrapper around core::config)
    commands/            # subcommands: doctor, search, providers, mcp, fetch
    core/                # core types and logic
      mod.rs             # re-exports (AppConfig, WebSearchRequest, etc.)
      config.rs          # AppConfig, SearchSection, FetchSection, validation
      error.rs           # CoreError, CoreResult (thiserror)
      query.rs           # WebSearchRequest, resolve_max_results, MaxResultsResolution
      result.rs          # SearchWarning, TrustLevel
      source_card.rs     # SourceCard output type
      document.rs        # FetchDocument, DocumentKind, RenderFormat, BlockKind, etc.
      sanitize.rs        # prompt-injection hardening (strip, frame, scan)
      provider.rs        # ProviderKind, ProviderCapabilities, ProviderDescriptor
      fetch.rs           # fetch-related types (ExtractMode, WebFetchRequest, etc.)
    meta/                # MetadataSearchAdapter + vendored engines
      mod.rs             # re-exports
      adapter.rs         # MetadataSearchAdapter, convert_aggregated, provider_status
      mock.rs            # MockEngine (feature-gated behind `mock`)
      response.rs        # WebSearchResponse, ProviderFailure
      engines/           # vendored search engine implementations
    fetch/               # HTTP fetch client, HTML structural rendering, and extraction
      mod.rs             # re-exports
      client.rs          # FetchClient, sanitize_field
      extract.rs         # HTML/text extraction logic (returns 6-tuple including text_truncated)
      limits.rs          # FetchLimits struct
      types.rs           # internal fetch types
    mcp/                 # MCP server (rmcp)
      mod.rs             # re-exports
      server.rs          # EggsearchServer, tool_router, EGGSEARCH_INSTRUCTIONS
      tools.rs           # run_web_search, run_web_fetch, run_provider_status
      state.rs           # ServerState (Arc<AppConfig> + Arc<MetadataSearchAdapter>)
      policy.rs          # live_allowed, fetch_allowed, deny messages
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
- Tools: `web_search` (live metasearch with optional `intent`/`freshness` retrieval hints), `web_fetch` (bounded URL fetch), and `provider_status` (diagnostic/host-facing)
- Transport: stdio only (no HTTP/SSE)
- Server instructions are in `EGGSEARCH_INSTRUCTIONS` constant in `mcp/server.rs`

### Configuration
- Config file: `$XDG_CONFIG_HOME/eggsearch/config.toml`
- `AppConfig` is the root type, contains `SearchSection`
- `SearchSection` is the `[search]` section: `mode`, `default_max_results` (alias: `max_results`), `max_results_cap`, `max_query_chars`, `timeout_ms`, `default_providers`, `providers`, `searxng`, `api`, `live`, `sanitize_output`
- `FetchSection` is the `[fetch]` section: enables/disables `web_fetch` and configures fetch limits (enabled, timeout_ms, max_bytes, max_chars_default, max_chars_cap, redirect_limit, allow_private_network, allow_localhost, include_links_default, user_agent, sanitize_output)
- `SearxngConfig` is the `[search].searxng` section: enables the optional `searxng` provider (`enabled`, `base_url`)
- `ApiProviderConfig` is the `[search.api.<id>]` section: API-key provider config (`enabled`, `api_key_env`, `base_url`)
- `Mode` enum: `Live` or `Off`
- `ServerState` holds `Arc<AppConfig>` + `Arc<MetadataSearchAdapter>`
- Both `SearchSection` and `FetchSection` have `sanitize_output: bool` (default `true`). When `true`, Tier 2 (framing) and Tier 3 (marker scan) prompt-injection defenses are active. Tier 1 (control-char strip + length bound) is always on.

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
- Each card includes deterministic `metadata` with `source_kind` (enum: `official_docs`, `package_registry`, `source_repository`, `issue_thread`, `release_notes`, `security_advisory`, `reference`, `news`, `tutorial`, `forum`, `unknown`), `domain`, and `rank_reasons` (e.g. `rrf_multi_provider`, `intent_match`, `domain_prior_docs`)
- Trust level is always `external_untrusted` for live web results
- Deduplication happens via URL normalization in the vendored `aggregate_rrf()` function
- `WebFetchResponse` is the output type returned by `web_fetch`; trust is always `external_untrusted` for live web content

### Document Model

`web_fetch` returns an optional `document: Option<FetchDocument>` alongside the legacy `text` field. Existing agents can keep reading `text`; newer agents can inspect the structured `document` object.

Key types (all in `src/core/document.rs`):
- `DocumentKind`: `html`, `plain_text`, `markdown`, `code`, `json`, `toml`, `yaml`, `diff`, `patch`, `pdf`, `unknown`
- `RenderFormat`: `legacy_text`, `agent_blocks_v1`
- `BlockKind`: `heading`, `paragraph`, `list_item`, `code`, `table`, `block_quote`, `definition`, `horizontal_rule`, `page_break`, `raw_text`
- `FetchDocument`: kind, render_format, text_format, text_chars_returned, text_truncated, block_truncated, link_truncated, metadata, outline, blocks, chunks
- `FetchRenderMetadata`: bytes_read, content_length, charset, redirects_followed, source_extension, detected_language
- `DocumentOutlineEntry`: level, title, anchor, block_index
- `RenderedBlock`: kind, text, level, anchor, language, line_start, line_end, page
- `DocumentChunk`: chunk_id, text, heading_path, block_start, block_end, page_start, page_end

Phase 1 builds a minimal compatibility document: HTML gets `kind=html` with a single `paragraph` block, plain text gets `kind=plain_text` with a `raw_text` block. Chunks are a single chunk wrapping all blocks. Block text passes through Tier 1 (control-char strip + length bound) but is NOT framed (unlike the legacy `text` field).

Phase 3 adds full content-type detection (`src/fetch/detect.rs`) and line-preserving renderers. `web_fetch` now classifies non-HTML responses using Content-Type headers, URL file extensions, and byte heuristics. Source code, JSON, TOML, YAML, diffs, and patches are rendered as line-preserving `Code` blocks with `line_start`/`line_end` metadata. Markdown source files are parsed with `pulldown-cmark` into heading, code, and paragraph blocks with an outline. Plain text is split into paragraph blocks. The `FetchRenderMetadata.detected_language` field is populated when a language can be determined.

The `src/fetch/render/` module contains the HTML structural renderer:
- `blocks.rs` parses HTML and produces `Vec<RenderedBlock>` with proper element mapping
- `text.rs` renders blocks as plain text
- `markdown.rs` renders blocks as Markdown
Content root selection prefers `main` > `article` > `[role=main]` > `body`.

`text_truncated` (character-level) is distinct from `truncated` (byte-level body cap). Both are reported.

### Content Detection

`src/fetch/detect.rs` provides a deterministic `classify(content_type, url, body)` function that returns a `DetectedContent` struct with `kind`, `language`, and `line_preserving` fields. Detection priority: Content-Type header > URL file extension > byte heuristics. Byte heuristics look for shebangs, import statements, function definitions, and struct/class patterns to identify code-like content under `text/plain`.

### Non-HTML Renderers

`src/fetch/render/code.rs` provides `render_code()`, `render_diff()`, and `render_plaintext()` for line-preserving rendering. `src/fetch/render/markdown_source.rs` provides `render_markdown_source()` using `pulldown-cmark` for Markdown file parsing with heading extraction, fenced code block detection, and outline generation.

### Search Intent and Freshness

`web_search` accepts optional `intent` and `freshness` fields as
retrieval hints. These are NOT workflow triggers — they only influence
post-RRF reranking with bounded domain priors. Both fields accept
common aliases from weaker models (e.g. `"documentation"` -> `docs`,
`"24h"` -> `day`, `"latest"` -> `month`) without hiding truly
ambiguous mistakes.

`SearchIntent` enum: `web` (default), `docs`, `code`, `issues`,
`releases`, `security`, `news`.

`Freshness` enum: `any` (default), `day`, `week`, `month`, `year`.

Intent-aware reranking boosts results whose `source_kind` matches the
requested intent (e.g. `docs` intent boosts `official_docs` and
`package_registry` sources). Boosts are bounded (+10-30% of max base
score) so provider evidence remains dominant. Intent/freshness
reranking operates on a candidate pool larger than the final
`max_results` so intent-matching results just outside the final
window can be promoted.

`FreshnessMatch` is only emitted when a result has actual recency
date metadata. Currently no providers expose result-level dates, so
`FreshnessMatch` is never emitted. The `freshness` field is retained
as a best-effort hint for future provider support.

### Candidate Pool Flow

`MetadataSearchAdapter::web_search(req, effective_max_results,
max_results_cap)` runs a discovery-only metasearch and is the entry
point for the MCP `web_search` tool. The flow is:

1. Compute a `candidate_limit` (typically `min(effective_max_results *
   3, max_results_cap)`; never less than `effective_max_results`,
   never panics when `effective_max_results > max_results_cap`)
   **before** provider fan-out.
2. Fan out to each enabled provider with `candidate_limit` as the
   per-engine `max_results` argument. Each provider is responsible
   for returning up to that many compact `SearchResult` values. No
   page bodies are fetched — the extra headroom is only used to
   expand the compact candidate pool.
3. Aggregate the provider results via the vendored `aggregate_rrf`
   up to `candidate_limit` (URL-normalized dedup).
4. Convert each aggregated row to a `SourceCard` with deterministic
   `source_kind` / `domain` / `rank_reasons` metadata.
5. Apply bounded intent-aware post-RRF reranking.
6. Truncate the final response to `effective_max_results` so an
   intent-matching result just outside the final window can be
   promoted.

The MCP `run_web_search` caller passes
`state.config.search.max_results_cap` to the adapter so the candidate
pool is config-aware. The CLI `search` and `doctor` paths pass the
same value from `AppConfig`. Provider fan-out logs distinguish
`final_max_results` from `candidate_limit` for debugging.

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
- `engines/brave_api.rs` — Brave Search API provider (API-key, JSON; added in 0.3.0)
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
- `cargo test --all-features` passes (371 tests)
- `cargo publish --dry-run` succeeds
- The version in `Cargo.toml` is bumped
- `CHANGELOG.md` is updated

The crates.io package includes the README, LICENSE files, and CHANGELOG via
the `include` field in `Cargo.toml`.
