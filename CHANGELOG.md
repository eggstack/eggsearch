# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Binary-first release artifacts, checksums, and bootstrap installers for supported desktop and SBC targets.
- Binary-first `eggsearch update` and `update --check` commands. Updates use
  crates.io stable metadata, exact GitHub release assets, bounded SHA-256 and
  candidate identity verification, and exact-version Cargo fallback only for
  unsupported hosts or confirmed asset 404 responses.

## [0.3.8] - 2026-09-04

### Added

- **Tavily search provider and workstream closure (phase 5).** Opt-in `tavily` engine (`POST https://api.tavily.com/search`, `Authorization: Bearer TAVILY_API_KEY`) using the provider-neutral request/evidence model. Stable `search_depth=basic` with `chunks_per_source` 1-3 derived from `excerpt_count`; native `topic` news routing, `time_range`/`start_date`/`end_date`, strict `include_domains`/`exclude_domains` (`include_domains_mode=filter`), `country` (ISO-to-name mapping, general topic only), `language` with `filter_by_language=true`, and boolean `safe_search` (`Off -> false`, `Moderate|Strict -> true` with `Strict` documented as approximate). Source `content` split on `[...]` into bounded `ProviderSnippet` excerpts with first chunk as snippet; `include_answer`, `include_raw_content`, `include_images`, and `auto_parameters` always false. No result timestamps claimed. Missing credentials stay provider-local; defaults/keyless baseline and MCP tool count unchanged. New `tests/tavily.rs` and `tests/phase5_closure.rs` suites (constraint matrix, determinism/budget, trust, CodeGG compatibility); docs, `architecture/` deep dives, skills, and provider inventories updated. Deferred extensions recorded as insufficient demonstrated need (no `site_map` tool, no Firecrawl Research Index integration).
- **Exa semantic search provider (phase 4).** Opt-in `exa` engine (`POST https://api.exa.ai/search`, `x-api-key: EXA_API_KEY`) for semantic/neural discovery alongside the HTML/SERP sources. Native `freshness`/exact date-range mapping to `startPublishedDate`/`endPublishedDate` (UTC day boundaries for exact ranges; UTC lower bound with omitted end for relative windows), native `includeDomains`/`excludeDomains`, and parseable `publishedDate` timestamps feeding freshness reranking. `contents: { highlights: true }` is sent only on `excerpt_count` demand; `highlights`/`highlightScores` become bounded `ProviderHighlight` excerpts with provider-local scores. Summaries, output schemas, system prompts, full text, subpages, and live crawl are never requested. Missing/invalid credentials stay provider-local; defaults/keyless baseline unchanged. New `tests/exa.rs` suite; docs (`provider-setup`, `config`, `agent-workflows`, `tool-matrix`, `safety`), `architecture/` deep dives, skills, and provider inventories updated.
- **Extractive evidence and fetch controls (phase 2).** `web_search` accepts opt-in `excerpt_count` (0-3); cards carry bounded source-derived `excerpts` (3 per card, 500 chars each, 1,200 total) merged deterministically across providers and sanitized through the trust pipeline, plus a generic `published_at` timestamp that feeds freshness reranking. Brave Search API sends `extra_snippets=true` only on excerpt demand and preserves parseable `age` timestamps. `web_fetch` gains deterministic query-focused `focus` reads (lexical chunk ranking, no extra traversal, additive output) and agent-visible cache controls (`cache_policy`, per-item on `batch_fetch`, plus tightening-only `max_cache_age_seconds`). HTTP 304 is now handled as revalidation rather than a redirect error in conditional fetches, and batch cache stores record `fetched_at` so batch hits work. New `tests/phase2_extract_fetch.rs` suite; docs (`tool-matrix`, `safety`, `provider-setup`, `agent-workflows`), `architecture/` deep dives, skills, and the CodeGG contract updated.

### Changed

