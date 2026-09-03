# Metasearch Adapter Deep Dive

**Location:** `src/meta/` (34 top-level files, plus `engines/` with 35 engine implementations and 5 support modules)
**Purpose:** Central orchestrator for all search operations. Wraps vendored search engines, handles RRF aggregation, sanitization, provider health, and multi-subquery dispatch.

---

## Module Map

| File | Responsibility |
|------|---------------|
| `adapter.rs` | `MetadataSearchAdapter` — central orchestrator: engine fan-out, RRF aggregation, sanitization, intent reranking, provider health, local domain enforcement, web capability telemetry |
| `dispatch.rs` | `dispatch_subqueries()` — bounded parallel executor with priority queue, global/per-provider concurrency limits, panic recovery; all jobs use `EngineSearchRequest` (including optional `RepoScope`) and return `EngineSearchBatch` retrieval metadata |
| `planner.rs` | `build_search_plan()`, `SearchPlan` — transforms `WebSearchRequest` into provider-specific queries while preserving date/domain/language/region constraints for native parameters |
| `response.rs` | `WebSearchResponse`, `ProviderFailure` |
| `grouping.rs` | RRF aggregation, deduplication, `AggregatedResult` merging |
| `repo_planner.rs` | `build_repo_search_plan()`, `RepoSearchPlan`, `RepoSubquery` — multi-subquery generation from repo hints |
| `repo_grouping.rs` | `classify_group()`, `group_results()` — categories (docs, registry, code, issues, releases) |
| `repo_mapper.rs` | Repository structure discovery: tree listing, important file/directory classification |
| `research_planner.rs` | `build_research_search_plan()`, `ResearchSearchPlan` — multi-depth research subquery generation |
| `research_grouping.rs` | Research result grouping by evidence quality and source class |
| `research_evidence_analysis.rs` | `analyze_research_evidence()` — evidence quality classification |
| `research_suggested_fetches.rs` | Suggested fetch ranking for research workflows |
| `research_workflow.rs` | Research workflow scaffolding (architecture_decision, library_comparison, etc.) |
| `security_search.rs` | Security search orchestration: advisory lookups, CVE/GHSA/RustSec/OSV |
| `security_grouping.rs` | Security result grouping and tier classification |
| `security_suggested_fetches.rs` | Suggested fetch ranking for security results |
| `error_planner.rs` | `build_error_plan()` — exact-error mode: parses compiler/runtime errors, generates targeted subqueries |
| `evidence_bundle.rs` | `build_evidence_bundle()` — pure logic for constructing evidence bundles |
| `fetch_ranking.rs` | Deterministic ranking pipeline for suggested fetch candidates |
| `suggested_fetches.rs` | Generic suggested fetch generation |
| `forge_adapter.rs` | Forge API client for Gitea/Forgejo (with `Policy::none()`, `read_bounded_body()`, `ForgeReadBudget`) |
| `local_backend.rs` | `LocalWorkspaceBackend` — bounded file walking, scoring, SourceCard conversion |
| `local_inventory.rs` | `discover_local_repos()`, `LocalRepoIdentity` — Git worktree discovery, remote URL normalization |
| `local_inventory_cache.rs` | Fast in-memory file inventory cache for local workspace search |
| `local_ignore.rs` | Minimal `.gitignore` matcher |
| `safe_open.rs` | Race-resistant file opening via component-wise path walking |
| `package_resolver.rs` | Bounded HTTP lookups for package registries (crates.io, PyPI, npm, Go, Maven, NuGet, RubyGems, Packagist) |
| `dependency_parse.rs` | Dependency/lock file parser for extracting package coordinates |
| `advisory_range.rs` | Advisory affected/fixed range extraction |
| `version_compare.rs` | Version comparison utilities for package ecosystems |
| `provider_diagnostics.rs` | `ProviderHealthRegistry`, `ProviderHealthSnapshot`, `ProviderRoutingDecision`, `CapabilityEnforcementTelemetry` |
| `recipe_catalog.rs` | Built-in recipe catalog and capability-to-recipe gating |
| `mock.rs` | Test-only mock engine harness (feature-gated `mock`) |

---

## MetadataSearchAdapter (`adapter.rs`)

The central orchestrator. Methods:

| Method | Purpose |
|--------|---------|
| `web_search()` | Live metasearch over configured providers |
| `repo_search()` | Structured repository evidence discovery |
| `repo_fetch()` | Structured repository file fetch |
| `repo_map()` | Repository structure discovery |
| `security_search()` | Security-oriented retrieval with normalized vulnerability metadata |
| `research_search()` | Research-oriented multi-source evidence discovery |
| `lookup_advisory()` | Single advisory lookup (CVE, GHSA, OSV, RustSec, KEV) |
| `provider_status()` | Diagnostic report of configured providers |

### Internal Flow

```
Request
  → build_search_plan() / build_repo_search_plan() / etc.
  → dispatch_subqueries()  (bounded parallel execution)
  → engines[].search()     (per-provider implementations)
  → group_results()        (RRF aggregation, deduplication)
  → sanitize output        (3-tier sanitization)
  → SourceCard[] response
```

---

## Search Engine Implementations (`engines/`)

Per-engine inventory, the `SearchEngine` trait contract, credential resolution, and guidance for adding engines live in the dedicated deep dive: [engines.md](engines.md).

### Engine Categories

