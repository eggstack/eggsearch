# Phase 9 — Startup Supervision, Croncheck, and Restart

Status: planned
Depends on: phase 8 persistent Streamable HTTP MCP; phase 7 update hooks for restart integration
Baseline for planning: `f595683b8ebdec0afb13363ec9e8ad7654f9824b` (`eggsearch` 0.3.8)
Roadmap: `plans/deployment-roadmap.md`
Reference implementation: `eggstack/gregg` startup/croncheck/restart behavior

## Objective

Make persistent eggsearch deployments easy to keep running across reboot/login on Linux, macOS, Windows, and non-systemd Unix/Linux systems, while preserving one-manager ownership and conservative privilege behavior.

This phase adds:

```text
eggsearch croncheck
eggsearch restart

eggsearch startup status
eggsearch startup install [--method auto|systemd|launchd|cron|windows]
eggsearch startup instructions [--method ...]
eggsearch startup uninstall [--method ...]
```

The CLI, not the shell installer, owns manager detection/rendering/install/restart semantics. `packaging/install.sh --service` and Windows equivalent delegate to this surface after installing the binary.

## Non-goals

- No remote/LAN MCP exposure or auth.
- No arbitrary process manager ecosystem (supervisord, OpenRC, runit, Kubernetes, etc.) in this phase.
- No unattended update schedule.
- No hidden `sudo`/UAC elevation.
- No service supervision of `eggsearch mcp stdio`.

## Invariants

1. Startup supervision always targets persistent `eggsearch mcp serve`, never stdio.
2. Exactly one manager owns a configured persistent instance.
3. Auto detection never falls back to cron merely because a preferred manager needs privilege.
4. Install/uninstall operations are idempotent and preserve unrelated cron/config state.
5. `croncheck` starts only when eggsearch is definitely absent; ambiguous health failures do not spawn duplicates.
6. Restart uses the active registered manager rather than killing arbitrary `eggsearch` processes.
7. CLI/installers never invoke `sudo` or trigger UAC themselves.
8. Service definitions use absolute executable/config paths and shell-safe quoting.
9. Update restarts only a service that was running before the update; a stopped service remains stopped.
10. Manager health verification uses phase 8 `/healthz`, not external providers.

## Production changes

### 1. Add a startup module with injectable platform detection

Create a module such as `src/startup.rs` that owns:

- `StartupMethod::{Auto,Systemd,Launchd,Cron,Windows}` with platform gating;
- manager availability/detection;
- canonical rendered commands/templates;
- install/uninstall/status/restart operations;
- startup state returned to phase 7 updater;
- instruction rendering.

Keep detection functions injectable/testable rather than reading the live host directly in every code path.

Auto policy:

```text
windows                    -> Windows
macOS                      -> launchd
Linux with active systemd  -> systemd
other Unix/Linux           -> cron
```

On Linux, detect an actually running/usable systemd environment, not merely presence of `/bin/systemctl`.

### 2. Define persistent runtime command once

Manager templates and cron must resolve one canonical command, conceptually:

```text
<absolute-eggsearch> [--config <absolute-config>] mcp serve --bind 127.0.0.1:11320 --path /mcp
```

Do not hand-compose subtly different variants in every backend. Create a structured runtime specification and render it per manager.

The configured health URL derives from the same runtime bind/path defaults so `croncheck` and restart verification cannot drift.

### 3. Embed canonical service templates

Add source templates under:

```text
packaging/systemd/
packaging/launchd/
packaging/windows/   # docs/template metadata if needed
```

Embed canonical textual templates in the binary with `include_str!()` or generate from Rust-owned templates so `cargo install eggsearch` can still run `startup instructions/install` without a repository checkout.

Repository packaging files remain human-reviewable reference copies; add a test that embedded/rendered content matches the canonical source where duplication exists.

### 4. Systemd backend

Target a system service suitable for unattended SBC/fleet startup.

Recommended canonical paths for a privileged/system install:

