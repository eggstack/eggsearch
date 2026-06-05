# eggsearch — Requirements, Design, and Implementation Plan

This is the canonical plan for the eggsearch project. It supersedes the
older `plans/eggsearch.md` (Tantivy-centered draft) and the earlier
`plans/changeover.md` (Tantivy → metasearch changeover). The
metasearch-first direction described in `changeover.md` is the current
direction and the one this document tracks. Tantivy and the local
indexing path are out of scope. Future integration with the Codegg
coding agent is preserved.

## 1. Purpose

eggsearch is a lightweight MCP (Model Context Protocol) **metasearch**
server for agents. It queries configured upstream search providers at
request time, normalizes and deduplicates results, and returns compact,
provenance-preserving **source cards** suitable for agentic use.

It is not a crawler, not a local web index, and does not require
SearXNG, Tantivy, or a paid search API for the default configuration.
The intended baseline is low operational overhead: one Rust binary, no
external daemon, no API key requirement, and no persistent state
required for normal web search.

Goals:

1. Provide a no-key, fully local-running metasearch MCP server for
   agent hosts (primary target: **Codegg**).
2. Expose search through MCP over stdio.
3. Keep upstream types behind a small adapter boundary; the rest of
   eggsearch depends only on Codegg-owned types.
4. Return compact, structured, provenance-rich source cards instead of
   dumping raw web content into model context.
5. Treat all live web content as untrusted external input.
6. Avoid hard dependency on SearXNG, Brave, Tavily, Exa, or any paid
   API in the default build.

Non-goals for the current implementation:

- Building a local full-text search index (Tantivy, SQLite FTS5, etc.).
- Building a general-purpose crawler at internet scale.
- Reimplementing all of SearXNG.
- Supporting image/video/news search in MVP.
- Returning large raw documents directly to the model by default.
- Requiring Docker, SearXNG, Meilisearch, or any external daemon for
  baseline function.
- Browser automation / JavaScript-rendered SERP handling.
- Embedding-based, vector, or learned ranking.

## 2. Terminology

`metasearch`: Local orchestration over one or more upstream sources.
The code runs locally; the sources may be remote web services or HTML
endpoints.

`provider`: A backend capable of returning search results for a query.
Examples: DuckDuckGo HTML, Startpage HTML, Yahoo HTML, Brave HTML.

`source card`: A compact normalized representation of a search result
suitable for model context. This is the primary unit passed back to
agents.

`upstream`: The third-party `metadata-search-engine-rs` crate that
provides the actual HTTP-based search engine adapters. Eggsearch wraps
it but does not leak its types past the `eggsearch-meta` boundary.

`policy`: The runtime gate that decides whether a given tool is
allowed under the current configuration mode.

`Codegg`: The target MCP host. The server is designed to be invoked
from Codegg (or any other MCP-compatible client) over stdio. The Codegg
host is responsible for user confirmation in `mode = "ask"`.

## 3. High-Level Architecture

```text
eggsearch/
  Cargo.toml
  crates/
    eggsearch-core/   # types: SourceCard, TrustLevel, config, errors
    eggsearch-meta/   # MetadataSearchAdapter wrapping metadata-search-engine-rs
    eggsearch-mcp/    # MCP server (rmcp): web_search + provider_status
    eggsearch-cli/    # binary: doctor, search, providers, mcp stdio
```

The four-crate layout is the canonical shape. Any additional
capability (fetch, local index, etc.) should land as a new crate, not
as a module inside an existing one.

### 3.1 Crate Responsibilities

#### `eggsearch-core`

Core types, source card model, configuration loader, URL canonicalization,
error types. Must not depend on MCP, HTTP, or any search-engine
implementation.

Contains:

```text
config.rs     # AppConfig, SearchSection, Mode, LiveConfig
source_card.rs# SourceCard
query.rs      # WebSearchRequest, SafeSearch
result.rs     # TrustLevel, SearchWarning
error.rs      # CoreError
normalize.rs  # URL canonicalization
lib.rs
```

#### `eggsearch-meta`

Adapter around the upstream `metadata-search-engine-rs` crate.
Constructs the configured engines, dispatches concurrent queries,
classifies provider failures, and converts upstream `AggregatedResult`
values into Codegg-owned `SourceCard` values.

