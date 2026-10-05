# Search Engines Deep Dive

**Location:** `src/meta/engines/` (49 files: 43 engine implementations plus 6 support modules)
**Purpose:** One self-contained implementation per upstream provider. Engines are internal to the
metasearch adapter — engine types never leak past `MetadataSearchAdapter`; callers receive
`crate::core::SourceCard` values. MCP tools call the adapter, never engines directly.

The 43 engines plus the local workspace backend cover the 44 registered provider IDs
(`KNOWN_PROVIDER_IDS` in `src/core/provider.rs`); `local_workspace` is served by the local
workspace backend (`src/meta/local_backend.rs`), not an engine file.

---

## File Map

| File | Responsibility |
|------|---------------|
| `mod.rs` | `SearchEngine` trait, `AdvisoryCapabilities`, engine struct definitions, shared transport builders (`build_http_client`, `build_http_client_with_egress`, `engine_timeout`), bounded body reader (`read_bounded_body`, `push_bounded_chunk`), `map_request_error` |
| `request.rs` | `EngineSearchRequest` — provider-neutral structured request; `RepoScope` (`owner`/`repo`, never reparsed from free text) |
| `models.rs` | `SearchResult`, `ResultMetadata` (issue/release/advisory/code-search merge), `AggregatedResult`, `EngineSearchBatch` / `EngineRetrievalMetadata` / `ScopeIndexStatus` |
| `normalizer.rs` | URL canonicalization: fragment strip, tracking-param strip, query-param sort, locale-prefix strip, index-file strip, trailing-slash trim, scheme/host lowercase |
| `error.rs` | `EngineError`: `Timeout`, `Http`, `BadStatus`, `ParseFailed`, `NetworkError`, `Unsupported` |
| `kev.rs` | Shared `KevClient`: fetches and caches the CISA Known Exploited Vulnerabilities catalog (used by the `cisa_kev` engine and by `ServerState` for KEV enrichment) |
| `<provider>.rs` × 43 | One upstream mapping each: request shaping, bounded read, response parse, `SearchResult` construction |

Verify the count with `ls src/meta/engines | wc -l` (49, including `mod.rs`, `request.rs`,
`models.rs`, `normalizer.rs`, `error.rs`, `kev.rs`).

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

Contract notes:

- `search(&EngineSearchRequest)` is the only required method. Engines read query, budgets,
  intent, and optional constraints from the request and ignore constraints they do not natively
  support. Unsupported capabilities stay explicit — an engine simply does not map that field
  upstream — and dispatch records the gap rather than silently dropping it (see below).
- `search_batch` defaults to `search` plus empty `EngineRetrievalMetadata`. Only specialist
  engines with retrieval-state evidence override it; today that is `FirecrawlDeveloperEngine`,
  which preserves the `repos`/`sources` scope-index echo.
- `supports_role` defaults to `true` (conservative: assume generic search can reach any evidence
  role) and is overridden only where an engine provably cannot serve a role. Today only
  `FirecrawlDeveloperEngine` overrides it, accepting `OfficialDocumentation`,
  `IssueOrIncidentDiscussion`, and `PullRequestOrDesignReview`.
- `AdvisoryCapabilities { lookup_by_id, query_by_package }` defaults to all-false. Overrides
  exist in `OsvEngine` (both true), `GithubAdvisoryEngine` (both true), `NvdEngine` (id only),
  and `RustSecEngine` (id only). `CisaKevEngine` implements `lookup_advisory` (CVE-only, via the
  shared `KevClient` catalog) without setting advisory-capability flags.
- Defaulted advisory methods return `Ok(None)` / `Ok(vec![])`: absence of support is
  indistinguishable from absence of results at this layer. The adapter's capability partitioning
  prevents unsupported advisory lookups from being dispatched at all.
- Timeout is supplied by the adapter and bounded above by the configured global timeout. Engines
  never set their own timeouts; they convert it with `engine_timeout(timeout)` per request.
- Boxed futures (`Pin<Box<dyn Future + Send>>`) keep the trait dyn-compatible across tokio's
  multi-thread runtime.

---

## `EngineSearchRequest` (`request.rs`)

Provider-neutral input every engine receives:

