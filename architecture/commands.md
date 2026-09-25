# CLI Commands Deep Dive

**Binary:** `src/main.rs` + `src/commands/` + `src/config.rs` (binary-only).
**Library contract:** thin wrappers only. Dependency flow is
`core <- meta <- mcp <- commands`; `fetch` stays independent of `meta`.
No engine is ever called directly from a command; every search path goes
through `MetadataSearchAdapter` via `ServerState`, every MCP serve path
goes through `src/mcp/`.

`src/commands/` contains 10 entries: `mod.rs` plus 9 command modules
(`doctor`, `search`, `fetch`, `providers`, `update`, `integrate`, `mcp`,
`browser_login`, `browser_profiles`). There is no `startup.rs` command
module: `startup`, `restart`, and `croncheck` are parsed in `src/main.rs`
and dispatched straight to the `eggsearch::startup` library API.

---

## Entry point (`src/main.rs`)

`Cli` is a clap `Parser` with two globals and one subcommand enum:

- `--config <PATH>` (global, optional): explicit config file path.
- `-v` / `-vv` (global, counted): `info` / `debug` / `trace` via
  `init_tracing()`, which writes to stderr with target disabled.
- `Commands`: the subcommand set listed below. Nothing else is accepted.

Top-level subcommands, exactly as declared in `Commands`:

| Subcommand | Shape | Config load |
|------------|-------|-------------|
| `doctor [--probe]` | leaf | loads config |
| `search <QUERY> [--max-results N] [--json] [--providers a,b]` | leaf | loads config |
| `mcp stdio` / `mcp serve [--bind ADDR] [--path PATH] [--pid-file PATH]` | two levels | loads config |
| `providers [--json]` | leaf | loads config |
| `fetch <URL> [--max-chars N] [--timeout-ms N] [--metadata-only] [--markdown] [--include-links] [--json]` | leaf | loads config |
| `update [--check]` | leaf | **no** config load; `update::run_with_config(check, config)` resolves its own lifecycle snapshot |
| `croncheck` | leaf | **no** config load; calls `startup::croncheck()` directly |
| `restart` | leaf | **no** config load; calls `startup::restart()` directly |
| `startup status [--json]` / `startup install [--method M]` / `startup instructions [--method M]` / `startup uninstall [--method M]` | two levels | **no** config load; calls `startup::{startup_state,install,instructions,uninstall}` directly |
| `integrate [--json] [--executable PATH] list` / `integrate <client> [--transport T] [--apply]` | two levels | **no** config load; delegates to `src/integrations/` |
| `browser-login <ORIGIN> [--profile NAME]` | leaf, `browser` feature only | loads config |
| `browser-profiles list` / `browser-profiles inspect <NAME>` / `browser-profiles remove <NAME>` | two levels, `browser` feature only | loads config |
| `windows-service` | hidden, `cfg(windows)` only | **no** config load; calls `startup::run_windows_service()` |

The `match` in `main()` therefore has two arms: the pre-config group
(`update`, `integrate`, `croncheck`, `restart`, `startup`,
`windows-service`) runs first, and everything else falls into the
`config::load()` arm. `update`, `croncheck`, `restart`, and `startup`
need the raw `--config` path to build a `RuntimeSpec`, not a loaded
`AppConfig`, which is why they bypass the loader.

`src/config.rs` is 14 lines: if `--config` is present use it, otherwise
use `core::config::default_config_path()`, then `AppConfig::load()`.

---

## `doctor` (`commands/doctor.rs`)

`eggsearch doctor [--probe]`: environment and configuration diagnosis.

1. `cfg.validate()`; resolve display path (`--config` or default).
2. Report `config_file_exists` / `config_file_loaded` by probing the
   resolved path, plus `mode`, `default_max_results`,
   `max_results_cap`, SearXNG status, API credential status
   (`api_key_env` set or not per enabled `[search.api]` entry), fetch
   limits, and `collect_warnings()` (misconfigured defaults, SearXNG
   configured-but-disabled, enabled API provider without key,
   permissive `allow_private_network` / `allow_localhost`).
