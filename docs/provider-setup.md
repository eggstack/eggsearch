# Provider Setup

eggsearch supports 44 search providers across ten categories: web search (HTML scrapers), API-key providers, keyless-optional developer index, aggregators, code search hosts, security advisory databases, package registries, scholarly search, reference/discussion sources, and special-purpose providers. Providers can be enabled individually in config and selected per-request or via `default_providers`.

## Provider Categories at a Glance

| Category | Examples | User Requirement |
|----------|----------|-----------------|
| **Keyless defaults** | DuckDuckGo, Startpage, Yahoo | none |
| **Keyless specialist** | OSV, NVD, CISA KEV, RustSec, OpenAlex, Crossref, all package registries | none |
| **Keyless source-specific** | Wikipedia, arXiv, Hacker News (Algolia) | none |
| **Keyless-optional specialist** | Firecrawl Developer Index, PubMed, GitHub Repositories | none (optional key/contact raises limits) |
| **Optional configured endpoint** | SearXNG, self-hosted forge base URL | operator configuration |
| **Optional credentialed** | GitHub/GitLab/Gitea code search, Sourcegraph, Brave API, Exa, Tavily, SerpApi, Kagi, Semantic Scholar, GitHub Advisory | opt-in credential |
| **Optional local** | local workspace | configured local root |

All credentialed providers are disabled or non-routable unless explicitly configured. Missing optional credentials produce provider-scoped skip telemetry and never make the server globally unhealthy. The Firecrawl Developer Index, PubMed, and GitHub Repositories providers route keyless when enabled; a missing optional key never produces `missing_api_key`.

## Web Search Providers

These providers require no API key and work via HTML scraping.

### DuckDuckGo (default)

- ID: `duckduckgo`
- Enabled by default: yes
- Included in `default_providers`: yes
- No configuration needed
- Rate limits: moderate; provider enters cooldown after repeated failures and recovers automatically

### Brave Search (HTML)

- ID: `brave`
- Enabled by default: yes
- Included in `default_providers`: no
- Enable: set `brave = true` in `[search.providers]` (already true by default)
- No API key required

### Startpage (default)

- ID: `startpage`
- Enabled by default: yes
- Included in `default_providers`: yes
- No configuration needed

### Yahoo (default)

- ID: `yahoo`
- Enabled by default: yes
- Included in `default_providers`: yes
- No configuration needed

### Mojeek

- ID: `mojeek`
- Enabled by default: **no**
- Independent search engine with its own index
- Enable: set `mojeek = true` in `[search.providers]`

## Keyless-Optional Developer Index

### Firecrawl Developer Index

- ID: `firecrawl_developer`
- Dedicated `POST https://api.firecrawl.dev/v2/search/developer` (never the generic `/v2/search` SERP)
- Enabled by default: **no**; opt-in specialist for `repo_search` coding evidence
- Included in `default_providers`: **no**; never becomes default automatically
- Enable keyless:

```toml
[search.providers]
firecrawl_developer = true
```

- Optionally raise rate limits with a key (never required, never logged):

```toml
[search.api.firecrawl_developer]
enabled = true
api_key_env = "FIRECRAWL_API_KEY"
```

A missing or empty optional env var falls back keyless with a startup warning; it never yields `missing_api_key` and never makes the provider unroutable. An explicitly invalid base URL in `[search.api.firecrawl_developer]` is still a configuration error.

Native behavior: `issue_search` + `repo_filter` (no `code_search`, no `release_search`, no `scholarly_search`, no `repo_indexing`). Artifact kinds `issue:`/`pull_request:`/`readme:`/`doc:` map via URL classification with deterministic URL-fallback titles when upstream titles are absent. Matched markdown passages become bounded `ProviderPassage` excerpts (at most 3 per card, never `fetched=true`). `repos` scope comes from `repo_search` resolved `owner/repo` (never reparsed from free text); `types` is restricted only for `Docs` (`doc`+`readme`) and `Issues` (`issue`+`pull_request`) intents. Scoped `repos`/`sources` echo with `indexed=false` surfaces as a stable `scope_unindexed` warning so "scope not indexed" is never mislabeled as ordinary zero evidence. `provider_status` reports `requires_api_key=false`, `configured=true` when enabled, and `routable=true` keyless.

