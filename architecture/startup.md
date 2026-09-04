# Startup supervision architecture

`src/startup.rs` is the single owner of persistent service lifecycle policy.
It is deliberately outside `src/mcp/`: the MCP module owns protocol and
transport behavior, while startup owns manager registration and process
ownership.

## Runtime specification

`RuntimeSpec` resolves an absolute executable, an absolute config path, the
loopback bind `127.0.0.1:11320`, MCP path `/mcp`, and the health URL
`/healthz`. Its argv is reused by systemd, launchd, cron, and Windows SCM.
Service definitions are embedded with `include_str!` and mirrored under
`packaging/`, so installed binaries do not require a checkout.

## Manager policy

`PlatformInfo` isolates host detection from policy. Auto selection is Windows
SCM, macOS launchd, active Linux systemd, then cron for other Unix/Linux
hosts. Linux systemd detection runs `systemctl is-system-running` and accepts
only usable `running` or `degraded` states; the presence of a systemctl binary
alone is insufficient. Explicit manager selection is platform-gated.

Systemd owns `/etc/systemd/system/eggsearch.service` and uses a dynamic service
identity, bounded failure restart, and filesystem/network hardening. macOS owns
the per-user LaunchAgent `com.eggstack.eggsearch`. Windows owns the SCM service
`Eggsearch` and its service entry point. Cron owns one marked user-crontab line
and preserves all unrelated lines.

## Health and ownership

`probe_health` first performs a bounded loopback TCP connect and then validates
the bounded JSON `/healthz` response. Only `service=eggsearch` and
`status=ready` is healthy. Refused means definitely absent; timeout, malformed
JSON, wrong service, non-ready status, and other errors are ambiguous and never
authorize a spawn.

`croncheck` takes a create-once startup lock, rechecks health, launches the
canonical command detached, and polls health. The persistent process writes an
owned PID record containing its executable and Linux `/proc` start token.
Cron restart/uninstall sends SIGTERM only after both identity checks pass, which
prevents a stale PID record from targeting an unrelated or reused process.

`startup status` reports registration, manager state, health, and conflicts.
`restart` refuses conflicts and delegates to the one registered manager. No
startup path searches by process name, uses `pkill`, or touches stdio children.

## Update integration

Normal updates snapshot startup state before replacement. After checksum,
identity, and atomic replacement verification, a previously healthy registered
service is restarted through its manager and health-checked. Stopped services,
unregistered binaries, and client-owned stdio processes are left alone. A
successful replacement followed by restart failure reports the installed
version and exact restart command without rolling back the verified binary.