- **Architecture docs consolidated into a single location.** All deep dives now live in `architecture/`; the seven unique cross-cutting documents (`codegg-contract.md`, `config.md`, `evidence-workflow.md`, `hardening.md`, `local-workspace.md`, `research.md`, `security.md`) moved from `docs/architecture/`, and eight stale duplicate copies (`overview`, `core`, `meta`, `engines`, `fetch`, `mcp`, `testing`, `commands`) were removed. `docs/architecture/` no longer exists. README and in-doc links updated; `tests/docs_keyless_contract.rs` reads the new path; `Cargo.toml` ships `architecture/**/*.md`.
- **AGENTS.md restructured as an index** into the `architecture/` deep dives with a topic-to-file table; provider-model wording corrected (33 vendored engine structs covering 34 registered provider IDs); exact verified test counts recorded (4,774 passed / 21 ignored with `--all-features`; 4,545 with `--features mock`).
- **Skills refreshed** (`skills/` canonical): architecture skill provider/profile lists corrected and deep-dive index added; dev skill gains `make bench-check`, non-exhaustive-test-table note, and fuzz-target count; release skill Makefile table gains `bench-check`; MCP skill points at the stable response contract.
- **`plans/` removed.** All ~100 phase/milestone/closure planning documents described work shipped through 0.3.7; history preserves them.
- **Docs accuracy fixes.** `docs/test-inventory.md` refreshed against a clean `--all-features` run (per-suite counts corrected, suite-group headers fixed); `docs/provider-setup.md` category count corrected to eight; stale `docs/architecture/` link targets repointed at `architecture/`.

## [0.3.7] - 2026-08-19

### Changed

- **README trimmed and detail offloaded to docs.** README reduced from 137 to 78 lines. PDF extraction, browser rendering, and browser profiles sections moved to new `docs/features.md`. README now links to individual docs for in-depth coverage. All links use relative paths for crates.io compatibility.

## [0.3.6] - 2026-08-15

## [0.3.5] - 2026-07-08

### Fixed

- **Fetch SSRF: `192.0.0.0/24` overblocking corrected.** The `is_blocked_v4` predicate previously blocked all of `192.0.0.0/16` instead of only the documented `192.0.0.0/24` IETF protocol assignment range. The fix adds an `o[2] == 0` check so only `192.0.0.0/24` and `192.0.2.0/24` (TEST-NET-1) are blocked; `192.0.3.1` and other routable addresses in the range are now correctly allowed. (`src/fetch/limits.rs`)
- **Fetch SSRF: removed dead `ipv4_to_u32` and `ipv4_in_cidr` helpers.** These unused private functions were marked `#[allow(dead_code)]` in the address-blocking predicate. Removed for clarity in a security-sensitive code path. (`src/fetch/limits.rs`)
- **Fetch SSRF: exact IPv4 and IPv6 boundary tests.** Added 3 new test sections (n1-n3 for IPv4, o1-o2 for IPv6) covering blocked and allowed addresses at every range boundary. Tests are offline and deterministic. (`tests/fetch_safety.rs`)
- **Rustdoc: fixed private-item link in `provider_diagnostics`.** Replaced a doc link to private `MAX_ERROR_MESSAGE_LEN` with its literal value (`512 chars`) to pass `rustdoc` warnings-as-errors. (`src/meta/provider_diagnostics.rs`)
- **Docs: fixed engine count** in `docs/architecture/meta.md` from 38 to 35 (matching actual vendored engine count).
- **Docs: fixed `health_view` scope description** in `docs/provider-setup.md` — health data is a top-level `health_views` map, not nested per provider.
- **Docs: fixed `skip_code` casing** in `docs/provider-setup.md` — `CooldownActive` corrected to `cooldown_active` to match serialized form.
- **Docs: added `quality_metadata`** to `provider_status` output descriptions in `docs/tool-matrix.md` and `docs/config.md`.
- **Clippy: fixed uninlined format args** in `src/meta/repo_mapper.rs:352` — `format!("{:?}", dir_kind)` changed to `format!("{dir_kind:?}")`.

### Changed

- **Architecture docs: SSRF description updated.** `docs/architecture/fetch.md` now references RFC 1918/6890, loopback, link-local, multicast, documentation ranges, and IPv6 equivalents with a cross-link to `docs/safety.md`, replacing the older narrower shorthand. (`docs/architecture/fetch.md`)

## [0.3.4] - 2026-07-03

