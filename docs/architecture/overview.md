# eggsearch Architecture Overview

**Version:** 0.3.4 · **Rust edition:** 2021 · **MSRV:** 1.88
**Crate type:** Single library + binary (no workspace)

eggsearch is a lightweight MCP (Model Context Protocol) search/fetch server for AI agents. It queries upstream search providers, deduplicates results with reciprocal rank fusion, returns compact source cards, and fetches explicit HTTP(S) URLs on demand with bounded text extraction.

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI (main.rs)                        │
│  clap subcommands: doctor | search | mcp | fetch | providers│
└──────────┬──────────────────────────────────┬───────────────┘
           │                                  │
           ▼                                  ▼
┌──────────────────────┐          ┌───────────────────────────┐
│    commands/          │          │    mcp/                    │
│  CLI subcommand impls │          │  MCP server (rmcp, stdio) │
│  doctor, search,      │          │  10 tool handlers          │
│  fetch, providers     │          │  ServerState, Policy       │
└──────────┬───────────┘          └─────────────┬─────────────┘
           │                                    │
           └──────────────┬─────────────────────┘
                          ▼
              ┌───────────────────────┐
              │    MetadataSearchAdapter│ ← meta/
              │    (orchestrator)       │
              │  • engine dispatch      │
              │  • RRF aggregation      │
              │  • provider health      │
              └───────┬───────────────┘
                      │
        ┌─────────────┼─────────────────┐
        ▼             ▼                 ▼
  ┌──────────┐  ┌──────────┐     ┌───────────┐
  │ engines/ │  │ engines/ │     │ engines/  │
  │  HTML    │  │  JSON    │     │  API-key  │
  │ scrapers │  │  APIs    │     │ providers │
  └──────────┘  └──────────┘     └───────────┘

              ┌───────────────────────┐
              │      fetch/            │
              │  FetchClient (reqwest) │
              │  • SSRF protection     │
              │  • HTML extraction     │
              │  • PDF extraction      │
              │  • span selection      │
              └───────────────────────┘

              ┌───────────────────────┐
              │       core/            │
              │  Pure domain types     │
              │  config, error, query  │
              │  identity, sanitize    │
              │  source_card, warning  │
              └───────────────────────┘
