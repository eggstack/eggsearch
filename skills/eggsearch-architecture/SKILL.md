---
name: eggsearch-architecture
description: Use when working with eggsearch internals, understanding crate layout, provider model, adapter pattern, deterministic IDs, sanitization tiers, or config structure.
---

# eggsearch Architecture Skill

Use when working with eggsearch internals, understanding crate layout, provider model, adapter pattern, deterministic IDs, sanitization tiers, or config structure.

## Crate Layout

Single library + binary crate (not a workspace). All source under `src/`:

- `main.rs` — binary entry point (clap, tokio main)
- `lib.rs` — library root, re-exports `core`, `fetch`, `mcp`, `meta`
- `config.rs` — CLI config loader
- `commands/` — subcommands: doctor, search, providers, mcp, fetch, browser_login, browser_profiles
- `core/` — pure domain types, config model, error types, identity, sanitization, warnings, source cards, evidence roles, workflow coverage, conflict, retrieval status
- `meta/` — MetadataSearchAdapter + 34 vendored engines + forge tree adapter + local workspace backend + inventory cache
- `fetch/` — HTTP fetch client, HTML rendering, PDF extraction, span selection, SSRF protection, two-tier raw/derived cache, and optional anonymous or request-scoped persistent browser execution
- `mcp/` — MCP server over stdio (rmcp), 10 tool definitions, server state, policy

## Module Responsibilities

| Module | Key Files | Purpose |
|--------|-----------|---------|
| `core` | `identity.rs`, `sanitize.rs`, `warning.rs`, `evidence_role.rs`, `workflow_coverage.rs`, `conflict.rs`, `retrieval_status.rs`, `evidence_postprocess.rs`, `local.rs` | Canonical data model with zero external dependencies beyond serialization |
| `meta` | `adapter.rs`, `forge_adapter.rs`, `local_backend.rs`, `local_inventory_cache.rs`, `local_inventory.rs`, `dispatch.rs`, `planner.rs` | Search orchestration, RRF aggregation, provider health, forge API client, local workspace |
| `fetch` | `client.rs`, `extract.rs`, `detect.rs`, `limits.rs`, `render/`, `span.rs` | Outbound HTTP, SSRF protection, content extraction, cache, and browser transport |
| `mcp` | `server.rs`, `state.rs`, `tools.rs`, `policy.rs` | MCP protocol, 10 tool handlers, shared state |

## Adapter Pattern

`MetadataSearchAdapter` wraps all search engines. MCP tools call the adapter, never engines directly. The adapter handles:
- Engine dispatch and RRF aggregation
- Provider health tracking (3 failures → cooldown)
- Sanitization and result grouping
- Evidence postprocessing (roles, coverage, conflicts, retrieval summaries)
- Capability-partitioned dispatch (supported roles execute; unsupported roles become explicit capability-skip attempts)
- Provider-scoped advisory operations with explicit `AdvisoryCapabilities` and preserved provider outcomes

## Provider Model

`ProviderKind` enum: `HtmlScrape`, `JsonApi`, `ApiKey`, `Local`.

34 known providers across 4 search profiles:
- `generic` — DuckDuckGo, Startpage, Yahoo
- `coding` — adds GitHub/GitLab/Gitea code/issues/releases
- `security` — adds OSV, NVD, CISA KEV, RustSec
- `research` — adds OpenAlex, Crossref, Semantic Scholar

Profiles are advisory; unavailable providers are skipped with warnings.

## Deterministic Identity System

All stable output types use FNV-1a 64-bit content-derived hashes (`src/core/identity.rs`), never random UUIDs. Key prefixes: `src_`, `suggested_`, `fetch_`, `span_`, `bundle_`, `loc_`, `doc_`, `chunk_`.

URLs are canonicalized before hashing (lowercase scheme/host, strip `www.`, default ports, fragments, normalize percent-encoding). Versioned input prefix: `eggsearch-id-v1\0`.

## Three-Tier Sanitization

All untrusted text flows through `src/core/sanitize.rs`:

| Tier | When Active | What It Does |
|------|-------------|--------------|
| Tier 1 | Always | Strip control chars + length bound |
| Tier 2 | `sanitize_output = true` | Frame in `<<<EXTERNAL_UNTRUSTED>>>` delimiters |
| Tier 3 | `sanitize_output = true` | Scan for 7 prompt-injection marker patterns |

Production defaults `sanitize_output = true`.

## Config Structure

`$XDG_CONFIG_HOME/eggsearch/config.toml`. Root type is `AppConfig` with:
- `SearchSection` — mode, defaults, profiles, provider map, API config
- `FetchSection` — enabled, timeout, byte/char caps, redirect limit, network policy
- `LocalConfig` — enabled, roots, file size/index limits, gitignore/symlink policy

## Evidence Postprocessing

`evidence_postprocess.rs` runs on all result conversion paths:
- Assigns deterministic evidence roles (19 variants) from source metadata
- Computes workflow coverage from the requested model
- Detects conflicts scoped to canonical entities
- Generates retrieval summaries distinguishing success-zero, failure, timeout, rate limit, skip
- Records `TruncationEvidence`; exact candidate-limit saturation is `LimitReachedUnknown` unless truncation is confirmed

Native advisory lookups are scoped to the adapter's resolved provider set. Each
selected provider yields a terminal outcome, including capability unavailable,
deadline, zero results, success, or failure. Deduplicating advisory records does
not remove the underlying attempts.

## Key Invariants

- No comments unless explicitly requested
- Deterministic IDs for all stable output types
- All untrusted text through sanitization
- Partial failures are soft (adapter returns `WebSearchResponse`, never errors)
- MCP tools return `Result<serde_json::Value, ToolError>`
- Additive schema evolution (new optional fields, never removal)
- Anonymous browser rendering uses the warm ephemeral lifecycle; profile-scoped rendering uses the resolved opaque profile's Eggsearch-owned `chrome-data` directory and default browser context for one request
- Raw cache entries distinguish original HTTP bytes from rendered browser DOM; a fresh raw hit may be re-derived locally without another network request
- `NotApplicable` is reserved for operations that do not apply; provider incapability is `SkippedCapabilityUnavailable`
- `RetrievalAttempt` carries an optional `operation_id` field; ledger uniqueness is `(provider_id, operation_id, role)`
- State-aware helpers prefer `RetrievalDimensionState` when present, falling back to `absence_kind` for legacy dimensions
- `not_applicable_job_count` is attempt-level; `not_applicable_count` is dimension-level; subtype counts are subsets, not partitions
