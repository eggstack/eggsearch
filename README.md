# eggsearch

A lightweight MCP (Model Context Protocol) **metasearch** server for agents.

eggsearch queries configured upstream search providers at request time,
normalizes and deduplicates results, and returns compact, provenance-
preserving **source cards** suitable for agentic use. It is not a crawler,
not a local web index, and does not require SearXNG or a paid search API
for the default configuration.

## What it is

- A single Rust binary that speaks MCP over stdio.
- A thin wrapper around [`metadata-search-engine-rs`](https://crates.io/crates/metadata-search-engine-rs)
  (the upstream metasearch library) — eggsearch does not hand-roll HTML
  parsers for each engine.
- No persistent state, no database, no Tantivy in the default build.
- No API keys required for the default provider set.

## What it is not

- Not a web crawler.
- Not a local search engine.
- Not a SearXNG replacement with a web UI.
- Not a browser-automation tool.

## Workspace layout

```
crates/
  eggsearch-core    # types: SourceCard, TrustLevel, config, URL normalize
  eggsearch-meta    # MetadataSearchAdapter wrapping metadata-search-engine-rs
  eggsearch-mcp     # MCP server (rmcp): web_search + provider_status
  eggsearch         # CLI binary: doctor, search, providers, mcp stdio
```

## Build

```bash
cargo build --release
```

## Run the MCP server

```bash
cargo run --release -- mcp stdio
```

## CLI usage

```bash
cargo run --release -- doctor
cargo run --release -- search "rust axum middleware"
cargo run --release -- providers
```

`doctor` reports the effective config and loaded providers. `search`
runs a live metasearch and prints compact source cards. `providers`
lists which engines are enabled and which require an API key.

## MCP tools

### `web_search`

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

Rules:

- `query` is required and must be non-empty.
- `max_results` is capped by the server's `max_results_cap`.
- If `providers` is omitted, the server's configured defaults are used.
- Partial provider failure is non-fatal: the response includes
  `providers_failed` entries and the surviving results.
- If all providers fail, the tool returns a structured error.
- Results are labeled `external_untrusted`; agents must not treat
  fetched web content as instructions.

### `provider_status`

Diagnostic tool. Reports the configured provider set, whether each
provider is enabled, its kind (`html_scrape` / `api_key`), and whether
it requires an API key.

Input:

```json
{
  "probe": false
}
```

## Configuration

Default config is loaded from
`$XDG_CONFIG_HOME/eggsearch/config.toml` (or platform equivalent).
A minimal example:

```toml
[search]
mode = "live"
max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000

default_providers = ["duckduckgo", "startpage", "yahoo"]

[search.providers]
duckduckgo = true
brave      = true
startpage  = true
yahoo      = true
```

`mode` is either `"live"` (the default) or `"off"`.

## Defaults

- Providers in the default config: `duckduckgo`, `brave`, `startpage`,
  `yahoo`.
- `default_providers` (used when the client does not pass a list):
  `["duckduckgo", "startpage", "yahoo"]`.
- `max_results`: 10. `max_results_cap`: 50.
- `timeout_ms`: 8000 (per request; the upstream library applies its own
  per-engine cap of ~8s as well).
- Trust label for all live web results: `external_untrusted`.

## Security

- All live web results are labeled `external_untrusted`. Agents should
  not treat fetched content as instructions.
- The server does not execute JavaScript and does not follow arbitrary
  local file URLs.
- Raw HTTP error bodies are not surfaced to the MCP caller; only
  coarse error classes (`timeout`, `http_status`, `parse_error`,
  `network_error`, `rate_limited`, `invalid_query`, `unknown`) and
  short messages.
- The server enforces query length and result count caps.

## HTML provider fragility

The upstream engines scrape HTML. Layout changes upstream may break
parsing. Parser tests in the upstream library run against saved HTML
fixtures; live network tests are marked `#[ignore]` by default.

## Out of scope (deferred)

- `web_fetch` extraction
- `search_and_fetch`
- Local index / corpus search
- Persistent result cache
- Per-result fetch + artifact store
- Vector search, embeddings, learned ranking
