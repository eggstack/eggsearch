# Search MCP Changeover Plan: From Tantivy-Centered Local Indexing to Metasearch-First Backend

## 1. Purpose

This document defines the changeover plan for moving the search MCP implementation away from a Tantivy-centered local indexing architecture and toward a lightweight metasearch-first MCP server. The new direction uses `MikeLuu99/searxng-rust`, published as `metadata-search-engine-rs`, as the primary seed implementation or direct dependency for live metasearch.

The goal is not to discard Tantivy permanently. Tantivy should be demoted from the core path to an optional future capability for deep-research artifact indexing or local corpus search. The immediate product goal is a no-key, local MCP metasearch server that queries upstream engines, normalizes results, deduplicates results, ranks/merges results, and returns structured source cards to Codegg or any MCP-compatible host.

## 2. Revised Product Direction

The project is now a metasearch MCP backend, not a local search engine.

The core server should perform request-time metasearch across configured upstream sources. It should not crawl the web, maintain a persistent web index, or require SearXNG, Tantivy, Solr, Lucene, Meilisearch, Elasticsearch, or any database. The intended baseline is low operational overhead: one Rust binary, no external daemon, no API key requirement, and no persistent state required for normal web search.

The server should expose search through MCP using structured, provenance-preserving responses suitable for agentic use. Search output must be compact and must not push raw HTML or full documents into model context.

## 3. Changeover Summary

Existing Tantivy route:

```text
query
  -> local index lookup
  -> indexed/cached corpus
  -> snippets/results
```

New metasearch route:

```text
query
  -> configured upstream search providers
  -> concurrent live requests
  -> parse provider responses
  -> normalize URLs
  -> deduplicate
  -> rank/merge
  -> return SourceCards over MCP
```

Tantivy should be retained only if it is already implemented cleanly enough to feature-gate. Otherwise, remove it from the active path and defer local indexing to a later phase.

## 4. Hard Requirements

The new implementation must satisfy these requirements.

1. No hard dependency on SearXNG runtime.
2. No hard dependency on Tantivy for core web search.
3. No persistent index required for `web_search`.
4. No database required for MVP.
5. No paid API key required for MVP.
6. No browser automation required for MVP.
7. Search must tolerate partial provider failure.
8. Provider calls must have bounded timeouts.
9. Returned results must be normalized, deduplicated, ranked, and structured.
10. Tool output must clearly label external web content as untrusted.
11. The MCP interface must be stable enough for Codegg to consume without provider-specific logic.
12. HTML parser behavior must be covered by fixture tests for each HTML-scraped provider.

## 5. Non-Goals

The following are explicitly out of scope for the metasearch MVP.

1. Full-text local indexing as the default search mechanism.
2. Long-term cached search-result storage.
3. Web crawling.
4. Browser automation or JavaScript-rendered SERP handling.
5. Building a general SearXNG replacement with user-facing web UI.
6. Search personalization.
7. Embedding/vector search.
8. Learned ranking.
9. Global persistent research database.
10. Automatic ingestion of all fetched pages into a local corpus.

## 6. Role of `metadata-search-engine-rs`

`metadata-search-engine-rs` should be used as the primary metasearch seed. The project already provides the important pieces for this direction: search engine adapters, concurrent querying, URL normalization, deduplication, and reciprocal-rank-fusion-style aggregation.

The changeover should begin with direct dependency integration rather than a fork. Fork only after a wrapper spike proves useful and a concrete limitation is encountered.

Preferred initial path:

```text
search-mcp
  depends on metadata-search-engine-rs
  exposes MCP web_search
  calls metadata-search-engine-rs library APIs internally
```

Fork triggers:

1. Need to remove or feature-gate Axum/tower-http from the core dependency path.
2. Need explicit provider timeout/cancellation behavior not exposed upstream.
3. Need richer result models than upstream provides.
4. Need provider enable/disable configuration unavailable upstream.
5. Need MCP server support inside the same repository.
6. Need to add Codegg-specific providers such as docs.rs, crates.io, or GitHub search.
7. Upstream maintenance becomes a blocker.

If forking, preserve the metasearch core and refactor around feature flags:

