# Agent and IDE integrations

`src/integrations/` owns the client adapter layer behind
`eggsearch integrate`. It is separate from `mcp/` and `startup.rs`: MCP owns
the server protocol, startup owns the persistent process, and integrations own
client registration.

## Contract

Every client exposes a print path for stdio and loopback Streamable HTTP. The
default is stdio. Apply is opt-in and is one of:

| Client | Apply path | Configuration surface |
|---|---|---|
| CodeGG | atomic JSON + backup | `search.backend`, optional `mcp.eggsearch` |
| Codex | native CLI | `codex mcp add` |
| Claude Code | native CLI | `claude mcp add --scope user` |
| VS Code | native CLI | `code --add-mcp` |
| Cursor | atomic JSON + backup | `~/.cursor/mcp.json` |
| OpenCode | strict JSON only | `mcp.servers`; JSONC is print-only |
| Zed | print-only | JSONC `context_servers` settings |

The common report contains client, transport, availability, apply mode,
configuration path or argv, and verification state. It excludes provider
credentials and unrelated configuration.

## Safety

Native commands receive argv directly. Direct edits parse the complete JSON
document before creating a timestamped backup and replacing the file through a
same-directory temporary file. A malformed document, non-object root, or
unsupported JSONC format fails before mutation. Only the exact `eggsearch`
entry is changed.

Applied registrations run an MCP initialize and `tools/list` check. Stdio
verification launches `eggsearch mcp stdio`; HTTP verification checks the
loopback `/healthz` identity before using Streamable HTTP. `web_search` and
`web_fetch` are required; other tools are recommended coverage.