```

---

## Module Map

| Module | Path | Responsibility | Deep Dive |
|--------|------|----------------|-----------|
| **core** | `src/core/` | Pure domain types, config model, error types, identity system, sanitization, warnings, source cards, quality heuristics, security/research/repo types | [core.md](core.md) |
| **meta** | `src/meta/` | Metasearch adapter + 38 vendored search engines. RRF aggregation, query planning, provider health, result grouping | [meta.md](meta.md) |
| **fetch** | `src/fetch/` | HTTP fetch client, HTML content extraction, PDF extraction, span selection, SSRF protection | [fetch.md](fetch.md) |
| **mcp** | `src/mcp/` | MCP server over stdio (rmcp), 10 tool definitions, shared server state, policy enforcement | [mcp.md](mcp.md) |
| **commands** | `src/commands/` | CLI subcommands: doctor, search, mcp, fetch, providers | [commands.md](commands.md) |
| **testing** | `tests/` | Integration, corpus, schema/contract, and documentation contract tests | [testing.md](testing.md) |

---

## MCP Tools (10)

| Tool | Purpose |
|------|---------|
| `web_search` | Live metasearch over configured providers |
| `web_fetch` | Bounded extraction of one HTTP(S) URL |
| `batch_fetch` | Batch fetch over URLs or repo locators |
| `provider_status` | Diagnostic provider configuration report |
| `repo_search` | Structured repository evidence discovery |
| `repo_fetch` | Structured repository file fetch by locator |
| `repo_map` | Repository structure discovery |
| `security_search` | Security vulnerability and advisory search |
| `research_search` | Research-oriented multi-source evidence discovery |
| `build_evidence_bundle` | Package selected evidence into a portable container |

Tools are defined in `src/mcp/tools.rs`. The MCP server uses `rmcp` with `tool_router` proc macros.

---

## Provider Ecosystem

**34 known providers** across 4 kinds:

| Kind | Examples | Capability |
|------|----------|------------|
| `HtmlScrape` | DuckDuckGo, Startpage, Yahoo, Mojeek | Generic web search |
| `JsonApi` | SearXNG, OSV, NVD, CISA KEV, RustSec | Structured APIs |
| `ApiKey` | Brave API, GitHub/GitLab/Gitea code/issues/releases, Semantic Scholar, Sourcegraph | Richer results, requires config |
| `Local` | Local workspace search | Filesystem search |

**Capability flags** (25+): `code_search`, `issue_search`, `release_search`, `security_search`, `scholarly_search`, `package_search`, `repo_structure`, `advisory_search`, and more.

**4 search profiles** influence provider selection:
- `generic` — broad web search (DuckDuckGo, Startpage, Yahoo)
- `coding` — code-focused (adds GitHub, GitLab, Gitea, Sourcegraph)
- `security` — vulnerability-focused (adds OSV, NVD, CISA KEV, RustSec)
- `research` — scholarly (adds OpenAlex, Crossref, Semantic Scholar)

---

## Data Flow

### Search Flow (web_search / repo_search / security_search / research_search)

```
1. Policy check (mode == Live?)
2. Query validation
3. Provider resolution (resolve_providers / resolve_profile_providers)
4. SearchPlan construction (planner.rs / repo_planner.rs / research_planner.rs)
5. Parallel dispatch across engines (dispatch.rs)
6. RRF aggregation (adapter.rs)
7. SourceCard construction with deterministic IDs (identity.rs)
8. Sanitization (sanitize.rs) — control chars, framing, injection scan
9. Quality metadata (quality.rs)
10. Result grouping (grouping.rs / repo_grouping.rs / etc.)
11. Suggested fetches (suggested_fetches.rs / fetch_ranking.rs)
12. Next-action hints (recipe_catalog.rs)
13. Structured warnings (warning.rs)
```

### Fetch Flow (web_fetch / repo_fetch / batch_fetch)

```
1. Policy check (fetch_enabled?)
2. URL validation (limits.rs) — SSRF, localhost, private-network
3. Code-host URL rewriting (code_host_fetch.rs) — GitHub/GitLab/Codeberg → raw
4. HTTP request (reqwest)
5. Redirect revalidation
6. Content detection (detect.rs) — HTML, markdown, code, PDF, plain text
7. Extraction (extract.rs) — text, links, metadata
8. HTML rendering (render/) — blocks, outline, chunks
9. Span selection (span.rs) — symbol/line-range expansion
10. Sanitization (sanitize.rs)
11. Document construction (document.rs)
12. Response with trust markers
```

---

## Key Architectural Patterns

- **Adapter pattern** — `MetadataSearchAdapter` wraps all engines. MCP tools never call engines directly.
- **Deterministic IDs** — FNV-1a 64-bit hashes with versioned prefix (`eggsearch-id-v1\0`). URL canonicalization prevents spurious ID changes.
- **Soft failures** — Adapter returns `WebSearchResponse` with warnings, never errors. Partial provider failures are surfaced as warnings. Non-routable providers include a machine-readable `skip_code` for programmatic diagnostics.
- **Trust model** — All web content is `external_untrusted`. Local content is `local_trusted` (provenance only, not instruction trust). Three sanitization tiers. See [threat-model.md](../threat-model.md) for the full operator threat model.
- **Profile-based routing** — 4 profiles influence provider selection. Degraded profiles fall back to defaults with warnings.
- **Bounded everything** — Timeouts, max_results, max_chars, max_bytes, redirect limits, link caps, import scan limits.
- **No comments** in code (enforced by convention).
- **Feature flags** — `mock` (test engines), `pdf` (PDF extraction), `live-smoke` (network tests).

---

## Build & Verification

```bash
make check            # full CI gate (fmt + clippy + tests + schema-corpus)
cargo fmt --check     # format check
cargo clippy --all-targets --all-features -- -D warnings  # zero warnings
cargo test --all-features  # all tests
cargo test --no-default-features  # no-default compilation
cargo build --release  # release build
cargo publish --dry-run  # pre-publish check
```

---

## Deep Dives

For detailed analysis of each component:

1. [core.md](core.md) — Domain types, config, identity, sanitization, warnings
2. [meta.md](meta.md) — Metasearch adapter, engines, RRF, query planning
3. [fetch.md](fetch.md) — HTTP client, content extraction, SSRF protection
4. [mcp.md](mcp.md) — MCP server, tool definitions, state management
5. [commands.md](commands.md) — CLI subcommands and their implementations
6. [testing.md](testing.md) — Test strategy, CI pipeline, feature flags
7. [codegg-contract.md](codegg-contract.md) — Stable MCP response contract for harness developers
8. [../threat-model.md](../threat-model.md) — Operator threat model, trust boundaries, and safety documentation