| Field | Meaning |
|-------|---------|
| `query` | Raw query text |
| `max_results` | Per-engine result budget |
| `timeout` | Per-engine deadline (adapter-bounded) |
| `intent` | `SearchIntent` (web, news, docs, issues, …); drives endpoint/topic selection |
| `safe_search` | Optional `SafeSearch`; mapped upstream only by engines with native support |
| `freshness` / `date_range` | Relative freshness and exact date ranges; mapped upstream only by engines with native support |
| `include_domains` / `exclude_domains` | Host allow/deny lists; mapped upstream only by engines with native support |
| `language` / `region` | BCP-47 / ISO codes; mapped upstream only by engines with native support |
| `excerpt_count` | Bounded excerpt demand (`wants_excerpts()`); engines fetch highlights/chunks only when nonzero |
| `repo_scope` | Optional `RepoScope { owner, repo }`; populated from the resolved repo locator by the planner, never reparsed from free text; used only by engines with native repo filtering |

---

## Result Models (`models.rs`)

- `SearchResult { title, url, snippet, source_engine, metadata, excerpts, published_at }` is the
  per-engine row. Structured payloads ride in `ResultMetadata`: `Issue`, `Release`, `Advisory`,
  `CodeSearch` (matched symbol plus text fragment), or `None`.
- `ResultMetadata::merge` is idempotent and order-independent: the richer variant wins, so a
  `github_issues` row carrying real `IssueMetadata` is never overwritten by a `None` from a
  generic scraper that happened to return the same URL during RRF aggregation.
- `EngineSearchBatch { results, retrieval_metadata }` pairs results with provider-neutral
  retrieval state. `EngineRetrievalMetadata` currently carries only `scope_index` evidence
  (`ScopeIndexStatus { scope, indexed }`), letting callers distinguish "scope not indexed" from
  "indexed but zero matches". `EngineSearchBatch::from_results` builds the empty-metadata case.
- `normalizer::normalize` runs before aggregation: fragments stripped, tracking params
  (`utm_*`, `fbclid`, `gclid`, `msclkid`, `yclid`, `ref`, `source`) stripped, remaining query
  params sorted, locale prefixes and `index.*` files stripped, case and trailing slashes folded.
- `EngineError` keeps failures provider-scoped: timeouts, HTTP transport errors, non-success
  statuses, bounded-body overflows (`ParseFailed`), and explicit `Unsupported` operations. A
  failing engine degrades its own attempts; it never fails the whole search.

---

## Shared Transport Ownership

Engines own no HTTP stack. All HTTP goes through one shared `Arc<eggfetch_core::Client>` built
once in `build_default_engines` (`src/meta/adapter/builders.rs`) via `build_http_client_with_egress`
in `engines/mod.rs`, and cloned into each engine struct. Consequences:

- There is no local `reqwest` usage in engines: connection pooling, redirect policy (bounded,
  strict, max 10), timeouts, and optional egress routing are configured in exactly one place.
- Keyed engines receive their key at construction; keys resolve from env at startup
  (`api_key_env`, plus direct `SEMANTIC_SCHOLAR_API_KEY` / `SOURCEGRAPH_API_KEY` / `NVD_API_KEY`
  reads) and are never logged.
- Per-request deadlines reuse the shared client through `engine_timeout(timeout)`; fetch-timeout
  overrides keep equal/shorter values on the shared transport and build one widened client only
  for longer values.
- Every upstream body is forged only via `read_bounded_body` (streaming byte cap with an upfront
  `Content-Length` check, `ParseFailed` on overflow; `MAX_BODY_BYTES` is 2 MiB per engine).
- The client carries no cookie store: a long-lived MCP server must not persist cookies across
  requests or operator sessions.
- The default user agent is a browser-like fallback used only when the operator has not configured
  their own; HTML scrape engines rely on automatic gzip/Brotli negotiation and transparent
  decompression on every search request.

---

## Engine Inventory

IDs below are exactly `KNOWN_PROVIDER_IDS` order, grouped by category. Capabilities quoted are the
`ProviderCapabilities` flags in `src/core/provider.rs`; do not invent others.

### Generic web (11) plus Firecrawl Developer Index specialist

