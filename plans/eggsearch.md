# Eggsearch MCP Search Server — Requirements and Implementation Spec

## 1. Purpose

Build a standalone Rust MCP search server that can be used by Codegg and other MCP-capable clients as a local-first search substrate.

The server should provide normalized web search, URL fetching, local indexed search, and compact source-card output suitable for agentic coding workflows. It should not require SearXNG as a runtime dependency. SearXNG-compatible behavior and adapters may be supported, but SearXNG must remain optional.

The project may use `search-engine-rs` / `searxng-rust` as architectural inspiration or as source material if licensing permits. The long-term design should be provider-agnostic and should not inherit unnecessary SearXNG complexity such as UI preferences, broad engine category systems, public instance management, or administrative features.

Primary goals:

- Provide a no-key, local-first search backend for Codegg.
- Expose search through MCP over stdio and eventually HTTP/SSE if useful.
- Keep search, fetch, extraction, cache, and indexing as separate layers.
- Support both live metasearch and fully local indexed search.
- Return compact, structured, provenance-rich source cards instead of dumping raw web content into model context.
- Treat all live web and fetched content as untrusted external input.
- Avoid hard dependency on SearXNG, Brave, Tavily, Exa, or any paid API.

Non-goals for the initial implementation:

- Building a full browser automation stack.
- Building a general-purpose crawler at internet scale.
- Reimplementing all of SearXNG.
- Supporting image/video/news search in MVP unless trivial through a provider.
- Returning large raw documents directly to the model by default.
- Requiring Docker, SearXNG, Meilisearch, or any external daemon for baseline function.

## 2. Terminology

`metasearch`: Local orchestration over one or more upstream sources. The code runs locally, but the sources may be remote web services or HTML endpoints.

`local search`: Search over a local index populated from cached pages, local files, docs, project repositories, or curated corpora.

`provider`: A backend capable of returning search results for a query. Examples: DuckDuckGo HTML, Wikipedia API, crates.io API, docs.rs, SearXNG adapter, Brave API.

`fetcher`: A component that retrieves content from a known URL.

`extractor`: A component that converts fetched content into structured text, metadata, and snippets.

`source card`: A compact normalized representation of a search result or fetched document, suitable for model context.

`artifact`: Stored external content or extracted document data that can be referenced by ID instead of inserted fully into context.

## 3. High-Level Architecture

Recommended repository layout:

```text
eggsearch/
  Cargo.toml
  crates/
    eggsearch-core/
    eggsearch-meta/
    eggsearch-fetch/
    eggsearch-local/
    eggsearch-mcp/
    eggsearch-cli/
```

Alternative name is acceptable. Internally, use boring module names even if the repository name is project-branded.

### 3.1 Crate Responsibilities

#### `eggsearch-core`

Core traits, types, normalization, deduplication, ranking, source cards, errors, and configuration structures.

Must not depend on MCP-specific types.

Contains:

```text
query.rs
result.rs
provider.rs
normalize.rs
dedupe.rs
rank.rs
source_card.rs
error.rs
config.rs
```

#### `eggsearch-meta`

Live metasearch providers.

Initial providers:

```text
providers/duckduckgo_html.rs
providers/wikipedia.rs
providers/crates_io.rs
providers/docs_rs.rs
```

Optional/future providers:

```text
providers/searxng.rs
providers/brave_api.rs
providers/tavily.rs
providers/exa.rs
providers/kagi.rs
providers/bing_html.rs
```

HTML-scraping providers must be considered fragile and must have offline parser fixture tests.

#### `eggsearch-fetch`

URL fetching, robots policy, content-type detection, extraction, readability, cache, and artifact creation.

Contains:

```text
fetch.rs
robots.rs
extract.rs
html.rs
markdown.rs
cache.rs
artifact.rs
```

MVP extraction should support HTML and plain text. PDF support should be feature-gated or deferred.

#### `eggsearch-local`

Local indexing and search.

Initial backend should prefer Tantivy for a Rust-native embedded search index. SQLite FTS5 may be added as a simpler optional backend if Codegg already standardizes around SQLite state.

