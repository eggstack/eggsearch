---
name: eggsearch-dev
description: Use when building, testing, or contributing to eggsearch. Covers cargo commands, project structure, test conventions, code style, and common pitfalls.
---

# eggsearch Development Skill

## Build & Verify

```bash
# Full CI gate (fmt + clippy + all tests + schema-corpus + docs + publish-check)
make check

# Individual targets
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo test --locked --features mock
cargo test --locked --features pdf
cargo build --release
cargo publish --dry-run --locked

# Hardening tests (property tests + adversarial corpus + fault injection)
make hardening
cargo test --locked --all-features --test property_sanitize --test property_identity --test property_identity2 --test property_identity3 --test property_fetch_limits --test property_fetch_redirects --test property_fetch_url_edge --test property_fetch_response --test property_render_safety --test property_render_code --test property_render_metadata --test property_local_fs --test property_local_fs_extended
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test adversarial_corpus
```

**Critical:** Integration/corpus tests require `--features mock`. Running `cargo test` without features misses most integration tests. CI runs 4 feature combos: `--all-features`, `--no-default-features`, `--features mock`, `--features pdf`.

## Project Structure

Single library + binary crate (not a workspace). Submodules under `src/`:

- `main.rs` — binary entry point (clap, tokio main)
- `lib.rs` — library root, re-exports core/meta/fetch/mcp
- `config.rs` — CLI config loader
- `commands/` — subcommands: doctor, search, providers, mcp, fetch
- `core/` — types, config, error, query, sanitize, identity, warning
- `meta/` — MetadataSearchAdapter + vendored engines + forge tree adapter + local inventory cache
- `fetch/` — HTTP fetch client, HTML rendering, extraction, span selection
- `mcp/` — MCP server (rmcp), tool definitions, server state
- `tests/` — integration, corpus, and contract tests

## Reference Docs

- `README.md` — concise product and surface overview
- `docs/config.md` — config defaults, provider enablement, provider_status semantics
- `docs/safety.md` — trust model, fetch safety, `metadata_only`
- `docs/tool-matrix.md` — stable tool reference
- `docs/agent-workflows.md` — recipe catalog and chaining guidance
- `docs/architecture/codegg-contract.md` — stable contract, IDs, warnings, and trust model
- `docs/release.md` — authoritative release process and pre-release command sequence
- `docs/release-checklist.md` — short operational checklist (links to release.md)

## Adding Tests

- **New file** when testing a distinct subsystem or targeting a specific bug class
- **Extend `integration.rs`** for MCP tool input validation, provider failures, tool response shape
- **Extend `corpus_runner.rs`** for multi-step workflows
- **Unit tests** at bottom of source file for private functions
- Always run `cargo clippy --all-targets --all-features -- -D warnings` after adding
- **Property tests** in `tests/property_*.rs` for pure functions (sanitize, identity, fetch limits, render, local FS, dispatch) using `proptest`
- **Inventory unit tests** in `src/meta/local_inventory_cache.rs` `#[cfg(test)]` for inventory building, invalidation, git fast path
- **Adversarial corpus** in `tests/corpus/adversarial/` for malformed/edge-case inputs (245+ cases across 9 files)
- **Fault injection** in `tests/dispatch_fault_injection.rs` for provider failures, timeouts, concurrency, and health transitions
- **Fuzz harness** in `fuzz/` using `cargo-fuzz` + `libfuzzer` for URL validation, HTML extraction, PDF parsing, sanitization, and document chunking

## Key Conventions

- No comments unless explicitly requested
- Formatter: `cargo fmt` (standard rustfmt)
- Linter: `cargo clippy --all-targets --all-features -- -D warnings` — zero warnings
- Error handling: `core` defines `CoreError`/`CoreResult<T>` via `thiserror`. Adapter returns `WebSearchResponse` (never errors; partial failures are soft). MCP tools return `Result<serde_json::Value, ToolError>`.
- Deterministic IDs: SourceCard IDs, suggested fetches, and grouping use content-derived FNV-1a hashes (`src/core/identity.rs`). Never use random IDs for stable output types.
- Sanitization: All untrusted text flows through `src/core/sanitize.rs` (3 tiers: control-char strip, framing, injection scan). Production defaults `sanitize_output = true`; tests default to `false`.

## Pitfalls

- Forgetting `--features mock` — integration/corpus tests won't compile without it
- Adding random UUIDs to stable output types — use FNV-1a hashes via `src/core/identity.rs`
- Bypassing sanitization — all untrusted text must flow through `sanitize.rs` or `sanitize_field()`
- Hardcoding provider lists — use `resolve_providers()` which validates enabled/known status
- Changing deterministic IDs — breaks regression corpus tests and cross-tool deduplication
- Missing `cargo fmt` — CI will fail on `cargo fmt --check`
