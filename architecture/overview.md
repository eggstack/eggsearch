# eggsearch Architecture Overview

**eggsearch** is a lightweight MCP (Model Context Protocol) metasearch server for AI agents. It queries upstream search providers, deduplicates results with reciprocal rank fusion, returns compact source cards, and fetches HTTP(S) URLs on demand. Transport is MCP over stdio.

Single library + binary crate (not a workspace). All source under `src/`.

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
│  src/meta/      │ │  src/fetch/ │ │  src/meta/local  │
└────────┬────────┘ └──────┬──────┘ └─────────────────┘
         │                 │
         ▼                 ▼
┌─────────────────┐ ┌─────────────────┐
│  34 Vendored    │ │  Browser Render │
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

| Component | Location | Description | Deep Dive |
|-----------|----------|-------------|-----------|
| **Core Types** | `src/core/` (35 files) | Pure domain types, config, error, identity, sanitization, source cards, evidence roles | [core.md](core.md) |
| **Metasearch Adapter** | `src/meta/` (35 files) | Central orchestrator: engine fan-out, RRF aggregation, sanitization, provider health | [meta.md](meta.md) |
| **Search Engines** | `src/meta/engines/` (34 files) | Vendored search engine implementations (HTML scrape, JSON API, API key, security, package, scholarly) | [meta.md](meta.md#search-engine-implementations) |
| **HTTP Fetch** | `src/fetch/` (12 files + 2 subdirs) | HTTP client, content extraction, caching, SSRF protection, origin control | [fetch.md](fetch.md) |
| **Browser Rendering** | `src/fetch/browser/` (8 files) | Headless Chrome/Chromium via CDP (feature-gated `browser`) | [fetch.md](fetch.md#browser-rendering) |
| **HTML Rendering** | `src/fetch/render/` (7 files) | Structural HTML rendering (blocks, text, markdown, CSV, notebooks) | [fetch.md](fetch.md#html-rendering) |
| **MCP Server** | `src/mcp/` (5 files) | MCP protocol handler, 10 tool definitions, server state, policy | [mcp.md](mcp.md) |
| **CLI Commands** | `src/commands/` (8 files) | CLI subcommands: doctor, search, fetch, providers, mcp, browser-login, browser-profiles | [commands.md](commands.md) |
| **Testing** | `tests/` (54 files), `fuzz/` (23 targets) | Integration, corpus, property, fault injection, security, browser, contract tests | [testing.md](testing.md) |
| **Build & CI** | `Cargo.toml`, `Makefile` | Build config, CI pipeline, feature flags, release process | [build.md](build.md) |

---

## Key Abstractions

| Abstraction | Location | Purpose |
|-------------|----------|---------|
| `SourceCard` | `src/core/source_card.rs` | Canonical output type for all search results. Deterministic FNV-1a IDs, URL classification (16 kinds), quality scoring |
| `MetadataSearchAdapter` | `src/meta/adapter.rs` | Central orchestrator: engine fan-out, RRF aggregation, sanitization, intent reranking, provider health |
| `SearchEngine` trait | `src/meta/engines/mod.rs` | 34 implementations (one per engine). Methods: `search()`, `lookup_advisory()`, `supports_role()` |
| `FetchClient` | `src/fetch/client.rs` | HTTP fetch with limits enforcement, SSRF protection, bounded body reading |
| `FetchCache` | `src/fetch/cache.rs` | Two-tier LRU cache (raw + derived). Raw stores original bytes; derived stores extracted content |
| `OriginController` | `src/fetch/origin.rs` | Per-origin concurrency limits, circuit breaker, retry policy |
| `EvidenceBundle` | `src/core/evidence_bundle.rs` | Multi-agent handoff container with trust summary and gaps |
| `FnvHasher` | `src/core/identity.rs` | Deterministic FNV-1a 64-bit hashes for stable IDs (never random UUIDs) |
| `AppConfig` | `src/core/config.rs` | Root config type with `SearchSection`, `FetchSection`, `LocalConfig` |
| `EggsearchServer` | `src/mcp/server.rs` | MCP `ServerHandler` impl with 10 `#[tool]` handlers |
| `ServerState` | `src/mcp/state.rs` | Shared state: config, adapter, fetch client, cache, browser lifecycle |
| `Policy` | `src/mcp/policy.rs` | Controls operations: `Live`, `DryRun`, `Offline` |

---

## The 10 MCP Tools

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
| `build_evidence_bundle` | Package evidence into portable container | `sources`, `fetched` |

---

## Data Flow

```
MCP tool call
  → mcp::tools::run_* receives structured request
  → Core types validate input (query length, URL format, etc.)
  → MetadataSearchAdapter builds search plan
  → dispatch_subqueries() fans out to engines (bounded parallel)
  → engines[].search() (34 implementations)
  → RRF aggregation deduplicates and ranks results
  → SourceCard generation (deterministic FNV-1a IDs, sanitization)
  → Evidence postprocessing (roles, coverage, conflicts, retrieval summaries)
  → Structured JSON response back through MCP transport
```

---

## Provider Model

**ProviderKind** enum: `HtmlScrape`, `JsonApi`, `ApiKey`, `Local`

34 known providers across 4 search profiles:

| Profile | Providers |
|---------|-----------|
| `generic` | DuckDuckGo, Startpage, Yahoo, Mojeek, Brave (HTML), SearXNG |
| `coding` | + GitHub Code/Issues/Releases, GitLab Code/Issues/Release, Gitea Code/Issues/Releases, Sourcegraph |
| `security` | + OSV, GitHub Advisory, NVD, CISA KEV, RustSec |
| `research` | + OpenAlex, Crossref, Semantic Scholar |

Profiles are advisory; unavailable providers are skipped with warnings, not errors.

24 boolean capability flags per provider (`ProviderCapabilities`): `web_search`, `code_search`, `issue_search`, `release_search`, `advisory_search`, `package_lookup`, `scholarly_search`, `local_search`, `repo_fetch`, `repo_map`, etc.

---

## Feature Flags

| Flag | Purpose | Gate |
|------|---------|------|
| `mock` | Test-only mock engine harness | Integration/corpus tests (required) |
| `pdf` | PDF text extraction via `lopdf` | `src/fetch/pdf.rs` |
| `browser` | Headless Chrome/Chromium via CDP | `src/fetch/browser/` |
| `live-smoke` | Live network smoke tests (implies `mock`) | Ignored by default |

---

## Deterministic Identity System

All stable output types use FNV-1a 64-bit content-derived hashes (`src/core/identity.rs`), never random UUIDs.

| Function | Prefix | Purpose |
|----------|--------|---------|
| `source_id()` | `src_` | Deduplication across tools |
| `fetch_id()` | `fetch_` | Cache keys |
| `suggested_fetch_id()` | `suggested_` | Suggested fetch deduplication |
| `batch_fetch_id()` | (none) | Batch fetch grouping |
| `locator_id()` | `loc_` | Repo locator stable IDs |
| `doc_id()` | `doc_` | Document chunk IDs |
| `code_span_id()` | `span_` | Code span IDs |

URLs are canonicalized before hashing (lowercase scheme/host, strip `www.`, default ports, fragments, normalize percent-encoding). Versioned input prefix: `eggsearch-id-v1\0`.

---

## Three-Tier Sanitization

All untrusted text flows through `src/core/sanitize.rs`:

| Tier | When Active | What It Does |
|------|-------------|--------------|
| Tier 1 | Always | Strip control chars + length bound |
| Tier 2 | `sanitize_output = true` | Frame in `<<<EXTERNAL_UNTRUSTED>>>` delimiters |
| Tier 3 | `sanitize_output = true` | Scan for 7 prompt-injection marker patterns |

Production defaults `sanitize_output = true`; tests default to `false`.

---

## Config Structure

`$XDG_CONFIG_HOME/eggsearch/config.toml`. Root type is `AppConfig`:

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

---

## Design Principles

- **No comments** in code unless explicitly requested
- **Deterministic IDs** via FNV-1a hashes (never random UUIDs for stable types)
- **All untrusted text** flows through `src/core/sanitize.rs`
- **Bounded resource usage** — per-origin concurrency, body byte caps, process groups
- **Keyless core** — no config and no credential env vars must produce a healthy server
- **Tests never require network access** (use `--features mock` for integration tests)
- **Partial failures are soft** — adapter returns `WebSearchResponse`, never errors
- **Additive schema evolution** — new optional fields, never removal
- **Forge safety** — `Policy::none()` (redirects rejected), `read_bounded_body()` with hard byte cap

---

## Security Invariants

- SSRF prevention: `validate_fetch_target()` blocks localhost/private IPs
- Bounded reads: never read unbounded response bodies
- Redirect limits: max redirect hops to prevent loops
- Content-type validation: only process expected content types
- Circuit breakers: prevent cascading failures
- Bounded git execution: `run_bounded_command()` with process groups and timeouts
- Forge read budget: `ForgeReadBudget` tracks aggregate bytes across requests

---

## Quick Reference

### Build & Test

```bash
make check                    # fmt + clippy + no-default + all-features tests
make release-check            # routine + docs + release-build + publish-dry-run
cargo test --locked --all-features  # all tests
cargo test --locked --features mock --test integration  # integration only
```

### Run

```bash
cargo run -- mcp stdio        # Start MCP server
cargo run -- search "query"   # CLI search
cargo run -- fetch <URL>      # CLI fetch
cargo run -- doctor --probe   # Diagnose providers
```

---

[← Back to Overview](overview.md) | [Core Types →](core.md) | [Metasearch Adapter →](meta.md) | [HTTP Fetch →](fetch.md) | [MCP Server →](mcp.md) | [CLI Commands →](commands.md) | [Testing →](testing.md) | [Build & CI →](build.md)
