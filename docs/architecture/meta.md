# meta Module Deep Dive

**Path:** `src/meta/`
**Purpose:** Metasearch adapter + 33 vendored search engines. Handles engine dispatch, RRF aggregation, query planning, provider health tracking, and result grouping.

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
| `adapter.rs` | `MetadataSearchAdapter` — core orchestrator (~4758 lines). Engine construction, search dispatch, RRF aggregation, sanitization, provider health |
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
| `repo_mapper.rs` | Repository map planning and classification for the `repo_map` MCP tool |
| `forge_adapter.rs` | Native remote repository tree retrieval for GitHub, GitLab, Gitea, Forgejo, and Codeberg. Bounded response reading, endpoint safety validation, nested map assembly |
| `security_search.rs` | Security search orchestration: coordinates web search, native advisory lookups, KEV enrichment, grouping |
| `security_suggested_fetches.rs` | Suggested fetch generation for security search result groups |
| `research_evidence_analysis.rs` | Deterministic research evidence analysis: claim extraction, conflict detection, quality classification, gap identification |
| `research_suggested_fetches.rs` | Suggested fetch generation for research search results |
| `research_workflow.rs` | Workflow-aware research scaffolding: dimensions, coverage computation, gap detection, diversity caps |

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
| `local_backend.rs` | Local workspace search backend with auto-build inventory on first search, bounded file walking, scoring, and SymbolBackend trait |
| `local_ignore.rs` | Minimal `.gitignore` matcher |
| `local_inventory.rs` | Git worktree discovery, remote URL normalization, identity matching |
| `local_inventory_cache.rs` | File inventory service: cached entries, Git fast path (`git ls-files -z --cached --others --exclude-standard`), native walking, XXH3 fingerprinting, invalidation. Bounded command runner with timeout, stdout/stderr caps, concurrent pipe drainage, kill-on-timeout watchdog thread |

### Test Support

| File | Responsibility |
|------|----------------|
| `mock.rs` | Mock engine for tests (feature-gated: `mock`) |

---

## Engine Inventory (33 engines)

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

### Shared Engine Infrastructure

| File | Purpose |
|------|---------|
| `engines/models.rs` | `SearchResult`, `ResultMetadata` — shared engine output types |
| `engines/error.rs` | `EngineError` — engine-level error type |
| `engines/normalizer.rs` | Result normalization utilities |
| `engines/kev.rs` | `KevClient` — CISA KEV catalog client with TTL cache (infrastructure used by `CisaKevEngine`, not a `SearchEngine` impl) |

---

## Engine Trait

All engines implement the `SearchEngine` trait:

```rust
trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn search<'a>(
        &'a self,
        query: &str,
        max_results: usize,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>>;
    // Optional:
    fn lookup_advisory(&self, vuln_id: &str, timeout: Duration)
        -> BoxFuture<'_, Result<Option<VulnerabilityMetadata>, EngineError>>;
    fn query_advisories_by_package(
        &self, ecosystem: &str, package: &str, version: Option<&str>,
        max_results: usize, timeout: Duration,
    ) -> BoxFuture<'_, Result<Vec<VulnerabilityMetadata>, EngineError>>;
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
- **NetworkError / HttpStatus**: 30s cooldown
- **Panic / ParseError / Unknown**: 30s cooldown (fallback)

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

## Forge Adapter

The forge adapter (`forge_adapter.rs`) handles native remote repository tree retrieval for GitHub, GitLab, Gitea, Forgejo, and Codeberg without cloning repositories.

### Response Reading

Primary tree and paginated forge API responses are read through `read_bounded_response()` which enforces a hard byte cap during streaming. The function checks `Content-Length` upfront and accumulates bytes incrementally, returning `response_too_large` when the cap is exceeded. Error-body previews (e.g., rate-limit detection, permission-denied diagnostics) are read through `read_error_preview()` with an 8KB cap and control-character sanitization. Default-branch metadata lookups use bounded response reading.

### Endpoint Safety

`validate_base_url()` enforces URL safety before any forge API request:
- Rejects embedded credentials (username/password in URL)
- Rejects HTTPS URLs pointing to localhost, loopback, or private IPv4/IPv6 ranges (when policy disallows)
- Rejects HTTP URLs with API keys except for localhost development use
- Full IPv6 classification: loopback, private (ULA), link-local, documentation, reserved, public

**Architecture decision — ForgeEndpointPolicy:** `ForgeEndpointPolicy` defaults to `allow_loopback: false`, `allow_private_network: false`, `require_https: true`. This makes forge API requests safe by default for general MCP exposure while allowing operator override for localhost development (Gitea, Forgejo). The three independent flags let operators configure exactly which address classes and schemes are permitted without requiring code changes.

**Architecture decision — Redirect handling:** Forge API clients use `Policy::none()`, rejecting all HTTP redirects. Redirects are treated as failure rather than followed. This prevents SSRF via redirect chains and ensures `validate_base_url()` preflight checks are not bypassed. The fetch client also uses `Policy::none()` for outbound requests; redirect revalidation is handled in the fetch layer for user-initiated fetches.

**Architecture decision — DNS pinning:** `validate_base_url()` performs preflight DNS resolution via `std::net::ToSocketAddrs` and classifies every resolved address against the endpoint policy. Literal IP addresses are classified directly. This pins the address set at validation time. **Residual risk:** DNS rebinding can occur between the validation check and the actual HTTP connection, since `reqwest` resolves DNS independently. The preflight check eliminates the most common SSRF vector (direct DNS to private IP) but does not provide TOCTOU-safe DNS pinning. This trade-off is acceptable for the forge adapter's threat model (operator-controlled base URLs, not arbitrary user input).

### Commit Provenance

`commit_sha` in `RepoMapResponse` is set from `resolved_ref` in the forge API response. For GitHub, this is the tree SHA from the Git Trees API (not a commit SHA). For GitLab, this is the commit SHA from the ref resolution endpoint. The `resolved_ref_name` field records the original branch or tag name used for the tree lookup. Object SHAs and commit SHAs are independently represented in the internal `ForgeRawEntry` type but are not propagated to the public `RepoMapEntry` type.

**Architecture decision — ResolvedRepositoryIdentity:** `ResolvedRepositoryIdentity` separates four distinct fields: `requested_ref` (the caller-supplied branch/tag/commit), `resolved_ref_name` (the provider-resolved branch or tag name), `resolved_commit_sha` (the actual commit SHA from provider resolution), and `tree_sha` (the root tree SHA). This prevents treating tree SHAs, blob SHAs, or branch names as commit SHAs in permalink construction. Each forge provider populates these fields from its own resolution endpoint.

### Nested Repository Map Assembly

`build_response()` assembles the repository map from forge tree entries:
- **`entries`**: all retained entries within `max_depth`, including nested paths (e.g. `src/lib.rs`)
- **`root_entries`**: root-level entries only (backward-compatible, no `/` in path)
- Depth calculation: root-level entry = depth 1, `src/lib.rs` = depth 2
- Language hints and manifests are populated from all retained entries, not just root entries
- Entries exceeding `max_entries` are truncated with a `ForgeTreeTruncated` warning

---

**Back to:** [overview.md](overview.md)
