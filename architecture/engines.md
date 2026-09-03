# Vendored Search Engines Deep Dive

**Location:** `src/meta/engines/` (40 files: 35 engine implementations plus support modules)
**Purpose:** One self-contained implementation per upstream provider. Internal to the metasearch adapter — engine types never leak past `MetadataSearchAdapter`; callers receive `crate::core::SourceCard` values.

Engines are paired 1:1 with the 36 registered provider IDs (`KNOWN_PROVIDER_IDS` in `src/core/provider.rs`), except that `local_workspace` is served by the local workspace backend (`src/meta/local_backend.rs`), not an engine file.

---

## File Map

| File | Responsibility |
|------|---------------|
| `mod.rs` | `SearchEngine` trait (`search(&EngineSearchRequest)` + `search_batch` with retrieval metadata, `supports_role`), `AdvisoryCapabilities`, engine struct definitions |
| `request.rs` | `EngineSearchRequest` — provider-neutral structured request (query, budgets, intent, safe-search, freshness, date range, domains, language, region, bounded excerpt demand, optional `RepoScope` for native repo filtering) |
| `models.rs` | `SearchResult` and structured metadata payloads (`CodeSearchMetadata`, issue/release metadata) promoted into source cards during conversion; `EngineSearchBatch`/`EngineRetrievalMetadata`/`ScopeIndexStatus` preserve scope-index evidence without touching `SourceCard` |
| `normalizer.rs` | URL canonicalization + tracking-param stripping (`utm_*`, `fbclid`, `gclid`, `msclkid`, `yclid`) before results enter aggregation |
| `error.rs` | `EngineError` |
| `exa.rs` | Exa semantic search (`POST /search`): `numResults`/`type=auto`/`includeDomains`/`excludeDomains`/`startPublishedDate`/`endPublishedDate` mapping, `ProviderHighlight` excerpts with provider-local scores, parseable `publishedDate` timestamps; never requests summaries, output schemas, system prompts, full text, subpages, or live crawl |
| `firecrawl_developer.rs` | Firecrawl Developer Index (`POST /v2/search/developer`): `k`/`passages`/`types`/`repos` mapping, artifact-kind conversion, `ProviderPassage` excerpts, `repos`/`sources` indexed echo |
| `kev.rs` | Shared `KevClient`: fetches/caches the CISA Known Exploited Vulnerabilities catalog (used by the `cisa_kev` engine and by `ServerState` for KEV enrichment) |

---

## The `SearchEngine` Trait (`mod.rs`)

```rust
pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;

    fn search<'a>(&'a self, request: &'a EngineSearchRequest)
        -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>>;

    fn search_batch<'a>(&'a self, request: &'a EngineSearchRequest)
        -> BoxFuture<'a, Result<EngineSearchBatch, EngineError>>; // default: search + empty metadata

    fn supports_role(&self, role: &EvidenceRole) -> bool { true }

    fn advisory_capabilities(&self) -> AdvisoryCapabilities { default }

    fn lookup_advisory<'a>(&'a self, vuln_id: &'a str, timeout: Duration)
        -> BoxFuture<'a, Result<Option<VulnerabilityMetadata>, EngineError>> { Ok(None) }

    fn query_advisories_by_package<'a>(&'a self, ecosystem: &'a str, package: &'a str,
        version: Option<&'a str>, max_results: usize, timeout: Duration)
        -> BoxFuture<'a, Result<Vec<VulnerabilityMetadata>, EngineError>> { Ok(vec![]) }
}
```

Key semantics:

