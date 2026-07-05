# eggsearch Development Skill

## Build & Verify

```bash
# Full CI gate (fmt + clippy + tests + schema-corpus)
make check

# Individual targets
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo build --release
cargo publish --dry-run
```

**Critical:** Integration/corpus tests require `--features mock`. Running `cargo test` without features misses most integration tests. CI runs 4 feature combos: `--all-features`, `--no-default-features`, `--features mock`, `--features pdf`.

## Project Structure

Single library + binary crate (not a workspace). Submodules under `src/`:

- `main.rs` — binary entry point (clap, tokio main)
- `lib.rs` — library root, re-exports core/meta/fetch/mcp
- `config.rs` — CLI config loader
- `commands/` — subcommands: doctor, search, providers, mcp, fetch
- `core/` — types, config, error, query, sanitize, identity, warning
- `meta/` — MetadataSearchAdapter + vendored engines
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

## Adding Tests

- **New file** when testing a distinct subsystem or targeting a specific bug class
- **Extend `integration.rs`** for MCP tool input validation, provider failures, tool response shape
- **Extend `corpus_runner.rs`** for multi-step workflows
- **Unit tests** at bottom of source file for private functions
- Always run `cargo clippy --all-features -- -D warnings` after adding

## Key Conventions

- No comments unless explicitly requested
- Formatter: `cargo fmt` (standard rustfmt)
- Linter: `cargo clippy --all-features -- -D warnings` — zero warnings
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
