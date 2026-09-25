# Agent and IDE integrations

**Location:** `src/integrations/` (9 files), `src/commands/integrate.rs`

`src/integrations/` owns the client adapter layer behind `eggsearch integrate`.
It is separate from `mcp/` and `startup.rs`: MCP owns the server protocol,
startup owns the persistent process, and integrations own client registration.
The CLI wiring in `src/commands/integrate.rs` is a thin dispatcher over
`integrations::run()` and `integrations::summaries()`; all rendering, mutation,
and verification policy lives in `src/integrations/common.rs` with one small
per-client module each.

## File inventory

`src/integrations/` contains exactly 9 files for 7 clients:

| File | Role |
|------|------|
| `mod.rs` | Module declarations plus re-exports of `render`, `run`, `summaries`, `Client`, `Transport` |
| `common.rs` | Shared policy: client/transport enums, render dispatch, apply dispatch, atomic JSON edits, executable resolution, availability probes, protocol verification |
| `codegg.rs` | CodeGG JSON entry (`search.backend`, optional remote `mcp.eggsearch`) |
| `zed.rs` | Zed JSON entry (stdio command/args or remote URL) |
| `codex.rs` | Codex native `mcp add` / `mcp remove` argv |
| `claude.rs` | Claude native `mcp add --scope user` argv |
| `cursor.rs` | Cursor JSON entry (`command`/`args` or `url`) |
| `vscode.rs` | VS Code native `code --add-mcp '<json>'` argv |
| `opencode.rs` | OpenCode JSON entry (`type: local` array command or `type: remote`) |

No client logic lives outside these files except the `IntegrateCommand`
dispatch in `src/commands/integrate.rs`. `mod.rs` carries
`#![allow(missing_docs)]`; the public surface is the `Client`, `Transport`,
`IntegrationReport`, and `IntegrationSummary` types plus `run`, `render`,
and `summaries`.

## Client and transport matrix

`Client::ALL` fixes the supported set at 7. `Transport` has exactly two
variants, `Stdio` and `Http`. Every client renders both transports; stdio is
the default everywhere.

| Client | Display name | Transports | Apply mode | Configuration surface |
|--------|--------------|------------|------------|-----------------------|
| CodeGG | CodeGG | stdio, http | `json-atomic-backup` | JSON config file, `search.backend` plus optional `mcp.eggsearch` |
| Zed | Zed | stdio, http | `print-only` | Rendered JSONC `context_servers` snippet applied by hand in the client |
| Codex | Codex | stdio, http | `native-cli` | `codex mcp add` / `codex mcp remove` |
| Claude | Claude | stdio, http | `native-cli` | `claude mcp add --scope user` / `claude mcp remove --scope user` |
| Cursor | Cursor | stdio, http | `json-atomic-backup` | `~/.cursor/mcp.json`, `mcpServers.eggsearch` |
| VS Code | VS Code | stdio, http | `native-cli` | `code --add-mcp '<entry>'` |
| OpenCode | OpenCode | stdio, http | `json-atomic-backup-if-json` | `mcp.servers.eggsearch`; strict JSON only, JSONC is print-only |

`summaries()` reports one row per client with `available`, `stdio: true`,
`http: true`, and the static `apply_mode` above. `integrate list` prints that
table, or JSON with `--json`. Per-client `integrate <client>` reports client,
transport, availability, apply mode, config path or argv, the rendered
configuration when one exists, and verification state. Reports never include
provider credentials or unrelated configuration.

## CLI surface

```bash
eggsearch integrate list [--json]
eggsearch integrate <client> [--transport stdio|http] [--apply] [--json] [--executable PATH]
```

Global flags on `eggsearch integrate` are `--json` (machine-readable report)
and `--executable PATH` (installed binary to register for stdio clients).
Transport defaults to stdio. The default for every client is print-first:
without `--apply` the command renders the argv or JSON snippet, prints the
config path, and exits with `rendered only; pass --apply to register and
verify`. Nothing is mutated, no child process is spawned, and no network
connection is opened on the render path.

`--apply` is opt-in registration plus verification. It fails closed when:

- the client binary is not on `PATH` (`{client} is not available on PATH`);
- the client is print-only (Zed, and OpenCode JSONC) — the error tells the
  operator to add the snippet in the client;
- the transport is stdio, no `--executable` was given, and the running binary
  resolves to a development path (see below).

## Transports

Stdio is the client-owned lifecycle: the client spawns
`<executable> mcp stdio` per session. It is the default because it needs no
persistent process and no port.

HTTP is the explicit loopback remote: the client connects to the persistent
server started by `eggsearch mcp serve` or a registered startup manager.
The shared constant is:

```text
HTTP_ENDPOINT = http://127.0.0.1:11320/mcp
```

HTTP verification additionally checks `http://127.0.0.1:11320/healthz` and
requires the payload to identify `service: eggsearch` with `status: ready`
before opening the Streamable HTTP session. No other host, port, or path is
ever rendered; loopback is not configurable from `integrate`.

## Per-client rendering

`render_internal()` resolves the executable once, then dispatches. Native-CLI
clients produce `command` argv and no `configuration`; JSON clients produce a
`configuration` value and no `command`.

Native-CLI argv (stdio first, then HTTP):

- Codex stdio: `codex mcp add eggsearch -- <exe> mcp stdio`; HTTP:
  `codex mcp add eggsearch --url http://127.0.0.1:11320/mcp`.
- Claude stdio: `claude mcp add --scope user eggsearch -- <exe> mcp stdio`;
  HTTP: `claude mcp add --scope user --transport http eggsearch
  http://127.0.0.1:11320/mcp`.
