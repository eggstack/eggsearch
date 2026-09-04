# Managed persistent service

eggsearch has two separate lifecycles:

- `eggsearch mcp stdio` is client-owned. MCP clients start and stop it, and
  `eggsearch restart` never targets it.
- `eggsearch mcp serve` is a persistent loopback-only HTTP service. The
  `startup` commands manage this lifecycle.

The default runtime is `127.0.0.1:11320`, MCP path `/mcp`, and health URL
`http://127.0.0.1:11320/healthz`. Health is local and does not call providers.

## Automatic setup

Inspect exact manager commands without changing anything:

```bash
eggsearch startup instructions
```

Install, inspect, restart, and remove the detected manager with:

```bash
eggsearch startup install
eggsearch startup status
eggsearch restart
eggsearch startup uninstall
```

Automatic selection is Windows SCM on Windows, launchd on macOS, systemd on a
running Linux systemd host, and the user's crontab on other Unix/Linux hosts.
The CLI never invokes `sudo` or requests UAC. A privilege failure prints the
exact elevated command and never creates a cron duplicate.

## systemd

The system backend owns `/etc/systemd/system/eggsearch.service`, uses an
absolute executable path, a dedicated dynamic service identity, bounded
failure restarts, and hardening directives. It uses
`/etc/eggsearch/eggsearch.toml` when no explicit `--config` is supplied.

```bash
sudo /usr/local/bin/eggsearch startup install --method systemd
sudo systemctl status eggsearch.service
curl --fail --silent http://127.0.0.1:11320/healthz
sudo /usr/local/bin/eggsearch startup uninstall --method systemd
```

## macOS launchd

macOS uses a per-user LaunchAgent at
`~/Library/LaunchAgents/com.eggstack.eggsearch.plist`. It starts at login,
keeps the service alive after abnormal exits, and writes logs under
`~/Library/Logs/eggsearch.log`.

```bash
eggsearch startup instructions --method launchd
eggsearch startup install --method launchd
eggsearch startup status
eggsearch startup uninstall --method launchd
```

This is a login service, not a pre-login system LaunchDaemon.

## Windows SCM

Windows uses the Service Control Manager service named `Eggsearch`, with
automatic start and bounded SCM failure actions. Run the installer or CLI from
an elevated PowerShell prompt; the binary and service command use absolute
paths.

```powershell
& 'C:\Program Files\Eggsearch\eggsearch.exe' startup install --method windows
sc.exe query Eggsearch
Invoke-WebRequest http://127.0.0.1:11320/healthz
& 'C:\Program Files\Eggsearch\eggsearch.exe' startup uninstall --method windows
```

Unprivileged commands explain the required Administrator rerun and do not
trigger UAC automatically.

## Cron fallback

Cron installation changes only the user's crontab entry marked
`# eggsearch-managed`:

```cron
* * * * * '/absolute/path/eggsearch' --config '/absolute/path/config.toml' croncheck # eggsearch-managed
```

Unrelated entries are retained. `croncheck` probes `/healthz`, starts a
detached persistent server only on an explicit connection refusal, takes a
startup lock, and verifies health after spawning. Timeout, malformed, wrong-
service, and non-ready responses never spawn a duplicate. Restart and
uninstall can signal only a process whose owned PID record matches its
executable and `/proc` start token; arbitrary `eggsearch` or stdio processes
are never killed.

Logs and the cron control record are kept under the platform user data
directory in `eggsearch/`. Inspect exact paths with:

```bash
eggsearch startup instructions --method cron
```
