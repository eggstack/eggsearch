# meta Module Deep Dive

**Path:** `src/meta/`
**Purpose:** Metasearch adapter + 38 vendored search engines. Handles engine dispatch, RRF aggregation, query planning, provider health tracking, and result grouping.

---

## Adapter Architecture

The `MetadataSearchAdapter` is the central orchestrator. MCP tools never call engines directly — they always go through the adapter.

```
MCP Tool → MetadataSearchAdapter
              ├── build engines from config
              ├── build search plan (intent-aware query rewriting)
              ├── parallel dispatch across engines
              ├── RRF aggregation
              ├── SourceCard construction
              ├── sanitization
              ├── quality metadata
              ├── result grouping
              ├── suggested fetches
              └── structured warnings
```

### Key Files

| File | Responsibility |
|------|----------------|
| `adapter.rs` | `MetadataSearchAdapter` — core orchestrator (~4700 lines). Engine construction, search dispatch, RRF aggregation, sanitization, provider health |
| `planner.rs` | `build_search_plan()` — intent-aware query rewriting with repo-hint parsing |
| `repo_planner.rs` | `build_repo_search_plan()` — multi-subquery planner for repo_search |
| `research_planner.rs` | `build_research_search_plan()` — research-oriented multi-subquery planner |
| `error_planner.rs` | Exact-error planner for compiler/runtime error messages |
| `dispatch.rs` | Bounded parallel dispatch for multi-subquery searches |
| `response.rs` | `WebSearchResponse`, `ProviderFailure` — response types |
| `provider_diagnostics.rs` | Provider health tracking, routing decisions, capability enforcement telemetry |
| `recipe_catalog.rs` | Built-in recipe catalog and next-action generation |
| `evidence_bundle.rs` | `build_evidence_bundle()` — pure logic for constructing evidence bundles |
| `package_resolver.rs` | Package registry resolver for 10 ecosystems |
| `version_compare.rs` | Version comparison utilities |
| `advisory_range.rs` | Advisory range extraction |
| `dependency_parse.rs` | Dependency/lock file parser |

### Grouping & Suggested Fetches

| File | Responsibility |
|------|----------------|
| `grouping.rs` | Shared deterministic `SourceCard` result grouping helpers |
| `repo_grouping.rs` | Repo-specific result grouping (docs, source, issues, releases) |
| `research_grouping.rs` | Research result grouping |
| `security_grouping.rs` | Security result grouping |
| `suggested_fetches.rs` | Suggested fetch generation using `fetch_ranking` pipeline |
| `fetch_ranking.rs` | Deterministic ranking pipeline for suggested fetch candidates |

### Local Search

| File | Responsibility |
|------|----------------|
| `local_backend.rs` | Local workspace search backend |
| `local_ignore.rs` | Minimal `.gitignore` matcher |
| `local_inventory.rs` | Git worktree discovery, remote URL normalization, identity matching |

### Test Support

| File | Responsibility |
|------|----------------|
| `mock.rs` | Mock engine for tests (feature-gated: `mock`) |

---

## Engine Inventory (38 engines)

### HTML Scrapers

| Engine | File | Purpose |
|--------|------|---------|
| DuckDuckGo | `engines/duckduckgo.rs` | Generic web search |
| Startpage | `engines/startpage.rs` | Privacy-focused web search |
| Yahoo | `engines/yahoo.rs` | Generic web search |
| Mojeek | `engines/mojeek.rs` | Independent web search |
| Brave (HTML) | `engines/brave.rs` | Web search via HTML scraping |

### JSON APIs

| Engine | File | Purpose |
|--------|------|---------|
| SearXNG | `engines/searxng.rs` | Self-hosted metasearch |
| OSV | `engines/osv.rs` | Open Source Vulnerabilities |
| NVD | `engines/nvd.rs` | National Vulnerability Database |
| CISA KEV | `engines/cisa_kev.rs` | CISA Known Exploited Vulnerabilities |
| RustSec | `engines/rustsec.rs` | Rust security advisories |

### API-Key Providers

| Engine | File | Purpose |
|--------|------|---------|
| Brave API | `engines/brave_api.rs` | Brave Search API |
| GitHub Code | `engines/github_code.rs` | GitHub code search |
| GitHub Issues | `engines/github_issues.rs` | GitHub issue search |
| GitHub Releases | `engines/github_releases.rs` | GitHub release search |
| GitLab Code | `engines/gitlab_code.rs` | GitLab code search |
| GitLab Issues | `engines/gitlab_issues.rs` | GitLab issue search |
| GitLab Releases | `engines/gitlab_releases.rs` | GitLab release search |
| Gitea Code | `engines/gitea_code.rs` | Gitea code search |
| Gitea Issues | `engines/gitea_issues.rs` | Gitea issue search |
| Gitea Releases | `engines/gitea_releases.rs` | Gitea release search |
| GitHub Advisory | `engines/github_advisory.rs` | GitHub Security Advisories |
| Semantic Scholar | `engines/semantic_scholar.rs` | Academic paper search |
| Sourcegraph | `engines/sourcegraph.rs` | Code search |

