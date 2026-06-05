# Eggsearch

Eggsearch is a local-first MCP (Model Context Protocol) search server for agents.
It exposes web search, URL fetch/extraction, and local indexed search as MCP tools
that return compact "source cards" with provenance and trust labels.

## Workspace layout

```
crates/
  eggsearch-core    # types, traits, normalize, dedupe, rank, source cards
  eggsearch-meta    # live metasearch providers (DuckDuckGo, Wikipedia, crates.io, docs.rs)
  eggsearch-fetch   # URL fetcher, HTML/text extraction, cache, artifact store
  eggsearch-local   # local Tantivy index, ingestion
  eggsearch-mcp     # MCP server (rmcp) exposing tools
  eggsearch-cli     # command-line entry point
```

## Build

```bash
cargo build --release
```

## Run MCP server

```bash
cargo run --release -- mcp stdio
```

## CLI usage

```bash
cargo run --release -- doctor
cargo run --release -- search "rust axum middleware"
cargo run --release -- fetch https://example.com
cargo run --release -- index add ./docs
cargo run --release -- index search "middleware"
```

See `plans/eggsearch.md` for the full design spec.