- VS Code stdio: `code --add-mcp '{"name":"eggsearch","type":"stdio",
  "command":"<exe>","args":["mcp","stdio"]}'`; HTTP replaces the entry with
  `{"name":"eggsearch","type":"http","url":"http://127.0.0.1:11320/mcp"}`.
  `code-insiders` counts as available for the pre-apply availability probe,
  but the rendered argv always invokes `code`.

JSON entries:

- CodeGG stdio: `{"search":{"backend":"eggsearch"}}`; HTTP adds
  `{"mcp":{"eggsearch":{"type":"remote","url":"...","enabled":true}}}`.
  Apply merges `search.backend` always and merges `mcp.eggsearch` only when
  the rendered HTTP configuration carries it.
- Cursor stdio: `{"command":"<exe>","args":["mcp","stdio"]}`; HTTP:
  `{"url":"http://127.0.0.1:11320/mcp"}`. Applied under `mcpServers.eggsearch`.
- Zed stdio: `{"command":"<exe>","args":["mcp","stdio"],"env":{}}`; HTTP:
  `{"url":"http://127.0.0.1:11320/mcp"}`. Render-only; the operator pastes it
  into the JSONC `context_servers` settings.
- OpenCode stdio: `{"type":"local","command":["<exe>","mcp","stdio"]}`; HTTP:
  `{"type":"remote","url":"...","oauth":false}`. Applied under
  `mcp.servers.eggsearch` in strict JSON only.

## Print-first apply and atomic JSON edits

Direct JSON edits go through `update_json_file()`. The sequence is fixed:

1. Read the whole file if it exists, otherwise start from `{}` (creating
   parent directories as needed).
2. Parse the complete document. Malformed JSON, a non-object root, or a
   Cursor/OpenCode shape violation fails before any mutation and leaves the
   file byte-identical.
3. Apply only the exact `eggsearch` entry (`search.backend` /
   `mcp.eggsearch` for CodeGG, `mcpServers.eggsearch` for Cursor,
   `mcp.servers.eggsearch` for OpenCode). Unrelated servers and keys are
   preserved; the update is a no-op without a backup when the tree is already
   identical after formatting.
4. On change with a pre-existing file, copy the original to a timestamped
   backup `<path>.bak.<YYYYMMDDTHHMMSSZ>` and print `backup: <path>`.
5. Serialize pretty JSON plus trailing newline, write to a same-directory
   `NamedTempFile`, `sync_all`, preserve the original permission bits, and
   atomically persist over the destination.

OpenCode adds a format gate: when the resolved config path ends in `.jsonc`
the apply aborts as print-only with a message directing the operator to
`opencode mcp add` or a manual edit, because JSONC comments cannot round-trip
through `serde_json`. Stdio shapes are re-validated after merge
(`command` must stay a string for Cursor, an argument array for OpenCode).

## Argv boundaries

Native commands are executed as argv arrays through `run_command()`, never
through a shell. The `--` separator before the eggsearch executable in Codex
and Claude stdio argv keeps the executable path out of the client's option
parsing even when it starts with `-` or contains spaces. `shell_join()` is
display-only for the human-readable report. `apply_native()` for Codex and
Claude first probes `<cli> mcp get eggsearch`; when an entry already exists
it runs the matching `remove` argv before `add`, so re-apply is idempotent.
VS Code apply runs the single `code --add-mcp` argv directly.

## Executable resolution

`resolve_executable()` prefers `--executable PATH` verbatim (empty values
rejected) and otherwise uses `std::env::current_exe()`. A current executable
with any `target`, `debug`, or `deps` path component is classified ephemeral:
render substitutes the bare `eggsearch` name on `PATH`, and stdio `--apply`
without an explicit `--executable` aborts with `install eggsearch or pass
--executable /path/to/eggsearch before using --apply`. HTTP transports are
exempt because they register a URL, not a binary. Never register
`target/debug` binaries: require an installed executable or an explicit
`--executable` pointing at one.

Availability probes check `PATH` for an executable file (executable bit on
Unix): `codegg`, `zed`, `codex`, `claude`, `cursor` or `cursor-agent`,
`code` or `code-insiders`, `opencode`.

## Protocol verification

Every successful `--apply` ends with `verify()`, and verification failure
fails the command after registration. Stdio verification spawns
`<executable> mcp stdio` over `rmcp::transport::TokioChildProcess`, runs MCP
initialize plus `tools/list`, and cancels the child. HTTP verification first
GETs `/healthz` with a 5-second bounded client and a 64 KiB body cap, checks
the `eggsearch`/`ready` identity, then runs initialize plus `tools/list` over
`StreamableHttpClientTransport` against the loopback endpoint.

`check_tools()` requires `web_search` and `web_fetch`; any other absence is a
stderr warning listing the missing recommended tools (`batch_fetch`,
`repo_search`, `repo_fetch`, `repo_map`, `security_search`,
`research_search`, `build_evidence_bundle`, `provider_status`). The success
message is `registered and verified minimum MCP tool set`.

## Failure semantics

Integration errors are fail-closed and non-destructive: bad JSON, non-object
roots, JSONC applies, missing clients, ephemeral executables, health-identity
mismatches, and missing required tools all return errors without partial
writes (backups aside, no destination is touched until the new document is
fully staged). `integrate` never touches provider credentials, never edits
entries other than `eggsearch`, never elevates, and never starts a persistent
server; HTTP apply assumes the loopback server is already running.