| Provider ID | Engine | Transport / Credential |
|-------------|--------|------------------------|
| `duckduckgo` | `DuckDuckGoEngine` | HTML scrape, keyless |
| `brave` | `BraveEngine` | HTML scrape, keyless (distinct from `brave_api`) |
| `startpage` | `StartpageEngine` | HTML scrape, keyless |
| `yahoo` | `YahooEngine` | HTML scrape, keyless |
| `mojeek` | `MojeekEngine` | HTML scrape, keyless |
| `searxng` | `SearxngEngine` | JSON API, self-hosted `base_url` required; skipped as `missing_searxng_config` otherwise; no native capability flags claimed |
| `brave_api` | `BraveApiEngine` | JSON API, `BRAVE_API_KEY`; native safe-search, freshness/date-range, language, region, news, result timestamps |
| `exa` | `ExaEngine` | JSON API (`POST /search`), `EXA_API_KEY`; native freshness/date-range, domain filters, result timestamps; highlights fetched only on excerpt demand |
| `tavily` | `TavilyEngine` | JSON API (`POST https://api.tavily.com/search`, `Authorization: Bearer`), `TAVILY_API_KEY`; native safe-search, freshness/date-range, language, region, domain filters, news; `chunks_per_source` 1–3 from excerpt demand; always `include_answer=false`, `include_raw_content=false`, `include_images=false`, `auto_parameters=false` |
| `serpapi` | `SerpapiEngine` | Google SERP through SerpApi (`q`, `safe`, `hl`, `gl`; `tbs` deliberately left unmapped), required key via `[search.api.serpapi]`; native safe-search, language, region; claims no freshness, domain-filter, news, or result-timestamp support |
| `kagi` | `KagiEngine` | JSON API, required key via `[search.api.kagi]`; native safe-search (`safe_search`), freshness/date-range (`filters.after` / `filters.before`), region, domain filters (`sites_included` / `sites_excluded`), result timestamps (`data.search[].time`) |
| `firecrawl_developer` | `FirecrawlDeveloperEngine` | Keyless-optional `JsonApi` specialist for `POST /v2/search/developer` (never the generic `/v2/search` SERP). `[search.providers].firecrawl_developer = true` routes keyless; optional `[search.api.firecrawl_developer]` attaches `Authorization: Bearer` for higher limits. `k` clamped 1–20, passages default 2 max 3, `types` restricted only for Docs/Issues intents, `repos` from `RepoScope`. Never claims `supports_code_search`. |

### Forge code / issues / releases (9) plus Sourcegraph

Three hosts × three capabilities. Credentials and base URLs resolve through
`[search.api_providers]` (`api_key_env`, `base_url`); missing keys skip as `missing_api_key`,
and Gitea additionally requires an explicit base URL (`missing_base_url`).

| Host | Code | Issues | Releases |
|------|------|--------|----------|
| GitHub | `github_code` | `github_issues` | `github_releases` |
| GitLab | `gitlab_code` | `gitlab_issues` | `gitlab_releases` |
| Gitea/Forgejo | `gitea_code` | `gitea_issues` | `gitea_releases` |

Capability shape is conservative per host: GitHub code claims code/repo/org/path/language-filter/
symbol-hint search; GitHub issues claims issue search plus repo/org filters and result timestamps;
GitHub releases claims release search plus repo filter and result timestamps; GitLab mirrors that
minus language-filter/symbol-hint; Gitea claims only the single search kind per engine (plus result
timestamps on issues/releases) with no repo/path/language filters. The forge tree/structure APIs
used by `repo_map` live under `src/meta/forge_adapter/`, not in these engines.

| Provider ID | Engine | Credential |
|-------------|--------|------------|
| `sourcegraph` | `SourcegraphCodeEngine` | Optional `SOURCEGRAPH_API_KEY`; claims code search, path/language filters, repo indexing |

### Security advisories (5)