Contains:

```text
index.rs
tantivy_backend.rs
sqlite_fts_backend.rs
corpus.rs
ingest.rs
schema.rs
```

#### `eggsearch-mcp`

Thin MCP adapter over core/search/fetch/local services.

Use the official Rust MCP SDK (`rmcp`) unless there is a strong reason not to.

Initial MCP tools:

```text
web_search
web_fetch
local_search
search_and_fetch
```

#### `eggsearch-cli`

Manual diagnostics and local operation.

Commands:

```text
eggsearch doctor
eggsearch search <query>
eggsearch fetch <url>
eggsearch index add <path-or-url>
eggsearch index search <query>
eggsearch mcp stdio
```

## 4. Core Data Model

### 4.1 Search Query

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub max_results: usize,
    pub language: Option<String>,
    pub region: Option<String>,
    pub safe_search: SafeSearch,
    pub freshness: Option<Freshness>,
    pub include_domains: Vec<String>,
    pub exclude_domains: Vec<String>,
    pub categories: Vec<SearchCategory>,
}
```

Recommended defaults:

```text
max_results = 8
safe_search = Moderate
language = None
region = None
freshness = None
categories = [General]
```

### 4.2 Search Provider Trait

```rust
#[async_trait::async_trait]
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> &'static str;

    async fn search(
        &self,
        query: SearchQuery,
        ctx: SearchContext,
    ) -> Result<SearchProviderResponse, SearchError>;
}
```

`SearchContext` should include timeout, user-agent, network mode, request ID, and optional provider-specific config.

```rust
#[derive(Clone, Debug)]
pub struct SearchContext {
    pub request_id: uuid::Uuid,
    pub timeout: std::time::Duration,
    pub user_agent: String,
    pub network_mode: NetworkMode,
}
```

### 4.3 Provider Response

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchProviderResponse {
    pub provider_id: String,
    pub query: SearchQuery,
    pub results: Vec<SearchResult>,
    pub warnings: Vec<SearchWarning>,
    pub raw_response_hash: Option<String>,
    pub elapsed_ms: u64,
}
```

### 4.4 Search Result

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: url::Url,
    pub snippet: Option<String>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub rank: usize,
    pub score: Option<f32>,
    pub provider_id: String,
    pub source_kind: SourceKind,
    pub trust_level: TrustLevel,
}
```

`trust_level` should default to `ExternalUntrusted` for live web results.

### 4.5 Source Card

Source cards are the primary unit passed back to agents.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceCard {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub snippet: Option<String>,
    pub provider_id: String,
    pub source_kind: SourceKind,
    pub trust_level: TrustLevel,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub fetched_at: Option<chrono::DateTime<chrono::Utc>>,
    pub artifact_id: Option<String>,
    pub warnings: Vec<String>,
}
```

The MCP server should return source cards and optional artifact references, not full raw pages by default.

## 5. Provider Strategy

### 5.1 Required MVP Providers

#### DuckDuckGo HTML / Lite

Purpose: no-key general web metasearch.

Requirements:

- Use a stable/simple endpoint if possible, preferably HTML or lite HTML.
- Parse with `scraper` or equivalent.
- Never panic on unexpected layout changes.
- Emit parser warnings if result extraction is suspiciously empty.
- Include offline fixture tests.
- Apply strict timeout and max response-size limits.

Risks:

- HTML layout may change.
- Rate limiting may occur.
- Upstream may block automated requests.

Mitigation:

- Treat as best-effort.
- Provide clear warnings.
- Add more no-key structured providers to reduce dependency on one fragile source.

#### Wikipedia

Purpose: structured, stable, no-key knowledge search.

Requirements:

- Use Wikipedia API or REST endpoint.
- Return articles as `SourceKind::Reference`.
- Prefer page title, canonical URL, short extract/snippet.
- Support language if easy.

#### crates.io

Purpose: Rust package search useful for Codegg.

Requirements:

