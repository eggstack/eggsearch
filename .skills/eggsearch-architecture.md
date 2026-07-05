# eggsearch Architecture Skill

## Crate Layout

Single library + binary crate. `src/lib.rs` re-exports four modules:

```
src/
  lib.rs           → pub mod core, fetch, mcp, meta
  main.rs          → binary entry (clap + tokio)
  config.rs        → CLI config loader
  commands/        → doctor, search, providers, mcp, fetch
  core/            → types, config, error, query, sanitize, identity, warning
  meta/            → MetadataSearchAdapter, vendored engines, dispatch, health
  fetch/           → HTTP client, HTML rendering, extraction, span selection
  mcp/             → MCP server (rmcp), 10 tool definitions, server state
```

## Adapter Pattern

`MetadataSearchAdapter` wraps all search engines. MCP tools call the adapter, never engines directly. The adapter handles:
- Provider resolution and profile routing
- RRF (reciprocal rank fusion) aggregation
- Sanitization of untrusted text
- Provider health tracking and cooldown
- Bounded parallel subquery dispatch

## Provider Model

`ProviderKind` enum: `HtmlScrape`, `JsonApi`, `ApiKey`, `Local`. Capability flags are conservative — HTML scrapers report `ProviderCapabilities::none()`.

### Known Providers (18 built-ins)
HTML scrapers: `duckduckgo`, `brave`, `startpage`, `yahoo`, `mojeek`
JSON API: `searxng`
API-key: `brave_api`, `github_code`, `github_issues`, `github_releases`, `gitlab_code`, `gitlab_issues`, `gitlab_releases`, `gitea_code`, `gitea_issues`, `gitea_releases`
Native advisory: `osv`
Local: `local_workspace`

## Profiles

`SearchProfile` enum: `generic`, `coding`, `security`, `research`. Profiles influence provider selection. Advisory — unavailable providers are skipped with warnings, not errors.

## Deterministic IDs

SourceCard IDs, suggested fetches, and grouping use content-derived FNV-1a hashes (`src/core/identity.rs`). Key structs: `SourceKey`, `FetchKey`, `SuggestedFetchKey`, `BatchFetchKey`. Versioned input prefix: `eggsearch-id-v1\0`.

## Sanitization (3 Tiers)

1. **Tier 1 (always on):** Control-char strip + length bounds
2. **Tier 2 (default on):** Framing with `<<<EXTERNAL_UNTRUSTED>>>` delimiters
3. **Tier 3 (default on):** Prompt-injection pattern scan

Configured via `[search].sanitize_output` and `[fetch].sanitize_output` (both default `true`).
`web_fetch` supports `extract_mode = "metadata_only"`: HTML returns title/description only, non-HTML suppresses body text, and PDF returns a minimal metadata document when the `pdf` feature is enabled.

## Config

`$XDG_CONFIG_HOME/eggsearch/config.toml`. Root type: `AppConfig` with `SearchSection`, `FetchSection`, `LocalConfig`.

## Transport

MCP over stdio only. Server instructions in `EGGSEARCH_INSTRUCTIONS` constant in `mcp/server.rs`.

## Key Architecture Docs

- `docs/config.md` — config defaults, provider enablement, provider_status semantics
- `docs/safety.md` — trust model, fetch safety, `metadata_only`
- `docs/architecture/codegg-contract.md` — deterministic IDs, warnings, trust model, schema stability
- `docs/agent-workflows.md` — tool call sequences and workflow recipes
- `docs/tool-matrix.md` — compact tool reference table
