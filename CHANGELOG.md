# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Global timeout now preserves partial results from engines that responded in time
- Per-request `timeout_ms` override is now honored (bounded by global timeout)
- Duplicate `providers_failed` entries on global timeout eliminated
- `AppConfig::save` TOML serialization error now has a dedicated error variant

### Changed
- `safe_search` parameter documented as reserved for future use (upstream engines don't support it)
- Removed unused dependencies (`sha2`, `hex`, `anyhow`, `chrono`) from `eggsearch-core`

### Removed
- `source_identity` method from `SourceCard` (dead code, never called in production)
- `provider_enabled` method from `AppConfig` (dead code, never called)
- `ErrorClass::InvalidQuery` variant (dead code, never constructed)
- `domain_of` function from `normalize` module (dead code, never called)

### Added
- Unit tests for `SafeSearch::as_str`, `TrustLevel::as_str`, `SearchWarning::new`
- Integration tests for partial timeout, per-request timeout override, mixed provider config
- Integration tests for config save/load round-trip, malformed TOML handling
- `LICENSE`, `LICENSE-APACHE`, `LICENSE-MIT` files
- `AGENTS.md` for AI coding agents
- Publishing metadata (`repository`, `homepage`, `keywords`, `categories`) to all sub-crates

## [0.1.0] - 2025-01-01

### Added
- Initial release
- MCP server with `web_search` and `provider_status` tools
- CLI with `doctor`, `search`, `providers`, and `mcp stdio` commands
- Support for DuckDuckGo, Brave, Startpage, and Yahoo providers
- Reciprocal rank fusion for result deduplication
- Configuration via TOML file
- Mock engine support for testing
