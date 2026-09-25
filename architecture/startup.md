# Startup Supervision Deep Dive

**Owner:** `src/startup.rs` (single owner of persistent lifecycle policy).
**Mirrors:** `packaging/systemd/eggsearch.service`,
`packaging/launchd/com.eggstack.eggsearch.plist`,
`packaging/windows/eggsearch-service.txt`.
**Scope:** the canonical persistent runtime targets `mcp serve` only.
Client-owned `mcp stdio` processes are never registered, restarted, or
probed by anything in this module. See `architecture/commands.md` for
the CLI surface and `architecture/packaging.md` for update/release.

---

## Canonical runtime (`RuntimeSpec`)

Every manager launches the same argv, built once by `RuntimeSpec`:

```
<exe> --config <config> mcp serve --bind 127.0.0.1:11320 --path /mcp [--pid-file <pid>]
```

- `RuntimeSpec::current(config, pid_file)` canonicalizes
  `current_exe()` and resolves the config to an absolute path
  (`--config` or `default_config_path()`), rejecting embedded
  NUL/CR/LF in either path.
- `bind` defaults to `mcp::http::DEFAULT_BIND`, `path` to
  `DEFAULT_PATH` (`/mcp`); `health_url()` appends `HEALTH_PATH`
  (`/healthz`). `args()` / `command_line()` render the shell-safe
  form reused by installers, instructions, and cron lines.
- `runtime_spec()` applies one special case: a systemd install with
  no explicit `--config` pins the config to
  `/etc/eggsearch/eggsearch.toml` instead of the invoking user's
  default path.
- Service definitions are embedded with `include_str!` (systemd unit
  and launchd plist templates) so an installed binary renders them
  without a repository checkout; `packaging/` holds the mirrored
  source of truth.

## Manager detection and selection

`PlatformInfo {os, systemd_active, launchd_available,
windows_scm_available}` separates host facts from policy so tests can
inject platforms. `platform_info()` fills it per host: Windows checks
for `sc.exe`, macOS checks for `launchctl`, Linux checks
`systemctl is-system-running` for `running`/`degraded` (a systemctl
binary alone is insufficient), other Unix gets no manager.

`select_method(requested, info)` enforces the policy:

| Request | Result |
|---------|--------|
| `windows` | only on Windows with SCM available |
| `launchd` | only on macOS with `launchctl` |
| `systemd` | only on Linux with systemd active |
| `cron` | only on Linux / other Unix |
| `auto` | Windows SCM, macOS launchd, active Linux systemd, else cron fallback |

`resolve_method()` adds registration awareness: explicit `--method`
values go straight through validation, while `auto` first checks
`registered_methods()` and reuses the single existing registration
instead of re-detecting. Multiple registrations are a hard
`AlreadyExists` error, never a guess.

`manager_running()` distinguishes registration from liveness:
systemd asks `systemctl is-active`, launchd asks `launchctl print`
in the caller's GUI domain, Windows matches `RUNNING` in
`sc.exe query`, and cron always reports false because a crontab line
is a schedule, not a supervisor.

## Per-manager rendering

- **systemd** owns `/etc/systemd/system/eggsearch.service`, rendered
  by substituting `{{EXEC_START}}` with the systemd-quoted canonical
  argv. The template sets `Type=simple`, `Restart=on-failure` with a
  5s delay and burst cap, `DynamicUser=yes` with state/cache
  directories, and hardening (`NoNewPrivileges`, `ProtectSystem`,
  `ProtectHome`, `PrivateTmp`, restricted address families).
  Install requires root and runs
  `daemon-reload` + `enable --now`; uninstall runs
  `disable --now`, removes the unit, and reloads.
- **launchd** owns the per-user agent
  `~/Library/LaunchAgents/com.eggstack.eggsearch.plist`, rendered by
  substituting `{{PROGRAM_ARGUMENTS}}` with the XML-escaped canonical
  argv and `{{STDOUT_PATH}}` / `{{STDERR_PATH}}` with
  `~/Library/Logs/eggsearch.log`. Install writes the plist
  atomically, then `bootout` + `bootstrap` + `kickstart -k` in the
  `gui/<uid>` domain; uninstall reverses it and removes the file.