### Fixed

- **CI: `web_fetch_structured_warnings_present` test no longer hits external endpoint.** The test now uses `httpmock::MockServer` instead of `httpbin.org`, eliminating 503 failures that blocked CI. (`src/mcp/tools.rs`)
- **CI: `cargo fmt` applied to all test files.** Many test files had formatting inconsistencies that failed `cargo fmt --check` in CI. (`tests/integration.rs`, `tests/schema_identity_registry.rs`, `tests/fetch_safety.rs`, `tests/security_applicability_corpus.rs`, `tests/security_applicability_phase8.rs`, `tests/research_evidence_corpus.rs`, `tests/recipes_next_actions.rs`, `tests/evidence_bundle_handoff.rs`)
- **API-only live mode now requires configured providers.** Config validation now checks that at least one API provider has a resolvable `api_key_env` environment variable, not just `enabled = true`. Deployments with all scrape providers disabled and only unconfigured API providers are now correctly rejected. (`src/core/config.rs`)
- **Fetch-layer warnings no longer misclassify as provider failure.** Unrecognized fetch warning strings now map to the new `FetchWarning` code instead of `ProviderFailed`. Unrecognized search warnings map to `UnknownWarning` instead of `ProviderFailed`. Recognized provider failure patterns (`[timeout]`, `[rate_limited]`) still map to the correct provider-specific codes. (`src/core/warning.rs`)
- **Dispatch executor preserves scheduling priority order.** The bounded parallel dispatcher now uses `Vec::remove()` instead of `swap_remove()` when starting pending jobs, preventing incidental queue reordering. Scheduling respects sorted priority except for intentional provider-capacity bypass. (`src/meta/dispatch.rs`)
- **Deterministic identity uses FNV-1a 64-bit instead of `DefaultHasher`.** Public stable IDs now use an explicit, documented FNV-1a 64-bit hash with a versioned input prefix (`eggsearch-id-v1\0`), removing dependency on stdlib hasher internals. Golden tests protect stable outputs. (`src/core/identity.rs`, `src/core/evidence_bundle.rs`)
- **Documentation examples validated against real MCP schemas.** Fixed `repo_search` examples that used nonexistent `intent` field, `research_search` examples with nonexistent `include_benchmarks` field, and `research_domain` values using PascalCase instead of snake_case. Added deserialization tests to prevent future schema drift. (`docs/agent-workflows.md`, `src/mcp/tools.rs`)
- **Critical: inverted `>=` in security applicability range evaluation.** The legacy `advisory_range::evaluate_range()` accepted versions *below* the floor for `>= X` clauses, producing false `affected` verdicts and false-negative `not_affected` verdicts. The corrected evaluator now uses the tri-state `RangeMatch` (`Affected | NotAffected | Unknown`) and exposes `evaluate_clause()` and `evaluate_range_expression()` as the public API. The legacy `(bool, reasons)` `version_in_ranges()` form is preserved for backward compatibility. (`src/meta/advisory_range.rs`)
- **Security applicability now preserves `Unknown` vs `NotAffected`.** A new internal `RangeMatch::combine()` collapses `NotAffected` to `Unknown` only when another range could not be evaluated, eliminating the silent false-negative path where unparseable range syntax was treated as `not_affected`. The `security_search` MCP tool now maps through the tri-state result so `applicability[].status` reflects `unknown` honestly. (`src/core/security_applicability.rs`, `src/meta/security_search.rs`)
- `version_compare::evaluate_semver_range` no longer treats an empty range string as `Some(true)`; empty input is now `None` (unknown) so callers don't silently accept degenerate ranges.
- `parse_requirements_txt` no longer extracts `>=`-style ranges as installed versions; only `==` pinned versions are kept, so version ranges are not treated as exact installed versions.
- `.github/workflows/ci.yml` adds `--locked` to the test and publish-check jobs (Cargo.lock is committed), grants minimal `contents: read` permissions, and documents its introduction.
- `web_fetch` now validates `timeout_ms > 0` and rejects zero with a clear error. (`src/mcp/tools.rs`)
- `security_search` provider routing now correctly respects profile-level provider selection. (`src/mcp/tools.rs`)