| Category | Engines | Transport |
|----------|---------|-----------|
| **HTML Scrape** | DuckDuckGo, Brave, Startpage, Yahoo, Mojeek | HTML parsing |
| **JSON API** | SearXNG, Firecrawl Developer (keyless-optional specialist) | JSON API |
| **API Key** | Brave API, GitHub Code/Issues/Releases, GitLab Code/Issues/Releases, Gitea Code/Issues/Releases, Semantic Scholar, Sourcegraph | Authenticated API |
| **Security** | OSV, GitHub Advisory, NVD, CISA KEV, RustSec | Advisory APIs |
| **Package Registries** | crates.io, PyPI, npm, Go, Maven Central, NuGet, RubyGems, Packagist | Registry APIs |
| **Scholarly** | OpenAlex, CrossRef, Semantic Scholar | Academic APIs |

### SearchEngine Trait

```rust
trait SearchEngine {
    fn search(&self, request: &EngineSearchRequest) -> Result<Vec<SearchResult>>;
    fn search_batch(&self, request: &EngineSearchRequest) -> Result<EngineSearchBatch>;
    fn lookup_advisory(&self, ...) -> Result<VulnerabilityMetadata>;
    fn query_advisories_by_package(&self, ...) -> Result<Vec<VulnerabilityMetadata>>;
    fn supports_role(&self, role: EvidenceRole) -> bool;
    fn advisory_capabilities(&self) -> AdvisoryCapabilities;
}
```

`EngineSearchRequest` (`engines/request.rs`) is the single structured request contract: query, max-results, timeout, intent, safe-search, freshness, exact date range, include/exclude domains, language, region, bounded excerpt demand (`excerpt_count`, default 0), and optional provider-neutral `RepoScope` (`owner`/`repo` from `repo_search` resolved identity, never reparsed from free text). Direct web fan-out uses `from_web_request()`; multiquery dispatch carries `repo_scope`/`excerpt_count` per `DispatchJob` and calls `search_batch()` so Firecrawl scope-index evidence survives as `EngineRetrievalMetadata`.

`SearchResult`/`AggregatedResult` (`engines/models.rs`) carry optional `excerpts` (`Vec<SourceExcerpt>`) and a provider-neutral `published_at` timestamp alongside title/URL/snippet/engine/metadata. `EngineSearchBatch` pairs results with `EngineRetrievalMetadata { scope_index }`; `repo_search` translates unindexed scopes into stable `scope_unindexed` warnings so "scope not indexed" is never mislabeled as ordinary zero evidence. `aggregate_rrf` merges excerpts deterministically (per-provider score order, normalized-text dedup including the primary snippet, hard caps) and keeps the first valid timestamp in sorted engine order. `web_search` clears unrequested excerpts when demand is zero; `convert_aggregated` sanitizes excerpt text through the normal trust pipeline (500 chars per excerpt, 1,200 total per card) and surfaces `published_at` additively in `SourceMetadata` without touching stable IDs. Freshness reranking consumes the generic timestamp before specialist issue/release metadata.

### Provider Model

- `ProviderKind` enum: `HtmlScrape`, `JsonApi`, `ApiKey`, `Local`
- `ProviderCapabilities` — 24 boolean flags per provider
- `KNOWN_PROVIDER_IDS` — 36 registered provider identifiers
- `CredentialRequirement` — `None`/`Optional`/`Required`; `firecrawl_developer` is the only `Optional` provider (keyless routes, key raises limits); `exa` is `Required` (opt-in semantic search with native freshness/domain/timestamp support)

---

## Dispatch System (`dispatch.rs`)

Bounded parallel executor for (subquery, provider) jobs:

- **Priority queue** — high-priority subqueries first
- **Global concurrency limit** — prevents overwhelming the system
- **Per-provider concurrency limits** — prevents rate limiting
- **Panic recovery** — individual engine failures don't crash the system
- **Timeout handling** — bounded execution time per subquery

---

## RRF Aggregation (`grouping.rs`)

Reciprocal Rank Fusion for result deduplication:

1. Collect results from all engines
2. Normalize scores across engines
3. Compute RRF scores: `score = Σ 1/(k + rank_i)` for each engine `i`
4. Deduplicate by URL/title similarity
5. Sort by RRF score

---

## Provider Health (`provider_diagnostics.rs`)

Tracks provider health in real-time:

- `ProviderHealthRegistry` — aggregates health snapshots
- `ProviderHealthSnapshot` — success rate, latency, error counts
- `ProviderRoutingDecision` — whether to use/skip a provider
- `CapabilityEnforcementTelemetry` — tracks capability-based routing

---

## Local Workspace Search

### LocalBackend (`local_backend.rs`)
- Bounded file walking with `.gitignore` support
- Fuzzy file matching and scoring
- SourceCard conversion for local files

### LocalInventory (`local_inventory.rs`)
- Git worktree discovery
- Remote URL normalization (GitHub/GitLab/Codeberg)
- Identity matching for local repos

### SafeOpen (`safe_open.rs`)
- Race-resistant file opening
- Component-wise path walking
- Prevents path traversal attacks

---

## Forge Adapter (`forge_adapter.rs`)

API client for Gitea/Forgejo instances:

- `Policy::none()` — redirects rejected
- `read_bounded_body()` — hard byte cap on responses
- `ForgeReadBudget` — aggregate byte tracking across requests

---

## Package Resolver (`package_resolver.rs`)

Bounded HTTP lookups for package registries:

- **Supported ecosystems:** crates.io, PyPI, npm, Go, Maven, NuGet, RubyGems, Packagist
- **Bounded requests** — timeout and size limits
- **Dependency parsing** — extract coordinates from lock files

---

## Error Planning (`error_planner.rs`)

Exact-error search mode:

1. Parse compiler/runtime error messages
2. Extract error code, message, stack frames
3. Generate targeted subqueries
4. Rank by error relevance

---

[← Back to Overview](overview.md)
