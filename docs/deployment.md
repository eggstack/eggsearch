# Deployment

eggsearch has two MCP transport lifecycles:

| Use case | Command | Lifecycle |
|---|---|---|
| Local client integration | `eggsearch mcp stdio` | The client starts and stops a child process |
| Persistent local endpoint | `eggsearch mcp serve` | eggsearch owns a foreground loopback listener |

## Persistent Streamable HTTP

Start the persistent endpoint explicitly:

```bash
eggsearch mcp serve --bind 127.0.0.1:11320 --path /mcp
```

Defaults are `127.0.0.1:11320` and `/mcp`. `--bind` is a typed socket
address and this release rejects every non-loopback address, including
`0.0.0.0`; there is no insecure public/LAN escape hatch. The MCP endpoint is
`http://127.0.0.1:11320/mcp`.

The HTTP transport uses rmcp 3.2.0's Streamable HTTP implementation and keeps
the same server identity, ten tools, schemas, validation, and tool behavior as
stdio. Legacy clients can use the initialize/initialized session flow. Current
MCP clients can use the stateless discovery/request-metadata flow supported by
rmcp.

The listener applies loopback Host and local Origin validation. MCP POST bodies
are capped at 1 MiB, request headers are bounded, and requests have a
120-second timeout. These transport limits are additive to the existing tool,
fetch, provider, and sanitization limits.

## Health endpoint

`GET /healthz` is outside MCP session state and returns a small JSON response:

```json
{
  "service": "eggsearch",
  "status": "ready",
  "version": "0.3.8",
  "protocol": "streamable-http"
}
```

HTTP 200 means the process has bound its listener and is ready to accept MCP
initialization or discovery. Health does not call providers, search, fetch, or
browser services and never exposes credentials or configuration paths.

## Foreground operation and shutdown

Persistent mode logs to stderr, making it suitable for capture by a future
supervisor. Ctrl-C and Unix SIGTERM cancel active rmcp sessions and drain
connections for at most ten seconds. The same cancellation seam is retained
for the Windows service-control integration planned for phase 9.

The ordinary installer only installs the binary. It does not start or register
a service; systemd, launchd, Windows SCM, and cron integration are not part of
this phase.