### Added

- **Deterministic cross-tool identity model** (`src/core/identity.rs`): canonical key structs (`SourceKey`, `FetchKey`, `SuggestedFetchKey`, `BatchFetchKey`) and deterministic ID generation functions (`compute_source_id`, `compute_fetch_id`, `compute_suggested_fetch_id`, `compute_batch_fetch_id`) using FNV-1a 64-bit with a versioned input prefix (`eggsearch-id-v1\0`). All tool output types now carry a `stable_id: Option<String>` field for deduplication and cross-referencing across tools. `SourceCard.stable_id` is populated in the adapter; fetch/suggested/batch IDs are populated at construction sites. Evidence bundle builder now delegates to the canonical identity functions. Backward compatible — the existing random UUID `id` field is preserved on `SourceCard`.
- **Code-aware fetch and repo evidence enhancements** (phase 6): `SourceRole` expanded with `Manifest`, `Lockfile`, `SecurityPolicy`, `Ci`, `Generated`, `Vendor` variants for finer-grained file classification. `CodeEvidence` gains `imports` field for top-level import extraction. New `CodeContext` type (`src/core/code_context.rs`) provides lightweight line-oriented extraction of imports and enclosing symbols for Rust, Python, TypeScript/JavaScript, and Go. `repo_fetch` responses now include `code_context` when fetching source files. Suggested fetches now include complementary hints (test files, examples, manifests) for implementation results. Evidence bundles detect missing complementary evidence (tests, examples, manifests, changelogs, security policies) as structured gaps.
- **Gitea/Forgejo configured-host wiring audit**: end-to-end audit with 16 new tests covering unconfigured-host fallback (`web_fetch` does not rewrite arbitrary Gitea-like URLs), configured-host URL construction (`repo_fetch` builds correct browser and raw URLs from `base_url`), Codeberg regression (still rewrites), and capability-flag conservatism (no `tree API` or `repo_map` overclaim).
- **OSV/RustSec affected/fixed range fixtures**: 7 new tests in `advisory_range.rs` covering introduced/fixed events, explicit affected version lists, RustSec patched/unaffected ranges, and unsupported `GIT` range forms.
- **Dependency parser malformed-input audit**: 9 new tests covering invalid lockfile JSON, broken XML, missing versions, YAML workflow `uses:` validation, and Dockerfile variable tags. Plus 3 confidence-semantics tests (`lockfile_yields_high_confidence`, `manifest_yields_high_confidence`, `version_range_not_treated_as_installed`).
- **`assess_version_applicability` regression suite**: `tests/security_applicability_regression.rs` with 19 tests directly exercising the tri-state evaluator to lock in the inverted-`>=` fix and the conservative `Unknown` collapse rules. Plus 5 corpus scenario stubs under `tests/corpus/scenarios/`.
- `tests/security_applicability_regression.rs` covers 19 boundary cases including operator-by-operator `>=`, `>`, `<=`, `<`, `=`, comma-separated intersections, known-then-unknown clause ordering, mixed multi-range outcomes, and the inverted-`>=` regression.
- **Neutral fallback warning codes** (`src/core/warning.rs`): `FetchWarning` for unrecognized fetch-layer warnings and `UnknownWarning` for unrecognized search warnings, replacing incorrect `ProviderFailed` fallback classification. Total warning codes: 58.
- **Refactored MCP argument parsing** (`src/mcp/tools.rs`): extracted `parse_code_host_arg()`, `parse_symbol_kind_arg()`, and `workspace_relative_path_arg()` helpers for cleaner code-host and workspace-host argument handling.

### Changed

- `src/meta/package_resolver.rs` module header now lists all 10 supported ecosystems (CratesIo, PyPI, npm, Go, Maven, NuGet, RubyGems, Packagist, OCI, GitHub Actions) and clarifies that resolution is metadata-only — no dependency solving, no artifact downloading. OCI/GitHub Actions are documented as exact-match only.
- `repo_search` tool description in `src/mcp/tools.rs` lists all 10 package ecosystems (previously only crates.io/PyPI/npm).
- MCP server instructions updated to document workspace host mode for `repo_fetch` (`src/mcp/server.rs`).
- `repo_fetch` documentation updated for workspace host mode (`docs/tool-matrix.md`).