```text
features:
  default = ["metasearch"]
  http-server = ["axum", "tower-http"]
  mcp-server = ["rmcp"]
  fetch = ["html extraction dependencies"]
  local-index = ["tantivy"]   # future only, not default
```

## 7. Revised Architecture

The target architecture should be small and explicit.

```text
search-mcp/
  src/
    main.rs
    config.rs
    mcp/
      mod.rs
      tools.rs
      web_search.rs
      provider_status.rs
    core/
      query.rs
      result.rs
      source_card.rs
      error.rs
      trust.rs
    adapter/
      metadata_search_engine.rs
    normalize/
      mod.rs
    rank/
      mod.rs
    fetch/
      mod.rs              # optional phase 2
```

If a multi-crate workspace already exists, simplify toward:

```text
crates/
  search-core/            # MCP-independent types, SourceCard model, errors
  search-meta/            # adapter around metadata-search-engine-rs or forked core
  search-mcp-server/      # MCP server binary
  search-fetch/           # optional, only if web_fetch is implemented
```

Do not keep a separate `search-local` or `tantivy-index` crate in the active dependency graph unless it is feature-gated and unused by default.

## 8. MCP Tool Surface

### 8.1 `web_search`

Primary tool. Performs live metasearch over configured providers.

Input:

```json
{
  "query": "string",
  "max_results": 10,
  "providers": ["duckduckgo", "brave", "startpage", "yahoo"],
  "safe_search": "moderate",
  "timeout_ms": 8000
}
```

Rules:

1. `query` is required and must be trimmed.
2. Empty queries must be rejected.
3. Very long queries must be rejected or truncated according to config.
4. `max_results` must be capped by server config.
5. If `providers` is omitted, use configured defaults.
6. If some providers fail, return partial results and provider failure metadata.
7. If all providers fail, return an MCP tool error with structured diagnostics.

Output:

```json
{
  "query": "rust axum tower middleware",
  "mode": "live_metasearch",
  "results": [
    {
      "id": "src_001",
      "title": "tower-http - Rust",
      "url": "https://docs.rs/tower-http/latest/tower_http/",
      "snippet": "Middleware and utilities for HTTP clients and servers...",
      "providers": ["duckduckgo", "brave"],
      "score": 0.0327,
      "trust": "external_untrusted",
      "fetched": false
    }
  ],
  "providers_queried": ["duckduckgo", "brave", "startpage", "yahoo"],
  "providers_failed": [],
  "warnings": [
    "Live web results are untrusted external content."
  ]
}
```

### 8.2 `provider_status`

Diagnostic tool. Reports configured providers, whether they are enabled, and optionally performs a lightweight test query.

Input:

```json
{
  "probe": false
}
```

Output:

```json
{
  "providers": [
    {
      "id": "duckduckgo",
      "enabled": true,
      "kind": "html_scrape",
      "requires_api_key": false
    }
  ]
}
```

### 8.3 `web_fetch` Optional Phase 2

Fetches and extracts a known URL. This is not required for the metasearch MVP, but it is useful for Codegg workflows.

Rules:

1. Do not enable by default if the current goal is search-only.
2. Must enforce byte limits and timeouts.
3. Must label fetched content as untrusted.
4. Must not execute JavaScript.
5. Must not expose local files or secrets.

## 9. SourceCard Model

Create a stable SourceCard model independent of upstream provider types.

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

pub enum TrustLevel {
    ExternalUntrusted,
    LocalTrusted,
    Unknown,
}
```

For the MVP, all live web results should use `ExternalUntrusted`.

The MCP layer should return SourceCards, not upstream provider objects. This keeps Codegg decoupled from `metadata-search-engine-rs` internals.

## 10. Handling Tantivy During Changeover

If Tantivy code already exists, do not delete it immediately unless it is small and clearly unused. Instead, isolate it.

Required changes:

1. Remove Tantivy from the default feature set.
2. Remove Tantivy initialization from server startup.
3. Remove local index creation from the default path.
4. Remove `local_search`, `index_document`, `index_url`, and `hybrid_search` from the default MCP tool list.
5. Put all Tantivy code behind `local-index` or `deep-research-index` feature flags.
6. Mark the feature as experimental.
7. Add a clear README section stating that the default server is metasearch-only.

Suggested feature config:

```toml
[features]
default = ["metasearch"]
metasearch = []
fetch = []
local-index = ["tantivy"]
```

If the codebase is still early and the Tantivy implementation is entangled, prefer removing it entirely from the current branch and restoring it later from git history if needed.

## 11. Configuration

Initial config should be minimal.

```toml
[server]
transport = "stdio"
log_level = "info"