- Search crates by query.
- Return crate name, description, version if available, crate URL.
- Mark as `SourceKind::PackageRegistry`.
- This provider gives Codegg high-value technical discovery without scraping general search.

#### docs.rs

Purpose: Rust documentation discovery.

Requirements:

- Provide search over docs.rs pages if feasible.
- If no stable search endpoint exists, implement a conservative docs.rs URL/doc lookup provider rather than brittle broad scraping.
- Useful query forms:
  - crate name
  - crate name plus symbol
  - docs.rs URL normalization

### 5.2 Optional Providers

#### SearXNG Adapter

SearXNG must be optional.

Requirements:

- Accept `base_url` config.
- Use JSON output.
- Fail clearly if JSON output disabled.
- Treat as just another provider.

#### Brave API

Optional API-key provider.

Requirements:

- Read key from environment variable or config secret reference.
- Do not log key.
- Normalize results into standard source cards.

#### Tavily / Exa / Kagi

Optional API-key providers.

Requirements:

- Keep provider-specific fields in metadata.
- Normalize into common result model.
- Do not make any of these required for default operation.

#### Bing / Google HTML

Experimental only.

Requirements:

- Disabled by default.
- Clearly marked fragile.
- Must have parser fixture tests.

## 6. Local Indexing

### 6.1 Local Index Backend

Use Tantivy as the first-class embedded local search backend.

Required indexed fields:

```text
id
title
body
url_or_path
source_kind
trust_level
fetched_at
published_at
content_hash
tags
```

Recommended schema:

```rust
pub struct IndexedDocument {
    pub id: String,
    pub title: String,
    pub body: String,
    pub url: Option<String>,
    pub path: Option<PathBuf>,
    pub source_kind: SourceKind,
    pub trust_level: TrustLevel,
    pub fetched_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub content_hash: String,
    pub tags: Vec<String>,
}
```

### 6.2 Ingestion Sources

MVP ingestion:

- Local Markdown files.
- Local plain text files.
- Fetched web pages from cache/artifact store.
- Project README/docs files.

Future ingestion:

- Rustdoc output.
- docs.rs mirrored pages.
- GitHub repositories.
- Dependency documentation.
- PDFs with feature-gated extraction.

### 6.3 Local Search Behavior

`local_search` must never perform live network access.

This is a hard invariant.

If no local index exists, return a structured error or empty result with diagnostic instructions.

Example response warning:

```text
Local index is empty. Run `eggsearch index add <path>` or enable cached web indexing.
```

## 7. Fetching and Extraction

### 7.1 Fetch Request

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FetchRequest {
    pub url: url::Url,
    pub max_bytes: usize,
    pub timeout_ms: u64,
    pub extract_mode: ExtractMode,
    pub respect_robots_txt: bool,
}
```

Defaults:

```text
max_bytes = 2 MiB
timeout_ms = 8000
extract_mode = Readability
respect_robots_txt = true
```

### 7.2 Fetch Provider Trait

```rust
#[async_trait::async_trait]
pub trait FetchProvider: Send + Sync {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchedDocument, FetchError>;
}
```

### 7.3 Extracted Document

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtractedDocument {
    pub title: Option<String>,
    pub url: String,
    pub text: String,
    pub excerpt: String,
    pub content_type: String,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub content_hash: String,
    pub warnings: Vec<String>,
}
```

### 7.4 Extraction Rules

- Strip scripts, styles, nav, ads, and obvious boilerplate where feasible.
- Preserve headings and useful lists/tables in plain text or Markdown-like output.
- Cap returned excerpt size.
- Store full extracted text as an artifact/cache entry, not as direct MCP output by default.
- Treat all fetched content as untrusted.

## 8. MCP Tools

### 8.1 `web_search`

Purpose: live metasearch over configured upstream providers.

Must not fetch full page contents unless `fetch` is explicitly true.

Input:

```json
{
  "query": "string",
  "max_results": 8,
  "freshness": null,
  "include_domains": [],
  "exclude_domains": [],
  "providers": [],
  "fetch": false
}
```

