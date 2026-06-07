# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-06-07

### Added
- `web_fetch` MCP tool and CLI command for fetching one explicit HTTP(S) URL
- `fetch` config section with limits (timeout_ms, max_bytes, max_chars_default, max_chars_cap)
- Private-network blocking by default in web_fetch
- `doctor --probe` for live provider health checks
- Config validation for provider defaults and enabled/disabled states
- `authors` field in `Cargo.toml`
- `[fetch]` config table in `README.md`

### Changed
- `safe_search` parameter now emits a warning when used (not enforced by HTML providers)
- User-agent is now configurable via `[fetch] user_agent` config (previously overridden by a hard-coded Mozilla header in the metasearch client; that override is now removed)
- `resolve_providers()` now validates explicit provider lists against enabled providers
- `provider_status` remains non-probing (no network access)
- `FetchClient` is now constructed once at server startup and reused across MCP calls
- `AppConfig::validate()` now checks config invariants (e.g. `max_chars_cap >= max_chars_default`)
- Dead config fields `search.live.user_agent` and `search.live.respect_robots_txt` now warn at startup if set

### Fixed
- `resolve_providers()` now filters `default_providers` to only enabled providers
- Provider config errors now return clear validation messages
- `web_fetch` MCP tool now respects `[fetch].enabled` config (previously ignored)
- `web_fetch` MCP tool now returns a validation error for `extract_mode: "markdown"` (not yet implemented) instead of silently treating it as text
- `web_fetch` MCP tool now honors `[fetch].include_links_default`
- CLI `search` now respects `[search].mode = "off"`
- Private-network SSRF gap closed: `web_fetch` now resolves DNS and validates resolved IPs, blocking hostname-based bypasses
- `max_chars = 0` now returns a validation error instead of returning empty text
- `web_fetch` now pre-checks `Content-Length` and fails fast for bodies exceeding `max_bytes`
- Cookie store removed from the metasearch HTTP client (privacy / no longer needed)
- Engine timeouts are now derived from the per-request `effective_timeout` instead of a hardcoded 8s
- Non-UTF-8 fetch response bodies now produce a warning instead of silently becoming empty text

## [0.1.1] - 2026-06-05

### Changed
- Bumped version to 0.1.1 to work around crates.io deleted-crate name-reuse cooldown on `eggsearch`

## [0.1.0] - 2026-06-05

### Fixed
- Global timeout now preserves partial results from engines that responded in time
- Per-request `timeout_ms` override is now honored (bounded by global timeout)
- Duplicate `providers_failed` entries on global timeout eliminated
- `AppConfig::save` TOML serialization error now has a dedicated error variant
- Brave provider no longer incorrectly reports `requires_api_key: true`
- DuckDuckGo URL extraction fallback: `extract_destination_url` now correctly
  pulls the `uddg` query parameter from the redirect URL.

### Changed
- Vendored search engine implementations from `metadata-search-engine-rs` into `src/meta/engines/`
- Removed `metadata-search-engine-rs` dependency (eliminated 34 transitive deps)
- Release binary shrunk from 7.3 MB to 6.3 MB (14% reduction)
- `safe_search` parameter documented as reserved for future use (upstream engines don't support it)
- Removed unused dependencies (`sha2`, `hex`, `chrono`, `futures`, `clap_complete`, `wiremock`)
- **Flattened the four-crate workspace into a single `eggsearch` crate** for
  the crates.io release. The `core`, `meta`, and `mcp` sub-crates have been
  folded into `src/{core,meta,mcp}/` modules. Only the unified `eggsearch`
  crate is published; the sub-crates are not on crates.io.

### Removed
- `source_identity` method from `SourceCard` (dead code, never called in production)
- `provider_enabled` method from `AppConfig` (dead code, never called)
- `ErrorClass::InvalidQuery` variant (dead code, never constructed)
- `domain_of` function from `normalize` module (dead code, never called)
- The `metasearch` feature flag (metasearch code is now always compiled)
- Workspace root `Cargo.toml` and the `eggsearch-core`, `eggsearch-meta`,
  `eggsearch-mcp` crate directories

### Added
- Unit tests for `SafeSearch::as_str`, `TrustLevel::as_str`, `SearchWarning::new`
- Integration tests for partial timeout, per-request timeout override, mixed provider config
- Integration tests for config save/load round-trip, malformed TOML handling
- DuckDuckGo engine unit tests for URL extraction, parsing, max_results, and snippet handling
- `LICENSE`, `LICENSE-APACHE`, `LICENSE-MIT` files
- `AGENTS.md` for AI coding agents
- Publishing metadata (`repository`, `homepage`, `keywords`, `categories`, `readme`, `include`) to the unified `eggsearch` crate
- GitHub Actions CI at `.github/workflows/ci.yml` (build, test, clippy, publish dry-run)