3. Provider enablement is computed per ID, not copied from config:
   `local_workspace` follows `LocalWorkspaceBackend::is_enabled()`,
   `searxng` requires both the providers-map flag and
   `[search.searxng].enabled`, API providers require
   `[search.api.<id>].enabled`, the rest follow
   `[search.providers.<id>]`. The capability table is built with
   `built_in_provider_descriptor()` over `KNOWN_PROVIDER_IDS`.
4. `mode=off` bails as unhealthy before any network use. An empty
   routable provider set also bails.
5. `--probe` calls `meta::probe::probe_providers()` with
   `ProviderProbeRequest::default()` and prints per-provider
   `[OK]` / `[FAIL]` / `[SKIP]` lines plus a
   `started/succeeded/failed/skipped` summary. All-failed or
   all-skipped is an error exit.

This is the same probe service the MCP `provider_status(probe=true)`
tool uses, so CLI and tool results agree by construction. See
`architecture/meta.md` and `architecture/mcp.md` for the probe and
tool sides.

## `search` (`commands/search.rs`)

`eggsearch search <QUERY> [--max-results 10] [--json]
[--providers p1,p2]`: manual live metasearch.

- Policy first: `Mode::Off` bails with a
  `[search].mode = "live"` hint; empty queries bail.
- Builds `ServerState` from the loaded config, resolves
  `--providers` through `cfg.resolve_providers()` (empty means server
  defaults), and rejects unknown IDs via
  `adapter.select_engines()` before issuing any request.
- Constructs a `WebSearchRequest` with default intent/freshness and no
  domain/language/region filters, validates against
  `max_query_chars`, resolves the effective count with
  `resolve_max_results(default, cap)`, and calls
  `adapter.web_search()`.
- Human output prints numbered cards (title, URL, provider list,
  single-line snippet) plus warnings and the failed-provider list.
  `--json` prints `{query, mode, results, providers_queried,
  providers_failed, warnings}`.

## `providers` (`commands/providers.rs`)

`eggsearch providers [--json]`: static configuration plus live health.

- Builds `ServerState`, takes `adapter.provider_status()` descriptors,
  and patches the `local_workspace` entry from
  `state.local_backend.is_some()` so the CLI never reports a local
  provider as configured when the backend failed to construct.
- Pairs each descriptor with `adapter.health().health_view(&id)`.
- `--json` emits `{providers: [...descriptor + health...], mode}`.
  Human output is an aligned table:
  `ID Enabled Default Kind Key Configured Routable SkipCode Health
  Capabilities`.

Unlike `doctor`, this command never probes; it reports routability
and cached health only.

## `fetch` (`commands/fetch.rs`)

`eggsearch fetch <URL> [flags]`: one-shot bounded extraction.

- Policy first: `cfg.validate()`, then `cfg.fetch.enabled` must be
  true or the command bails with the `[fetch].enabled` hint.
- URL must parse and use `http`/`https`. `max_chars=Some(0)` and
  `timeout_ms=Some(0)` are rejected before any I/O.
- Starts from `cfg.fetch_limits()`, overrides only `timeout_ms` when
  the flag is present, and builds `FetchClient::new()` with the
  configured user agent and sanitize flag.
- `ExtractMode` is `MetadataOnly` when `--metadata-only`, else
  `Markdown` when `--markdown`, else `Text`. `include_links` is the
  flag OR `include_links_default`.
- `--json` emits the full response envelope (`url`, `final_url`,
  `title`, `description`, `content_type`, `status`, `fetched`,
  `truncated`, `text`, `links`, `trust_markers`, `document`,
  `warnings`). Human output prints status/final-URL/content-type,
  the text block with char count, at most `CLI_DISPLAY_MAX_LINKS`
  (20) links with seen/truncated accounting, and warnings.

## `mcp` (`commands/mcp.rs`)

