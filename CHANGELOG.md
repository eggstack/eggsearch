# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-06-07

### Changed
- `search.max_results` config field is deprecated in favor of `search.default_max_results`. Old configs using `max_results` are still accepted via a serde alias.
- MCP request `max_results` is now a per-call final SourceCard count preference. When the request exceeds the server's `max_results_cap`, the response is clamped and a warning is included instead of returning a validation error.

### Added
- Centralized `resolve_max_results()` function in `core::query` for resolving the effective result count with clamping and warning generation.
- Warning in `web_search` response when requested `max_results` exceeds configured `max_results_cap`.
- `search` section in `doctor` output reporting `default_max_results` and `max_results_cap`.
- MCP-level integration tests for `web_fetch` end-to-end (response shape, trust label, trust_markers, sanitize/framing behavior) and for the three-tool surface (`web_search`, `web_fetch`, `provider_status`) under mock state.

### Fixed
- Documentation: `README.md` "Project Structure" tree now lists `fetch` as a top-level library module (matches `src/lib.rs`); the "Search Engines" section now mentions the `brave_api` adapter and the "Security" section documents the distinct error classes for `web_search` and `web_fetch`.
- Documentation: `src/mcp/mod.rs` module-level docs now list `web_fetch` alongside `web_search` and `provider_status`.
- Test code: `field_reassign_with_default` clippy lint in `sanitize::tests` and `content-length` borrow in `fetch::client::tests` are written in a clippy-clean form.

## [0.3.0] - 2026-06-07

### Added
- **Provider capabilities model**: new `ProviderKind`, `ProviderCapabilities`, and `ProviderDescriptor` types. `provider_status` now returns full descriptors with kind, enabled/default/configured state, API-key requirement, and capability flags.
- **API-backed provider architecture**: new `[search].api` config section for API-key providers. `brave_api` added as reference implementation (disabled by default). Configure with `[search.api.brave] enabled = true, api_key_env = "BRAVE_SEARCH_API_KEY"`.
- **Fetch redirect hardening**: `web_fetch` now uses a manual redirect loop with per-redirect URL validation. Redirects to localhost, private-network, or credential-bearing URLs are blocked. New error variants: `RedirectLimitExceeded`, `RedirectTargetBlocked`, `InvalidRedirectLocation`, `EmbeddedCredentialsBlocked`.
- **Config validation improvements**: hard error for unknown provider IDs in config or explicit requests; distinct error message for disabled vs unknown providers; SearXNG base_url validation; API provider credential validation.
- **CLI `doctor` enhancements**: reports provider capabilities, API credential status (without printing secrets), fetch network policy, and misconfiguration warnings. `--probe` flag for live health checks.
- **CLI `providers` enhancements**: displays descriptor fields (kind, API key required, configured, capabilities) in a formatted table.
- **CLI `fetch` enhancements**: `--include-links` flag (renamed from `--links` with backward-compatible alias).

### Changed
- `MetadataSearchAdapter::provider_status()` now returns `Vec<ProviderDescriptor>` instead of `Vec<ProviderStatus>`.
- `resolve_providers()` now filters disabled default providers silently and returns distinct errors for disabled vs unknown provider IDs.
- `AppConfig::validate()` rejects unknown provider IDs in `default_providers` and `providers` map, and validates SearXNG and API provider configs.

### Notes
- The `brave_api` provider is opt-in and requires a Brave Search API key via environment variable.
- All existing HTML scrape providers (duckduckgo, brave, startpage, yahoo, mojeek) are unchanged.
- This is a polish release. All core features (web_search, web_fetch, provider_status, CLI commands, prompt-injection hardening) were already present in 0.2.1.

## [0.2.1] - 2026-06-07

### Added
- Prompt-injection hardening for `web_search` and `web_fetch`:
  - All untrusted text fields (snippet, title, fetched page text) are
    stripped of control characters (NUL, CR, ASCII control range,
    bidi controls, zero-width) and length-bounded (titles 200 chars,
    snippets 500 chars).
  - When `[search].sanitize_output` and `[fetch].sanitize_output` are
    `true` (the default), untrusted text is wrapped with
    `<<<EXTERNAL_UNTRUSTED field=... id=...>>>` ... `<<<END>>>`
    framing delimiters so a string-scanning model can see the
    boundary between the tool's output structure and external content.
  - When the same flag is `true`, a small allowlisted set of
    prompt-injection markers (e.g. "ignore previous instructions",
    ChatML-style tags) is scanned for in untrusted text. Detected
    markers are surfaced as advisory entries in the response's
    `warnings` array; the content is still returned.
  - A new `trust_markers` object on every response summarizes what
    eggsearch did to the untrusted text in that call: whether it
    was sanitized, truncated, framed, how many control chars were
    removed, and how many injection markers were found.
- `MetadataSearchAdapter` and `FetchClient` constructors take a new
  `sanitize_output: bool` parameter. `MetadataSearchAdapter::from_engines`
  defaults the flag to `false` for back-compat with test fixtures.
- `[search].sanitize_output` (default `true`) and
  `[fetch].sanitize_output` (default `true`) configuration knobs.

### Notes
- The new defenses are *defense in depth*; the host's system prompt
  and instruction-following discipline remain the primary defense.
- Hosts that need raw, unprocessed text (e.g. they have their own
  downstream sanitizer) can opt out by setting both flags to
  `false`. Control-char stripping and length bounding remain on
  even when the flags are `false`.

## [0.2.0] - 2026-06-07

### Added
- `mojeek` search engine adapter (HTML scrape). No API key required.
  Disabled by default; enable with `[search].providers.mojeek = true`.
- `searxng` search engine adapter. Connects to a self-hosted SearXNG
  instance over its JSON API (`{base_url}/search?format=json`). Disabled
  by default. Configure with `[search].searxng].enabled = true` and
  `[search].searxng.base_url = "https://searx.example.org"`. The
  `searxng` provider id can be a high-leverage addition because a
  single SearXNG instance can aggregate many underlying engines
  (including Qwant, when the instance's admin has enabled it).
- New `[search].searxng` config section (`enabled`, `base_url`).
- New fixture-based unit tests for the `mojeek` and `searxng` engines
  (parse and convert paths, max_results, missing fields, edge cases).

### Notes
- Qwant was investigated as a direct HTML scrape but is not viable in
  the current build: `qwant.com` and `lite.qwant.com` are JavaScript
  shells that load results via authenticated XHR to `api.qwant.com/v3`,
  and the API returns 403 for unauthenticated requests. Operators who
  want Qwant coverage should point `searxng.base_url` at a self-hosted
  SearXNG instance that has the Qwant engine enabled.

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
