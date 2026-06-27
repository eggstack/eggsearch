# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.3] - Unreleased

### Changed

- Provider capability audit: searxng and brave_api capabilities corrected to reflect what the adapter actually forwards (not what the upstream API supports); github_releases `org_filter` corrected to false
- Capability warnings emitted when requests ask for behavior providers cannot enforce (safe_search, freshness, intent without native providers)

### Added

- `server_capabilities` object in `provider_status` response for MCP capability discovery
- Capability warning system in adapter (6 advisory warning types)
- Regression tests for intent-neutral generic search, intent reranking, and provider status
- README stable baseline section documenting tool contracts

### Added

- Content-type detection classifier (`src/fetch/detect.rs`) for deterministic document kind and language identification from Content-Type headers, URL extensions, and byte heuristics
- Line-preserving code renderer (`src/fetch/render/code.rs`) for source code, JSON, TOML, YAML, and config files with line-range metadata
- Diff/patch renderer with hunk preservation
- Markdown source file renderer (`src/fetch/render/markdown_source.rs`) using pulldown-cmark with heading outline extraction
- Plain text prose renderer with paragraph-based block splitting
- `web_fetch` now accepts `application/json`, `text/markdown`, `text/toml`, `text/yaml`, `text/x-diff`, and other text-based Content-Type headers
- `FetchRenderMetadata.detected_language` field populated from content detection
- 12 new integration tests covering JSON, Markdown, TOML, YAML, diff, code, plaintext, and truncation behavior

### Fixed

- **UTF-8-safe snippet truncation in `github_issues` and `github_releases` engines**: the legacy `truncate_body` helper sliced on byte offsets and panicked when the slice landed inside a multi-byte code point (e.g. CJK characters or emoji). The new implementation counts Unicode scalar values and only returns substrings at valid char boundaries, preserving the historical word-boundary trim semantics. Added 10 new unit tests covering multibyte UTF-8, CJK, emoji-only text, zero `max_chars`, and word-boundary-with-emoji cases.

## [0.4.0] - Unreleased

### Added
- `web_fetch` now supports `extract_mode: "markdown"` for Markdown-rendered output. HTML pages are rendered as structured Markdown with headings, code blocks, tables, lists, and inline formatting.
- HTML pages now produce structured blocks (`headings`, `paragraphs`, `list_item`, `code`, `table`, `block_quote`, `definitions`, `horizontal_rule`) instead of a single flat text block.
- Document outline is populated from HTML heading elements.
- Code blocks preserve whitespace and detect language classes from `<code>` elements.
- Content root selection prefers `<main>`, `<article>`, `[role=main]`, then `<body>`.
- New `src/fetch/render/` module with HTML structural renderer: `blocks.rs` (HTML-to-blocks parser), `text.rs` (plain text renderer), `markdown.rs` (Markdown renderer).

## [Unreleased]

### Changed
- `MetadataSearchAdapter::web_search` now takes a `max_results_cap` argument alongside the caller's effective `max_results`. The candidate-pool limit is computed from these two values before provider fan-out, so each provider is asked for the candidate limit rather than the final return count. This lets intent-aware reranking promote intent-matching results that would otherwise be truncated before ranking.
- `candidate_pool_size` is now config-aware (bounded by the configured cap) and cannot panic when `effective_max_results > max_results_cap`. The previous helper used `usize::clamp(min, max)` which panicked on that path.
- Provider fan-out logs now distinguish `final_max_results` from `candidate_limit` for debugging.