The upstream HTTP engine implementations (DuckDuckGo, Brave,
Startpage, Yahoo HTML) are reused as-is. Eggsearch does not hand-roll
HTML parsers for any provider.

#### `eggsearch-mcp`

MCP server adapter using `rmcp`. Exposes a minimal, stable tool
surface (`web_search`, `provider_status`) and policy enforcement
(`mode = off` denies live tools).

#### `eggsearch-cli`

Manual diagnostics and local operation. The CLI binary `eggsearch`
subcommands:

```text
eggsearch doctor          # report effective config + adapter
eggsearch search <query>  # run a live metasearch, print source cards
eggsearch providers       # list configured providers
eggsearch mcp stdio       # run the MCP server
```

## 4. Core Data Model

### 4.1 SourceCard

Source cards are the primary unit passed back to agents.

```rust
pub struct SourceCard {
    pub id: String,
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub providers: Vec<String>,
    pub score: Option<f64>,
    pub trust: TrustLevel,
    pub fetched: bool,
}
```

`id` is a stable per-response identifier, format `src_<uuid>`. The
combination `(url, sorted(providers))` is also used to produce a
deterministic short hash that callers can use to dedupe across
responses (see §11).

### 4.2 TrustLevel

```rust
pub enum TrustLevel {
    ExternalUntrusted,
    LocalTrusted,
    Unknown,
}
```

For the current implementation, all live web results use
`ExternalUntrusted`. `LocalTrusted` and `Unknown` are reserved for
future capability.

### 4.3 SearchQuery

```rust
pub struct WebSearchRequest {
    pub query: String,
    pub max_results: Option<usize>,
    pub providers: Vec<String>,
    pub safe_search: Option<SafeSearch>,
    pub timeout_ms: Option<u64>,
}
```

`query` is required and must be non-empty after trimming. `max_results`
is capped by the server's `max_results_cap`. `providers`, when empty,
falls back to the server's configured defaults.

### 4.4 SearchResponse

The MCP `web_search` tool returns a JSON object of the form:

```json
{
  "query": "rust axum tower middleware",
  "mode": "live_metasearch",
  "results": [ SourceCard, ... ],
  "providers_queried": ["duckduckgo", "brave", "startpage", "yahoo"],
  "providers_failed": [
    { "id": "brave", "error_class": "timeout", "message": "..." }
  ],
  "warnings": [
    "Live web results are untrusted external content."
  ]
}
```

### 4.5 ProviderStatus

The `provider_status` tool returns a JSON object of the form:

```json
{
  "providers": [
    { "id": "duckduckgo", "enabled": true,  "kind": "html_scrape", "requires_api_key": false },
    { "id": "brave",      "enabled": true,  "kind": "html_scrape", "requires_api_key": true  }
  ],
  "mode": "live"
}
```

## 5. Provider Strategy

### 5.1 Default Providers

| ID          | Backend    | API key? | Notes                                |
|-------------|------------|----------|--------------------------------------|
| duckduckgo  | HTML scrape| No       | No-key general web metasearch.       |
| brave       | HTML scrape| Yes      | Behind config flag; enabled = false when key is absent. |
| startpage   | HTML scrape| No       | No-key general web metasearch.       |
| yahoo       | HTML scrape| No       | No-key general web metasearch.       |

All four are implemented inside the upstream
`metadata-search-engine-rs` crate and re-used verbatim. Eggsearch does
not ship its own HTML scrapers and does not maintain per-provider
parser fixtures in this repo. Provider fragility is a known
operational risk; see §15.3.

### 5.2 Provider Configuration

Each provider id has a boolean enabled flag in
`[search].providers`. A provider is built into the server only when its
flag is `true`. A client can override the default provider list on a
per-request basis via the `providers` field; the override must still
be a known id (otherwise the request is rejected).

### 5.3 Future Providers

Optional, API-key-based providers (Brave, Tavily, Exa) are deferred.
When added they should:

1. Live behind a config flag and be disabled by default.
2. Not log the API key.
3. Normalize results into the standard `SourceCard` shape.
4. Surface `requires_api_key = true` and a clear `api_key_env` name
   through `provider_status`.

