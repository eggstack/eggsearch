# LM Studio

Eggsearch can run as a local stdio MCP server in LM Studio. The `compact`
presentation keeps all ten Eggsearch tools available while shortening their
advertised schemas. It also selects the `brief` response default; callers can
request `response_detail = "diagnostic"` for a full diagnostic response.
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
that the server connects and all ten Eggsearch tool names appear. stdio is a
client-owned process: stdout carries MCP protocol messages, so do not wrap the
command in a script that writes status text to stdout. Eggsearch logs to stderr.

To use the compatibility profile, remove `--config` and its path from `args`,
or set `schema_profile = "full"`. The executable must still be a build that
understands the `[mcp]` section. Changes to the profile require a fresh server
process and may require a new chat because MCP clients can cache tool lists.

To verify an installed candidate's stdio handshake, tool list and local
diagnostic call without starting a model, run:

```bash
python3 scripts/lm_studio_stdio_smoke.py /absolute/path/to/eggsearch
```

The smoke starts the executable with temporary full and compact configs,
checks all ten names and the compact byte limit, calls `provider_status` with
probing disabled, and compares its structured output across profiles.

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

## What the measurements establish

Eggsearch's offline fixtures measure serialized schema and result sizes and
exercise the MCP protocol. They do not establish LM Studio tokenizer counts,
model-visible prompt inclusion, tool-choice quality, or end-to-end task
success. Those require a live LM Studio chat with the actual local model. The
recorded environment used LM Studio 0.4.26+4. After the user enabled
authentication and configured-server access, a native `/api/v1/chat` request
through `mcp/eggsearch` completed a `web_fetch` call and returned LM Studio's
usage counters; see the M003 operational record for the task and limits. A
controlled direct-fetch pair found that compact used fewer reported input
tokens and returned the same document evidence, while other specialist tasks
showed no-call, argument, and host plugin-loading failures. The interactive
app also rendered an Eggsearch `web_fetch` result card. These observations do
not establish a general context or completion-quality improvement, and host
handling of `structuredContent` versus the text fallback or profile refresh
has not been isolated.

For configuration keys and bounds, see [Eggsearch configuration](config.md)
and the [context-efficiency measurements](retrieval-quality-context-efficiency.md).
