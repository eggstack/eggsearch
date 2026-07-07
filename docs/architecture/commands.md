# commands Module Deep Dive

**Path:** `src/commands/`
**Purpose:** CLI subcommands that wrap the library for direct human/agent use.

---

## Subcommand Inventory

| File | Command | Purpose |
|------|---------|---------|
| `doctor.rs` | `eggsearch doctor` | Config validation, provider status, capability summary, probe (live query test) |
| `search.rs` | `eggsearch search` | Manual live metasearch via CLI |
| `mcp.rs` | `eggsearch mcp stdio` | Run MCP server over stdio |
| `fetch.rs` | `eggsearch fetch` | Fetch and extract content from a URL |
| `providers.rs` | `eggsearch providers` | Report provider configuration and status |

---

## Entry Point

`src/main.rs` uses `clap` for CLI parsing:

```
eggsearch
  ├── doctor      # config validation + probe
  ├── search      # manual search
  ├── fetch       # manual fetch
  ├── providers   # provider status
  └── mcp stdio   # MCP server (stdio transport)
```

---

## Subcommand Details

### `doctor`

Validates configuration and reports provider health:

1. Load config from `$XDG_CONFIG_HOME/eggsearch/config.toml`
2. Validate config structure
3. Resolve providers (check enabled/known status)
4. Report capability summary
5. Optional: run a live probe query to test connectivity

### `search`

Runs a metasearch from the CLI:

1. Parse query from args
2. Load config
3. Build adapter from config
4. Execute search
5. Format and display results

### `fetch`

Fetches and extracts content from a URL:

1. Parse URL from args
2. Load config
3. Build fetch client
4. Execute fetch
5. Display extracted content

### `providers`

Reports provider configuration:

1. Load config
2. Resolve providers
3. Display status (enabled/disabled, capabilities, kind)

### `mcp stdio`

Runs the MCP server:

1. Load config
2. Build `ServerState`
3. Build `EggsearchServer`
4. Run over stdio transport

---

## Config Loading

All commands use `config::load()` from `src/config.rs`:

```
$XDG_CONFIG_HOME/eggsearch/config.toml
  ├── [search]       — SearchSection
  ├── [fetch]        — FetchSection
  └── [local]        — LocalConfig
```

Config defaults are applied for missing sections.

---

**Back to:** [overview.md](overview.md)
