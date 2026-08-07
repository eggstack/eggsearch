# eggsearch Architecture Overview

**eggsearch** is a lightweight MCP (Model Context Protocol) metasearch server for AI agents. It queries upstream search providers, deduplicates results with reciprocal rank fusion, returns compact source cards, and fetches HTTP(S) URLs on demand. Transport is MCP over stdio.

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLI Entry Point                          │
│                     src/main.rs + src/commands/                 │
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

| Component | Location | Deep Dive |
|-----------|----------|-----------|
| **Core Types** | `src/core/` | [core.md](core.md) |
| **Metasearch Adapter & Engines** | `src/meta/` | [meta.md](meta.md) |
| **HTTP Fetch & Extraction** | `src/fetch/` | [fetch.md](fetch.md) |
| **MCP Server** | `src/mcp/` | [mcp.md](mcp.md) |
| **CLI Commands** | `src/commands/` | [commands.md](commands.md) |
| **Testing Infrastructure** | `tests/` | [testing.md](testing.md) |
| **Build & CI** | `Cargo.toml`, `Makefile` | [build.md](build.md) |

---

## Key Abstractions

| Abstraction | Purpose |
|-------------|---------|
| `SourceCard` | Canonical output type for all search results |
| `MetadataSearchAdapter` | Central orchestrator: engine fan-out, RRF aggregation, sanitization |
| `SearchEngine` trait | 34 implementations (one per engine) |
| `EvidenceBundle` | Multi-agent handoff container |
| `FnvHasher` | Deterministic FNV-1a 64-bit hashes for stable IDs |
| `AppConfig` | Root config type with `SearchSection`, `FetchSection`, `LocalConfig` |

---

## Feature Flags

| Flag | Purpose | Gate |
|------|---------|------|
| `mock` | Test-only mock engine harness | Integration/corpus tests |
| `pdf` | PDF text extraction via `lopdf` | `src/fetch/pdf.rs` |
| `browser` | Headless Chrome/Chromium via CDP | `src/fetch/browser/` |
| `live-smoke` | Live network smoke tests (implies `mock`) | Ignored by default |

---

## Data Flow Summary

1. **MCP tool call** → `mcp::tools::run_*` receives structured request
2. **Validation** → Core types validate input (query length, URL format, etc.)
3. **Adapter dispatch** → `MetadataSearchAdapter` builds search plan, fans out to engines
4. **Engine execution** → Parallel HTTP requests to 34 vendored engines
5. **RRF aggregation** → Reciprocal rank fusion deduplicates and ranks results
6. **SourceCard generation** → Deterministic FNV-1a IDs, sanitization, quality scoring
7. **Response** → Structured JSON response back through MCP transport

---

## Design Principles

- **No comments** in code unless explicitly requested
- **Deterministic IDs** via FNV-1a hashes (never random UUIDs for stable types)
- **All untrusted text** flows through `src/core/sanitize.rs`
- **Bounded resource usage** — per-origin concurrency, body byte caps, process groups
- **Keyless core** — no config and no credential env vars must produce a healthy server
- **Tests never require network access** (use `--features mock` for integration tests)