- **Defaulted advisory methods** — engines without advisory support return `Ok(None)` / `Ok(vec![])`. Absence of support is indistinguishable from absence of results at this layer; the adapter's capability partitioning prevents unsupported lookups from being dispatched at all.
- **Defaulted `search_batch`** — returns `search` results with empty `EngineRetrievalMetadata`. Specialist engines (Firecrawl Developer) override to preserve scope-index evidence; all other engines need no changes.
- **Conservative `supports_role`** — defaults to `true` (assume generic search can reach any evidence role); only overridden where an engine provably cannot serve a role. Firecrawl Developer supports only `OfficialDocumentation`, `IssueOrIncidentDiscussion`, `PullRequestOrDesignReview`.
- **`AdvisoryCapabilities { lookup_by_id, query_by_package }`** — declared per engine; drives which native advisory operations are attempted.
- **Timeout is supplied by the adapter**, bounded above by the configured global timeout. Engines never set their own timeouts.
- **Boxed futures** (`Pin<Box<dyn Future + Send>>`) — required for dyn-compatible dispatch across tokio's multi-thread runtime.

---

## Engine Inventory

### Generic web — HTML scrape (keyless)

| Provider ID | Engine | Notes |
|-------------|--------|-------|
| `duckduckgo` | `DuckDuckGoEngine` | HTML parsing |
| `brave` | `BraveEngine` | HTML parsing (distinct from `brave_api`) |
| `startpage` | `StartpageEngine` | HTML parsing |
| `yahoo` | `YahooEngine` | HTML parsing |
| `mojeek` | `MojeekEngine` | HTML parsing |

### Generic web — JSON API

| Provider ID | Engine | Credential / Config |
|-------------|--------|---------------------|
| `searxng` | `SearxngEngine` | Requires `base_url` from config; skipped `[missing_searxng_config]` otherwise |
| `brave_api` | `BraveApiEngine` | API key via `ApiProviderConfig.api_key_env` (default `BRAVE_API_KEY`); natively maps safe-search, freshness/date-range, `search_lang`, `country`, and news intent (`/res/v1/news/search`); sends `extra_snippets=true` only when excerpt demand is present and preserves parseable `age` timestamps; never requests summaries |
| `exa` | `ExaEngine` | API key via `ApiProviderConfig.api_key_env` (`EXA_API_KEY`); semantic retrieval with native freshness/date-range (`startPublishedDate`/`endPublishedDate`), domain filters (`includeDomains`/`excludeDomains`), and `publishedDate` timestamps; sends `contents: { highlights: true }` only on excerpt demand and converts `highlights`/`highlightScores` to `ProviderHighlight` excerpts; never requests summaries, output schemas, system prompts, full text, subpages, or live crawl |
| `firecrawl_developer` | `FirecrawlDeveloperEngine` | Keyless-optional `JsonApi` specialist for the dedicated `POST /v2/search/developer` endpoint (never the generic `/v2/search` SERP). `[search.providers].firecrawl_developer = true` routes keyless; optional `[search.api.firecrawl_developer]` with `FIRECRAWL_API_KEY` attaches `Authorization: Bearer` for higher limits and is never logged. Maps `query`/`k` (clamped 1–20)/`passages` (default 2, max 3)/`types` (Docs→`doc`+`readme`, Issues→`issue`+`pull_request`, else omitted)/`repos` (from `RepoScope`, never parsed from free text). Converts `issue:`/`pull_request:`/`readme:`/`doc:` artifacts via URL classification with deterministic URL-fallback titles; passages become `ProviderPassage` excerpts (never `fetched=true`); `repos`/`sources` indexed echo is preserved as `EngineRetrievalMetadata`, surfaced as stable `scope_unindexed` warnings rather than ordinary zero evidence. Never claims `supports_code_search`. |

### Forge code/issues/releases (API key + optional base URL)

Three hosts × three capabilities = nine engines. Credentials and base URLs resolve through `[search.api_providers]` entries (`api_key_env`, `base_url`); missing keys skip as `[missing_api_key]`, Gitea additionally requires an explicit base URL (`[missing_base_url]`).

| Host | Engines |
|------|---------|
| GitHub | `github_code`, `github_issues`, `github_releases` |
| GitLab | `gitlab_code`, `gitlab_issues`, `gitlab_releases` |
| Gitea/Forgejo | `gitea_code`, `gitea_issues`, `gitea_releases` |

The forge tree/structure APIs used by `repo_map` live separately in `src/meta/forge_adapter.rs` (redirect-free policy, bounded reads, aggregate `ForgeReadBudget`).

