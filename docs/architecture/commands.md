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
eggsearch [--config <PATH>] [-v|-vv]
  ├── doctor [--probe]
  ├── search <query> [--max-results N] [--json] [--providers csv]
  ├── fetch <url> [--max-chars N] [--timeout-ms N] [--markdown] [--metadata-only] [--include-links] [--json]
  ├── providers [--json]
  └── mcp stdio
```

### Global Flags

| Flag | Purpose |
|------|---------|
| `--config <PATH>` | Override config file path (default: `$XDG_CONFIG_HOME/eggsearch/config.toml`) |
| `-v` / `-vv` | Verbosity: `-v` = info, `-vv` = debug (controls tracing level) |

---

## Subcommand Details

### `doctor`

Validates configuration and reports provider health. Receives `&AppConfig` from `main.rs` (config is loaded once in `main.rs`).

1. Validate config structure (`cfg.validate()`)
2. Check config file existence and loadability
3. Check local backend availability
4. Output structured JSON status report: config path, mode, provider lists (enabled/default/disabled/capabilities), searxng status, api credential status, fetch status, warnings
5. If `--probe` and `mode != Off`: run a test query against each provider with 3000ms timeout; display `[OK]`/`[FAIL]` per provider with latency

### `search`

Runs a metasearch from the CLI. Query is parsed by clap in `main.rs`, config loaded once in `main.rs`.

1. If `mode == Off`: bail early
2. Resolve providers via `resolve_providers()`
3. Validate selected engines via `select_engines()`
4. Compute `max_results` via `resolve_max_results()`
5. Execute search
6. Format and display results (or output JSON with `--json`: query, mode, results, warnings)

**CLI args:** `<query>` (positional), `--max-results` (default 10), `--json`, `--providers` (comma-delimited provider override)

### `fetch`

Fetches and extracts content from a URL. URL is parsed by clap in `main.rs`, config loaded once in `main.rs`.

1. If `[fetch].enabled == false`: bail
2. Validate URL (max_chars != 0)
3. Construct `FetchLimits` with optional `--timeout-ms` override
4. Select `ExtractMode` from flags: `--markdown` → Markdown, `--metadata-only` → MetadataOnly, default → Text
5. Execute fetch
6. Display extracted content (or output JSON with `--json`: url, final_url, title, status, trust, links, trust_markers, etc.)

**CLI args:** `<url>` (positional), `--max-chars`, `--timeout-ms`, `--markdown`, `--metadata-only`, `--include-links`, `--json`

### `providers`

Reports provider configuration. Supports both tabular and JSON output.

1. Load config
2. Resolve providers
3. Display status: ID, Enabled, Default, Kind, API Key, Configured, Routable, SkipCode, Health, Capabilities

**CLI args:** `--json`

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
