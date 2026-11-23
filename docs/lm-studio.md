# LM Studio

Eggsearch can run as a local stdio MCP server in LM Studio. The `compact`
presentation keeps all nine canonical Eggsearch tools available while shortening
their advertised schemas. It also selects the `brief` response default; callers
can request `response_detail = "diagnostic"` for a full diagnostic response.
The ordinary `full` profile and its diagnostic default remain unchanged.

## Install and configure

Install a candidate or published Eggsearch executable that supports the
`[mcp]` settings below. The compact option is not available in older releases.
Create a config file named `eggsearch-lm-studio.toml`:

```toml
[mcp]
schema_profile = "compact"
brief_max_chars = 8000
```

LM Studio reads MCP server registrations from `mcp.json`. On macOS and Linux,
the user file is `~/.lmstudio/mcp.json`; on Windows it is
`%USERPROFILE%\.lmstudio\mcp.json`. Replace the example executable and config
paths with the paths on your machine:

```json
{
  "mcpServers": {
    "eggsearch": {
      "command": "/absolute/path/to/eggsearch",
      "args": [
        "--config",
        "/absolute/path/to/eggsearch-lm-studio.toml",
        "mcp",
        "stdio"
      ]
    }
  }
}
```

For Windows, use an absolute executable path such as
`C:\\Users\\you\\AppData\\Local\\Eggsearch\\eggsearch.exe` and a TOML
path such as `C:\\Users\\you\\.config\\eggsearch\\lm-studio.toml` in the
JSON strings. Keep the JSON escaping for backslashes.

Restart LM Studio or refresh its MCP server list, then start a new chat. Confirm
that the server connects and all nine canonical tool names appear. stdio is a
client-owned process: stdout carries MCP protocol messages, so do not wrap the
command in a script that writes status text to stdout. Eggsearch logs to stderr.

To use the compatibility profile, remove `--config` and its path from `args`,
or set `schema_profile = "full"`. The executable must still be a build that
understands the `[mcp]` section. Changes to the profile require a fresh server
process and may require a new chat because MCP clients can cache tool lists.

## API integrations

LM Studio's API can address configured MCP servers as `mcp/eggsearch` and can
restrict a request to selected tools with `allowed_tools`. API access to MCP
servers declared in `mcp.json` is controlled by LM Studio's **Allow calling
servers from mcp.json** server setting and authentication. Enabling it expands
which API clients can invoke locally configured MCP tools; use LM Studio's
authentication and access controls before enabling it. This setting is not
needed for ordinary interactive app use.

See LM Studio's [MCP app guide](https://lmstudio.ai/docs/app/mcp),
[MCP API guide](https://lmstudio.ai/docs/developer/core/mcp), and
[server settings](https://lmstudio.ai/docs/developer/core/server/settings).

## Schema and response size

The compact profile reduces advertised schema size and defaults search/fetch
responses to `brief`. It does not guarantee a fixed model-token reduction or
better task results. See [Eggsearch configuration](config.md) for the setting
and its bounds.