`eggsearch mcp stdio` runs the client-owned transport.
`eggsearch mcp serve` runs the persistent loopback transport.
Both build the same `EggsearchServer` from the same `AppConfig`;
only the transport differs.

### `mcp stdio`: client-owned

`run_stdio()` calls `mcp::build_server(cfg)` and serves
`rmcp::transport::stdio()`: stdin carries MCP frames in, stdout
carries frames out, logs go to stderr. The process lifetime belongs
to the MCP client. Startup supervision never manages stdio
processes: `restart` refuses with a "client-owned" error when no
manager is registered, and `update` never restarts them.

### `mcp serve`: loopback persistent

```
eggsearch mcp serve --bind 127.0.0.1:11320 --path /mcp [--pid-file PATH]
```

- `--bind` is a typed `SocketAddr` defaulting to
  `mcp::http::DEFAULT_BIND` (`127.0.0.1:11320`).
  `ServeOptions::validate()` rejects non-loopback binds.
- `--path` is a typed `McpPath` defaulting to `/mcp`. Parsing
  rejects `/healthz`, `/`, empty segments, overlong values, and
  unsafe characters. This is the only persistent runtime the
  startup managers target; see `architecture/startup.md`.
- `--pid-file` is hidden and manager-owned. `run_http_with_pid()`
  creates a `startup::PidFileGuard` before entering
  `mcp::http::run()`, so a cron-managed server either holds its
  owned PID record or exits with `AlreadyExists`.
- `GET /healthz` is served alongside, but outside, the MCP session
  state. `SIGTERM` / Ctrl-C cancel rmcp sessions and drain for a
  bounded period. Logs go to stderr; stdout stays a log-free zone.

## `update` (`commands/update.rs`)

`eggsearch update [--check]`: 9-line wrapper over
`update::run_with_config(check, config)`.

- `--check` performs crates.io version discovery and comparison
  only: never downloads, compiles, or replaces.
- Without `--check`, the updater prefers the exact GitHub Release
  asset for `platform::current_target()`, verifies checksum plus
  `--version` identity, and atomically replaces the executable.
  Confirmed exact-asset 404s and unsupported hosts fall back to an
  isolated exact-version Cargo build; other failures stop.
- Service handling is snapshot-then-restart: state is captured
  before replacement, and only a previously **healthy** registered
  service is restarted through its manager (see
  `architecture/startup.md` and `architecture/packaging.md`).

## `startup` / `restart` / `croncheck` (no command module)

Parsed in `main.rs`, implemented in `src/startup.rs`. Full policy
lives in `architecture/startup.md`; the CLI surface is:

- `startup status [--json]`: one registered manager plus
  `registered/running/healthy/conflict` and a detail line, or
  pretty JSON when `--json` is passed.
- `startup install --method auto`: render, register, start, and
  verify health. `--method` defaults to `auto`.
- `startup instructions --method auto`: render only. Prints the
  detected method, executable, config, `GET <health-url>`, the full
  canonical command line, and the manager-specific unit/plist/
  crontab/service text.
- `startup uninstall --method auto`: stop, remove the one owned
  registration, leave unrelated state alone.
- `restart`: delegate to the one registered manager and verify
  `/healthz`. Refuses when several managers are registered and
  errors when none is.
- `croncheck`: the cron watchdog entry point. Exits quietly when
  healthy, spawns only when health is definitively refused.

`--method` accepts `auto` (default), `systemd`, `launchd`, `cron`,
`windows`, matching `StartupMethod`. Explicit values are
platform-gated.

## `integrate` (`commands/integrate.rs`)

```
eggsearch integrate [--json] [--executable PATH] list
eggsearch integrate [--json] [--executable PATH] <codegg|zed|codex|claude|cursor|vscode|opencode> [--transport stdio|http] [--apply]
```

`IntegrateCommand` is `List` or `Client {client, transport, apply}`.
`--json` and `--executable` are global to the `integrate` group.