```text
/usr/local/bin/eggsearch
/etc/eggsearch/eggsearch.toml          # optional explicit config
/etc/eggsearch/eggsearch.env           # optional provider secrets, mode 0600
/etc/systemd/system/eggsearch.service
```

The unit should:

- run `eggsearch mcp serve` on loopback;
- use an explicit absolute executable path;
- restart on abnormal failure with bounded delay/rate limiting;
- stop gracefully;
- use a conservative service identity. Prefer a dedicated `eggsearch` system user for keyless deployments if config/cache paths can be made correct; if implementation evidence shows per-user provider/config ownership is materially simpler/safer, document and test that model before landing rather than mixing identities;
- optionally consume an EnvironmentFile for provider credentials without embedding secrets into the unit;
- set a stable working/runtime/cache directory with permissions appropriate to the selected service user;
- avoid unnecessary privileges/capabilities;
- use `After=network-online.target` only if actually needed for startup semantics; health readiness itself must not require network connectivity.

`startup install --method systemd` should atomically write/update the unit/config scaffolding it owns, run `systemctl daemon-reload`, enable and start the unit when privileged, then verify `/healthz` within a bound.

Unprivileged invocation on a detected systemd host must not install cron instead. Print the exact elevated rerun command, for example:

```text
sudo /home/user/.local/bin/eggsearch startup install --method systemd
```

If the system-service template requires `/usr/local/bin/eggsearch` but the current executable is user-local, either copy/install through an explicit privileged flow or print the exact system-wide installer command. Do not create a unit pointing into an ephemeral/incorrect home path unless that is the deliberate documented service model.

### 5. macOS launchd backend

Use a per-user LaunchAgent as the default unprivileged macOS startup mechanism:

```text
~/Library/LaunchAgents/<stable-label>.plist
```

It should run at login, keep the persistent loopback server alive according to conservative launchd semantics, and capture stdout/stderr without interfering with MCP because HTTP transport is used.

Use absolute executable/config paths and XML-safe values. Install should use the current recommended `launchctl bootstrap`/`kickstart` family rather than relying on deprecated commands if current macOS guidance has changed by implementation time.

A system LaunchDaemon is not required in this workstream unless implementation finds it necessary for the user's fleet use case. If omitted, document that the default macOS service starts at user login rather than pre-login boot.

Permission failure must print exact manual/elevated instructions, not silently choose cron.

### 6. Windows service backend

Implement Windows SCM integration for persistent `mcp serve` rather than Task Scheduler if the current Rust ecosystem/service API remains appropriate.

Use a small maintained Windows service crate or the same proven approach as Gregg. Add a Windows-only internal service entry path as needed so the executable can:

- register with SCM;
- report service start/running/stopped states correctly;
- respond to stop controls by triggering phase 8 graceful shutdown;
- start automatically;
- run the persistent loopback MCP command with explicit config/runtime settings;
- configure bounded restart-on-failure behavior where SCM supports it.

Canonical install location when privileged should align with phase 6 PowerShell installer (e.g. `%ProgramFiles%\Eggsearch\eggsearch.exe`). Service-specific config may live under `%ProgramData%\eggsearch`.

Unprivileged Windows invocation must explain that Administrator rights are required and print the exact PowerShell/CLI rerun instructions. Do not request UAC automatically.

If SCM support becomes blocked by a dependency/MSRV issue, do not silently substitute an undocumented scheduler mechanism; record the blocker and create a scoped corrective plan.

### 7. Cron backend

Cron is the fallback for Unix/Linux environments without a usable native service manager.

Use the user's existing crontab via `crontab -l` / `crontab <temp>`; never edit spool files directly.

Managed entry should be clearly marked and shell-quoted, for example:

```text
* * * * * /absolute/path/eggsearch croncheck # eggsearch-managed
```

Implementation must:

- preserve unrelated entries byte-for-byte where feasible;
- replace/update exactly its own marker on repeated install;
- remove exactly its own marker on uninstall;
- reject newline/control-character/path injection in rendered values;
- use an absolute executable path;
- ensure the spawned persistent server is detached from cron stdio and does not create mail/log spam under normal operation.

