---
name: eggsearch-dev
description: Use when building, testing, or contributing to eggsearch. Covers cargo commands, project structure, test conventions, code style, and common pitfalls.
---

# eggsearch Development Skill

Use when building, testing, or contributing to eggsearch. Covers cargo commands, project structure, test conventions, code style, and common pitfalls.

## Quick Commands

```bash
# Routine verification gate
make check

# Individual targets
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features
cargo build --release
cargo publish --dry-run --locked
```

## Critical: Feature Flags

| Flag | Purpose | Required For |
|------|---------|-------------|
| `mock` | Test-only mock engine (`src/meta/mock.rs`) | Integration/corpus tests |
| `pdf` | PDF text extraction via `lopdf` | PDF tests |
| `browser` | Headless Chrome/Chromium rendering via `chromiumoxide` | Browser rendering tests |
| `live-smoke` | Live network smoke tests (implies `mock`) | Manual only |

**Integration/corpus tests require `--features mock`.** Running `cargo test` without features misses most tests.

## Test Locations

| Location | Feature Gate | Purpose |
|----------|-------------|---------|
| `src/*/mod.rs` | Varies | Unit tests |
| `tests/integration.rs` | `mock` | MCP tool contracts |
| `tests/corpus_runner.rs` | `mock` | Multi-step workflow regression |
| `tests/property_*.rs` | None | Property tests (sanitize, identity, fetch, render, local FS) |
| `tests/forge_adapter.rs` | None | Forge adapter unit tests |
| `tests/dispatch_fault_injection.rs` | `mock` | Provider failure/timeout/concurrency |
| `tests/adversarial_corpus.rs` | None | Malformed input validation |
| `tests/docs_*.rs` | None | Documentation contract tests |
| `tests/schema_identity_registry.rs` | None | Schema + deterministic ID fixtures |

## Running Specific Suites

```bash
cargo test --locked --features mock --test integration
cargo test --locked --features mock --test corpus_runner
cargo test --locked --all-features --test forge_adapter
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test adversarial_corpus
cargo test --locked --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names --test docs_safety_vocabulary --test static_guards
```

## Code Style

- **No comments** unless explicitly requested
- **Formatter:** `cargo fmt` (standard rustfmt)
- **Linter:** `cargo clippy --all-targets --all-features -- -D warnings` — zero warnings
- **Error handling:** `CoreError`/`CoreResult<T>` via `thiserror` in core. Adapter returns `WebSearchResponse` (never errors). MCP tools return `Result<serde_json::Value, ToolError>`.

## Adding Tests

- New file for distinct subsystems or specific bug classes
- Extend `integration.rs` for MCP tool input validation, provider failures, tool response shape
- Extend `corpus_runner.rs` for multi-step workflows
- Unit tests at bottom of source file for private functions
- Property tests in `tests/property_*.rs` using `proptest`
- Adversarial corpus in `tests/corpus/adversarial/`
- Always run `cargo clippy --all-targets --all-features -- -D warnings` after adding

## Common Pitfalls

- **Forgetting `--features mock`** — integration/corpus tests won't compile
- **Adding random UUIDs** to stable output types — use FNV-1a hashes via `src/core/identity.rs`
- **Bypassing sanitization** — all untrusted text must flow through `sanitize.rs`
- **Hardcoding provider lists** — use `resolve_providers()` which validates enabled/known status
- **Changing deterministic IDs** — breaks regression corpus tests and cross-tool deduplication
- **Missing `cargo fmt`** — CI will fail on `cargo fmt --check`
- **Bypassing forge response bounds** — all forge API responses must use `read_bounded_response()`; no `.text().await` or `.bytes().await` without a prior hard bound
- **Changing commit_sha semantics** — `commit_sha` must come from `resolved_ref` (actual commit SHA), not from entry object SHA
- **Collapsing provider-scoped advisory outcomes** — preserve every selected provider's identity, zero result, capability skip, deadline, and error attempt
- **Treating exact candidate-limit saturation as confirmed truncation** — use `TruncationEvidence::LimitReachedUnknown` unless missing data is proven
- **Using fallback smoke results as native release evidence** — native smoke requires credentials and fixture configuration; these are maintainer-only diagnostics, not release evidence
- **Running no-default full test suite in routine gate** — the routine gate runs `cargo check --locked --no-default-features` (compile-only); full test pass uses `--all-features` only
- **Using anonymous browser state for a selected profile** — profile-scoped browser fetches must use the profile manager's opaque-ID-resolved `chrome-data` directory and the configured browser runtime values
- **Skipping raw-cache re-derivation** — a fresh raw hit with a derived miss must run the shared extraction pipeline locally rather than issuing another network request

## Fuzz Targets

Fuzz targets live under `fuzz/fuzz_targets` and are registered in `fuzz/Cargo.toml`.