### Added
- `repo_search` MCP tool for structured repository evidence discovery with grouped result bundles and suggested fetch URLs
- `RepoSearchRequest`, `RepoResultGroup`, `RepoSearchResponse`, `RepoSuggestedFetch` types in `src/core/repo_search.rs`
- `repo_grouping` deterministic classification of SourceCards into group kinds (OfficialDocs, PackageRegistry, Repository, Readme, Examples, Tests, SourceFiles, Issues, PullRequests, Releases, MigrationNotes, Changelog, CommunityDiscovery, Other)
- `repo_planner` subquery generation for repo search bundles
- `suggested_fetches` suggested fetch URL generation for each group
- `server_capabilities.repo_search` field now reports `true`
- `security_search` MCP tool for security-oriented retrieval with normalized vulnerability metadata and grouped source cards
- `osv` provider: native OSV (Open Source Vulnerabilities) JSON API adapter for querying vulnerability databases by package+ecosystem or vulnerability ID. No API key required. Enabled by default.
- `SecuritySearchRequest`, `SecurityIdentifiers`, `VulnerabilityMetadata`, `SecurityResultGroup`, `SecuritySearchResponse` types in `src/core/security.rs`
- Deterministic identifier parser for CVE, GHSA, OSV, RustSec, and package/ecosystem/version hints
- `ResultMetadata::Advisory` variant for native advisory provider results
- `SourceMetadata.vulnerability` field for structured vulnerability metadata on source cards
- `ProviderCapabilities.supports_security_search` flag
- `RankReason::ProviderNativeAdvisorySearch` variant
- Security result grouping logic (AuthoritativeAdvisories, VendorAdvisories, PackageAdvisories, KevEntries, PatchCommitsOrReleases, ExploitDiscussion, DefensiveGuidance, GeneralContext, Other)
- `server_capabilities.security_search` field now reports `true`
- `KevMetadata` type for CISA Known Exploited Vulnerabilities data
- `VulnerabilitySource` enum (Osv, GithubAdvisory, Nvd, Rustsec, CisaKev, Generic)
- `SeverityLevel` enum (Critical, High, Medium, Low, Unknown) with loose parsing

### Fixed
- `MockEngine::search` now respects the `max_results` argument and truncates its canned results accordingly. Previously the mock ignored the limit and returned all canned results, masking the candidate-pool bug where production providers were called with `final_max_results` instead of `candidate_limit`.
- `run_web_fetch` MCP tool now includes `links_seen` and `links_truncated` fields in the JSON payload.
- CLI `eggsearch fetch --json` now includes `trust_markers` and `document` fields in JSON output.
- **PDF `metadata_only` no longer leaks body content**: when `extract_mode: "metadata_only"` targets a PDF, the response returns `text: null` with empty `document.blocks` and `document.chunks` instead of extracting and returning page text.
- **Document link truncation metadata consistency**: `FetchDocument.link_truncated` now mirrors the top-level `links_truncated` field instead of being hardcoded to `false`.
- **HTML outline entries filtered after truncation**: outline entries whose `block_index` points to a block removed by block-boundary truncation are now removed, preventing stale index references.
- **HTML outline pruning helper**: `src/fetch/render/blocks.rs` exposes `prune_outline_to_blocks(&mut outline, blocks.len())` that retains only entries with `block_index < blocks.len()`, called immediately after `blocks.truncate(last_valid)`. Unit tests cover the in-range, out-of-range, and `None` block_index branches; an integration test verifies the invariant end-to-end through `web_fetch`.
- **Code/diff/plaintext renderers enforce hard output bounds**: oversized single lines or paragraphs are now truncated to the configured `max_chars` budget instead of being pushed in full, preventing block text from exceeding the character limit.
- **PDF document metadata includes real fetch context**: `FetchRenderMetadata` for PDFs now reports actual `bytes_read`, `content_length`, and `redirects_followed` instead of hardcoded zeros.
- **HTML sparse-root fallback**: when `main` or `article` exists but produces no or minimal content, the renderer falls back to `body` instead of returning an empty document.
- **Content-type classifier parity**: `application/javascript`, `application/x-javascript`, `application/typescript`, and `application/x-sh` are now classified as code by the content detection classifier.