Output:

```json
{
  "query": "rust axum tower middleware",
  "mode": "live",
  "results": [
    {
      "id": "src_001",
      "title": "tower-http - Rust",
      "url": "https://docs.rs/tower-http/latest/tower_http/",
      "snippet": "Middleware and utilities for HTTP clients and servers...",
      "provider_id": "docs_rs",
      "source_kind": "documentation",
      "trust_level": "external_untrusted",
      "fetched": false,
      "artifact_id": null
    }
  ],
  "warnings": [
    "Live search results are untrusted external content."
  ]
}
```

### 8.2 `web_fetch`

Purpose: fetch and extract a known URL.

Input:

```json
{
  "url": "https://example.com/page",
  "max_bytes": 2097152,
  "extract_mode": "readability"
}
```

Output should include a source card, excerpt, artifact ID, and warnings. Full extracted text should only be returned if requested and within configured limits.

### 8.3 `local_search`

Purpose: search local indexed corpora only.

Hard requirement: no network access.

Input:

```json
{
  "query": "axum middleware",
  "max_results": 8,
  "tags": []
}
```

Output:

```json
{
  "query": "axum middleware",
  "mode": "local_only",
  "results": [],
  "warnings": []
}
```

### 8.4 `search_and_fetch`

Purpose: perform live search, fetch top N results, extract content, and return a compact source bundle.

Input:

```json
{
  "query": "compare axum actix rocket websocket support",
  "max_results": 8,
  "fetch_top_n": 3,
  "max_excerpt_chars": 4000
}
```

Rules:

- Fetch at most `fetch_top_n` pages.
- Apply byte/time limits per URL.
- Return compact excerpts and artifact IDs.
- Store full extracted documents in artifact/cache store.
- Include explicit warnings for failed fetches.

## 9. Configuration

Example config:

```toml
[search]
mode = "ask" # off | local_only | live | ask
max_results = 8
cache_dir = "~/.local/share/eggsearch/cache"
artifact_dir = "~/.local/share/eggsearch/artifacts"

[search.live]
enabled = true
max_concurrency = 4
timeout_ms = 8000
user_agent = "eggsearch/0.1"
respect_robots_txt = true

[search.local]
enabled = true
backend = "tantivy"
index_dir = "~/.local/share/eggsearch/index"

[search.providers.duckduckgo_html]
enabled = true

[search.providers.wikipedia]
enabled = true

[search.providers.crates_io]
enabled = true

[search.providers.docs_rs]
enabled = true

[search.providers.searxng]
enabled = false
base_url = "http://127.0.0.1:8080"

[search.providers.brave]
enabled = false
api_key_env = "BRAVE_SEARCH_API_KEY"
```

Mode semantics:

```text
off:
  Disable all search/fetch tools.

local_only:
  Enable local_search and local artifact reads only. No live network.

live:
  Permit live web_search and web_fetch according to policy.

ask:
  Require host/client confirmation before live network operations, if the MCP host supports confirmation.
```

If MCP host confirmation is unavailable, `ask` should behave conservatively. For Codegg, the host can mediate confirmation before invoking live tools.

## 10. Ranking and Deduplication

### 10.1 URL Normalization

Must:

- Lowercase scheme and host.
- Remove URL fragments unless semantically needed.
- Strip common tracking parameters, including `utm_*`, `fbclid`, `gclid`, and similar.
- Normalize trailing slashes conservatively.
- Preserve query parameters that appear semantically relevant.

### 10.2 Deduplication

Apply:

- Exact canonical URL dedupe.
- Similar title dedupe.
- Optional domain diversity cap.

### 10.3 Rank Fusion

Use reciprocal rank fusion for multi-provider results:

```text
score(result) = sum over providers of 1 / (k + provider_rank)
```

Initial `k = 60`.

Provider-specific scores should not be trusted as comparable across providers.

### 10.4 Source Diversity

Avoid returning eight results from the same domain unless the query explicitly scopes to that domain.

