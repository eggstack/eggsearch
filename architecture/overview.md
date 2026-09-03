# eggsearch Architecture Overview

**eggsearch** is a lightweight MCP (Model Context Protocol) metasearch server for AI agents. It queries upstream search providers, deduplicates results with reciprocal rank fusion, returns compact source cards, and fetches HTTP(S) URLs on demand. Transport is MCP over stdio.

Single library + binary crate (not a workspace). All source under `src/`. Version `0.3.7`, edition 2021, MSRV 1.88. Linux/macOS only — Windows is unsupported (Unix-specific APIs: `openat2`, `setsid`, process groups).

This document is the bird's-eye view: what each module is for, how they connect, and where to go for depth. Each component links to a dedicated deep dive in this directory. Use it as the entry point when focusing review on one discrete aspect of the codebase.

---

## System Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLI Entry Point                          │
│                     src/main.rs + src/commands/                 │
│  doctor | search | fetch | providers | mcp stdio | browser-*   │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                        MCP Server                               │
│                  src/mcp/ (rmcp stdio transport)                │
│  10 tools: web_search, web_fetch, batch_fetch, provider_status │
│            repo_search, repo_fetch, repo_map, security_search  │
│            research_search, build_evidence_bundle               │
└───────────────────────────┬─────────────────────────────────────┘
                            │
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
┌─────────────────┐ ┌─────────────┐ ┌─────────────────┐
│  MetadataSearch │ │  FetchClient│ │  LocalWorkspace  │
│    Adapter      │ │  + Cache    │ │    Backend       │
│  src/meta/      │ │  src/fetch/ │ │  src/meta/local* │
└────────┬────────┘ └──────┬──────┘ └─────────────────┘
         │                 │
         ▼                 ▼
┌─────────────────┐ ┌─────────────────┐
│  Vendored       │ │  Browser Render │
│  Search Engines │ │  (optional)     │
│  src/meta/      │ │  src/fetch/     │
│  engines/       │ │  browser/       │
└─────────────────┘ └─────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Core Types                                │
│                     src/core/ (pure, no HTTP)                    │
│  SourceCard, AppConfig, EvidenceBundle, SecuritySearch, etc.    │
└─────────────────────────────────────────────────────────────────┘
```

---

## Module Dependency Flow

```
core ← meta ← mcp ← commands
      ↗