- `list` prints every supported client with availability, stdio/http
  support, and apply mode (table, or JSON with `--json`).
- Per-client commands default to `--transport stdio` and only print.
  `--apply` performs the mutation, then runs protocol verification
  (stdio handshake or HTTP health, depending on transport).
- Print-first is the rule: without `--apply` the output ends with a
  "rendered only; pass --apply" note and touches nothing.
- Executable rule: `--apply` requires an installed executable or an
  explicit `--executable`. A development/test `target/debug`-style
  path without the override is rejected before any file is edited.
- Edits are atomic and backed up (`json-atomic-backup` family;
  OpenCode uses backup-if-JSON). `zed apply` is print-only by
  design because its settings format cannot be edited safely.
- Client set is exactly the seven `IntegrateCmd` variants:
  `codegg`, `zed`, `codex`, `claude`, `cursor`, `vscode`,
  `opencode`. Adding a client means extending the enum, the
  `match` in `main()`, and `src/integrations/` together. See
  `architecture/integrations.md`.

## `browser-login` (`commands/browser_login.rs`)

`eggsearch browser-login <ORIGIN> [--profile NAME]`
(`browser` feature only): headed manual login for one persistent
profile.

- Requires `[fetch.browser].persistent_profiles_enabled`; otherwise
  bails before creating state.
- Validates the origin strictly: must parse, must be `http`/`https`,
  must have a host, must not embed credentials.
- Discovers the browser **before** creating profile state, so an
  invalid explicit `[fetch.browser].executable` (the
  `ExplicitPathInvalid` case) cannot leave an orphaned profile.
- Creates (or reuses) the profile by display name, ensures the
  `chrome-data` directory, launches headed Chrome with a locked-down
  flag set, and waits for Enter or
  `profile_process_timeout_ms`. On success it records last-used time
  plus browser family/major version and prints the
  `web_fetch`-ready `browser_profile` hint.

## `browser-profiles` (`commands/browser_profiles.rs`)

`eggsearch browser-profiles <COMMAND>` (`browser` feature only).
The subcommand enum is exactly `List`, `Inspect {name}`, and
`Remove {name}`:

- `list`: table of `NAME ORIGIN STATE LAST USED` plus a total.
  `ready` means browser family metadata is recorded; otherwise
  `incomplete`. Prints the `browser-login` hint when empty.
- `inspect <NAME>`: resolves the display name to the **opaque**
  profile ID (`resolve_by_name`), then reports ID, origin, created /
  last-used timestamps, browser family/major, schema version, lock
  state, profile and chrome-data directories with sizes, cache scope
  (the opaque ID, never the display name), and browser-version
  compatibility.
- `remove <NAME>`: deletes the profile and its data, prints the
  removed opaque ID.

`CacheScope::Profile` keys on the opaque ID throughout, so renames
and display-name collisions cannot cross-contaminate cached fetches.
See `architecture/fetch.md`.

## Hidden `windows-service` (`cfg(windows)` only)

`eggsearch windows-service` (hidden from help) is the SCM entry
point: it calls `startup::run_windows_service()` with the raw
`--config` path. It is never invoked interactively; the registered
SCM `binPath` points at it. Operator-visible Windows management
stays `startup install/uninstall`, `restart`, and `update`.

---

## Cross-command invariants

- Global `--config` flows either into `config::load()` (search-side
  commands) or into `RuntimeSpec::current()` (service-side
  commands). There is no third config path.
- Every policy denial names the config key that fixes it
  (`[search].mode`, `[search].providers`, `[fetch].enabled`,
  `[fetch.browser].persistent_profiles_enabled`).
- Human output is the default; `--json` selects pretty JSON and is
  accepted by `search`, `fetch`, `providers`, `startup status`, and
  `integrate`. `doctor` always prints its JSON status block first
  and appends probe lines only with `--probe`.
- Verbosity is orthogonal: `-v`/`-vv` only widens stderr tracing.

[← Back to Overview](overview.md)