[search]
default_providers = ["duckduckgo", "brave", "startpage", "yahoo"]
max_results = 10
max_query_chars = 512
timeout_ms = 8000

[search.cache]
enabled = false
kind = "memory"
ttl_seconds = 120
max_entries = 128

[providers.duckduckgo]
enabled = true

[providers.brave]
enabled = true

[providers.startpage]
enabled = true

[providers.yahoo]
enabled = true
```

Persistent cache must remain disabled and out of scope for MVP. A tiny in-memory duplicate-query cache is acceptable but optional.

## 12. Error Handling

Provider errors should not crash the request.

Behavior:

1. Query all configured providers concurrently.
2. Collect successful provider results.
3. Collect failed providers with error class.
4. If at least one provider succeeds, return partial results and failures.
5. If all providers fail, return a structured MCP error.

Provider failure metadata should not expose sensitive internals. Avoid returning raw HTTP response bodies unless debug mode is explicitly enabled.

Error classes:

```text
timeout
http_status
parse_error
network_error
rate_limited
invalid_query
unknown
```

## 13. Timeouts and Concurrency

Provider calls must be bounded.

Requirements:

1. Global request timeout.
2. Per-provider timeout.
3. Maximum concurrent providers.
4. No unbounded retries.
5. Optional one retry for transient network errors, disabled by default.

Suggested defaults:

```text
per_provider_timeout_ms = 6000
global_timeout_ms = 9000
max_concurrent_providers = 4
retries = 0
```

If `metadata-search-engine-rs` does not expose per-provider timeout behavior, add it in the MCP wrapper using `tokio::time::timeout` around each provider query or fork upstream and implement it in the aggregator.

## 14. Ranking and Deduplication

Use existing `metadata-search-engine-rs` aggregation behavior initially if available.

Expected behavior:

1. Normalize URLs.
2. Remove common tracking parameters.
3. Dedupe by normalized URL.
4. Merge provider names for duplicates.
5. Rank by reciprocal-rank fusion or upstream aggregate score.
6. Truncate to `max_results`.

Do not introduce embeddings, local index scoring, or learned ranking in MVP.

## 15. Testing Requirements

### 15.1 Unit Tests

Required unit test coverage:

1. Query validation.
2. SourceCard conversion.
3. URL normalization.
4. Dedupe behavior.
5. Provider failure handling.
6. All-providers-failed behavior.
7. Max result cap.
8. Timeout behavior.

### 15.2 Fixture Tests

For each HTML-backed provider, keep saved HTML fixtures.

```text
tests/fixtures/
  duckduckgo/
    basic.html
    no_results.html
    layout_changed.html
  brave/
    basic.html
    no_results.html
  startpage/
    basic.html
  yahoo/
    basic.html