## [0.3.3] - 2026-06-30

### Added

- `research_search` MCP tool for research-oriented multi-source evidence discovery with grouped source-card bundles, subquery transparency, evidence-quality classification, workflow scaffolding, and domain-diverse suggested fetches.
- `security_search` MCP tool for security-oriented retrieval with normalized vulnerability metadata, OSV native lookups, KEV outcome warnings, CWE parsing, source-quality tiering, defensive guidance, and grouped source cards.
- `repo_search` MCP tool for structured repository evidence discovery, search profiles, package-aware query planning, exact-error mode, grouped result bundles, suggested fetches, local workspace integration, and provider-selection telemetry.
- `repo_fetch` and `batch_fetch` MCP tools for bounded repository file retrieval and bounded multi-item fetches over explicit URLs or structured repo locators.
- Structured `web_fetch` document model with HTML blocks, Markdown rendering, code/diff/plain-text renderers, content-type detection, link classification, code-host fetch transforms, and opt-in PDF text extraction behind the `pdf` feature.
- Result-quality metadata on `SourceCard`, group quality summaries, and repo-search uncertainty summaries.
- `provider_status` capability discovery metadata: `server_capabilities`, `tool_capabilities`, `code_hosts`, and `quality_metadata`.
- Stable advisory warning prefixes for capability limits, security context, KEV outcomes, profile degradation, deadline interruptions, and local fetch constraints.
- Bounded parallel subquery dispatch for `repo_search`, `security_search`, and `research_search`. Each (subquery, provider) pair is a dispatch job sorted by priority and executed concurrently with per-provider concurrency limits (`max_concurrent_per_provider`, default 2). Results are sorted deterministically before aggregation. New module: `src/meta/dispatch.rs`.
- `security_search` now uses the parallel dispatcher with security-specific priority levels (advisory > vendor > defensive) instead of a single sequential web search.
- Partial provider failure accounting: a provider is only reported as failed if all its jobs fail; mixed success/failure is reported as a warning instead.
- Config fields `[search].multiquery_concurrency` (default 8) and `[search].multiquery_provider_concurrency` (default 2) for tuning parallel dispatch limits.

### Changed

- Provider capability flags now reflect what adapters actually forward rather than all features an upstream API may support.
- Intent/freshness reranking uses a bounded candidate pool larger than the final requested result count, while preserving generic-search behavior.
- Repo, research, and security grouped responses share grouping/truncation/quality-summary behavior.
- `repo_search` and `research_search` use a single request-level timeout instead of multiplying per-subquery timeouts.
- Security search orchestration, grouping, and suggested-fetch logic are split into dedicated `meta` modules for maintainability.
- README examples and capability documentation were refreshed for the current eight-tool MCP surface.
- Lockfile updated from `quinn-proto` 0.11.14 to 0.11.15 to address RUSTSEC-2026-0185.

### Fixed

- Exact-error redaction now applies consistently to provider-facing exact phrases as well as normalized query text.
- UTF-8-safe snippet truncation in `github_issues` and `github_releases` prevents panics on multibyte text.
- `MockEngine::search` now respects the requested `max_results`, preserving coverage for candidate-pool regressions.
- `candidate_pool_size` is config-aware and cannot panic when the effective result count exceeds the cap.
- Repo grouping no longer misclassifies filenames such as `contest.rs` as tests.
- Security grouping avoids false positives for short exploit markers such as `poc` and truncates consistently at `max_per_group = 0`.
- `web_fetch` JSON now includes `links_seen`, `links_truncated`, `trust_markers`, and `document` where applicable.
- PDF `metadata_only` no longer leaks body content, and PDF document metadata reports real fetch context.
- HTML outline entries are pruned after truncation, and sparse `main`/`article` roots fall back to `body`.
- Code, diff, and plain-text renderers enforce hard character bounds for oversized lines or paragraphs.
- README `batch_fetch` examples now use the tagged item schema accepted by the MCP tool, and `provider_status` docs include the current batch capability fields.

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