fetch ↗
```

1. **core** is the foundation — pure types, no external service calls
2. **meta** builds on `core` — engines use `reqwest`, adapter orchestrates everything
3. **fetch** is independent of `meta` — HTTP client, extraction, caching, browser rendering
4. **mcp** wires `core` + `meta` + `fetch` into 10 tool endpoints
5. **commands** wires everything for CLI subcommands

---

## Component Index

| Component | Location | One-line Responsibility | Deep Dive |
|-----------|----------|-------------------------|-----------|
| Core domain types | `src/core/` (35 files) | Pure data model: source cards, config, identity, sanitization, evidence types. No HTTP, no engines | [core.md](core.md) |
| Metasearch adapter | `src/meta/` (34 top-level files) | Central orchestrator: planning, bounded dispatch, RRF aggregation, provider health, evidence postprocessing | [meta.md](meta.md) |
| Vendored search engines | `src/meta/engines/` (35 engines + 5 support modules) | One implementation per upstream provider: HTML scrape, JSON API, API key, advisory, registry, scholarly | [engines.md](engines.md) |
| HTTP fetch pipeline | `src/fetch/` (10 top-level files) | Bounded URL fetching: SSRF validation, extraction, span selection, two-tier cache, origin control | [fetch.md](fetch.md) |
| Browser rendering & profiles | `src/fetch/browser/` (8 files) | Optional headless Chrome/Chromium via CDP; persistent origin-scoped login profiles | [fetch.md](fetch.md#browser-rendering-fetchbrowser) |
| HTML rendering | `src/fetch/render/` (8 files) | Structural rendering: blocks, text, markdown, code, CSV, notebooks | [fetch.md](fetch.md#html-rendering-fetchrender) |
| MCP server & tools | `src/mcp/` (5 files) | rmcp ServerHandler with 10 tools, shared state, policy enforcement | [mcp.md](mcp.md) |
| CLI commands | `src/commands/` (8 files) | Subcommand wiring: doctor, search, fetch, providers, mcp stdio, browser-login/profiles | [commands.md](commands.md) |
| Testing infrastructure | `tests/` (50 test binaries), `fuzz/` (22 targets) | Integration, corpus, property, adversarial, fault injection, contract tests; libfuzzer harnesses | [testing.md](testing.md) |
| Build & CI | `Cargo.toml`, `Makefile` | Feature flags, dependency pins, CI pipeline, release gates | [build.md](build.md) |

---

## Cross-Cutting Deep Dives

| Deep Dive | Covers | Document |
|-----------|--------|----------|
| MCP response contract | Stable machine-readable response contract for harness consumers: trust markers, warnings, deterministic IDs, schema evolution rules | [codegg-contract.md](codegg-contract.md) |
| Configuration model | `AppConfig` type model, provider resolution, validation rules, CLI config loading | [config.md](config.md) |
| Evidence & workflow | Evidence bundles, 19-role taxonomy, workflow coverage models, conflict detection, retrieval ledger | [evidence-workflow.md](evidence-workflow.md) |
| Research subsystem | Research planner, claims/gaps/conflicts, depth control, semantic roles | [research.md](research.md) |
| Security subsystem | Advisory lookups (CVE/GHSA/OSV/RustSec/KEV), applicability assessment, severity filtering | [security.md](security.md) |
| Local workspace | Filesystem search backend, inventory cache, git-aware fast path, `openat2` safe opening | [local-workspace.md](local-workspace.md) |
| Hardening & fuzzing | Property-based testing, adversarial corpus, fuzz-target design | [hardening.md](hardening.md) |

Operator-facing documentation lives in `docs/` (config reference, safety, threat model, tool matrix, agent workflows, provider setup). This directory is the contributor-facing deep-dive set.

---

## Module Overviews

### core — pure domain model ([core.md](core.md))

Everything else speaks in these types. Zero external dependencies beyond serialization (`serde`, `schemars`, `thiserror`).

- `source_card.rs` — `SourceCard`, the canonical output type; `SourceKind` classifies URLs into 21 kinds
- `identity.rs` — deterministic FNV-1a content hashes for every stable ID (never random UUIDs)
- `sanitize.rs` — 3-tier sanitization all untrusted text flows through
- `provider.rs` — `ProviderKind`, 24-flag `ProviderCapabilities`, `KNOWN_PROVIDER_IDS` (36)
- Evidence subsystem — roles (19 variants), workflow coverage, conflicts, retrieval ledger, bundles
- Workflow request/response types per tool: web/repo/security/research/local/package

### meta — metasearch orchestration ([meta.md](meta.md))

Wraps all search behind `MetadataSearchAdapter`; callers never touch engines directly.

- Planners turn each tool's request into subqueries (generic, repo, security, research, exact-error)
- `dispatch.rs` fans out subqueries with bounded parallelism, priority queue, panic recovery
- `grouping.rs` deduplicates via reciprocal rank fusion (RRF)
- Evidence postprocessing assigns roles, computes coverage, detects conflicts, records retrieval attempts
- Forge adapter (Gitea/Forgejo APIs), package resolver, local workspace backend + inventory cache

### engines — vendored providers ([engines.md](engines.md))

35 engine structs plus the local workspace backend cover 36 registered provider IDs:

- Generic web: DuckDuckGo, Brave, Startpage, Yahoo, Mojeek, SearXNG, Brave Search API, Exa Semantic Search
- Developer index: Firecrawl Developer (keyless-optional, issues/PRs/READMEs/docs with passages)
- Forge (code/issues/releases × GitHub/GitLab/Gitea), Sourcegraph
- Security advisories: OSV, GitHub Advisory, NVD, CISA KEV, RustSec
- Package registries: crates.io, PyPI, npm, Go, Maven Central, NuGet, RubyGems, Packagist
- Scholarly: OpenAlex, Crossref, Semantic Scholar

All implement one trait (`SearchEngine`) with defaulted advisory methods; unsupported capabilities are explicit.

### fetch — bounded URL fetching ([fetch.md](fetch.md))

Independent of the search path; used by `web_fetch`/`batch_fetch`/`repo_fetch` and suggested-fetch follow-ups.

- `validate_fetch_target()` blocks SSRF vectors before any network I/O
- `FetchClient` enforces byte caps, timeouts, redirect limits, content-type checks
- Two-tier cache: raw bytes (HTTP or rendered DOM) + derived extractions; raw hits can be re-derived without refetching
- `OriginController`: per-origin concurrency, circuit breaker, retry classification
- Browser transport (feature-gated): anonymous ephemeral lifecycle vs profile-scoped persistent sessions

### mcp — protocol surface ([mcp.md](mcp.md))

`EggsearchServer` implements rmcp's `ServerHandler`; `ServerState` holds config, adapter, fetch client, cache, browser lifecycle. Every tool follows the same pattern: validate input → check policy → call adapter/fetch → sanitize → return JSON or `ToolError`.

### commands — CLI surface ([commands.md](commands.md))

Thin wrappers over the same library pieces; `mcp stdio` is how agents run the server. `doctor --probe` diagnoses provider health live.

---

## The 10 MCP Tools

Registration lives in `src/mcp/server.rs` (`#[tool]` attrs); implementations in `src/mcp/tools.rs`.

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `web_search` | Live metasearch over configured providers | `query`, `max_results`, `providers`, `freshness` |
| `web_fetch` | Bounded extraction of one HTTP(S) URL | `url`, `max_chars`, `extract_mode` |
| `batch_fetch` | Batch fetch over URLs or repo locators | `items`, `max_chars_per_item` |
| `provider_status` | Diagnostic provider configuration report | (none) |
| `repo_search` | Structured repository evidence discovery | `query`, `max_results`, `profile` |
| `repo_fetch` | Repository file fetch by locator | `locator`, `line_start`, `line_end`, `symbol` |
| `repo_map` | Repository structure discovery | `owner`, `repo`, `ref`, `path` |
| `security_search` | Security-oriented vulnerability retrieval | `query`, `identifiers`, `package`, `ecosystem` |
| `research_search` | Research-oriented evidence discovery | `query`, `depth`, `domain` |
| `build_evidence_bundle` | Package selected evidence into portable container | `sources`, `fetched` |