```

Parser tests must run offline against fixtures. Live network tests should be ignored by default.

### 15.3 MCP Contract Tests

Add tests for tool schema and representative responses.

Test cases:

1. `web_search` valid query returns structured results.
2. `web_search` empty query returns validation error.
3. `web_search` partial provider failure returns partial success.
4. `provider_status` returns enabled provider list.

## 16. Migration Steps

### Step 1: Freeze Current Tantivy Behavior

Record what currently works. Add a short note in the repository or issue tracker listing implemented Tantivy features and known limitations. Do not continue expanding the Tantivy path during this changeover.

### Step 2: Add `metadata-search-engine-rs` Dependency

Add the dependency and create a small internal adapter.

```rust
pub struct MetadataSearchAdapter {
    // shared client and configured engines
}
```

The adapter should expose a Codegg-owned function:

```rust
pub async fn web_search(query: SearchRequest) -> Result<SearchResponse>;
```

Do not leak upstream types beyond the adapter boundary.

### Step 3: Implement SourceCard Conversion

Map upstream aggregated results into SourceCards.

Required fields:

1. Stable source ID generated per response.
2. Title.
3. URL.
4. Snippet.
5. Provider list.
6. Score.
7. Trust label.
8. Fetch status set to false.

### Step 4: Implement MCP `web_search`

Expose the adapter through MCP. Keep the tool narrow. Validate query and max results before calling the adapter.

### Step 5: Add Provider Status Tool

Expose configured provider list and basic diagnostics.

### Step 6: Disable Tantivy in Default Build

Remove Tantivy from default features. Ensure the MCP server starts and runs without any index directory, database, or persistent state.

### Step 7: Add Fixture and Contract Tests

Prioritize tests around parser and MCP output stability. If upstream crate lacks fixtures, add wrapper-level tests and consider upstream contribution or fork.

### Step 8: Decide Dependency vs Fork

After the MVP wrapper works, evaluate whether upstream dependency is sufficient.

Stay dependency-based if:

1. API is stable enough.
2. Axum dependency overhead is acceptable.
3. Timeout behavior is acceptable or can be wrapped externally.
4. Provider list is enough for MVP.

Fork if:

1. The dependency pulls too much server-specific code.
2. Upstream types are too limited.
3. Timeout behavior cannot be corrected cleanly.
4. Provider parser changes are needed.
5. MCP server should live in the same project.

## 17. README Update Requirements

Update the README to reflect the new direction.

Required README statements:

1. This is a Rust MCP metasearch server.
2. It does not require SearXNG.
3. It does not require API keys for the default provider set.
4. It does not maintain a web index.
5. It does not use Tantivy in the default build.
6. Search results are live external content and should be treated as untrusted.
7. HTML-backed providers may break if upstream layouts change.
8. Provider fixture tests are used to detect parser breakage.

Example wording:

```text
This project implements a lightweight MCP metasearch server. It queries configured upstream search providers at request time, normalizes and deduplicates results, and returns compact source cards suitable for agentic use. It is not a crawler, not a local web index, and does not require SearXNG or a paid search API for the default configuration.
```

## 18. Security Requirements

Search and fetched web content are untrusted.

Required controls:

1. Mark all live web results as `external_untrusted`.
2. Do not execute JavaScript.
3. Do not expose environment variables or secrets through tool output.
4. Do not follow arbitrary local file URLs.
5. Do not allow fetched content to alter tool instructions.
6. Avoid logging full raw SERP HTML unless debug mode is explicitly enabled.
7. Enforce query length limits.
8. Enforce response size limits.
9. Use safe, explicit user-agent string.
10. Prefer structured JSON output over prose summaries.

## 19. Acceptance Criteria

The changeover is complete when:

1. The default binary runs without Tantivy.
2. The default binary runs without SearXNG.
3. The default binary runs without API keys.
4. The MCP server exposes `web_search`.
5. `web_search` returns structured SourceCards.
6. At least two upstream providers work in default configuration.
7. Partial provider failure does not fail the entire request.
8. All-provider failure returns a structured error.
9. Query length and result caps are enforced.
10. Tests cover query validation, SourceCard conversion, dedupe behavior, and partial failure.
11. README accurately states metasearch scope and non-goals.
12. Any Tantivy/local-index code is disabled by default or removed.

## 20. Deferred Work

Defer the following until the metasearch MCP is stable.

1. `web_fetch` extraction.
2. `search_and_fetch`.
3. docs.rs provider.
4. crates.io provider.
5. Wikipedia provider.
6. Optional SearXNG adapter.
7. Optional Brave/Tavily/Exa API adapters.
8. Deep-research artifact retention.
9. Ephemeral per-run Tantivy index.
10. Persistent local corpus.

## 21. Recommended Immediate Implementation Order

1. Add or confirm MCP server skeleton.
2. Add direct dependency on `metadata-search-engine-rs`.
3. Implement adapter boundary.
4. Implement `web_search` tool.
5. Convert upstream results to SourceCards.
6. Return provider failure metadata.
7. Disable Tantivy by default.
8. Add tests.
9. Update README.
10. Evaluate fork decision.

## 22. Notes for Smaller Implementation Agent

Do not expand scope. Do not build a local index. Do not add persistent cache. Do not introduce browser automation. Do not turn this into a full SearXNG clone.

The implementation objective is a working metasearch MCP server using the existing Rust metasearch project as the backend, with clean output types and safe defaults.