### Added
- **Link classification for `web_fetch`**: extracted links now include a deterministic `link_kind` classification (`same_page_anchor`, `same_domain`, `external`, `download`, `source_code`, `documentation`, `api_reference`, `issue`, `pull_request`, `release`, `security_advisory`, `pdf`, `image`, `feed`, `other`), optional `rel` attribute, and `same_domain` boolean flag. Classification uses cheap URL heuristics (host equality, path patterns, file extensions) with no external dependencies.
- **Link bounding metadata**: `WebFetchResponse` now includes `links_seen` (total `<a href>` elements encountered) and `links_truncated` (whether the link list was capped at 100) fields when `include_links` is enabled.
- **CLI link display**: `eggsearch fetch --links` now shows link classification kinds and link bounding metadata in both pretty and JSON output.
- 4 new integration tests covering link classification, link bounding metadata, empty links when not requested, and same-domain detection.
- `RecordingMockEngine` test helper (feature-gated behind `mock`) that records the `max_results` argument it was called with. Used by new regression tests to verify provider fan-out passes the candidate-pool limit to providers.
- Unit tests covering `candidate_pool_size` panic-safety, zero-handling, and the cap-clamping edge case.
- Integration tests covering the candidate-pool flow at the MCP tool boundary: provider receives candidate limit, candidate pool grows above the final count, candidate pool clamps to a small cap, and the intent-reranking regression test now actually exercises the bug fix.
- **Structured document model for `web_fetch`**: new `document` field on `WebFetchResponse` with `DocumentKind`, `RenderFormat`, `BlockKind`, `FetchDocument`, `FetchRenderMetadata`, `DocumentOutlineEntry`, `RenderedBlock`, and `DocumentChunk` types in `src/core/document.rs`. Phase 1 builds a minimal compatibility document from current extraction output: HTML gets `kind=html` with a single paragraph block, plain text gets `kind=plain_text` with a raw-text block. Block text passes through Tier 1 sanitization (control-char strip + length bound) but is not framed. The legacy `text` field remains fully populated for backward compatibility.
- `text_truncated` field on `FetchDocument` distinguishes character-level truncation from the existing byte-level `truncated` flag.
- `FetchRenderMetadata` reports `bytes_read`, `content_length`, `charset`, `redirects_followed`, `source_extension`, and `detected_language`.
- `HtmlExtractor::extract` and `extract_content` now return a 6-tuple with an additional `text_truncated: bool` field.
- 10 new integration tests covering document model acceptance criteria (kind/format, blocks/chunks, metadata, truncation, sanitization, legacy field compatibility).
- **PDF text extraction** (feature-gated behind `pdf`, opt-in, not default): `web_fetch` detects PDF responses via `Content-Type: application/pdf` or `.pdf` URL extension and extracts text using the `lopdf` crate. Per-page indexed blocks with `page_break` markers, per-page chunks. Bounded by `pdf_max_pages` (default 25), `pdf_max_chars_per_page` (default 12000), and `pdf_max_total_chars` (default 50000). New config fields: `pdf_enabled`, `pdf_max_pages`, `pdf_max_chars_per_page`, `pdf_max_total_chars` in `[fetch]`. New error variants: `pdf_not_compiled_in`, `pdf_disabled`, `pdf_parse_error`, `pdf_encrypted`, `pdf_no_extractable_text`. Limit hits produce a warning and partial content rather than a hard error. MSRV bumped from 1.80 to 1.85 (lopdf 0.42 requirement). No OCR, no embedded file extraction, no JavaScript.
- **Code-host source-file fetch**: `web_fetch` now recognizes source-file browser URLs from GitHub, GitLab, and Codeberg and internally rewrites them to raw content URLs for fetching. GitHub blob URLs are rewritten to `raw.githubusercontent.com`, GitLab blob URLs to `/-/raw/`, and Codeberg src URLs to `/raw/branch/`. The response includes a `fetch_transform` object (`kind`, `original_url`, `transformed_url`) when a rewrite occurs. Both the original and rewritten URLs pass the same SSRF/localhost/private-network validation. New types: `FetchTransform`, `FetchTransformKind` in `src/core/fetch.rs`; `resolve_code_host_fetch_target` in `src/core/code_host_fetch.rs`.
- 10 new unit tests for URL resolution (GitHub/GitLab/Codeberg blob URLs, non-file URLs, line anchors, safety validation) and 7 new integration tests for code-host fetch (transform metadata, serde roundtrips, response shape).