---

## Data Flows

### Search flow

```
MCP tool call
  → mcp::tools::run_* validates structured request
  → MetadataSearchAdapter builds a plan (planner / repo_planner / research_planner / error_planner)
  → dispatch_subqueries() fans out to engines (bounded parallel, per-provider limits)
  → engines[].search() returns SearchResult lists
  → group_results() RRF aggregation + deduplication
  → SourceCard conversion (deterministic FNV-1a IDs, sanitization)
  → evidence postprocessing (roles, coverage, conflicts, retrieval ledger)
  → WebSearchResponse back through MCP transport (partial failures are soft)
```

### Fetch flow

```
web_fetch / batch_fetch / repo_fetch call
  → policy check (Live/DryRun/Offline)
  → validate_fetch_target() SSRF/scheme validation
  → cache lookup (raw tier → derived tier; fresh raw hits re-derive locally)
  → miss: FetchClient bounded HTTP fetch (or browser transport if needed)
  → content detection → extraction/rendering → span selection (repo_fetch)
  → sanitized bounded response, cached under scope + params
```

---

## Provider Model

`ProviderKind` enum: `HtmlScrape`, `JsonApi`, `ApiKey`, `Local`.

36 registered providers across 4 search profiles:

| Profile | Providers |
|---------|-----------|
| `generic` | DuckDuckGo, Startpage, Yahoo, Mojeek, Brave (HTML), SearXNG |
| `coding` | + GitHub/GitLab/Gitea Code/Issues/Releases, Sourcegraph |
| `security` | + OSV, GitHub Advisory, NVD, CISA KEV, RustSec |
| `research` | + OpenAlex, Crossref, Semantic Scholar |