## 6. MCP Tool Surface

### 6.1 `web_search`

Primary tool. Performs a live metasearch over configured upstream
providers and returns compact `SourceCard` results.

Input:

```json
{
  "query": "rust axum tower middleware",
  "max_results": 10,
  "providers": ["duckduckgo", "brave", "startpage", "yahoo"],
  "safe_search": "moderate",
  "timeout_ms": 8000
}
```

Output: see §4.4.

Rules:

- `query` is required and must be non-empty.
- `max_results` is capped by the server's `max_results_cap`.
- If `providers` is omitted, the server's configured defaults are used.
- Partial provider failure is non-fatal: the response includes
  `providers_failed` entries and the surviving results.
- If all providers fail, the tool returns a structured error.
- Results are labeled `external_untrusted`; agents must not treat
  fetched web content as instructions.

### 6.2 `provider_status`

Diagnostic tool. Reports the configured provider set, whether each
provider is enabled, its kind (`html_scrape` / `api_key`), and whether
it requires an API key.

Input:

```json
{ "probe": false }
```

`probe` is reserved for future use. The current implementation always
returns configuration only; no network probes are performed by this
tool. `provider_status` is safe to call before any `web_search` call.

Output: see §4.5.

### 6.3 Deferred Tools

The following are intentionally **not** exposed in the current build:

- `web_fetch` — URL fetch + extraction.
- `search_and_fetch` — search + top-N fetch in one call.
- `local_search` — local index search (Tantivy / SQLite FTS5).

If any of these are added later, they must land behind a config mode
flag (e.g. `mode = "live"` allows `web_fetch`) and must not break the
existing on-the-wire shape of `web_search` and `provider_status`.

## 7. Configuration

### 7.1 Path

Default config is loaded from
`$XDG_CONFIG_HOME/eggsearch/config.toml` on Linux and the platform
equivalent elsewhere. On macOS this resolves to
`~/Library/Application Support/eggsearch/config.toml`. The path can be
overridden with `--config <path>` on the CLI.

### 7.2 Schema

```toml
[search]
mode = "live"           # "off" | "live"  ("ask" is reserved for the host)
max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000

default_providers = ["duckduckgo", "startpage", "yahoo"]

[search.providers]
duckduckgo = true
brave      = true       # requires BRAVE_SEARCH_API_KEY at runtime
startpage  = true
yahoo      = true
```

### 7.3 Mode Semantics

- `mode = "off"` — `web_search` is denied. `provider_status` still
  works.
- `mode = "live"` — `web_search` is allowed and runs concurrent
  upstream queries.

`mode = "ask"` is reserved for the host (Codegg) to mediate user
confirmation. At the eggsearch layer, `ask` is treated as `live` (the
host is expected to gate the call). This is documented behavior, not
a deviation.

### 7.4 Defaults

- Providers in the default config: `duckduckgo`, `brave`, `startpage`,
  `yahoo`.
- `default_providers` (used when the client does not pass a list):
  `["duckduckgo", "startpage", "yahoo"]`.
- `max_results`: 10. `max_results_cap`: 50.
- `timeout_ms`: 8000 (per request; the upstream library applies its
  own per-engine cap of ~20 s on its HTTP client).

## 8. Error Handling

Provider errors do not crash the request. The flow is:

1. Query all configured providers concurrently.
2. Collect successful provider results.
3. Collect failed providers with a coarse error class.
4. If at least one provider succeeds, return partial results and
   failures in `providers_failed`.
5. If all providers fail, return a structured tool error.

Provider failure messages do not include raw HTTP response bodies or
credentials. The error class is one of the documented values below.

```text
timeout        # per-provider or global timeout
http_status    # non-2xx upstream response
parse_error    # HTML parser failed
network_error  # transport-level failure
rate_limited   # upstream returned 429
invalid_query  # query rejected by the provider
unknown        # anything else
```

If the global request timeout fires, all in-flight providers are
cancelled and every provider is reported as `timeout` in
`providers_failed`.

## 9. Timeouts and Concurrency

- Global request timeout: `config.search.timeout_ms`. Enforced in the
  adapter via `tokio::time::timeout` around the upstream fan-out.