A one-minute cadence is sufficient; do not implement sub-minute cron hacks.

### 8. Implement identity-safe `croncheck`

`eggsearch croncheck` is a watchdog, not a generic "ensure any process named eggsearch exists" command.

Algorithm:

1. derive the configured loopback health endpoint from the canonical persistent runtime config;
2. issue a bounded local HTTP GET (target timeout around 500-1000 ms; use implementation evidence to choose exact value);
3. if HTTP 200 and JSON identifies `service=eggsearch` with ready status -> exit 0, do nothing;
4. if TCP connection is explicitly refused/no listener -> acquire a startup lock, recheck health, then spawn the persistent server detached and exit appropriately;
5. if connect times out, response is malformed, service identity is wrong, status is non-ready for an existing eggsearch process, or another listener occupies the endpoint -> return a diagnostic error and do not spawn;
6. release lock safely.

The second health check after acquiring the lock prevents simultaneous cron invocations from both spawning.

Use a runtime lock/pid coordination mechanism appropriate to the platform (Unix file lock under a user runtime/state directory is acceptable). Do not trust a stale pidfile alone as proof of process identity.

### 9. Detached spawn behavior for cron

On Unix cron fallback, start the exact canonical `mcp serve` command detached from the cron process:

- stdin null;
- stdout/stderr to documented log files or null/journal-equivalent as appropriate;
- new session/process group where required so the child survives cron command exit;
- no shell interpolation when Rust `Command` can pass argv directly.

After spawn, poll `/healthz` for a short bound. If it never becomes ready, return nonzero and include the log/config path needed to diagnose.

### 10. Implement `startup instructions`

Every backend must be able to render the exact commands/paths without mutation:

```text
eggsearch startup instructions
eggsearch startup instructions --method systemd
```

Instructions are the fallback when automatic install cannot proceed. They must include:

- detected method and why;
- executable/config paths;
- rendered manager commands;
- health verification command;
- exact elevated rerun command when privilege is the blocker;
- uninstall command.

This is important for fleet operators who want to audit commands before applying them.

### 11. Implement `startup status`

Return concise human-readable state and optionally machine-readable JSON if consistent with CLI conventions.

Distinguish:

- method detected/registered;
- registered but stopped;
- running and healthy;
- manager says running but health identity/readiness fails;
- not installed;
- ambiguous/multiple-manager registrations.

If multiple eggsearch-managed startup definitions are detected, report conflict and do not silently choose one for restart/update.

### 12. Implement `restart`

`eggsearch restart` resolves the active registered manager and delegates:

- systemd -> manager restart + bounded `/healthz` verification;
- launchd -> current kickstart/restart mechanism + health verification;
- Windows SCM -> stop/start or restart semantics + health verification;
- cron -> stop the specifically owned persistent instance through an identity-safe control path, then invoke the same start path/health verification.

Do not `pkill eggsearch`, `taskkill` every eggsearch process, or otherwise terminate stdio instances belonging to IDEs/agents.

If no persistent startup registration exists, return a clear message that stdio processes are client-owned and there is no managed service to restart.

### 13. Add an owned stop/control seam for cron

Cron has no manager stop primitive. Add the minimum identity-safe mechanism needed for restart/uninstall, such as:

- a runtime pid/lock file whose process identity is verified against the expected executable/start token; or
- a local-only control endpoint/token stored with restrictive permissions.

Prefer the simplest cross-Unix design that cannot kill an unrelated process after PID reuse. Health identity alone is not enough to send a termination signal safely.

Do not expose a network stop endpoint without authentication merely for convenience.

### 14. Integrate updater restart semantics

Extend phase 7 updater:

1. query `startup_state()` before replacement;
2. record whether the managed persistent service is registered and actually running/healthy;
3. perform verified replacement;
4. if it was running, restart through the same manager and verify health/version;
5. if it was stopped, leave it stopped;
6. if replacement succeeds but restart fails, return a typed nonzero outcome that reports installed version and exact restart command rather than rolling back blindly.