Profiles are advisory; unavailable providers are skipped with warnings, never global errors. Per-provider capability flags gate which roles a provider can serve; unsupported requested roles become explicit capability-skip attempts in the retrieval ledger rather than silent omissions.

---

## Cross-Cutting Invariants

These hold everywhere; deep dives assume them.

### Deterministic identity

All stable output IDs are FNV-1a 64-bit content hashes (`src/core/identity.rs`) with versioned prefix `eggsearch-id-v1\0`. Prefixes: `src_`, `fetch_`, `suggested_`, `loc_`, `doc_`, `chunk_`, `span_`, `bundle_`. URLs are canonicalized before hashing (lowercase scheme/host, strip `www.`, default ports, drop fragments, normalize percent-encoding). Never introduce random UUIDs into stable output types.

### Three-tier sanitization

All untrusted text flows through `src/core/sanitize.rs`:

| Tier | When Active | What It Does |
|------|-------------|--------------|
| Tier 1 | Always | Strip control chars + length bound |
| Tier 2 | `sanitize_output = true` | Frame in `<<<EXTERNAL_UNTRUSTED>>>` delimiters |
| Tier 3 | `sanitize_output = true` | Scan for prompt-injection marker patterns |

Production defaults `sanitize_output = true`; tests default to `false`.

### Soft failure semantics

The adapter always returns a `WebSearchResponse`; provider failures become `ProviderFailure` entries and warnings. MCP tools return `Result<serde_json::Value, ToolError>` where errors mean invalid input or internal faults — not "a provider was down".

### Bounded everything

Per-origin concurrency, body byte caps, char caps on output, redirect limits, process groups + timeouts for git execution, aggregate read budgets for forge API walks. Truncation is recorded as evidence (`TruncationEvidence`); candidate-limit saturation alone does not claim truncation.

### Keyless core

No config file and no credential env vars must yield a healthy, useful server. Missing optional credentials cause provider-scoped skips with warnings.

### Additive schema evolution

Response schemas grow only via new optional fields; removal breaks corpus regression tests and downstream agents.

---

## Feature Flags

| Flag | Purpose | Gate |
|------|---------|------|
| `mock` | Test-only mock engine harness | Integration/corpus tests (required) |
| `pdf` | PDF text extraction via `lopdf` | `src/fetch/pdf.rs` |
| `browser` | Headless Chrome/Chromium via CDP | `src/fetch/browser/` |
| `live-smoke` | Live network smoke tests (implies `mock`) | Ignored by default |

Tests never require network access. Live smoke: `cargo test --features live-smoke --test corpus_runner -- --ignored`.

---

## Config Structure

`$XDG_CONFIG_HOME/eggsearch/config.toml` (override with `--config`). Root type is `AppConfig`:

```toml
[search]
mode = "live"                    # live | offline
default_profile = "generic"      # generic | coding | security | research
providers = ["duckduckgo", "brave"]
sanitize_output = true

[fetch]
enabled = true
max_chars = 50000
timeout_ms = 30000
max_redirects = 10
allowed_schemes = ["https"]

[local]
enabled = false
roots = []
max_file_size_bytes = 1048576
```

See [core.md](core.md) for the full type model and `docs/config.md` for operator reference.

---

## Quick Reference

```bash
make check                    # fmt + clippy + no-default compile + all-features tests (= CI)
make release-check            # routine + docs + release build + publish dry-run
cargo test --locked --all-features          # ~4,800 tests, <2 min
cargo test --locked --features mock --test integration   # integration only

cargo run -- mcp stdio        # start MCP server
cargo run -- search "query"   # CLI search
cargo run -- fetch <URL>      # CLI fetch
cargo run -- doctor --probe   # diagnose providers
```

---

[Core Types →](core.md) | [Metasearch Adapter →](meta.md) | [Search Engines →](engines.md) | [HTTP Fetch →](fetch.md) | [MCP Server →](mcp.md) | [CLI Commands →](commands.md) | [Testing →](testing.md) | [Build & CI →](build.md)