- Per-provider timeout: delegated to the upstream HTTP client
  (~20 s socket-level cap). Eggsearch does not currently wrap each
  provider in its own `tokio::time::timeout`; see §16.6.
- No automatic retries in the current build. A failed provider
  surfaces immediately as a `providers_failed` entry.
- Concurrency is provided by the upstream `join_all` fan-out. Eggsearch
  does not impose its own concurrency cap.

## 10. Ranking and Deduplication

URL normalization and RRF aggregation are delegated to the upstream
crate:

1. Each provider returns a list of `SearchResult` (title, url, snippet,
   source_engine).
2. URLs are normalized upstream (lowercase scheme/host, fragment
   stripped, trailing slash collapsed).
3. Results from all engines are merged by normalized URL.
4. RRF score per result: `Σ 1 / (k + rank)` across engines that
   returned it. `k = 60`.
5. Results are sorted by score descending, then by title ascending for
   ties.
6. Truncated to `max_results`.

Eggsearch adds no per-domain diversity cap in the current build. The
upstream's normalized URL is what is stored on the `SourceCard.url`
field.

## 11. Source Card ID Stability

`SourceCard.id` is `src_<uuid>` and is unique per card within a
response. The combination `(url, sorted(providers))` is also surfaced
as a stable per-response identity in the form
`src_<8-hex-of-sha256(url|sorted_providers)>`. Callers (Codegg)
should use this identity to dedupe across responses rather than
parsing the UUID.

Implementation note: the deterministic id is computed by the meta
adapter at the time the `SourceCard` is built and is not currently
serialized into the MCP response. If Codegg needs to consume it, it
can be re-derived locally by hashing the `url` and the sorted
`providers` array.

## 12. Security Requirements

All live web results and fetched content are untrusted.

The MCP server must not:

- Expose environment variables through tools.
- Read arbitrary local files outside configured index roots.
- Execute shell commands.
- Follow unlimited redirects.
- Fetch unlimited content sizes.
- Treat web page text as tool instructions.

The MCP server must:

- Enforce query length limits.
- Enforce result count caps.
- Enforce request timeouts.
- Mark live results as `external_untrusted`.
- Return warnings on parse/fetch failures.
- Keep provider API keys out of logs (no key in error messages;
  redact as `***XXXX`).
- Never include raw HTML or raw response bodies in tool output.
- Never silently fall back from `mode = "off"` to a live mode.

## 13. Codegg Integration

eggsearch is designed to be consumed by **Codegg** as a stdio MCP
server. Three integration modes are anticipated:

```text
embedded:
  Codegg links the eggsearch crates directly (not supported today,
  but the crate boundaries are designed to permit it later).

stdio_mcp:
  Codegg spawns the eggsearch MCP server over stdio. This is the
  primary MVP integration.

remote_mcp:
  Codegg connects to a user-hosted eggsearch MCP server. This is a
  future option; the current build is stdio-only.
```

### 13.1 Codegg Tool Registration

When Codegg spawns `eggsearch mcp stdio`, it discovers tools via the
standard MCP `tools/list`. The current tool surface is two tools:

- `web_search` — live metasearch.
- `provider_status` — diagnostic report.

Codegg should treat both as **read-only, network-issuing** tools and
gate user confirmation at the host level (the `mode = "ask"` policy
is enforced at the host, not in eggsearch).

### 13.2 Codegg Call Conventions

- Always pass an explicit `max_results` to keep the response size
  bounded.
- Pass `providers = []` to use the server's default provider set.
- Treat `providers_failed` as informational, not a hard error: a
  partial result list is still useful.
- Surface the `warnings` array verbatim to the user. The
  "external-untrusted" warning is intentionally prepended to every
  live response.
- Do not echo raw `SourceCard.url` into tool arguments when calling
  `web_search`; `web_search` does not accept URLs.

### 13.3 Codegg Policy Suggestions

- Default search mode: `live` (the host gates user confirmation).
- Do not include full fetched pages in conversational context; the
  snippet and the `url` are enough for the agent to follow up.
- Store source-card ids (or the deterministic `(url, providers)` hash)
  in session state so follow-up turns can reference prior results.