### Code search

| Provider ID | Engine | Credential |
|-------------|--------|------------|
| `sourcegraph` | `SourcegraphCodeEngine` | Optional `SOURCEGRAPH_API_KEY` env |

### Security advisories

| Provider ID | Engine | Credential | Advisory Capabilities |
|-------------|--------|------------|----------------------|
| `osv` | `OsvEngine` | keyless | id + package |
| `github_advisory` | `GithubAdvisoryEngine` | via `api_providers` | id + package |
| `nvd` | `NvdEngine` | optional `NVD_API_KEY` env | id (+ package per capabilities) |
| `cisa_kev` | `CisaKevEngine` | keyless; uses shared `KevClient` catalog cache | id |
| `rustsec` | `RustSecEngine` | keyless | package |

### Package registries (all keyless)

| Provider ID | Engine |
|-------------|--------|
| `crates_io` | `CratesIoRegistryEngine` |
| `pypi` | `PypiRegistryEngine` |
| `npm_registry` | `NpmRegistryEngine` |
| `go_pkg` | `GoPkgRegistryEngine` |
| `maven_central` | `MavenCentralRegistryEngine` |
| `nuget` | `NugetRegistryEngine` |
| `rubygems` | `RubygemsRegistryEngine` |
| `packagist` | `PackagistRegistryEngine` |

Registry metadata lookups beyond plain search are also reachable through `package_resolver.rs`, which shares these upstreams.

### Scholarly

| Provider ID | Engine | Credential |
|-------------|--------|------------|
| `openalex` | `OpenAlexEngine` | keyless |
| `crossref` | `CrossRefEngine` | keyless |
| `semantic_scholar` | `SemanticScholarEngine` | optional `SEMANTIC_SCHOLAR_API_KEY` env |

### Local

| Provider ID | Implementation | Notes |
|-------------|---------------|-------|
| `local_workspace` | `LocalWorkspaceBackend` (`local_backend.rs`) | Not an engine file. Bounded file walking over configured roots, `.gitignore`-aware, inventory-cached |

---

## Construction (`build_default_engines` in `adapter.rs`)

```
build_default_engines(
    enabled_providers: &[String],
    user_agent: Option<String>,
    searxng_base_url: Option<String>,
    api_providers: &BTreeMap<String, ApiProviderConfig>,
) -> Result<(EngineList, Vec<SkippedProvider>)>
```

- All keyless engines share one `Arc<reqwest::Client>` built once (shared connection pool, UA override).
- Keyed engines receive their key at construction; keys are read from env at startup and never logged.
- Firecrawl Developer is keyless-optional: `[search.providers].firecrawl_developer = true` builds keyless; an enabled `[search.api.firecrawl_developer]` entry with a resolvable non-empty `api_key_env` attaches the bearer header. Missing/empty optional credentials fall back keyless with a startup warning, never `missing_api_key`.
- Every enabled provider ID resolves to exactly one outcome: a constructed engine or a `SkippedProvider` carrying a typed reason code (`missing_searxng_config`, `missing_api_key`, `missing_base_url`, `unknown_provider`, …). Skips surface later as provider-scoped warnings — never global failures.
- Credential resolution order: direct env vars for Semantic Scholar / Sourcegraph / NVD; `api_key_env`-named env vars for everything in `api_providers`; `optional_api_key()` for keyless-optional providers.

---

## Adding an Engine

1. Create `src/meta/engines/<provider>.rs` implementing `SearchEngine` (reuse defaulted advisory methods unless the upstream serves advisories)
2. Declare the module in `engines/mod.rs`
3. Add the provider ID to `KNOWN_PROVIDER_IDS` and its `ProviderCapabilities` flags in `src/core/provider.rs`
4. Register construction in `build_default_engines()` (and any `api_providers` handling)
5. Wire into profile/provider resolution so profiles can select it
6. Add contract tests under `tests/` (mock feature; no network) and update the docs provider inventory test if it asserts counts

---

[← Back to Overview](overview.md) | [Metasearch Adapter →](meta.md) | [Core Types →](core.md)