- **cron** owns exactly one user-crontab line:
  `* * * * * <canonical-command> # eggsearch-managed`.
  `update_crontab()` strips every line carrying the marker, appends
  the replacement (or nothing on uninstall), preserves all unrelated
  lines and the trailing newline, and rejects NUL/CR/LF injection.
  Reads go through `crontab -l`, writes through a staged
  `crontab <file>` replacement.
- **Windows SCM** owns the `Eggsearch` service (`start= auto`,
  restart-on-failure policy, description). `binPath` points at the
  hidden `windows-service` entry point with the canonical config
  (`"<exe>" --config "<config>" windows-service`), which runs the
  same Streamable HTTP server under SCM cancellation. The CLI never
  requests elevation itself; failures reprint the exact `sc.exe`
  command to run from an elevated prompt.

All file writes use atomic staged persistence (`tempfile` +
`persist` + `sync_all`). `instructions` renders each of the above
without mutating anything, plus the detected method, executable,
config, `GET <health-url>`, and canonical command line.

## `/healthz` readiness

The server side (`src/mcp/http.rs`) mounts `GET /healthz` next to,
not inside, the MCP session handler and answers
`{service: "eggsearch", status: "ready", version, protocol}` with a
256-byte body cap.

The client side (`probe_health()`) issues one bounded eggfetch
request: redirects disabled, 800ms pool/connect/write/read/total
timeouts, 256-byte decoded-body cap, content-length pre-check. The
result is a `HealthState`:

- `Healthy`: 2xx plus `service == "eggsearch"` and
  `status == "ready"`.
- `Refused`: typed connection-refused. This is the **only** state
  that means "definitely absent" and authorizes a spawn.
- `Timeout`, `Malformed`, `WrongService`, `NonReady`, `Error`:
  ambiguous. They never authorize a spawn and fail closed.

`install` and `restart` poll with `wait_for_health()` (30 attempts at
100ms) and fail when readiness never arrives.

## Cron watchdog and `croncheck`

Cron has no supervisor, so the one-minute crontab entry replays the
watchdog: `croncheck` builds the current spec with the cron PID
path, probes health, and returns "healthy; no action taken" on
`Healthy`. `Refused` proceeds; every ambiguous state errors out
without spawning.

The spawn path is double-guarded:

1. A create-once lock file (`croncheck.lock`) carrying the starter's
   process record. A live holder yields `WouldBlock`; a stale lock
   is removed and retaken. The lock is deleted after the attempt.
2. A health recheck after lock acquisition catches the race where
   another watchdog won.
3. `spawn_detached()` launches the canonical argv detached
   (`setsid`, stdio nulled, output appended to the cron log), then
   `wait_for_health()` verifies readiness.

The persistent process holds a `PidFileGuard` over `mcp-serve.pid`.
The record stores PID, canonical executable, and the Linux `/proc`
start-time token. `process_matches()` requires all three to agree:
same resolved executable **and** same start token, so a stale file
can never SIGTERM an unrelated or recycled PID. `restart_cron` and
cron `uninstall` stop only through `stop_owned_process()`; systemd,
launchd, and Windows restarts delegate to their managers instead.

## Identity-safe restart and service state

`restart()` never scans process names, never `pkill`s, and never
touches stdio children:

1. Load `startup_state()`. On `conflict` (several managers
   registered) refuse with the detail string.
2. On no registration refuse with "no managed persistent service is
   registered; stdio processes are client-owned".
3. Delegate to the one manager (`systemctl restart`, `launchctl
   kickstart -k`, cron stop-wait-spawn, `sc.exe stop/start`) using
   the canonical spec for that manager.
4. Verify `/healthz` before reporting success.

`StartupState {method, registered, running, healthy, conflict,
detail}` is what `startup status` prints: a one-line detail plus
`registered/running/healthy/conflict`, or pretty JSON with
`--json`. `running` reflects the manager; `healthy` reflects the
loopback probe, so "running but not healthy" is representable.

## Update restarts only the healthy registered service

`src/update.rs` snapshots `startup_state()` **before** replacing the
binary. After checksum, `--version` identity, and atomic replacement
verification, `finish_lifecycle()` restarts only when all hold:

```
registered && healthy && (running || method == cron)
```

Stopped services, unregistered binaries, and client-owned stdio
processes are left alone. A post-replacement restart failure keeps
the verified binary and reports the installed version plus the exact
`restart` command (`restart_command()`), without rolling back.

[← Back to Overview](overview.md)