## 11. Cache and Artifact Store

### 11.1 Search Cache

Cache key should include:

```text
provider_id
normalized query
max_results
language
region
freshness
include/exclude domains
safe_search
```

### 11.2 Fetch Cache

Cache key should include canonical URL and relevant request options.

Use HTTP caching headers where feasible:

- ETag
- Last-Modified
- Cache-Control

### 11.3 Artifact Store

Artifacts should store extracted documents by content hash or stable ID.

Artifact metadata:

```text
artifact_id
url/path
title
content_hash
fetched_at
content_type
trust_level
extractor_version
```

Codegg should receive artifact references and excerpts. Full content should be retrieved explicitly only when needed.

## 12. Security Requirements

All live web results and fetched content must be considered untrusted.

The MCP server must not:

- Expose environment variables through tools.
- Read arbitrary local files outside configured index roots.
- Execute shell commands.
- Follow unlimited redirects.
- Fetch unlimited content sizes.
- Automatically crawl additional links unless explicitly configured.
- Treat web page text as tool instructions.

The MCP server must:

- Enforce timeout limits.
- Enforce response-size limits.
- Mark live results as `ExternalUntrusted`.
- Return warnings on parse/fetch failures.
- Keep provider API keys out of logs.
- Keep live network access explicitly configurable.
- Avoid silently falling back from local-only mode to live mode.

Prompt injection mitigation:

- Fetched page content should be wrapped as data.
- Tool output should include a warning that external content is untrusted.
- The server should not summarize fetched content by itself using an LLM in MVP.
- If summarization is added later, it should be an explicit separate layer.

## 13. Testing Requirements

### 13.1 Unit Tests

Required:

- URL canonicalization.
- Tracking parameter stripping.
- Deduplication.
- Rank fusion.
- Source card generation.
- Config parsing.
- Error formatting.

### 13.2 Provider Parser Fixture Tests

Every HTML-backed provider must have offline fixture tests.

Example layout:

```text
tests/fixtures/duckduckgo/basic.html
tests/fixtures/duckduckgo/no_results.html
tests/fixtures/duckduckgo/layout_changed.html
```

Tests should verify:

- Parser extracts expected number of results.
- Parser does not panic on malformed HTML.
- Parser emits warnings on suspicious empty parse.
- URLs are normalized.

### 13.3 Integration Tests

Required:

- MCP server starts over stdio.
- `web_search` returns structured JSON for mocked provider.
- `web_fetch` returns artifact ID for mocked HTTP response.
- `local_search` never performs network access.
- Live mode disabled blocks `web_search` and `web_fetch`.

### 13.4 Golden Response Tests

Add JSON golden tests for MCP tool responses so downstream Codegg integration can rely on stable shape.

### 13.5 Network Tests

Live network tests must be opt-in and ignored by default.

Use environment variable:

```text
eggsearch_LIVE_TESTS=1
```

## 14. CLI Requirements

### 14.1 `doctor`

Should check:

- Config file readable.
- Cache directory writable.
- Artifact directory writable.
- Local index accessible.
- Enabled providers load.
- Optional SearXNG URL reachable if configured.
- MCP server can instantiate.

### 14.2 `search`

Manual live search:

```bash
eggsearch search "rust axum middleware"
```

Options:

```text
--provider duckduckgo_html
--max-results 8
--json
--fetch-top-n 3
```

### 14.3 `fetch`

Manual fetch:

```bash
eggsearch fetch https://docs.rs/tower-http/latest/tower_http/
```

### 14.4 `index`

Index local docs:

```bash
eggsearch index add ./README.md
eggsearch index add ./docs
eggsearch index search "middleware"
```

### 14.5 `mcp`

Run MCP server:

```bash
eggsearch mcp stdio
```

Future:

```bash
eggsearch mcp http --addr 127.0.0.1:9191
```

## 15. Codegg Integration Requirements

Codegg should support three integration modes:

