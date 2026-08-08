# AGENTS.md

## Project Overview

eggsearch is a lightweight MCP search/fetch server for AI agents. It queries upstream search providers, deduplicates with reciprocal rank fusion, returns compact source cards, and fetches HTTP(S) URLs on demand. Transport is MCP over stdio. Single library + binary crate (not a workspace).

## Build & Verification

All commands from project root. CI pins **Rust 1.88** (`rust-version = "1.88"` in Cargo.toml). **Run `make check` to replicate the full CI suite locally.**

```bash
# Routine gate (fmt + clippy + no-default compile check + all-features tests)
make check

# Release gate (routine + docs + release-build + publish-dry-run)
make release-check

# Individual targets
cargo fmt --check            # format check (CI fails on this)
cargo clippy --locked --all-targets --all-features -- -D warnings  # zero warnings required
cargo check --locked --no-default-features  # no-default compilation check
cargo test --locked --all-features    # all tests
cargo build --release        # release build
cargo publish --dry-run --locked  # pre-publish check
```

**Critical: Integration/corpus tests require `--features mock`.** Running `cargo test` without features misses most integration tests. `--all-features` includes `mock` and `pdf`.

Release: `cargo publish --locked` (manual, maintainer-controlled). Pre-publish: `make release-check` passes, version bumped in Cargo.toml, CHANGELOG.md updated. The authoritative release process is in `docs/release.md`.

## Project Structure

```
src/
  main.rs          # binary entry point (clap, tokio main)
  lib.rs           # library root, re-exports core/fetch/mcp/meta
  config.rs        # CLI config loader
  commands/        # subcommands: doctor, search, providers, mcp, fetch, browser_login, browser_profiles
  core/            # types, config, error, query, sanitize, identity, warning
  meta/            # MetadataSearchAdapter + 34 vendored engines + forge adapter + local workspace cache
  fetch/           # HTTP fetch client, HTML rendering, extraction, span selection, browser rendering + profiles
  mcp/             # MCP server (rmcp), tool definitions, server state
tests/             # integration, corpus, contract, property, adversarial, and browser_profiles tests
fuzz/              # cargo-fuzz + libfuzzer targets (23 registered)
```

Read `src/lib.rs` for the module map, then explore submodules as needed.

## CI Pipeline

| Job | What it runs |
|-----|-------------|
| **ci** | `make ci` — fmt, clippy, no-default-features compile check, all-features tests |

## Feature Flags

| Flag | Purpose |
|------|---------|
| `mock` | Test-only mock engine harness (`src/meta/mock.rs`) — **required for integration/corpus tests** |
| `pdf` | PDF text extraction via `lopdf` |
| `browser` | Optional headless Chrome/Chromium rendering via `chromiumoxide` |
| `live-smoke` | Live network smoke tests (implies `mock`); ignored by default |

Tests MUST NOT require network access. Run live smoke tests via: `cargo test --features live-smoke --test corpus_runner -- --ignored`.

## Testing

### Running specific suites

```bash
cargo test --locked --features mock --test integration              # integration only
cargo test --locked --features mock --test corpus_runner            # corpus regression
cargo test --locked --all-features --test security_applicability_regression --test security_applicability_phase8  # standalone
cargo test --locked --all-features --test dispatch_fault_injection  # dispatch fault injection (requires mock)
cargo test --locked --all-features --test adversarial_corpus  # adversarial corpus validation
cargo test --locked --all-features --test keyless_core  # keyless-core runtime contract tests
cargo test --locked --features browser --test browser_profiles     # browser profile management
cargo test --locked --features browser --test browser_transport    # browser transport orchestration
```

### Adding tests

- **New file** when testing a distinct subsystem or targeting a specific bug class
- **Extend `integration.rs`** for MCP tool input validation, provider failures, tool response shape
- **Extend `corpus_runner.rs`** for multi-step workflows
- **Unit tests** at bottom of source file for private functions
- Always run `cargo clippy --locked --all-targets --all-features -- -D warnings` after adding
- **Property tests** in `tests/property_*.rs` for pure functions using `proptest`
- **Adversarial corpus** in `tests/corpus/adversarial/` for malformed/edge-case inputs
- **Fault injection** in `tests/dispatch_fault_injection.rs` for provider failures, timeouts, concurrency
- **Fuzz harness** in `fuzz/` using `cargo-fuzz` + `libfuzzer`

## Code Conventions

- **No comments** unless explicitly requested
- **Formatter:** `cargo fmt` (standard rustfmt). CI checks `cargo fmt --check`.
- **Linter:** `cargo clippy --locked --all-targets --all-features -- -D warnings` — zero warnings.
- **Error handling:** `core` defines `CoreError`/`CoreResult<T>` via `thiserror`. Adapter returns `WebSearchResponse` (never errors; partial failures are soft). MCP tools return `Result<serde_json::Value, ToolError>`.
- **Deterministic IDs:** SourceCard IDs, suggested fetches, and grouping use content-derived FNV-1a hashes (`src/core/identity.rs`). Never use random IDs for stable output types.
- **Sanitization:** All untrusted text flows through `src/core/sanitize.rs` (3 tiers: control-char strip, framing, injection scan). Production defaults `sanitize_output = true`; tests default to `false`.
- **Forge safety:** Forge API client uses `Policy::none()` (redirects rejected). All forge response bodies are read through `read_bounded_body()` with a hard byte cap. `ForgeReadBudget` tracks aggregate bytes across all requests within a single tool invocation; pagination stops on budget exhaustion.
- **Bounded git execution:** `run_bounded_command()` drains stdout/stderr concurrently with independent capped reads, creates a process group via `setsid()`, and kills on timeout. Cap breaches trigger immediate process group termination.
- **Keyless core invariant:** No config and no credential environment variables must produce a healthy, useful server. Missing optional credentials are provider-scoped skips, never global failures.

