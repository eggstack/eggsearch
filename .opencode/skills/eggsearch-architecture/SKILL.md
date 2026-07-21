---
name: eggsearch-architecture
description: Use when working with eggsearch internals, understanding crate layout, provider model, adapter pattern, deterministic IDs, sanitization tiers, or config structure.
---

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
  meta/            → MetadataSearchAdapter, vendored engines, dispatch, health, forge tree adapter, local inventory cache
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

`ProviderDescriptor` includes `routable: bool` and `skip_reason: Option<String>` fields. A provider is routable only when enabled + fully configured. `SkippedProvider` struct tracks engine build failures with human-readable reasons.

### Known Providers (34 built-ins)
HTML scrapers: `duckduckgo`, `brave`, `startpage`, `yahoo`, `mojeek`
JSON API: `searxng`, `cisa_kev`, `rustsec`, `crates_io`, `pypi`, `npm_registry`, `go_pkg`, `maven_central`, `nuget`, `rubygems`, `packagist`, `openalex`, `crossref`
API-key: `brave_api`, `github_code`, `github_issues`, `github_releases`, `gitlab_code`, `gitlab_issues`, `gitlab_releases`, `gitea_code`, `gitea_issues`, `gitea_releases`, `github_advisory`, `semantic_scholar`, `sourcegraph`
Native advisory: `osv`, `nvd`
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

## Forge Tree Adapter (`src/meta/forge_adapter.rs`)

Native remote repository tree retrieval for `repo_map` without cloning. Supports GitHub, GitLab, Gitea, Forgejo, and Codeberg.

- `fetch_tree()` — async entry point that routes to host-specific adapters
- `build_response()` — converts raw forge entries into provider-neutral `RepoMapResponse`
- GitHub uses Git Trees API with recursive traversal; GitLab uses Repository tree API with pagination; Gitea/Forgejo/Codeberg share a forge-compatible adapter
- All adapters enforce entry, depth, page, byte, and timeout limits
- Authentication via API keys from `[search].api.<provider_id>` config
- Falls back to unauthenticated requests for public repositories

### Forge Response Safety

- `read_bounded_response()` enforces hard byte cap while streaming (not after full buffering)
- `validate_base_url()` validates scheme, embedded credentials, host presence, IP classification, DNS-resolved address classification, and normalized API base path
- HTTPS required for credential-bearing endpoints; loopback/private denied by default
- All forge API responses are read through bounded reader; no `.text().await` or `.bytes().await` without a prior hard bound

## Agent Workflow Integration (Phase 5)

### Evidence Roles and Postprocessing

`evidence_postprocess.rs` populates evidence roles, workflow coverage, retrieval summaries, and structured conflicts on all result conversion paths:

- `assign_evidence_role()` maps source kind/role to `EvidenceRole` (19 variants)
- `compute_evidence_role_summary()` aggregates roles across result sets
- `compute_coverage()` evaluates workflow models against returned evidence
- Conflict detection for version ranges, dates, provider metadata, mutable-vs-pinned, benchmarks

### Evidence Gap and Rationale

`AgentNextAction` in `core/workflow.rs` carries optional `evidence_gap` and `rationale` fields. All 8 evidence gap kinds in `research_evidence_analysis.rs` populate both fields:

- `NoPrimarySource` — "No primary or official source found"
- `NoRecentSource` — "No recent news or discussion found"
- `NoBenchmarkSource` — "No benchmark or performance data found"
- `NoSecuritySource` — "No security considerations found"
- `NoMigrationChangelog` — "No release notes or changelog found"
- `OnlySecondarySources` — "All groups contain only a single source"
- `ConflictingEvidenceUnresolved` — "Sources conflict but no high-confidence claim resolves"
- `VersionContextMissing` — "Query references versions but no release notes found"

## Local Workspace Search (Phase 4)

The local workspace backend uses a layered, cacheable inventory architecture:

- **`local_inventory_cache.rs`** — Builds and caches file inventories with XXH3 fingerprinting
  - Git-aware fast path: `git ls-files` for tracked file enumeration
  - Native fallback: bounded recursive directory walking
  - TTL-based invalidation + per-file lazy validation
  - **Entry revalidation:** `validate_entry()` called before every content read, skipping stale/deleted/oversized entries
  - **Bounded git execution:** `run_bounded_command()` enforces timeout (5s), stdout cap (16MB), stderr cap (64KB)
- **`local_backend.rs`** — Inventory-first search with `SymbolBackend` trait
  - Candidate filtering from inventory before content reads
  - Bounded content reads for filtered candidates
  - Regex-based symbol matching via `RegexSymbolBackend`
- **`local_inventory.rs`** — Git worktree discovery and identity matching
- **`local_ignore.rs`** — Minimal `.gitignore` matcher

Telemetry fields on `LocalSearchResult`: `backend_used`, `inventory_age_ms`, `files_considered`, `files_read`, `bytes_read`, `fallback_reason`.

### Freshness Confidence

`FreshnessConfidence` enum (`high`/`medium`/`low`) in `core/local.rs` computed from inventory age:
- `< 5 minutes` → `High`
- `< 30 minutes` → `Medium`
- `>= 30 minutes` → `Low`

Propagated through `InventoryTelemetry`, `RepoMapResponse`, and `LocalRepoMatch`.

## Key Architecture Docs

- `docs/config.md` — config defaults, provider enablement, provider_status semantics
- `docs/safety.md` — trust model, fetch safety, `metadata_only`
- `docs/architecture/codegg-contract.md` — deterministic IDs, warnings, trust model, schema stability
- `docs/agent-workflows.md` — tool call sequences and workflow recipes
- `docs/tool-matrix.md` — compact tool reference table

## Hardening (Phase 2)

Property-based testing and adversarial corpus validation cover the most security- and reliability-sensitive pure functions:

- **Sanitize module** (`tests/property_sanitize.rs`): strip_control_chars safety, idempotency, bound_text invariants, scan_injection_markers stability, frame structure
- **Identity module** (`tests/property_identity.rs`, `property_identity2.rs`, `property_identity3.rs`): deterministic IDs, correct prefixes/lengths, URL canonicalization idempotency, cross-type prefix uniqueness
- **Fetch limits** (`tests/property_fetch_limits.rs`, `property_fetch_redirects.rs`, `property_fetch_url_edge.rs`): URL scheme/host/policy validation, IP classification, TLD rejection, URL structure
- **Render safety** (`tests/property_render_safety.rs`): strip_control_chars, bound_text, frame, scan_injection_markers
- **Render code** (`tests/property_render_code.rs`): code/diff/plaintext/CSV renderers - bounded output, deterministic, line numbers
- **Local FS** (`tests/property_local_fs.rs`): path handling, binary detection, skip dirs, file size boundaries
- **Dispatch fault injection** (`tests/dispatch_fault_injection.rs`): provider failure/timeout/hang/dedup/concurrency (requires `mock` feature)
- **Adversarial corpus** (`tests/corpus/adversarial/`): 245+ cases across 9 files covering malformed HTML, structured text, URLs, sanitize edge cases, identity edge cases, PDFs, filesystem paths
- **Forge adapter** (`tests/forge_adapter.rs`): endpoint validation, nested maps, resolved ref
- **Bounded response reader** (`fuzz/fuzz_targets/bounded_response_reader.rs`): forge response UTF-8 validation and byte cap enforcement

Run all hardening tests with `make hardening`. Property tests use `proptest` (dev-dependency only, not in runtime graph).