```text
embedded:
  Codegg links the core crates directly.

stdio_mcp:
  Codegg spawns eggsearch MCP server over stdio.

remote_mcp:
  Codegg connects to user-hosted MCP server.
```

MVP can implement only `stdio_mcp` from Codegg’s perspective, but the crate design should not prevent embedded use later.

Codegg policy suggestions:

- Default search mode: `ask` or `local_only`.
- Allow explicit `/search`, `/fetch`, and `/research` commands.
- Do not allow background live web access unless user enables it.
- Do not include full fetched pages in conversational context.
- Store source cards/artifact IDs in session state.
- Use local_search automatically for indexed project docs and cached sources.

## 16. MVP Phases

### Phase 0 — Skeleton

Deliverables:

- Workspace with crates.
- Core types and traits.
- Basic config loading.
- MCP server skeleton over stdio.
- Mock provider.
- `web_search` tool returning mock source cards.
- CLI `doctor` and `mcp stdio` commands.

Acceptance criteria:

- `cargo test` passes.
- MCP client can list tools.
- `web_search` returns valid structured response using mock provider.

### Phase 1 — Bare-Bones Live Metasearch

Deliverables:

- DuckDuckGo HTML provider.
- Wikipedia provider.
- crates.io provider.
- URL normalization.
- Deduplication.
- Rank fusion.
- Provider fixture tests.

Acceptance criteria:

- Live search works when enabled.
- HTML parser tests pass offline.
- Empty/changed layout does not panic.
- Results are returned as source cards.

### Phase 2 — Fetch and Extract

Deliverables:

- `web_fetch` tool.
- Reqwest fetcher.
- Timeout and byte limits.
- HTML/plain-text extraction.
- Cache/artifact store.
- `search_and_fetch` tool.

Acceptance criteria:

- Fetch returns source card + excerpt + artifact ID.
- Full content is stored outside normal MCP response.
- Failed fetches return structured warnings.
- Configured byte/time limits are enforced.

### Phase 3 — Local Index

Deliverables:

- Tantivy local index.
- Ingest local Markdown/text files.
- Index fetched artifacts.
- `local_search` tool.
- CLI `index add` and `index search`.

Acceptance criteria:

- `local_search` works without network.
- Local-only mode blocks live tools.
- Indexed fetched pages retain external-untrusted label.
- Local project docs can be searched by Codegg.

### Phase 4 — Optional Provider Expansion

Deliverables:

- Optional SearXNG adapter.
- Optional Brave API provider.
- Optional Tavily or Exa provider.
- Provider-specific config and diagnostics.

Acceptance criteria:

- Optional providers are disabled unless configured.
- Missing API keys produce clear diagnostics.
- SearXNG JSON-disabled failure is clear.

### Phase 5 — Codegg Deep Research Integration

Deliverables:

- Codegg tool registration.
- `/search`, `/fetch`, `/local-search`, `/research-light` commands.
- Source-card session storage.
- Artifact pointer integration.
- Policy gating for live network access.

Acceptance criteria:

- Codegg can search without API keys through local MCP.
- Codegg can search local index with no network.
- Search results do not pollute context with full pages.
- User can inspect provenance for claims.

## 17. Licensing Requirements

Before copying or adapting code from `search-engine-rs` / `searxng-rust`, inspect its license.

Rules:

- If permissive license: copying/adapting may be acceptable with attribution and license compliance.
- If GPL/AGPL: decide deliberately whether the new project should use compatible licensing.
- If license is absent or unclear: do not copy code; use clean-room reimplementation.
- Avoid copying from SearXNG proper unless AGPL compatibility is intended.

Architecture and ideas may be referenced, but source code reuse must follow license obligations.

## 18. Recommended Dependencies

Core:

```text
anyhow or thiserror
async-trait
serde
serde_json
chrono
url
uuid
tracing
```

HTTP/fetch:

```text
reqwest
tokio
scraper
html5ever or kuchiki if needed
robotstxt or equivalent
sha2
```

MCP:

```text
rmcp
rmcp-macros
```

Local index:

```text
tantivy
walkdir
ignore
```