### Package Registries

| Engine | File | Purpose |
|--------|------|---------|
| crates.io | `engines/crates_io.rs` | Rust packages |
| PyPI | `engines/pypi.rs` | Python packages |
| npm | `engines/npm_registry.rs` | JavaScript packages |
| Go Proxy | `engines/go_pkg.rs` | Go packages |
| Maven Central | `engines/maven_central.rs` | Java packages |
| NuGet | `engines/nuget.rs` | .NET packages |
| RubyGems | `engines/rubygems.rs` | Ruby packages |
| Packagist | `engines/packagist.rs` | PHP packages |

### Scholarly

| Engine | File | Purpose |
|--------|------|---------|
| OpenAlex | `engines/openalex.rs` | Academic metadata |
| Crossref | `engines/crossref.rs` | DOI-based scholarly search |

### Other

| Engine | File | Purpose |
|--------|------|---------|
| KEV | `engines/kev.rs` | CISA KEV catalog client with TTL cache |

### Shared Engine Infrastructure

| File | Purpose |
|------|---------|
| `engines/models.rs` | `SearchResult`, `ResultMetadata` — shared engine output types |
| `engines/error.rs` | `EngineError` — engine-level error type |
| `engines/normalizer.rs` | Result normalization utilities |

---

## Engine Trait

All engines implement the `SearchEngine` trait:

```rust
trait SearchEngine {
    fn search(&self, query: &WebSearchRequest) -> EngineResult<Vec<SearchResult>>;
    // Optional:
    fn lookup_advisory(&self, id: &str) -> EngineResult<Option<SecurityIdentifier>>;
    fn query_advisories_by_package(&self, coord: &PackageCoordinate) -> EngineResult<Vec<SecurityIdentifier>>;
}
```

Engine types are classified by `ProviderKind`:
- `HtmlScrape` — HTML scraping with conservative capabilities
- `JsonApi` — Structured JSON APIs
- `ApiKey` — Requires authentication, richer results
- `Local` — Filesystem-based search

---

## RRF Aggregation

Reciprocal Rank Fusion combines results from multiple engines:

```
score(d) = Σ 1/(k + rank_i(d))  for each engine i
```

Where `k` is a constant (typically 60). This ranks documents that appear highly across multiple engines higher, while still surfacing unique results from individual engines.

---

## Query Planning

The planner rewrites queries based on intent:

1. **Intent detection** — Parse query for intent hints (code, issues, releases, security, etc.)
2. **Repo hints** — Extract `repo:owner/name` patterns from query text
3. **Subquery generation** — Generate multiple subqueries for multi-engine dispatch
4. **Provider selection** — Choose engines based on profile and capabilities
5. **Freshness routing** — Route freshness hints to engines that support them

---

## Provider Health

The adapter tracks per-provider health via `ProviderHealthRegistry` (process-local, `Mutex<BTreeMap>`):

### Health Recording

After every search call (`web_search`, `repo_search`, `security_search`, `research_search`), the adapter records success or failure per provider:
- **Success**: resets `consecutive_failures` to 0, clears cooldown, records latency.
- **Failure**: increments `consecutive_failures`, records failure class and message (bounded to 512 chars).

### Cooldown

After 3 consecutive failures (`COOLDOWN_THRESHOLD`), a provider enters cooldown:
- **Rate-limited**: 60s cooldown
- **Timeout**: 15s cooldown
- **Transport/NetworkError**: 30s cooldown
- **Panic**: 30s cooldown (mapped from dispatch panics)

Cooldown is cleared immediately on any success. Cooled-down providers are skipped for profile/default routing but **never** skipped for explicitly requested providers.

### Failure Classes

`FailureClass` enum: `Timeout`, `HttpStatus`, `ParseError`, `NetworkError`, `RateLimited`, `Panic`, `Unknown`.

Panics are detected by matching `"panicked during dispatch"` in `EngineError::NetworkError` reason strings (produced by `catch_unwind` in adapter/dispatch).

### Health Surfaces

| Surface | What it returns |
|---------|----------------|
| `provider_status` tool | `health_views` (per-provider `ProviderHealthView` with status, error class/message, timestamps, cooldown) + `health` (snapshots) |
| CLI `providers` command | Health column per provider (JSON and table mode) |
| `provider_diagnostics.rs` | `ProviderHealthView` type with `status`, `consecutive_failures`, `last_error_class`, `last_error_message`, `cooldown_until`, `cooldown_reason`, `last_latency_ms`, `last_success_at`, `last_failure_at` |

### Health Statuses

`ProviderHealthStatus` enum: `Healthy`, `Degraded`, `Cooldown`, `Unknown`.

- `Healthy`: at least one success recorded, no active cooldown
- `Degraded`: consecutive failures > 0 but below threshold
- `Cooldown`: 3+ consecutive failures, cooldown active
- `Unknown`: no health data recorded yet

---

**Back to:** [overview.md](overview.md)
