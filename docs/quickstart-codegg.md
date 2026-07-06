# Quickstart: Using eggsearch with CodeGG

This guide walks through installing eggsearch, configuring it, and using it from an MCP-compatible client such as CodeGG.

## Prerequisites

- Rust 1.88 or later installed
- An MCP-compatible client (CodeGG, Claude Code, or any client supporting MCP over stdio)

## Installation

```bash
cargo install eggsearch
```

Or build from source:

```bash
git clone https://github.com/eggstack/eggsearch.git
cd eggsearch
cargo build --release
```

The binary is written to `target/release/eggsearch`.

## Configuration

eggsearch reads `$XDG_CONFIG_HOME/eggsearch/config.toml`. On macOS this is typically `~/.config/eggsearch/config.toml`. The file is optional; eggsearch works with sensible defaults when it is absent.

A minimal config:

```toml
[search]
mode = "live"
default_providers = ["duckduckgo", "startpage", "yahoo"]

[fetch]
enabled = true
```

For coding work with GitHub integration:

```toml
[search]
mode = "live"
default_providers = ["duckduckgo", "brave"]

[search.api.github_code]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_issues]
enabled = true
api_key_env = "GITHUB_TOKEN"

[search.api.github_releases]
enabled = true
api_key_env = "GITHUB_TOKEN"

[fetch]
enabled = true
```

Set the `GITHUB_TOKEN` environment variable before starting the server.

## Running the MCP Server

Start the server on stdio transport:

```bash
eggsearch mcp stdio
```

Or set a custom config path:

```bash
eggsearch --config /path/to/config.toml mcp stdio
```

The server reads MCP JSON-RPC messages from stdin and writes responses to stdout.

## Client Configuration

Configure your MCP client to launch eggsearch as a child process over stdio. The exact configuration depends on your client.

### CodeGG

In your CodeGG MCP server configuration:

```json
{
  "mcpServers": {
    "eggsearch": {
      "command": "eggsearch",
      "args": ["mcp", "stdio"]
    }
  }
}
```

### Claude Code

```json
{
  "mcpServers": {
    "eggsearch": {
      "command": "eggsearch",
      "args": ["mcp", "stdio"]
    }
  }
}
```

For a custom config path, set the environment variable:

```json
{
  "mcpServers": {
    "eggsearch": {
      "command": "eggsearch",
      "args": ["mcp", "stdio"],
      "env": {
        "EGGSEARCH_CONFIG": "/path/to/config.toml"
      }
    }
  }
}
```

## Example Workflow

A typical research flow uses three tools:

### 1. Search

```
web_search({
  "query": "rust async runtime comparison",
  "intent": "research",
  "max_results": 10
})
```

Returns a list of source cards with `next_actions` and `suggested_fetches`.

### 2. Fetch

```
web_fetch({
  "url": "https://tokio.rs/blog/2020-04-preparing-for-tokio-0-3",
  "extract_mode": "markdown",
  "max_chars": 12000
})
```

Returns extracted text from the selected URL.

### 3. Batch Fetch

```
batch_fetch({
  "items": [
    "https://docs.rs/axum/latest/axum/",
    "https://docs.rs/actix-web/latest/actix_web/"
  ]
})
```

Fetches multiple URLs in bounded parallel.

### Discovering Capabilities

Call `provider_status` to see which providers and tools are available:

```
provider_status({})
```

This returns provider health, server capabilities, tool capabilities, and workflow recipes.

## Next Steps

- [Configuration](config.md) -- full config reference with all options and defaults
- [Provider Setup](provider-setup.md) -- how to enable and configure individual providers
- [Tool Matrix](tool-matrix.md) -- compact reference for all 10 MCP tools
- [Agent Workflows](agent-workflows.md) -- recommended tool call sequences and recipes