## Source-Specific Keyless Providers

These providers exist for explicit source selection, not for default fan-out. All of them are disabled by default, never appear in `default_providers`, and are reachable only through an explicit `providers` request field or an intentionally edited profile. None of them requires a credential.

### Wikipedia

- ID: `wikipedia`
- Structured MediaWiki Action API search (`action=query&list=search`) against `https://en.wikipedia.org/w/api.php`
- Enable in `[search.providers]`:

```toml
[search.providers]
wikipedia = true
```

- Select explicitly, e.g. `web_search({"query": "...", "providers": ["wikipedia"]})`

Results map to canonical article URLs (`https://en.wikipedia.org/wiki/<Title>`), the page title, and a bounded snippet. The API returns highlighted snippets as small HTML fragments; those are reduced to plain text through eggsearch's normal extraction path and then sanitized by the common pipeline. Native capabilities: `result_timestamps` only (`srprop=timestamp`, the page last-modified timestamp, preserved in `published_at`). Safe-search, language, region, domain filters, news, and freshness are not claimed — every one of those constraints is local approximation. An HTTP 200 body carrying a MediaWiki `error` object (for example `ratelimited`) fails the call as a provider-scoped parse error rather than returning empty success.

### arXiv

- ID: `arxiv`
- Public arXiv metadata API (`search_query=all:<query>`) returning an Atom 1.0 feed, parsed with the repository's existing `quick-xml` reader
- Enable in `[search.providers]`:

```toml
[search.providers]
arxiv = true
```

Native capabilities: `scholarly_search` and `result_timestamps` (`published`, falling back to `updated`, preserved in `published_at`). Results link to the canonical abstract page with the version suffix stripped so every revision of one paper shares one stable identity.

arXiv request pacing (required by the arXiv API terms of use): requests are spaced at least three seconds apart and only one request is ever in flight. A single process-wide pacing gate owns both rules, so concurrent calls queue behind it instead of bypassing it. A short-lived wait therefore consumes part of the per-engine timeout on the second call; that is the documented cost of the upstream policy, not a scheduler bug. No bulk-harvest behavior is ever requested.