## [0.3.4] - Unreleased

### Changed
- **Codeberg raw rewrite deferred**: `web_fetch` no longer rewrites Codeberg source-file browser URLs (`/src/branch/<ref>/<path>` or `/src/tag/<ref>/<path>`) to raw paths. Distinguishing branch refs from tag refs at the parser level is out of scope until the Codeberg raw URL shape is verified. Codeberg source-file URLs still classify as `SourceFile` and are fetched as ordinary web pages through the existing HTML extraction path; no `fetch_transform` block is emitted. The `FetchTransformKind::CodebergRawFile` variant has been removed; only `github_raw_file` and `gitlab_raw_file` are emitted.
- **Documented `supports_freshness` vs `supports_result_timestamps` semantics**: `ProviderCapabilities::supports_freshness` is the provider-side flag (upstream engine accepts a time-range parameter), while `supports_result_timestamps` is the client-side flag (provider payloads carry per-result timestamps usable for local freshness reranking). GitHub issues/releases set `supports_result_timestamps = true` and `supports_freshness = false`; the GitHub search API does not accept a freshness parameter but its payloads include `updated_at` / `published_at`, so eggsearch applies local freshness reranking on the response. `FreshnessMatch` is never emitted without timestamp evidence.
- **Metadata merge on RRF deduplication**: when the same URL is returned by multiple providers, the structured metadata (`ResultMetadata::Issue(...)`, `ResultMetadata::Release(...)`) wins over `ResultMetadata::None` from a generic HTML scraper. New `ResultMetadata::merge`, `IssueMetadata::merge`, and `ReleaseMetadata::merge` helpers in `src/meta/engines/models.rs` and `src/core/source_card.rs`. The merge is idempotent and order-independent; the left side wins for shared fields.

### Added
- 5 unit tests for `ResultMetadata::merge` semantics and 2 adapter-level tests proving that structured `IssueMetadata` / `ReleaseMetadata` survive RRF aggregation even when a generic HTML scraper also returns the same URL with `ResultMetadata::None`.
- 4 unit tests pinning the (false, true) capability shape for `github_issues` and `github_releases`, and the (false, false) shape for HTML scrape providers and `github_code`.
- New unit tests for the disabled Codeberg rewrite: `codeberg_src_branch_resolves_without_raw_rewrite`, `codeberg_src_tag_resolves_without_raw_rewrite`, and `codeberg_to_fetch_transform_returns_none`.

## [0.3.2] - 2026-06-07

### Changed
- Documentation cleanup pass before Codegg integration:
  - README: added "Search and fetch workflow" section distinguishing `web_search` (discovery) from `web_fetch` (explicit URL)
  - README: clarified `default_max_results` / `max_results_cap` / per-request `max_results` relationship and legacy `max_results` alias
  - README: tightened SSRF/DNS-rebinding claims; no longer claims "complete DNS-rebinding defense"
  - README: restructured "Search Engines" section to distinguish known IDs, enabled providers, and default providers; added build conditions for `searxng` and `brave_api`
  - `web_search` tool description now lists `brave_api` and marks `safe_search` as reserved
  - `provider_status` tool description now includes `api_key` as a kind
  - `SafeSearch` type-level doc and `WebSearchRequest.safe_search` field doc clarified as reserved for future use
  - `TrustLevel` doc no longer says "For the MVP"; `LocalTrusted` correctly described as reserved
  - `SourceCard.fetched` field doc no longer says "MVP"; clarifies `web_search` is discovery-only
  - Removed stale "MVP" wording from `ProviderStatusArgs.probe` doc

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
