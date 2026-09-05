# MCP client integrations

`eggsearch integrate` prints the exact registration for CodeGG and common
MCP-capable agents and IDEs. It defaults to stdio, so the client owns the
eggsearch process and no service is required.

```bash
eggsearch integrate list
eggsearch integrate codegg
eggsearch integrate codex --executable /usr/local/bin/eggsearch
```

Apply is explicit. Native CLI clients are changed through their supported
commands. Strict JSON clients use an atomic write and create a timestamped
`.bak.*` backup before changing only the `eggsearch` entry. A malformed file
is rejected before any backup or write. No provider credentials are copied
into client configuration.

## CodeGG

CodeGG is the first-class integration. Stdio needs only the backend selector;
CodeGG keeps its existing default command and arguments:

```bash
eggsearch integrate codegg --transport stdio --apply
```

For a previously installed persistent service, use the explicit remote MCP
path:

```bash
eggsearch startup install
eggsearch integrate codegg --transport http --apply
```

The HTTP form selects the eggsearch search backend and adds CodeGG's normal
remote `mcp.eggsearch` entry at `http://127.0.0.1:11320/mcp`. CodeGG still
requires `web_search` and `web_fetch`; its provider diagnostic is best effort.

## Native CLI clients

Codex, Claude Code, and VS Code use their current native registration commands:

```bash
eggsearch integrate codex --transport stdio --apply
eggsearch integrate claude --transport http --apply
eggsearch integrate vscode --transport stdio --apply
```

The adapters use `codex mcp add`, `claude mcp add --scope user`, and
`code --add-mcp` respectively. Existing entries named `eggsearch` are updated
by the client command; unrelated servers are not selected. The CLI checks the
native command is available before applying.

Current references: [Codex MCP CLI](https://developers.openai.com/codex/cli/mcp),
[Claude Code MCP](https://docs.anthropic.com/en/docs/claude-code/mcp), and
[VS Code MCP servers](https://code.visualstudio.com/docs/agent-customization/mcp-servers).

## Settings-file clients

Zed renders the current `context_servers` shape for its Settings Editor. Its
settings are JSONC-capable and there is no stable registration CLI, so
`--apply` intentionally remains print-only:

```bash
eggsearch integrate zed --transport stdio
eggsearch integrate zed --transport http
```

Cursor renders and can safely update the global `~/.cursor/mcp.json` entry:

```bash
eggsearch integrate cursor --transport stdio --apply
```

OpenCode renders the current `mcp.servers` shape. A strict JSON
`~/.config/opencode/opencode.json` can be updated with `--apply`; an existing
`opencode.jsonc` remains print-only so comments and trailing syntax are not
destroyed:

```bash
eggsearch integrate opencode --transport http --apply
```

Current references: [Zed MCP](https://zed.dev/docs/ai/mcp),
[Cursor MCP](https://docs.cursor.com/context/model-context-protocol), and
[OpenCode MCP servers](https://opencode.ai/v2/docs/mcp-servers).

## Verification and executable paths

An applied registration is verified with MCP `initialize` and `tools/list`.
The minimum required tools are `web_search` and `web_fetch`; missing
recommended tools are reported as a warning. HTTP verification first checks
`/healthz` and rejects an unhealthy or non-eggsearch service.

When invoked from a development checkout, stdio apply requires an installed
binary or an explicit `--executable /path/to/eggsearch`; it never writes a
`target/debug` path into client configuration.

The integrations command does not install clients, start remote services,
manage OAuth, or expose eggsearch beyond its loopback endpoint.