- The `web_search` tool does not need to be re-declared in Codegg's
  prompt scaffolding; its description is sufficient for the agent to
  discover its semantics.

### 13.4 Future Codegg Hooks

The following are anticipated but not built today:

- `web_fetch` — give Codegg the ability to pull a single URL's
  extracted text on demand.
- `search_and_fetch` — a single tool that does a search and fetches
  the top N results, returning a compact bundle.
- `local_search` — search over a local corpus (deferred; Tantivy /
  SQLite FTS5 decisions belong to a future change).

Any of these can be added without breaking the existing
`web_search` / `provider_status` shape, as long as the new tool is
gated by a config mode flag and a new variant of `Mode` (e.g. `Fetch`
or `LocalOnly`).

## 14. Testing Requirements

### 14.1 Unit Tests

Unit tests live alongside the code they cover, in `#[cfg(test)] mod
tests` blocks within each module. They must be fast, deterministic,
and free of network I/O. Every module that contains a unit test must
be runnable with `cargo test --lib` and must complete in under a
second on a developer laptop.

Required coverage (current implementation):

- URL canonicalization (lowercase scheme/host, fragment strip,
  tracking parameter strip, trailing slash normalize, invalid URL
  rejection).
- Mode parsing (`off`, `live`; aliases `ask`/`local_only`/`local` are
  intentionally not supported and are rejected).
- Default config loading.
- Config TOML round-trip.
- `resolve_providers` empty-override and dedup behavior.
- `SourceCard` defaults and `with_snippet`.
- `ErrorClass` string stability.
- `convert_aggregated` maps fields, drops empty URLs, drops invalid
  URLs, omits empty snippets.

### 14.2 Adapter Tests (Mock Engines)

The meta adapter is tested with a `MockEngine` implementation that
returns a fixed result set. Tests must cover at least:

- Two mock engines with overlapping results collapse into one card
  with both providers listed.
- An all-fail path returns a `WebSearchResponse` whose
  `providers_failed` lists every engine.
- A global timeout (mock engine sleeps forever, adapter timeout
  short) cancels all in-flight work and reports a timeout per
  provider.

### 14.3 MCP Integration Tests

MCP integration tests use a real `ServerState` with the upstream
`MockEngine` so the happy path can be exercised without network. They
must cover at least:

- Server `initialize` returns the documented server info and tools
  capability.
- `tools/list` returns exactly `web_search` and `provider_status` (and
  no deprecated `web_fetch`, `local_search`, `search_and_fetch`).
- `web_search` with a valid query returns a structured payload
  matching §4.4.
- `web_search` with an empty / whitespace-only query returns a
  validation error.
- `web_search` with a query longer than `max_query_chars` returns a
  validation error.
- `web_search` with `max_results = 0` returns a validation error.
- `web_search` with `max_results > cap` returns a validation error.
- `web_search` with an unknown provider id returns an error.
- `web_search` when `mode = "off"` is denied by policy.
- `web_search` when all providers fail returns a structured error.
- `web_search` with one provider failing returns a non-empty
  `results` and a non-empty `providers_failed`.
- `provider_status` returns one entry per configured provider, each
  with `id`, `enabled`, `kind`, `requires_api_key`.

### 14.4 Live Network Tests

Live network tests against real upstream providers are opt-in and
`#[ignore]`d by default. They are not part of `cargo test`; the
intended opt-in is `RUN_LIVE=1 cargo test -- --ignored`. Live tests
must enforce the same timeouts as the production path.

### 14.5 Provider Parser Fragility

The HTML scrapers live in the upstream crate and are not vendored
here. Layout changes upstream can break eggsearch silently. The
operational response to a breakage is to either:

1. Pin to a known-good upstream version and wait for a fix, or
2. Vendor a known-good revision of the upstream crate and patch the
  affected parser.

This trade-off is documented in the README and is a known operational
risk until parser fixture tests are vendored into this repo (future
work; see §16.7).

## 15. Operational Notes

### 15.1 Logging

`tracing` is used throughout. The CLI installs a `tracing-subscriber`
with an `EnvFilter` driven by `-v` / `-vv`. Setting `RUST_LOG=debug`
works as well.