CLI/config:

```text
clap
figment or config
dirs
```

Testing:

```text
wiremock or httpmock
insta for golden snapshots
pretty_assertions
```

## 19. Open Design Questions

- Should the local index use Tantivy only, or also offer SQLite FTS5 for simpler deployment?
- Should fetched web artifacts be automatically indexed, or only when configured?
- Should Codegg’s project docs be indexed automatically on session start?
- Should `search_and_fetch` be exposed to models, or should Codegg orchestrate search + fetch itself?
- Should there be an allowlist mode for live domains in high-security workflows?
- Should docs.rs be implemented as a direct provider, or should Rust docs be handled primarily through local dependency-doc indexing?
- Should hosted MCP mode require authentication from the start?

## 20. Initial Implementation Checklist

- [x] Create Rust workspace.
- [x] Add `eggsearch-core` types.
- [x] Add provider trait and mock provider.
- [x] Add source-card serialization.
- [x] Add URL normalization tests.
- [x] Add MCP stdio server with `web_search` mock.
- [x] Add CLI `doctor`.
- [x] Add DuckDuckGo HTML provider with fixtures.
- [x] Add Wikipedia provider.
- [x] Add crates.io provider.
- [x] Add result dedupe and rank fusion.
- [x] Add fetcher and extractor.
- [x] Add artifact store.
- [x] Add Tantivy local index.
- [x] Add `local_search` tool.
- [x] Add config policy enforcement.
- [ ] Add Codegg MCP registration plan. (Out of scope: lives in Codegg repo.)

## 21. Summary Recommendation

Build a new Rust search MCP project with a reusable core crate and an MCP adapter. Use `search-engine-rs` / `searxng-rust` as reference material, but avoid binding the project to SearXNG semantics or runtime requirements.

The first useful version should provide:

- No-key live metasearch through a small set of providers.
- Fully local indexed search through Tantivy.
- URL fetch/extraction with strict limits.
- MCP tools that return compact source cards.
- Clear trust labels and provenance.
- Codegg-friendly context discipline.

This gives Codegg a search layer that can work locally, through a self-hosted MCP server, or with optional provider APIs later, without making SearXNG a hard requirement.

## 22. Session Log

### 2026-06-05 — Initial implementation

Phases 0–3 of the MVP plan were implemented end-to-end in a single session.
Phase 4 (SearXNG / Brave / Tavily / Exa adapters) and Phase 5 (Codegg
integration) were intentionally deferred; the provider registry and config
plumbing are designed to accept them later without structural changes.

Workspace layout created exactly as specified in §3.1:

```text
crates/
  eggsearch-core/   # types, traits, normalize, dedupe, RRF, source cards
  eggsearch-meta/   # mock + DuckDuckGo HTML + Wikipedia + crates.io + docs.rs
  eggsearch-fetch/  # reqwest fetcher, HTML extract, cache, artifact store
  eggsearch-local/  # Tantivy backend, file ingest, local search
  eggsearch-mcp/    # rmcp server: web_search, web_fetch, local_search, search_and_fetch
  eggsearch-cli/    # doctor, search, fetch, index, mcp stdio
```

Phase 0 (Skeleton) — done:
- Workspace with all six crates, shared `Cargo.toml` workspace deps.
- Core types, traits, normalize, dedupe, RRF, source cards, config.
- Mock provider registered by default in the registry.
- MCP stdio server with `web_search` returning source cards.
- CLI `doctor` (config / cache / artifact / index writability + server instantiation).
- CLI `mcp stdio` command.

Phase 1 (Bare-bones live metasearch) — done:
- DuckDuckGo HTML provider with offline fixture tests
  (`tests/fixtures/duckduckgo/{basic,no_results}.html`); defensive parser
  that never panics on malformed HTML and emits a warning on empty parses.
- Wikipedia provider using the Action API search endpoint.
- crates.io provider using `/api/v1/crates?q=`.
- docs.rs conservative crate-name lookup (no public full-text endpoint
  available; the provider returns a source card for the canonical URL).