Attribution required by the current arXiv API guidance: results originate from arXiv, links resolve to `arxiv.org` abstract pages, and operators surfacing arXiv data should credit arXiv (see <https://info.arxiv.org/help/api/>).

### Hacker News (Algolia)

- ID: `hn_algolia`
- Public HN Search API (`https://hn.algolia.com/api/v1/search`) constrained to `tags=story`
- Enable in `[search.providers]`:

```toml
[search.providers]
hn_algolia = true
```

Results map to the original submitted URL when present, otherwise to the canonical discussion URL `https://news.ycombinator.com/item?id=<id>`; the snippet uses the story text when available and otherwise synthesizes bounded discussion metadata (points, comments, author). Native capabilities: `freshness` (relative `day|week|month|year` and exact `YYYY-MM-DD` ranges map to `numericFilters=created_at_i>…` / `created_at_i<=…`, with an exact range taking precedence) and `result_timestamps` (`created_at`). This provider is deliberately Hacker News only: it is not a Reddit/Lobsters/community aggregator.

### PubMed

- ID: `pubmed`
- NCBI E-utilities: one `esearch.fcgi` call for PMIDs followed by one bounded `esummary.fcgi` batch for citation metadata
- Enable in `[search.providers]`:

```toml
[search.providers]
pubmed = true
```

- Optionally raise the rate limit with an NCBI API key (never required, never logged):

```toml
[search.api.pubmed]
enabled = true
api_key_env = "NCBI_API_KEY"
```

- Optionally supply the contact identity requested by NCBI's E-utilities usage policy:

```bash
export NCBI_API_EMAIL="ops@example.org"
```

Every request identifies itself truthfully as `tool=eggsearch` with a real `eggsearch/<version>` User-Agent. No fabricated contact identity is ever sent: without `NCBI_API_EMAIL` the `email` parameter is simply omitted, which leaves the operator non-compliant with NCBI's request for contact information rather than silently impersonating a maintainer. Per-call result counts are bounded to 25 for interactive agent retrieval. If the summary phase fails, the whole call fails — partial or fabricated citation metadata is never returned.

Native capabilities: `scholarly_search` and `result_timestamps` (`epubdate`, falling back to `pubdate`, normalized from shapes such as `2024 Feb 3`, `2024 Feb`, or `2024`; unrecognized shapes are dropped rather than guessed).

### GitHub Repositories

- ID: `github_repositories`
- Official repository search endpoint (`GET /search/repositories`) — repository **discovery**, not code search
- Enable in `[search.providers]`:

```toml
[search.providers]
github_repositories = true
```

- Optionally raise GitHub's unauthenticated search limit (never required, never logged):

```toml
[search.api.github_repositories]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

Keyless routing works at GitHub's unauthenticated limits; a missing token never makes the provider unroutable and never yields `missing_api_key`. Results carry the `owner/repo` full name as the title, the canonical `html_url`, the repository description (or bounded language/star/fork/issue metadata when the description is absent), and `pushed_at` as the result timestamp. Requests are ordered `sort=updated&order=desc` for deterministic freshness.

Native capabilities: `result_timestamps` only. This provider deliberately does **not** claim `code_search` or `repo_indexing` — repository discovery is not file/code search — and it does not replace `github_code`, `github_issues`, or `github_releases`. Rate-limit responses (HTTP 403/429) flow through the normal provider failure and cooldown path; they are provider-scoped and never make the server unhealthy.

## API-Key Providers

These providers require an API key stored in an environment variable. All are disabled by default.

### Brave Search API

- ID: `brave_api`
- Enable and configure in `[search.api.brave_api]`:

```toml
[search.api.brave_api]
enabled = true
api_key_env = "BRAVE_API_KEY"
```

The environment variable `BRAVE_API_KEY` must be set at runtime.

Native capabilities: `safe_search`, `freshness` (relative `pd|pw|pm|py` and exact `YYYY-MM-DDtoYYYY-MM-DD`), `language` (`search_lang`), `region` (`country` for 2-letter codes), `news` (dedicated `/res/v1/news/search` endpoint for `intent=news`), and `result_timestamps` (the `age` field is preserved when it parses as RFC 3339 or `YYYY-MM-DD`, and feeds freshness reranking). Set `extra_snippets=true` automatically when `web_search` requests `excerpt_count > 0` (up to 3 alternate excerpts per result, converted to provider-neutral excerpts). Domain filters are enforced locally, not provider-native. Summaries are never requested.

### Exa Semantic Search

- ID: `exa`
- Semantic/neural web retrieval through `POST https://api.exa.ai/search` (auth `x-api-key: <EXA_API_KEY>`)
- Enabled by default: **no**; opt-in semantic complement to the HTML/SERP sources
- Included in `default_providers`: **no**; never becomes default automatically — select explicitly via `providers: ["exa"]`
- Enable and configure in `[search.api.exa]`:

```toml
[search.api.exa]
enabled = true
api_key_env = "EXA_API_KEY"
```

The environment variable `EXA_API_KEY` must be set at runtime. `base_url` remains overridable through the same section for tests/proxies; the production default is `https://api.exa.ai/search`.

Native capabilities: `freshness` (exact `YYYY-MM-DD` ranges map to `startPublishedDate`/`endPublishedDate` as UTC day boundaries; relative `day|week|month|year` maps to a UTC `startPublishedDate` lower bound 1/7/30/365 days ago with the end bound omitted), `domain_filters` (`includeDomains`/`excludeDomains` from the generic host lists), and `result_timestamps` (`publishedDate` preserved when it parses as RFC 3339 or `YYYY-MM-DD`, feeding freshness reranking). Safe-search, language, region, news, code, repo, issue, and release search are not claimed. crawl-date fields are never used.

Excerpts: `contents: { highlights: true }` is sent only when `web_search` requests `excerpt_count > 0`; `highlights[i]` becomes a bounded `ProviderHighlight` excerpt with the aligned `highlightScores[i]` as provider-local score (ordering only, never compared across providers). Unrequested highlights are never fetched or stored. Excerpt count/char caps and sanitization apply through the common pipeline.

eggsearch uses Exa only for search metadata/highlights. Generated summaries, output schemas, system prompts, additional queries, full-text retrieval, subpage crawling, live crawl, and agent/answer endpoints are never requested. Missing/invalid credentials and quota/rate failures are provider-scoped and never make the server unhealthy.

### Tavily Search

- ID: `tavily`
- General web retrieval through `POST https://api.tavily.com/search` (auth `Authorization: Bearer <TAVILY_API_KEY>`)
- Enabled by default: **no**; opt-in complement to the HTML/SERP sources
- Included in `default_providers`: **no**; never becomes default automatically — select explicitly via `providers: ["tavily"]`
- Enable and configure in `[search.api.tavily]`:

```toml
[search.api.tavily]
enabled = true
api_key_env = "TAVILY_API_KEY"
```

The environment variable `TAVILY_API_KEY` must be set at runtime. `base_url` remains overridable through the same section for tests/proxies; the production default is `https://api.tavily.com/search`.

Native capabilities: `safe_search` (`Off -> false`, `Moderate|Strict -> true`; `Strict` collapses to `true` and is therefore approximate), `freshness` (exact `YYYY-MM-DD` ranges map to `start_date`/`end_date`; relative `day|week|month|year` maps to `time_range`), `language` (BCP47 `en`/`en-US` normalized to lowercase with `filter_by_language=true` for strict enforcement; unrepresentable values omitted), `region` (2-letter ISO codes mapped to Tavily country names e.g. `US -> united states`, single-word names passed through when known; omitted for `topic=news` because Tavily country is general-topic only; unrepresentable values omitted), `domain_filters` (`include_domains`/`exclude_domains` with `include_domains_mode=filter` for strict filtering), and `news` (`intent=news -> topic=news`, otherwise `general`). Result timestamps are not claimed because Tavily results carry no per-result published dates.

Excerpts: `chunks_per_source` 1-3 is derived from `web_search` `excerpt_count` (1 when no excerpts requested, otherwise the demanded count clamped 1-3); result `content` is split on `[...]` into source chunks, the first chunk becomes the card snippet, and up to the demanded count become bounded `ProviderSnippet` excerpts. Unrequested chunks are discarded before card construction. Excerpt count/char caps and sanitization apply through the common pipeline.

eggsearch uses Tavily only for search metadata/chunks with `search_depth=basic`. Generated answers (`include_answer=false`), raw page content (`include_raw_content=false`), images (`include_images=false`), and automatic parameter rewriting (`auto_parameters=false`) are always disabled; `web_fetch` remains the fetch owner. Missing/invalid credentials and quota/rate failures are provider-scoped and never make the server unhealthy.

### SerpApi Google Search

- ID: `serpapi`
- Google web results through `GET https://serpapi.com/search` (auth `api_key` query parameter)
- Enabled by default: **no**; opt-in complement to the HTML/SERP sources
- Included in `default_providers`: **no**; never becomes default automatically — select explicitly via `providers: ["serpapi"]`
- Enable and configure in `[search.api.serpapi]`:

```toml
[search.api.serpapi]
enabled = true
api_key_env = "SERPAPI_API_KEY"
```

The environment variable `SERPAPI_API_KEY` must be set at runtime. `base_url` remains overridable through the same section for tests/proxies and must be the full endpoint URL; the production default is `https://serpapi.com/search`.

Native capabilities: `safe_search` (`Off -> safe=off`, `Moderate|Strict -> safe=active`; Google exposes a two-state filter, so the three-state model is approximate), `language` (`hl`, e.g. `en`, `en-US` -> `en-us`; unrepresentable values omitted), and `region` (`gl`, 2-letter country code lowercased; unrepresentable values omitted). Everything else is **not** claimed:

- `freshness` — the current engine page documents `tbs` only generically ("advanced search parameters ... dates") without value syntax, so eggsearch does not send it rather than guess; freshness and exact date ranges stay locally approximated.
- `domain_filters` — the Google engine exposes no site-restriction parameter; filtering stays local.
- `news` — news would require the separately billed `tbm=nws` vertical, which eggsearch never requests. Use a native-news provider instead.
- `result_timestamps` — `organic_results` entries carry no documented per-result date, so `published_at` is never synthesized.

Request discipline: `engine=google` is always sent explicitly so a future upstream default change cannot silently change which engine backs this id. No other SerpApi feature is used: no `tbm` verticals, no `async`/`no_cache`/`zero_trace`/`json_restrictor`, and no generated answers. The engine exposes no documented result-count parameter, so the result budget is enforced by bounded local truncation of `organic_results`. Missing/invalid credentials and quota/rate failures are provider-scoped (a `429` becomes a rate-limited failure with cooldown) and never make the server unhealthy.

Credential handling: SerpApi authenticates with a query parameter, so eggsearch attaches `api_key` only after the transport has accepted the endpoint, and never echoes it in errors, evidence, or provenance. Keep the key out of shared configuration files; the terms require treating it as a secret.

### Kagi Search

- ID: `kagi`
- Premium web results through `POST https://kagi.com/api/v1/search` (auth `Authorization: Bearer <KAGI_API_KEY>`)
- Enabled by default: **no**; opt-in complement to the HTML/SERP sources
- Included in `default_providers`: **no**; never becomes default automatically — select explicitly via `providers: ["kagi"]`
- Enable and configure in `[search.api.kagi]`:

```toml
[search.api.kagi]
enabled = true
api_key_env = "KAGI_API_KEY"
```

The environment variable `KAGI_API_KEY` must be set at runtime. API usage is billed separately from any Kagi subscription on a pay-as-you-go basis (`https://kagi.com/api/pricing`), so a Kagi account with a payment method and API billing enabled is an operator responsibility. `base_url` remains overridable through the same section for tests/proxies and is the v1 base (`https://kagi.com/api/v1`); the engine appends `/search`.

Native capabilities: `safe_search` (`Off -> false`, `Moderate|Strict -> true`; Kagi exposes a single boolean, so the three-state model is approximate), `freshness` (exact `YYYY-MM-DD` ranges map to `filters.after`/`filters.before`; relative `day|week|month|year` map to a UTC cutoff of 1/7/30/365 days ago, and an exact range always wins), `region` (`filters.region`, ISO 3166-1 alpha-2 uppercased; unrepresentable values omitted), `domain_filters` (the documented inline lens `lens.sites_included`/`lens.sites_excluded`, which restrict rather than boost), and `result_timestamps` (`data.search[].time` preserved when it parses, feeding freshness reranking). Language is not claimed — Kagi exposes region but no language field. `news` is not claimed — the news workflow is a separate result collection eggsearch never requests.

Request discipline: every request pins `workflow: "search"` and only `data.search[]` is read. Other collections Kagi may return in the same payload (`data.news`, `data.code`, `data.interesting_finds`, `data.related_search`, `data.infobox`, ...) are ignored rather than coerced into source cards. Billed extras (`extract`) and account personalization (`personalizations`) are never sent.

Terms compliance: the Kagi API Terms permit using returned Results in your own applications and services, including sending them to an AI model to answer your users' queries, and integrating them that way is not a prohibited transfer. eggsearch honors the two constraints that bind this integration: results are not cached, stored, or indexed by the engine (the only persistence is a user-requested evidence bundle artifact), and rate limits are never circumvented — a quota response is terminal for that attempt, is reported as a provider-scoped rate-limited failure, and starts a health cooldown. The historical v0 endpoint is never called. Missing/invalid credentials and quota failures are provider-scoped and never make the server unhealthy.

### GitHub Code Search

- ID: `github_code`
- Enable in `[search.api.github_code]`:

```toml
[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

### GitHub Issues Search

- ID: `github_issues`
- Enable in `[search.api.github_issues]`:

```toml
[search.api.github_issues]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

### GitHub Releases Search

- ID: `github_releases`
- Enable in `[search.api.github_releases]`:

```toml
[search.api.github_releases]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

All three GitHub providers share the same `GITHUB_TOKEN` environment variable.

### GitLab Code Search

- ID: `gitlab_code`
- Enable in `[search.api.gitlab_code]`:

```toml
[search.api.gitlab_code]
enabled = true
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"
```

### GitLab Issues Search

- ID: `gitlab_issues`
- Enable in `[search.api.gitlab_issues]`:

```toml
[search.api.gitlab_issues]
enabled = true
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"
```

### GitLab Releases Search

- ID: `gitlab_releases`
- Enable in `[search.api.gitlab_releases]`:

```toml
[search.api.gitlab_releases]
enabled = true
api_key_env = "GITLAB_TOKEN"
base_url = "https://gitlab.com"
```

For self-hosted GitLab, set `base_url` to your instance URL instead of `https://gitlab.com`.

### Gitea Code Search

- ID: `gitea_code`
- Enable in `[search.api.gitea_code]`:

```toml
[search.api.gitea_code]
enabled = true
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"
```

### Gitea Issues Search

- ID: `gitea_issues`
- Enable in `[search.api.gitea_issues]`:

```toml
[search.api.gitea_issues]
enabled = true
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"
```

### Gitea Releases Search

- ID: `gitea_releases`
- Enable in `[search.api.gitea_releases]`:

```toml
[search.api.gitea_releases]
enabled = true
api_key_env = "FORGEJO_TOKEN"
base_url = "https://git.example.com"
```

Gitea/Forgejo providers require a `base_url` pointing to your Gitea or Forgejo instance. The token is typically named `FORGEJO_TOKEN` but any environment variable name works.

**Never commit real keys.** Always use env-var indirection.

## Aggregator Providers

### SearXNG

- ID: `searxng`
- Requires a self-hosted SearXNG instance
- Enable in both `[search.providers]` and `[search.searxng]`:

```toml
[search.providers]
searxng = true

[search.searxng]
enabled = true
base_url = "https://searx.example.org"
```

Both flags must be set and `base_url` must be non-empty for the SearXNG provider to activate. The engine appends `/search` to the configured `base_url`.

## Special Providers

### OSV

- ID: `osv`
- Open Source Vulnerability database
- Enabled by default: yes
- No API key needed
- Used primarily by `security_search` for vulnerability lookups

## Security Advisory Providers

### GitHub Security Advisories

- ID: `github_advisory`
- Requires `GITHUB_TOKEN`
- Enable in `[search.api.github_advisory]`:

```toml
[search.api.github_advisory]
enabled = true
api_key_env = "GITHUB_TOKEN"
```

### NIST National Vulnerability Database

- ID: `nvd`
- No API key needed
- Enabled by default: yes
- Advisory lookup by CVE ID
- Native capability declared: `freshness` (`supports_freshness: true` in the provider descriptor). Note that NVD is an advisory service, not a generic-search one, and the current engine maps only `keywordSearch` onto the upstream request.

### CISA Known Exploited Vulnerabilities

- ID: `cisa_kev`
- No API key needed
- Enabled by default: yes
- KEV catalog for exploit status checks

### RustSec Advisory Database

- ID: `rustsec`
- No API key needed
- Enabled by default: yes
- Rust-specific security advisories

## Package Registry Providers

All package registry providers use JSON APIs and require no API key. They provide package metadata, version history, and structured changelogs.

### crates.io

- ID: `crates_io`
- Rust package metadata from crates.io

### PyPI

- ID: `pypi`
- Python package metadata from PyPI

### npm

- ID: `npm_registry`
- Node.js package metadata from npm

### Go Proxy

- ID: `go_pkg`
- Go module metadata from the Go module proxy

### Maven Central

- ID: `maven_central`
- Java/JVM artifact metadata from Maven Central

### NuGet

- ID: `nuget`
- .NET package metadata from NuGet

### RubyGems

- ID: `rubygems`
- Ruby gem metadata from RubyGems

### Packagist

- ID: `packagist`
- PHP package metadata from Packagist

## Scholarly Search Providers

### OpenAlex

- ID: `openalex`
- No API key needed
- Open-access scholarly literature search with DOI lookup

### Crossref

- ID: `crossref`
- No API key needed
- Scholarly literature search with DOI lookup

### Semantic Scholar

- ID: `semantic_scholar`
- Requires `SEMANTIC_SCHOLAR_API_KEY`
- Enable in `[search.api.semantic_scholar]`:

```toml
[search.api.semantic_scholar]
enabled = true
api_key_env = "SEMANTIC_SCHOLAR_API_KEY"
```

## Code Search Providers

### Sourcegraph

- ID: `sourcegraph`
- Requires `SOURCEGRAPH_API_KEY`
- Enable in `[search.api.sourcegraph]`:

```toml
[search.api.sourcegraph]
enabled = true
api_key_env = "SOURCEGRAPH_API_KEY"
```

### Local Workspace

- ID: `local_workspace`
- Indexes local files under configured roots
- Requires the `[local]` section in config:

```toml
[local]
enabled = true
roots = ["/Users/you/projects"]
max_file_bytes = 1048576
max_indexed_files = 50000
include_hidden = false
respect_gitignore = true
follow_symlinks = false
```

Local results appear in `repo_search` and are fetched via `repo_fetch` with `host = "workspace"`.

## Provider Selection

### Default Providers

The `default_providers` list controls which providers are queried when a tool call does not specify explicit providers:

```toml
[search]
default_providers = ["duckduckgo", "startpage", "yahoo"]
```

When a default provider is unavailable (cooldown, misconfigured API key, disabled), it is skipped with a warning. The search still runs with the remaining providers.

### Per-Request Override

Any search tool (`web_search`, `repo_search`, `security_search`, `research_search`) accepts an optional `providers` field to override the default list for that request.

### Profiles

Search profiles (`generic`, `coding`, `security`, `research`) have their own provider ordering. When a profile is used and its providers are unavailable, eggsearch falls back to `default_providers`. A `profile_degraded` warning is emitted in this case.

### Misconfigured Defaults

If a provider listed in `default_providers` is disabled or lacks a valid API key, eggsearch emits a startup warning. Run `eggsearch doctor` to diagnose configuration issues. Each skipped provider includes a `skip_code` (e.g. `missing_api_key`, `disabled_by_user`, `missing_searxng_config`) for machine-readable diagnostics alongside the human-readable `skip_reason`.

## Skip Codes

`provider_status` returns a `skip_code` for every non-routable provider. These are stable snake_case strings for machine-readable diagnostics.

| Code | Display Name | Meaning | Cause | Fix | Retry? |
|------|-------------|---------|-------|-----|--------|
| `unknown_provider` | Unknown provider | Provider ID not in the built-in inventory | Typo in config or referencing a removed provider | Correct the provider ID in config | No |
| `disabled_by_user` | Disabled by user | Provider is explicitly disabled in config | `provider = false` in `[search.providers]` | Set to `true` or remove from config | No |
| `missing_api_key` | Missing API key | API-key provider has no key configured | Missing `[search.api.<id>]` section or env var not set | Add `[search.api.<id>]` with `enabled = true` and `api_key_env`, then set the env var | No |
| `missing_searxng_config` | SearXNG not configured | SearXNG provider missing `base_url` or not enabled | Missing `[search.searxng]` or `searxng = false` in `[search.providers]` | Enable in both `[search.providers]` and `[search.searxng]` with `base_url` | No |
| `missing_base_url` | Missing base URL | Provider requires a base URL that is not set | Missing `base_url` in `[search.api.<id>]` (Gitea, GitLab self-hosted) | Add `base_url` to the provider's API config section | No |
| `invalid_base_url` | Invalid base URL | Base URL is malformed or unreachable | Typo or incorrect URL in `base_url` | Correct the URL; must be valid HTTP(S) | No |
| `missing_local_backend` | Local backend not available | `local_workspace` provider has no backend | `[local]` section missing or `enabled = false` | Add `[local]` with `enabled = true` and `roots` | No |
| `credential_not_configured` | Credential not configured | Credential entry exists but is not fully configured | Incomplete `[search.api.<id>]` section | Complete the API config section | No |
| `credential_env_missing` | Credential environment variable not set | `api_key_env` is set but the env var is not present at runtime | Environment variable not exported in the shell | Export the env var or add it to your shell profile | No |
| `credential_invalid` | Credential invalid (empty) | Environment variable is set but empty | Env var exported with empty value | Set the env var to a valid value | No |
| `cooldown_active` | Cooldown active | Provider is temporarily suppressed after repeated failures | 3+ consecutive failures (rate limit, timeout, network error) | Wait for cooldown to expire (15–60s depending on failure class) | **Yes** — auto-recovers |
| `not_built` | Not built | Provider was excluded at compile time | Feature-gated or compiled out of the binary | Rebuild with the required feature flag | No |
| `unknown` | Unknown | Catch-all for unrecognized skip conditions | Edge case or internal error | Run `eggsearch doctor` for details | No |

## Provider Health

eggsearch tracks per-provider health state using a built-in health registry. Health state is exposed in `provider_status` output via `health_views` (per-provider compact view) and `health` (full snapshots).

### Health States

| State | Meaning |
|-------|---------|
| `Healthy` | Provider has recorded at least one success, no active failures |
| `Degraded` | Provider has consecutive failures but is not yet in cooldown |
| `Cooldown` | Provider is temporarily suppressed; will auto-recover |
| `Unknown` | No health data recorded yet (fresh start or provider never queried) |

### Cooldown Behavior

After 3 consecutive failures (`COOLDOWN_THRESHOLD = 3`), a provider enters cooldown:

| Failure Class | Cooldown Duration | Recovery Trigger |
|---------------|-------------------|------------------|
| Rate limited | 60 seconds | Single successful query |
| Timeout | 15 seconds | Single successful query |
| Transport / HTTP error | 30 seconds | Single successful query |
| Other | 30 seconds | Single successful query |

A single successful query immediately clears cooldown, resets the failure counter, and restores the provider to `Healthy`.

### Health in `provider_status`

The `health_views` field in `provider_status` provides per-provider health views with:

- `status` — current health state (`Healthy`, `Degraded`, `Cooldown`, `Unknown`)
- `consecutive_failures` — current failure streak
- `last_error_class` — most recent failure class (e.g. `RateLimited`, `Timeout`, `NetworkError`)
- `last_error_message` — human-readable error from the last failure
- `cooldown_until` — remaining cooldown time (e.g. `"42s"`)
- `cooldown_reason` — why cooldown was triggered (e.g. `"rate limited"`, `"repeated timeouts"`)
- `last_latency_ms` — latency of the most recent query
- `last_success_at` — time since last success (e.g. `"15s ago"`)
- `last_failure_at` — time since last failure

### Routing Integration

When resolving which providers to query, eggsearch checks `is_in_cooldown()` for each candidate. Cooled-down providers are skipped with `skip_code: cooldown_active`. If all profile providers are unavailable, routing falls back to `default_providers`. If defaults are also unavailable, a `profile_degraded` warning is emitted.

### Live Probing

One shared probe service (`src/meta/probe.rs`) backs all diagnostics:

- `eggsearch doctor --probe` — prints `[OK]`/`[FAIL]`/`[SKIP]` per provider plus started/succeeded/failed/skipped summary
- MCP `provider_status` with `{"probe": true}` — returns a typed `probe` section with the same outcomes
- `make live-smoke` / `cargo test --features live-smoke --test corpus_runner -- --ignored` — explicit live verification

Probe policy: narrowest `test`/1-result request, 5s per-provider deadline, 20s aggregate deadline, max 4 concurrent probes, 256-char bounded sanitized messages. Failures update advisory health state but never override explicit provider selection. Missing credentials/config yield skipped outcomes with stable `skip_code` (never a misleading network failure). Credentials and raw upstream bodies are never echoed. Routine `make check` stays network-free; live probes run only when explicitly requested.

## Troubleshooting

| Symptom | Cause | Fix | Diagnostic |
|---------|-------|-----|------------|
| Provider not returning results | Disabled in `[search.providers]` | Set the provider to `true` | `provider_status` → check `enabled` field |
| API provider skipped | `api_key_env` not set or env var missing | Set the env var or disable the provider | `provider_status` → check `skip_code: missing_api_key` |
| SearXNG unavailable | Missing `base_url` or both flags not set | Set `base_url` in `[search.searxng]` and enable in `[search.providers]` | `provider_status` → check `skip_code: missing_searxng_config` |
| Gitea/GitLab provider fails | Missing `base_url` | Set `base_url` in the `[search.api.<provider>]` section | `provider_status` → check `skip_code: missing_base_url` |
| Profile degraded warning | A profile provider is unavailable | Configure the missing provider or accept fallback to defaults | `provider_status` → check `skip_code` and `health_view` |
| Provider in cooldown | 3+ consecutive failures | Wait for cooldown to expire (15–60s) or fix the underlying issue | `provider_status` → check `skip_code: cooldown_active` and `health_view.status` |
| Credential env var missing | Env var not exported in shell | Export the env var or add to shell profile | `provider_status` → check `skip_code: credential_env_missing` |
| All providers fail | Network issue or all providers in cooldown | Check network connectivity; wait for cooldown expiry | `provider_status` → check `health` for all providers |
| Provider built with wrong features | `skip_code: not_built` | Rebuild with the required feature flag | `cargo build --features <feature>` |

Run `eggsearch doctor` for a diagnostic summary of all configured providers, their enabled state, API key availability, and any misconfigurations.