### 15.2 Configuration Overrides

- `--config <path>` overrides the default config path.
- `EGGSEARCH_CONFIG` environment variable is not currently honored;
  use the CLI flag.

### 15.3 Provider Fragility

HTML-backed providers may break if upstream layouts change. The
mitigations are:

- The adapter never panics on a parse failure; it records a
  `providers_failed` entry and returns what it has.
- `provider_status` lets a host detect which providers are loaded
  and their kind, but it does not perform a network probe.
- A health check (`eggsearch doctor`) is a static configuration
  report. It does not probe upstream liveness.

### 15.4 No Persistent State

The server does not write a database, index, cache, or any other
persistent artifact in the default build. A future cache or artifact
store is out of scope.

## 16. Future Work (Deferred)

The following are intentionally not part of the current build. Each
is a candidate for a future change but must not be added to the
default build without an explicit config gate.

1. `web_fetch` extraction tool.
2. `search_and_fetch` tool.
3. Local corpus search (`local_search`).
4. Persistent result cache.
5. Per-result fetch + artifact store.
6. Per-provider `tokio::time::timeout` wrapping inside the adapter.
7. Vendored HTML parser fixture tests.
8. Optional API-key providers (Brave, Tavily, Exa, Kagi).
9. Optional SearXNG adapter.
10. Codegg `embedded` and `remote_mcp` integration modes.
11. Vector / embedding / learned ranking.
12. Background refresh, scheduled re-crawl, etc.
13. Per-tool rate limiting.
14. HTTP transport for the MCP server (SSE / Streamable HTTP).
15. Tracing/metrics for `web_search` calls (count, latency,
    per-provider outcomes).

## 17. Phased Implementation Status

The current build covers:

- Workspace with four crates, shared `Cargo.toml` workspace deps.
- `eggsearch-core` types (`SourceCard`, `TrustLevel`, `WebSearchRequest`,
  `AppConfig`, `Mode`, `LiveConfig`).
- `eggsearch-meta` adapter wrapping `metadata-search-engine-rs`,
  mapping upstream `AggregatedResult` to `SourceCard`, classifying
  provider failures into `ErrorClass`.
- `eggsearch-mcp` rmcp server with `web_search` and `provider_status`.
- `eggsearch-cli` with `doctor`, `search`, `providers`, `mcp stdio`.
- Unit tests across all four crates; 26 tests pass under `cargo test
  --all` in well under a second.
- One MCP integration test suite (6 tests) covering validation,
  policy, and `provider_status` shape.

The Tantivy path, the fetcher path, the artifact store, the local
index path, and the optional providers (SearXNG, Brave, Tavily, Exa)
have all been removed from the active build.

## 18. Decisions and Deviations

- The upstream `metadata-search-engine-rs` brings in `axum`,
  `tower-http`, `scraper`, and `tracing-subscriber` as transitive
  dependencies. Eggsearch uses only `aggregator` + `engines` +
  `models` + `error` + `normalizer`. A future fork of the upstream is
  an option if the binary size or the server-specific code becomes
  a problem (see §16 and the README).
- `rmcp` is pinned to `1.x`; `schemars` is pinned to `1.x` to match.
- The CLI's `mcp` subcommand is `stdio` only. HTTP transport is
  deferred.
- `Mode::from_str` accepts only `"off"` and `"live"`. The `ask`,
  `local_only`, `localonly`, and `local` aliases that were accepted
  in the previous build are rejected; `mode = "ask"` is documented as
  a host-level policy and is not a value the config can take.
- `web_search` does not perform `query` trimming. Whitespace-only
  queries are rejected by validation. Trailing whitespace is not
  silently trimmed because the underlying upstream does its own
  query handling and the original input is what the agent intended
  to search for.

## 19. Summary Recommendation

The current eggsearch build is a working metasearch MCP server that
exposes two stable tools (`web_search`, `provider_status`) over stdio
and can be consumed by Codegg (or any other MCP-compatible host) with
no API keys, no external daemon, and no persistent state. The next
substantive expansion is per-provider fixture tests plus a real
MCP-level happy-path test suite; both are scoped under §14 and §16.