- URL canonicalization (lowercase scheme/host, fragment strip, tracking
  parameter strip, trailing slash normalization).
- Deduplication by canonical URL and similar-title (Jaccard threshold).
- Reciprocal rank fusion with `k=60` for multi-provider results.
- Per-domain cap of 3 by default.

Phase 2 (Fetch and extract) — done:
- `web_fetch` tool with `Raw` / `Text` / `Readability` / `Markdown` modes.
- `ReqwestFetchProvider` with timeout, `Content-Length` pre-check, and
  streaming body cap to enforce `max_bytes`.
- HTML extraction that strips scripts, styles, nav, footer, sidebar,
  ad-classed elements; preserves headings/paragraphs/lists in plain text.
- robots.txt policy cache (one-hour TTL, wildcard `User-agent` parsing,
  pre-flight allow check before any fetch).
- `FetchCache` (in-memory with TTL + max entries).
- `ArtifactStore` (content-hash sharded on disk, JSON sidecar metadata).
- `search_and_fetch` tool that runs a search, fetches the top N results,
  stores full text in the artifact store, and returns compact excerpts.

Phase 3 (Local index) — done:
- `TantivyIndex` with schema: id, title, body, url, path, source_kind,
  trust_level, fetched_at, published_at, content_hash, tags.
- `LocalCorpus` high-level wrapper.
- File ingest for `.md`, `.markdown`, `.txt`, `.html`, `.htm`, `.rst`,
  `.adoc`; hidden directories (dotfiles) skipped, including the root
  tempdir case used in tests.
- `local_search` tool with optional tag filter; emits a structured
  warning when the index is empty.
- CLI `index add <path>`, `index search <query>`, `index stats`.
- Hard invariant enforced: `local_search` never performs network I/O.

Phase 4 (Optional providers) — deferred:
- Provider registry exposes the trait; SearXNG / Brave / Tavily / Exa
  can be added as `Arc<dyn SearchProvider>` impls without touching
  existing code. Default-mode configuration (`searxng.enabled = false`,
  `brave.enabled = false`) is already in `AppConfig::default()`.

Phase 5 (Codegg integration) — out of scope:
- Lives in the Codegg repo. eggsearch's stable JSON shape and rmcp
  `tools/list` schema are sufficient for Codegg to register and invoke
  all four tools.

### Verification

- `cargo build --release` — succeeds.
- `cargo test` — 45/45 pass (16 core, 9+3 fetch, 4 local, 5 mcp, 8 meta).
- `eggsearch doctor` — `healthy: true`, all 5 checks pass.
- `eggsearch search "hello" --provider mock` — 3 source cards.
- `eggsearch index add <dir>` + `eggsearch index search "axum middleware"`
  — 1 hit, 2 docs in index.
- `eggsearch mcp stdio` over a JSON-RPC stream — `initialize` succeeds
  with `serverInfo.name = "eggsearch"`; `tools/list` returns all four
  tools with full JSON schemas; `tools/call local_search` round-trips
  a SourceCard payload.
- Policy enforcement: `web_search` and `local_search` are denied when
  `[search].mode = "off"` and the call returns a structured policy
  error.

### Decisions / deviations

- `rmcp` is pinned to `1.x` (current `1.7.0`); the plan referenced
  `0.1`. `schemars` is also pinned to `1.x` to match rmcp's dep graph.
  `url` is enabled with the `serde` feature.
- The CLI's `mcp` subcommand is currently `stdio` only. The `http` /
  SSE transport from §14.5 is not implemented and has no concrete
  transport dependency wired up.
- `html2text` was dropped from the dependency tree in favor of a
  hand-rolled extractor built on `scraper`, since we already need
  `scraper` for HTML parsing and the hand-rolled path is simpler.
- `MCP_HOST` confirmation / `Mode::Ask` is a no-op at the eggsearch
  layer: it is the responsibility of the host (e.g. Codegg) to
  mediate confirmation. eggsearch trusts the host in `Ask` mode and
  allows live tools.

