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

## Managed startup and restart

Persistent services are managed explicitly and only for `mcp serve`:

```bash
eggsearch startup instructions
eggsearch startup install
eggsearch startup status --json
eggsearch restart
eggsearch startup uninstall
```

Auto selection uses launchd on macOS, a running systemd on Linux, Windows SCM
on Windows, and cron on other Unix/Linux hosts. A detected but unusable
preferred manager is an error; the CLI never silently installs a second cron
registration. See [Managed service](service.md) for manager-specific paths,
manual commands, permissions, logs, and cron identity controls.

`eggsearch croncheck` starts a detached server only after an explicit local
connection refusal, and rechecks under a startup lock before spawning.
Ambiguous health failures do not create duplicates.

## Foreground operation and shutdown

Persistent mode logs to stderr, making it suitable for capture by a supervisor.
Ctrl-C and Unix SIGTERM cancel active rmcp sessions and drain connections for at
most ten seconds. The same cancellation seam is used by service managers and
the Windows service entry point.

The ordinary installer only installs the binary. Pass explicit `--service` to
delegate registration to `eggsearch startup install`; installer scripts do not
duplicate manager logic or elevate.

## Client-spawned versus persistent topology

Use client-spawned stdio for a workstation or CodeGG's default backend:

```text
MCP client ──starts──> eggsearch mcp stdio
```

Use persistent loopback HTTP when several local clients or a startup manager
should share one process:

```text
systemd/launchd/SCM/cron ──starts──> eggsearch mcp serve
MCP client ──HTTP──> 127.0.0.1:11320/mcp
watchdog/status ──GET──> 127.0.0.1:11320/healthz
```

Register clients with `eggsearch integrate`. It prints configuration by
default and applies only with `--apply`; see [MCP integrations](integrations.md).