## Key Architecture

- **Adapter pattern:** `MetadataSearchAdapter` wraps all search engines, handles RRF aggregation, sanitization, and provider health. MCP tools call the adapter, never engines directly.
- **Provider model:** `ProviderKind` enum (`HtmlScrape`, `JsonApi`, `ApiKey`, `Local`). Capability flags are conservative — HTML scrapers report `ProviderCapabilities::none()`.
- **Profiles:** `SearchProfile` (`generic`, `coding`, `security`, `research`) influence provider selection. Profiles are advisory; unavailable providers are skipped with warnings, not errors. Defined in `src/core/repo_search.rs`.
- **Config:** `$XDG_CONFIG_HOME/eggsearch/config.toml`. Root type is `AppConfig` with `SearchSection`, `FetchSection`, and `LocalConfig`.
- **Browser profiles:** Named, origin-scoped persistent browser profiles are created through CLI-only headed login (`browser-login`). Profile metadata lives in `$XDG_DATA_HOME/eggsearch/browser-profiles/<opaque-id>/profile.toml`. Chrome data is in a sibling `chrome-data/` directory. MCP profile-scoped browser fetches launch a request-scoped browser against that exact directory and use its default browser context; anonymous browser fetches retain the warm ephemeral lifecycle. MCP callers select profiles by name; opaque IDs partition the cache. Profiles are disabled by default. Profile cache isolation uses opaque IDs internally; display names are used only in MCP response metadata.
- **Cache:** Two-tier in-memory LRU cache (`src/fetch/cache.rs`). Raw tier stores original bounded HTTP bytes or bounded rendered browser DOM before extraction. A fresh raw hit can create a missing derived representation locally, including changed HTML extraction/link settings and PDF page selection. Derived tier stores extracted/sanitized content keyed by scope + raw hash + extraction params and preserves transport provenance. Profile scope uses opaque IDs, not display names. `invalidate_scope` removes both raw and derived entries. Process-local only; CLI profile removal cannot invalidate the MCP server's cache across processes.
- **Transport:** MCP over stdio only. Server instructions are in `EGGSEARCH_INSTRUCTIONS` constant in `mcp/server.rs`.
- **Windows is unsupported:** The crate uses Unix-specific APIs (`openat2`, `setsid`, process groups). Windows is not included in the CI matrix.

## MCP Tools (10 total)

`web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, `build_evidence_bundle`.

Tools are defined in `src/mcp/tools.rs`. The MCP server uses `rmcp` crate with `tool_router` proc macros.

## Pitfalls

- **Forgetting `--features mock`** — integration/corpus tests won't compile without it
- **Adding random UUIDs** to stable output types — use FNV-1a hashes via `src/core/identity.rs`
- **Bypassing sanitization** — all untrusted text must flow through `sanitize.rs` or `sanitize_field()`
- **Hardcoding provider lists** — use `resolve_providers()` which validates enabled/known status
- **Changing deterministic IDs** — breaks regression corpus tests and cross-tool deduplication
- **Missing `cargo fmt`** — CI will fail on `cargo fmt --check`
- **Bypassing forge response bounds** — all forge API responses must use `read_bounded_response()`; no `.text().await` or `.bytes().await` without a prior hard bound
- **Changing commit_sha semantics** — `commit_sha` must come from `resolved_ref` (actual commit SHA), not from entry object SHA
- **Using opaque rq_* labels as the sole source of role inference** — research planner now provides typed intended roles via `intended_roles`; do not infer roles from `rq_*` subquery IDs
- **Using .first() on intended_roles for failure conversion** — must expand across all roles when converting retrieval failures
- **Silently discarding native advisory lookup errors** — all lookups (CVE, GHSA, OSV, RustSec, KEV) produce `RetrievalAttempt` records in the retrieval ledger
- **Treating limit saturation as proof of truncation** — use `TruncationEvidence`; `LimitReachedUnknown` does not set `truncated` or `has_truncation`
- **Allowing native smoke skips to promote a release** — missing credentials, fixture refs, malformed evidence, or missing provider outputs must fail the manual release workflow
- **Using display names as cache scope** — `CacheScope::Profile(id)` must use the opaque profile ID, never the display name; recreated profiles with the same name get distinct cache scopes
- **Silently ignoring invalid explicit browser paths** — `discover_browser` returns `ExplicitPathInvalid` when a configured path is invalid; it does not fall back to auto-discovery