Updating an executable used only by stdio requires no restart; future client spawns use the new binary.

### 15. Integrate bootstrap installers

Extend `packaging/install.sh` and `install.ps1` with explicit service mode.

Unix examples:

```text
# ordinary client-spawned stdio install
curl -fsSL .../install.sh | bash

# fleet/system service install
curl -fsSL .../install.sh | sudo bash -s -- --service
```

After verified binary installation, `--service` runs:

```text
<installed-eggsearch> startup install
```

with auto method. The script does not render its own systemd/cron logic.

If startup install fails due to privilege/config constraints, keep the successfully installed binary, print the CLI's exact instructions, and return a status that makes the partial outcome clear.

For non-root Unix installs on non-systemd cron hosts, user cron installation may succeed without elevation. For detected systemd hosts, do not silently use cron to avoid the permission error.

Windows PowerShell service mode follows the same delegation and requires Administrator for SCM.

## Tests

Add deterministic tests for:

- auto method selection under injected OS/systemd conditions;
- explicit method override;
- systemd running-vs-binary-present detection;
- all manager rendered paths/argv quoting;
- systemd unit content and security-relevant directives;
- launchd plist XML escaping and absolute paths;
- Windows service argument construction;
- cron marker add/update/remove while preserving unrelated entries;
- cron quoting for spaces/single quotes and rejection of newline injection;
- health classifications: healthy/refused/timeout/wrong-service/malformed/non-ready;
- croncheck only starts on refused/definitely absent;
- startup lock prevents double spawn;
- manager install idempotency;
- manager uninstall idempotency;
- status detects multiple-manager conflict;
- restart never targets arbitrary stdio processes;
- updater restarts only previously running service;
- updater leaves stopped service stopped;
- replacement-success/restart-failure outcome preserves installed version and instructions.

Privileged manager integration tests must not become required on generic CI. Use render/state unit tests plus targeted platform jobs/ignored tests. Native Windows CI should exercise SCM registration when the runner permits safe temporary service creation; otherwise keep a documented maintainer smoke script.

## Documentation changes

Add/update:

- `README.md`: ordinary installer vs fleet `--service` installer; `startup`/`restart`/`update` quick commands;
- `docs/deployment.md`: persistent mode and each startup method;
- `docs/installation.md`: privilege/install destinations and partial service-registration outcomes;
- `docs/update.md`: restart-if-running semantics;
- `docs/service.md` or equivalent: systemd, launchd, Windows SCM, cron, config/env paths, logs, manual instructions;
- `packaging/README.md`: canonical service assets and installer delegation;
- architecture docs for runtime ownership;
- `CHANGELOG.md`.

Include manual command examples equivalent to automatic behavior so an operator can install service definitions without trusting the installer.

## Acceptance criteria

Phase 9 is complete only when:

1. `startup instructions` renders correct non-mutating instructions for every supported backend;
2. auto detection selects systemd/launchd/Windows/cron according to the documented policy;
3. permission failure on a detected preferred manager prints an exact elevated command and does not install cron as a duplicate;
4. systemd install/update/uninstall is idempotent and health-verifiable on a representative Linux host;
5. launchd install/update/uninstall is idempotent and health-verifiable on macOS;
6. Windows SCM install/start/stop/restart/uninstall is exercised on a supported Windows host or has a concrete unresolved blocker keeping the phase open;
7. cron install preserves unrelated entries and `croncheck` starts only on definite absence;
8. concurrent croncheck invocations cannot spawn duplicate persistent servers;
9. `restart` restarts only the registered persistent instance and verifies `/healthz`;
10. stdio MCP processes remain untouched by restart/startup commands;
11. `eggsearch update` restarts a previously running managed service and leaves a stopped one stopped;
12. install script `--service` delegates to CLI startup logic and never duplicates manager rendering;
13. unprivileged installers never invoke elevation automatically;
14. docs include automatic and manual systemd/cron/launchd/Windows procedures;
15. `make check` passes on the exact final candidate;
16. `registry.md` and this phase status are updated in the closure commit.