| Provider ID | Engine | Credential | Engine advisory surface |
|-------------|--------|------------|-------------------------|
| `osv` | `OsvEngine` | keyless | `lookup_by_id` + `query_by_package`; parses CVE/GHSA/RustSec IDs and `package:`/`ecosystem:`/`version:` hints from free text |
| `github_advisory` | `GithubAdvisoryEngine` | via `api_providers` | `lookup_by_id` + `query_by_package` |
| `nvd` | `NvdEngine` | optional `NVD_API_KEY` | `lookup_by_id` only; keyword search path also available via `search` |
| `cisa_kev` | `CisaKevEngine` | keyless, shared `KevClient` catalog cache | `lookup_advisory` for CVE IDs; no `advisory_capabilities` override (provider descriptor still advertises lookup-by-id plus exploit/KEV status) |
| `rustsec` | `RustSecEngine` | keyless | `lookup_by_id` only; id-or-keyword query routing in `search` |

### Package registries (8, all keyless JSON APIs)

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

All eight advertise `supports_package_metadata` plus `supports_structured_changelog`. Registry
metadata lookups beyond plain search are also reachable through `package_resolver.rs`, which shares
these upstreams.

### Scholarly (5)

| Provider ID | Engine | Credential | Notes |
|-------------|--------|------------|-------|
| `openalex` | `OpenAlexEngine` | keyless | Scholarly search plus DOI lookup; result timestamps |
| `crossref` | `CrossRefEngine` | keyless | Scholarly search plus DOI lookup |
| `semantic_scholar` | `SemanticScholarEngine` | optional `SEMANTIC_SCHOLAR_API_KEY` | Scholarly search plus DOI lookup |
| `arxiv` | `ArxivEngine` | keyless, shared `arxiv::RequestGate` | Atom feed parsed with `quick_xml`; scholarly search plus result timestamps; see [arXiv Pacing Gate](#arxiv-pacing-gate) |
| `pubmed` | `PubmedEngine` | optional `NCBI_API_KEY`, optional `NCBI_API_EMAIL` contact identity | `esearch` + one bounded `esummary` batch; scholarly search plus result timestamps |

### Source-specific keyless (5 — the two scholarly ones, `arxiv` and `pubmed`, are inventoried above)

Explicit-source providers, disabled by default and never members of `default_providers`.

| Provider ID | Engine | Credential | Native surface |
|-------------|--------|------------|----------------|
| `wikipedia` | `WikipediaEngine` | keyless | MediaWiki `action=query&list=search`; result timestamps (`srprop=timestamp`) |
| `hn_algolia` | `HnAlgoliaEngine` | keyless | HN Search API constrained to `tags=story`; native freshness/date-range via `numericFilters=created_at_i…` plus result timestamps |
| `github_repositories` | `GithubRepositoriesEngine` | optional `GITHUB_TOKEN` | `GET /search/repositories` discovery; result timestamps (`pushed_at`); never claims `supports_code_search` or `supports_repo_indexing` |

`wikipedia` and `hn_algolia` ship without an operator `base_url` override; `pubmed` and
`github_repositories` read `base_url` from their optional `[search.api.<id>]` section the same
way `firecrawl_developer` does. Their engine `search()` functions still accept an optional base
URL so wire-level request/response fixtures can run against a local mock server.

`wikipedia`, `hn_algolia`, and `github_repositories` reduce API-provided HTML fragments
(MediaWiki `searchmatch` spans, HN `story_text`) through `fetch::extract::HtmlExtractor` instead of
a bespoke stripper, then hand plain text to the common sanitization pipeline.

#### arXiv Pacing Gate

`arxiv::RequestGate` is the smallest provider-local synchronization primitive in the engine
layer, added because the arXiv API terms require at most one request every three seconds on a
single connection. It holds a `tokio::sync::Mutex<Option<Instant>>`:

- acquiring sleeps until at least `interval` has elapsed since the previous request **started**,
  so concurrent calls cannot compress the request rate;
- the returned `GateTurn` keeps the lock, so only one arXiv request is ever in flight;
- `arxiv::shared_gate()` is a process-wide `OnceLock<Arc<RequestGate>>`, and `ArxivEngine` holds
  that shared instance, so the policy holds across every engine instance in the process rather
  than per instance.

The gate is deliberately provider-local: it introduces no generic scheduler, no global rate
limiter service, and no change to the shared `eggfetch-core` client. `RequestGate::new` is the
injection seam tests use to keep pacing assertions fast and deterministic.

Other providers deliberately do **not** get a gate: `pubmed` runs its two E-utilities calls
sequentially inside one search and relies on the existing per-provider concurrency cap, and the
remaining providers are unaffected. This asymmetry is a recorded limitation, not an oversight.

### Local workspace (backend, not an engine)

| Provider ID | Implementation | Notes |
|-------------|---------------|-------|
| `local_workspace` | `LocalWorkspaceBackend` (`src/meta/local_backend.rs`) | No engine file. Bounded file walking over configured `[local]` roots, `.gitignore`-aware, inventory-cached; claims code search plus path/language filters |

---

## Construction (`build_default_engines` in `adapter/builders.rs`)

Every enabled provider ID resolves to exactly one outcome: a constructed engine or a typed
`SkippedProvider` (`missing_searxng_config`, `missing_api_key`, `missing_base_url`,
`unknown_provider`, …). Skips surface as provider-scoped warnings — never global failures — and
feed `provider_status` skip codes. Firecrawl Developer, PubMed, and GitHub Repositories are
keyless-optional: enabling the provider routes keyless, and a missing or empty optional key falls
back keyless with a startup warning rather than `missing_api_key`. `serpapi` and `kagi` are
required-credential providers: they are built only when the operator enables `[search.api.<id>]`
and the referenced environment variable resolves, and they are never enabled by
`[search.providers].<id>` (that map is excluded from `API_PROVIDER_IDS` members). Direct env vars cover Semantic
Scholar, Sourcegraph, NVD, and the optional `NCBI_API_EMAIL` contact identity; everything in
`api_providers` resolves through its `api_key_env`.

---

## Native Enforcement Matrix

Six providers declare at least one natively enforced generic-search constraint (`brave_api`,
`exa`, `tavily`, `serpapi`, `kagi`, `hn_algolia`). The table below covers the five generic-search
services it has columns for; everything else either ignores
the constraint upstream (local approximation applies downstream) or is a specialist whose native
surface is its own API shape. Source of truth: `ProviderCapabilities` in `src/core/provider.rs`,
pinned by `tests/provider_capability_contract.rs`; operator prose in `docs/provider-setup.md`.

| Constraint | `brave_api` | `exa` | `tavily` | `serpapi` | `kagi` |
|------------|-------------|-------|----------|-----------|--------|
| Safe-search | native | — | native (`Strict` collapses to `true`, therefore approximate) | native (`safe=active\|off`; two-state, therefore approximate) | native (`safe_search` boolean; three-state model, therefore approximate) |
| Freshness / date-range | native (relative `pd\|pw\|pm\|py` and exact ranges) | native (`startPublishedDate` / `endPublishedDate` as UTC day boundaries) | native (exact ranges to `start_date`/`end_date`; relative to `time_range`) | — (`tbs` value syntax undocumented upstream; local approximation) | native (exact ranges to `filters.after`/`filters.before`; relative windows to a UTC cutoff) |
| Language | native (`search_lang`) | — | native (BCP-47 normalized, `filter_by_language=true`) | native (`hl`, e.g. `en`, `en-us`) | — (no language field; region only) |
| Region | native (`country`, 2-letter codes) | — | native (country names; general topic only, omitted for news) | native (`gl`, 2-letter code) | native (`filters.region`, ISO 3166-1 alpha-2) |
| Domain filters | local approximation | native (`includeDomains` / `excludeDomains`) | native (`include_domains` / `exclude_domains`, `include_domains_mode=filter`) | — (no site-restriction parameter) | native (inline lens `sites_included` / `sites_excluded`) |
| News | native (dedicated `/res/v1/news/search` on news intent) | — | native (`topic=news` on news intent) | — (would need the separately billed `tbm=nws` vertical) | — (news workflow is a separate collection eggsearch never requests) |
| Result timestamps | native (`age` when parseable) | native (`publishedDate` when parseable) | — (no per-result dates; freshness is request-side only) | — (`organic_results` carry no documented date) | native (`data.search[].time` when parseable) |

Rules that follow from the matrix:

- Domain filters are natively enforced only by providers advertising `supports_domain_filters`
  (currently `exa`, `kagi`, and `tavily`); all other domain filtering is local approximation.
- `supports_freshness` is provider-side (the upstream request carries the constraint);
  `supports_result_timestamps` is client-side (timestamps feed bounded freshness reranking after
  retrieval). GitHub/GitLab/Gitea issues and releases use the client-side model only.
- The five HTML scrapers (`duckduckgo`, `brave`, `startpage`, `yahoo`, `mojeek`) and `searxng`
  claim no native capabilities at all; every constraint on those paths is local approximation.
- Freshness is natively enforced by exactly five providers: `brave_api`, `exa`, `tavily`,
  `hn_algolia` (via `numericFilters=created_at_i…`), and `kagi` (via `filters.after` /
  `filters.before`). `nvd` also declares `supports_freshness: true` in
  `src/core/provider.rs` (asserted in `src/meta/engines/nvd.rs`), but it is an advisory
  service whose engine maps only `keywordSearch` onto the upstream request, so the declared
  flag is not yet backed by a wired date parameter. `serpapi`
  deliberately does not claim it: the current engine page documents
  `tbs` only generically and without value syntax, so sending it would be a guess. `wikipedia`,
  `arxiv`, `pubmed`, and `github_repositories` also do not claim it — none of them has a
  documented freshness parameter mapped, so the capability stays local approximation.
- `arxiv` is the only provider whose `ProviderKind` is `structured_api`; every other provider keeps
  its pre-existing kind, so no existing `provider_status.kind` wire value changed.
- `never invent tool-like or provider names in docs`: the inventory test derives tool names from
  `src/mcp/server.rs` and provider IDs from `KNOWN_PROVIDER_IDS`. Prose must agree with the code.

---

## Dispatch Capability-Skip Semantics

Dispatch never silently omits an unsupported capability. For each `(subquery, provider)` job,
`partition_roles_for_engine` (`src/meta/dispatch/types.rs`) splits intended evidence roles with
`supports_role` into supported and unsupported sets, recorded as `CapabilityDisposition`
(`FullySupported`, `PartiallySupported`, `Unsupported`, `NotApplicable`):

- Supported and partially-supported jobs execute normally against `search_batch`.
- Unsupported jobs are never sent upstream. Dispatch emits a `RetrievalAttempt` with outcome
  `SkippedCapabilityUnavailable` (or `NotApplicable`), zero results, and the query fingerprint —
  visible evidence that the provider was considered and skipped for cause.
- Capability skips do not count toward subquery completion and are never reported as timeouts or
  transport failures; provider health and cooldown state are untouched by skips.
- Advisory routing applies the same principle one layer up: the adapter consults
  `advisory_capabilities` (and the provider `supports_advisory_lookup_by_id` /
  `supports_advisory_lookup_by_package` flags) before attempting native advisory operations, so
  unsupported lookups are partitioned away rather than attempted and failed.

---

## Adding a New Provider

1. Create `src/meta/engines/<provider>.rs` implementing `SearchEngine`: shape the upstream
   request from `&EngineSearchRequest`, read via `read_bounded_body` (never bare `.text()` /
   `.bytes()`), sanitize untrusted text, and return `SearchResult` rows. Reuse the defaulted
   advisory methods unless the upstream serves advisories; leave unsupported constraints
   unmapped (explicit, never silent).
2. Declare the module in `engines/mod.rs` and add the engine struct plus its `SearchEngine` impl
   (shared `Arc<eggfetch_core::Client>`; keyed engines take their key at construction).
3. Add the ID to `KNOWN_PROVIDER_IDS` in `src/core/provider.rs` and declare its 24-flag
   `ProviderCapabilities` honestly in `built_in_provider_descriptor` — claim only natively
   enforced behavior, and document the native-vs-local split.
4. Register construction in `build_default_engines()` (`src/meta/adapter/builders.rs`), including
   any `api_providers` handling and typed skip reasons for missing keys or base URLs.
5. Wire the provider into profile and provider resolution so profiles can select it.
6. Document the native-vs-local enforcement in `docs/provider-setup.md` without inventing names.
7. Extend `tests/provider_capability_contract.rs` with the new provider's capability assertions,
   add behavioral coverage in the relevant suite (`web_search` integration, `provider_routing`,
   `provider_probe_conformance`, workflow suites), and keep the docs inventory tests green.

---

[← Back to Overview](overview.md) | [Metasearch Adapter →](meta.md) | [Core Types →](core.md) |
[Provider Setup](../docs/provider-setup.md) | [Testing](testing.md)
