# CLI Commands Deep Dive

**Location:** `src/commands/` (9 files)
**Purpose:** CLI subcommands for direct user interaction. Entry point is `src/main.rs`.

---

## Module Map

| File | Responsibility |
|------|---------------|
| `mod.rs` | Module declarations |
| `doctor.rs` | `eggsearch doctor` — diagnose configuration and provider status |
| `search.rs` | `eggsearch search` — live metasearch from CLI |
| `fetch.rs` | `eggsearch fetch` — fetch and extract a URL from CLI |
| `providers.rs` | `eggsearch providers` — show provider configuration/status |
| `update.rs` | `eggsearch update` — check and install the latest stable binary |
| `mcp.rs` | `eggsearch mcp stdio` — start MCP server |
| `browser_login.rs` | `eggsearch browser-login` — headed browser login (feature-gated `browser`) |
| `browser_profiles.rs` | `eggsearch browser-profiles` — manage persistent profiles (feature-gated `browser`) |

---

## CLI Structure

```
eggsearch [--config PATH] [--verbose] <COMMAND>

Commands:
  doctor           Diagnose configuration and provider status
  search           Run a live metasearch and print compact source cards
  mcp stdio        Run the MCP server over stdio
  providers        Show provider configuration and status
  fetch            Fetch and extract content from a URL
  update           Check for and install the latest stable release
  browser-login    Open a headed browser for manual login/verification
  browser-profiles Manage persistent browser profiles
```

---

## Commands

### `doctor` (`doctor.rs`)

Diagnose configuration and provider status.

```bash
eggsearch doctor [--probe]
```

**Flags:**
- `--probe` — Probe each provider with a live query

**Output:**
- Config file location and status
- Provider configuration summary
- Provider health (with `--probe`)
- Feature flag status

### `search` (`search.rs`)

Run a live metasearch and print compact source cards.

```bash
eggsearch search <QUERY> [--max-results N] [--json] [--providers p1,p2]
```

**Parameters:**
- `QUERY` — Search query (required)
- `--max-results` — Max results (default: 10)
- `--json` — Output as JSON
- `--providers` — Comma-separated provider IDs

**Output:**
- Compact source cards (human-readable)
- Or JSON array (with `--json`)

### `fetch` (`fetch.rs`)

Fetch and extract content from a URL.

```bash
eggsearch fetch <URL> [--max-chars N] [--timeout-ms N] [--metadata-only] [--markdown] [--include-links] [--json]
```

**Parameters:**
- `URL` — URL to fetch (required)
- `--max-chars` — Max extracted characters
- `--timeout-ms` — Request timeout in milliseconds
- `--metadata-only` — Extract metadata only, not body text
- `--markdown` — Render as Markdown instead of plain text
- `--include-links` — Include extracted links in output
- `--json` — Output as JSON

**Output:**
- Extracted content (human-readable)
- Or JSON object (with `--json`)

### `providers` (`providers.rs`)

Show provider configuration and status.

```bash
eggsearch providers [--json]
```

**Flags:**
- `--json` — Output as JSON

**Output:**
- Provider list with capabilities
- Configuration status
- Feature flag requirements

### `update` (`update.rs`)

Check for or install the latest stable release.

```bash
eggsearch update [--check]
```

`--check` performs crates.io version discovery and semantic comparison only.
Without it, the updater prefers the exact GitHub Release asset for the current
host, verifies its checksum and `--version` identity, and atomically replaces
the current executable. Unsupported hosts and confirmed exact-asset 404s use
an isolated exact-version Cargo build; other failures stop without compiling.
The command does not load search configuration and does not restart services.

### `mcp stdio` (`mcp.rs`)

Start the MCP server over stdio.

```bash
eggsearch mcp stdio
```

**Behavior:**
- Reads MCP protocol messages from stdin
- Writes responses to stdout
- Logs to stderr (controlled by `--verbose`)

### `browser-login` (`browser_login.rs`)

Open a headed browser for manual login/verification (feature-gated `browser`).

```bash
eggsearch browser-login <ORIGIN> [--profile NAME]
```

**Parameters:**
- `ORIGIN` — HTTP(S) origin to open (e.g., `https://example.com`)
- `--profile` — Profile name (default: "default")

**Behavior:**
- Opens Chrome/Chromium in headed mode
- User manually logs in
- Session saved to profile

### `browser-profiles` (`browser_profiles.rs`)

Manage persistent browser profiles (feature-gated `browser`).

```bash
eggsearch browser-profiles <COMMAND>
```

**Subcommands:**
- `list` — List all profiles
- `remove <NAME>` — Remove a profile
- `show <NAME>` — Show profile details

---

## Entry Point (`main.rs`)

### CLI Parser

```rust
struct Cli {
    config: Option<PathBuf>,  // --config
    verbose: u8,              // -v, -vv
    command: Commands,        // subcommand
}
```

### Tracing Init

```rust
fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    // ... stderr writer, env filter
}
```

### Config Loading

```rust
let cfg = config::load(cli.config.as_deref())?;
```

Resolves config from:
1. `--config` flag (if provided)
2. `$XDG_CONFIG_HOME/eggsearch/config.toml`
3. Default config (if no file exists)

---

## Integration with Other Modules

```
main.rs
  └→ commands::*
       ├→ core::config::AppConfig       (configuration)
       ├→ meta::MetadataSearchAdapter   (search operations)
       ├→ fetch::FetchClient            (URL fetching)
       └→ mcp::EggsearchServer          (MCP server)
```

---

## Output Formatting

### Human-Readable (default)
```
┌─────────────────────────────────────────────────────┐
│ Title                                               │
│ URL                                                 │
│ Snippet...                                          │
│ [SourceKind] Freshness: Week                        │
└─────────────────────────────────────────────────────┘
```

### JSON (`--json`)
```json
[
  {
    "id": "abc123",
    "url": "https://example.com",
    "title": "Example",
    "snippet": "Description...",
    "kind": "Documentation",
    "freshness": "Week"
  }
]
```

---

[← Back to Overview](overview.md)
