# Phase 10 — Agent/IDE Integration and Deployment Closure

Status: planned
Depends on: phases 6 and 8; phase 9 required for persistent-service integration/closure
Baseline for planning: `f595683b8ebdec0afb13363ec9e8ad7654f9824b` (`eggsearch` 0.3.8)
Roadmap: `plans/deployment-roadmap.md`
Primary downstream consumer reviewed: `dbowm91/codegg` main at `b3faf5050a1813bdef9e58d760f758318cb3dfcd`

## Objective

Make adding eggsearch to common MCP-capable agents and IDEs a predictable, low-friction operation after binary installation, with CodeGG as the first-class downstream consumer. Then perform a deployment-wide closure pass covering release assets, installers, updater, stdio/HTTP transports, startup managers, and client registration.

The integration surface should prefer native client MCP commands when available, render exact configuration when not, and mutate third-party configuration only through explicit `--apply` paths that preserve unrelated settings.

Intended CLI shape:

```text
eggsearch integrate list
eggsearch integrate codegg [--transport stdio|http] [--apply]
eggsearch integrate zed [--transport stdio|http] [--apply]
eggsearch integrate codex [--transport stdio|http] [--apply]
eggsearch integrate claude [--transport stdio|http] [--apply]
eggsearch integrate cursor [--transport stdio|http] [--apply]
eggsearch integrate vscode [--transport stdio|http] [--apply]
eggsearch integrate opencode [--transport stdio|http] [--apply]
```

Exact client names/options may follow current conventions, but print-only behavior should be available for every supported client and mutations must be opt-in.

## Current downstream evidence

CodeGG already has the strongest integration of the target clients:

- `src/search_backend/bootstrap.rs` treats eggsearch as the external search backend;
- when `[search.eggsearch]` is selected and no explicit `[mcp.eggsearch]` exists, CodeGG constructs a local stdio connection from configured/default command/args;
- an explicitly configured `mcp.eggsearch` entry is passed through CodeGG's existing `McpService::connect_from_config`, which supports local and remote transport forms;
- CodeGG requires `web_search` and `web_fetch` and treats the additional eggsearch tools as recommended coverage;
- eggsearch integration failure is intentionally non-fatal to CodeGG startup and is surfaced diagnostically.

Therefore phase 10 must not invent a second CodeGG-specific protocol. It should generate the minimal current CodeGG configuration needed to select eggsearch and, for persistent mode, use CodeGG's normal remote MCP configuration.

## External client baseline

Before implementation, refresh official/current client documentation and CLI help for each adapter. Planning baseline research indicates:

- Codex has native `codex mcp add` with stdio command or `--url` Streamable HTTP forms;
- Claude Code has native `claude mcp add` supporting stdio and HTTP plus scopes;
- VS Code exposes an MCP registration CLI/config surface;
- Zed supports local command/args MCP servers and URL-based remote servers;
- Cursor supports MCP config/install-link flows and Streamable HTTP;
- OpenCode supports local command arrays and remote URL MCP configuration;
- CodeGG already supports local stdio and explicit remote MCP servers.

Treat these as implementation hypotheses to verify against current official docs/CLI output before writing each adapter. Client config surfaces evolve quickly; do not freeze stale paths/flags into eggsearch without fixture/documentation evidence.

## Non-goals

- No generic "edit any MCP client JSON" framework.
- No secrets/OAuth credential manager for third-party clients.
- No automatic enabling of remote/LAN eggsearch; HTTP integration points to the loopback persistent endpoint from phase 8.
- No changes to CodeGG's search semantics/provider wrappers in this phase.
- No client plugin/extension marketplace submissions.
- No apt/Homebrew/etc. package distribution.
- No MCPB dependency unless architecture selection has become sufficient and a separate small follow-up is justified.

## Invariants

1. stdio integration uses the installed absolute/path-resolvable `eggsearch mcp stdio` command and requires no background service.
2. HTTP integration uses the configured phase-8 loopback endpoint and should verify `/healthz` before claiming success.
3. Client configuration mutation is explicit; print/render is always available.
4. `--apply` preserves unrelated configuration and creates a backup where direct file edits are necessary.
5. Native client registration commands are preferred over direct config edits when stable and scriptable.
6. Re-running an integration is idempotent: update/replace only the eggsearch entry, never duplicate it.
7. Existing non-eggsearch MCP servers remain untouched.
8. Integration commands never embed provider API keys into client config unless the user explicitly supplied a documented client-only environment mapping; default eggsearch configuration remains keyless.
9. CodeGG default stdio bootstrap remains supported and is the recommended zero-service path.
10. Integration failures do not corrupt client config; partial mutation must be recoverable from an automatically created backup/atomic write.

## Production changes

### 1. Add `src/integrations/` and CLI surface

Create a small adapter layer such as:

```text
src/integrations/
  mod.rs
  codegg.rs
  zed.rs
  codex.rs
  claude.rs
  cursor.rs
  vscode.rs
  opencode.rs
```

Each adapter should expose a common internal operation shape:

```text
probe() / availability
describe()
render(transport, scope/options)
apply(transport, scope/options)
verify()
```

Do not force every client through identical config internals. The abstraction is for CLI orchestration/outcomes, not to pretend TOML/JSON/native CLI clients are structurally identical.

Suggested CLI behavior:

```text
eggsearch integrate list
  client    available    stdio    http    apply-mode

eggsearch integrate <client>
  # print exact command/config only

eggsearch integrate <client> --apply
  # perform supported native command or safe config edit, then verify
```

Default transport should be `stdio` unless the user explicitly requests `http`, because it requires no daemon and matches CodeGG's current default behavior.

### 2. Resolve the installed executable safely

For rendered stdio config, prefer a stable absolute executable path when `current_exe()` points to the installed eggsearch binary and the client configuration benefits from avoiding PATH ambiguity.

If the current executable path is an ephemeral build/test path, do not write it into production client config. Detect common cargo-dev/test execution and require an installed binary or explicit `--executable` override if needed.

Where a client conventionally resolves command names through PATH reliably, `eggsearch` may be rendered instead of an absolute path. Make the policy per client and test it.

### 3. CodeGG adapter

CodeGG is first-class and should receive the most specific integration test.

Before implementation, re-audit current CodeGG config schema/defaults and `src/search_backend/bootstrap.rs`.

For stdio, render/apply the minimal supported configuration selecting the eggsearch backend and preserving CodeGG's existing default launch behavior. Do not add an explicit `[mcp.eggsearch]` entry if `[search.eggsearch]` already provides the intended default cleanly.

For persistent HTTP, render an explicit `mcp.eggsearch` remote entry using CodeGG's normal remote transport shape plus the search backend selection needed for wrappers to bind to that server name.

After apply, prefer invoking CodeGG's existing doctor/diagnostic command if one exposes the eggsearch bootstrap report. Verification should confirm:

- connected server name is eggsearch;
- required tools `web_search` and `web_fetch` are present;
- recommended tool coverage is reported;
- provider_status best-effort diagnostic does not make integration fail solely because optional provider credentials are absent.

Do not modify CodeGG source code as part of this phase. If current CodeGG lacks a necessary stable config/CLI seam, create a separate CodeGG implementation plan rather than coupling a cross-repo code change into eggsearch's closure commit.

### 4. Codex adapter

Prefer native CLI registration when current `codex mcp add` semantics remain stable.

Expected forms to verify at implementation time:

```text
codex mcp add eggsearch -- <eggsearch> mcp stdio
codex mcp add eggsearch --url http://127.0.0.1:11320/mcp
```

Before apply:

- detect `codex` availability/version;
- inspect whether an `eggsearch` entry already exists using native list/get where available;
- replace/update idempotently using native remove/add or a supported update path;
- never remove other servers.

Verification uses native `mcp get/list` plus, where possible, an actual MCP connection.

### 5. Claude Code adapter

Prefer native `claude mcp` commands and support the current documented scope model.

Print mode must show the exact command and default scope. `--apply` should require explicit scope only if current Claude behavior would otherwise surprise the user; otherwise use the least invasive user/local scope consistent with official defaults.

Support both stdio and loopback HTTP when current Claude Code supports them. Do not implement OAuth because local eggsearch HTTP is unauthenticated loopback-only in this workstream.

### 6. VS Code adapter

Prefer the current native CLI MCP registration surface (`code --add-mcp` or successor) when available.

If the installed `code` CLI does not support the expected option/version, print the current JSON configuration instead of guessing or directly editing an undocumented path.

Support workspace/user scope only when official semantics can be represented safely. Do not write `.vscode` project files unexpectedly from a global installation command without an explicit scope.

### 7. Zed adapter

Zed currently supports local command/args and URL-based context/MCP server configuration. Refresh the official settings schema before implementation.

Print mode should render the minimal `eggsearch` entry for:

```text
stdio -> command eggsearch + args ["mcp", "stdio"]
http  -> URL http://127.0.0.1:11320/mcp
```

If Zed has no stable native CLI registration command, `--apply` may edit its settings only when:

- the exact current user settings location is confidently detected per OS;
- the file is parsed structurally as JSON/JSONC with comments/trailing syntax preserved if Zed uses JSONC;
- a timestamped backup is written first;
- only the eggsearch entry is changed;
- an atomic write is used.

If comment-preserving structural edits cannot be implemented safely with a small dependency, keep Zed apply as print-only rather than destructively rewriting user settings.

### 8. Cursor adapter

Refresh current MCP config path/schema and install-link support.

Prefer a supported native/deeplink install mechanism when it can accurately represent the chosen stdio/HTTP form. Otherwise render current config and limit `--apply` to structure-preserving safe edits.

A README "Add Eggsearch to Cursor" deeplink/button may be added only if the generated payload is stable, reviewable, and does not embed machine-specific absolute paths that make the link non-portable.

### 9. OpenCode adapter

Refresh current OpenCode MCP schema. Render local stdio command-array and remote URL forms using current official syntax.

If OpenCode exposes a stable CLI/config mutation command, prefer it. Otherwise use the same backup/atomic/structural-edit rules as other config-file clients.

Do not overwrite user provider/model/tool settings outside the named eggsearch MCP entry.

### 10. Integration verification helper

Create a shared post-registration verification path where practical:

- stdio: launch the configured command directly, perform MCP initialize + `tools/list`, then exit;
- HTTP: GET `/healthz`, then initialize + `tools/list` against the configured URL;
- compare expected server identity and minimum tool set;
- report optional/recommended tool differences without treating missing credentials as server failure.

Client-native verification should supplement, not replace, this protocol-level check when possible.

### 11. Idempotency and conflict handling

Before apply, detect an existing eggsearch entry.

Cases:

- identical desired entry -> report already configured, no write;
- eggsearch entry differs -> show old/new summary and update only that entry;
- duplicate/conflicting eggsearch entries -> fail with actionable cleanup instructions rather than choosing unpredictably;
- config malformed -> fail before mutation and leave backup/original untouched.

Never deduplicate/delete unrelated MCP entries by heuristic name similarity.

### 12. Optional `--json` output

If CLI conventions support it, integration commands should expose machine-readable outcomes useful for fleet scripts:

```json
{
  "client": "zed",
  "transport": "stdio",
  "available": true,
  "applied": false,
  "verified": true,
  "config_path": "...",
  "command": "..."
}
```

Do not include secrets or full unrelated config in JSON output.

### 13. MCP Registry metadata

At closure, evaluate the current official MCP Registry schema and add a validated `server.json` (or successor filename/schema) if it can truthfully describe eggsearch's package/server forms.

Use the official schema/validator and keep version/repository/package metadata release-synchronized.

Do not block the core deployment work on MCPB or registry publication if the current schema cannot express the multi-architecture release assets cleanly. Metadata readiness is sufficient for this phase unless explicit publication is low-risk and desired.

## Deployment closure audit

### A. Release/installer contract audit

Verify phase 6 workflow, Unix installer, PowerShell installer, Rust target module, docs, and phase 7 updater all use the same public target/asset names.

Test every published asset URL for the exact closure release. No README command may point to a non-attached filename.

### B. SBC portability audit

For Linux AArch64 and ARMv7 published assets:

- verify target architecture;
- verify documented libc/loader compatibility;
- run on a representative Raspberry Pi/Le Potato class host or equivalent real-hardware smoke when available;
- record install time qualitatively versus local Cargo build only as operator evidence, not a performance SLA;
- verify keyless `mcp stdio` and persistent `/healthz` startup.

Do not mark ARMv7 closure based solely on successful cross-linking.

### C. Update audit

Exercise:

- already-current;
- new binary available;
- exact asset 404 -> Cargo fallback;
- transient GitHub failure -> no fallback;
- checksum mismatch -> no mutation;
- permission failure -> exact rerun command;
- managed service running -> update/restart/health/new version;
- managed service stopped -> update without start;
- stdio-only install -> update with no service restart.

### D. Startup-manager audit

Verify one representative environment per supported manager:

- Linux/systemd;
- Linux/non-systemd or isolated cron fixture;
- macOS/launchd;
- Windows SCM.

Check install, idempotent reinstall, status, restart, uninstall, and permission failure. Confirm no manager creates a second manager registration.

### E. MCP transport compatibility audit

Against the exact closure binary:

- stdio initialize/tools-list;
- HTTP initialize/tools-list;
- tool inventory/schema equality;
- CodeGG default stdio bootstrap;
- CodeGG explicit remote HTTP connection;
- at least one additional native-CLI client over stdio;
- at least one additional HTTP-capable client against persistent mode.

### F. Documentation audit

README should have a short path for each common goal:

```text
Install eggsearch
Install as a persistent service on a small server/SBC
Add to CodeGG
Add to Zed
Add to Codex/Claude/etc.
Update
Restart/status
Manual systemd/cron instructions
```

Deep details belong in dedicated docs, not a giant README.

Required docs by closure:

```text
docs/installation.md
docs/deployment.md
docs/update.md
docs/integrations.md
docs/release.md
packaging/README.md
```

Add a service-specific doc only if `docs/deployment.md` becomes unwieldy.

### G. Security/privilege audit

Verify:

- no installer/CLI auto-sudo/UAC;
- checksum before candidate execution;
- non-loopback persistent bind rejected by default;
- service env/config files do not expose credentials through world-readable permissions;
- client integrations do not leak provider keys;
- cron/status/restart cannot kill unrelated processes;
- config edits are atomic/backup-preserving;
- external client commands are passed as argv rather than shell-evaluated strings wherever possible.

## Tests

Add adapter unit tests using fixture versions/configs for every client. Do not require every third-party client to be installed in routine CI.

Test:

- render output for stdio/http;
- availability detection;
- native command argv construction;
- exact config merge preserving unrelated servers/settings;
- repeated apply idempotency;
- malformed config no-mutation behavior;
- duplicate eggsearch conflict behavior;
- backup/atomic-write behavior;
- CodeGG-specific minimal config forms;
- HTTP verification rejects unhealthy/wrong-service endpoint;
- stdio verification requires minimum tool set.

Keep official-client CLI integration as ignored/optional smoke where installation cost/licensing makes routine CI inappropriate.

## Documentation changes

Update:

- `README.md` with concise install/service/integration commands;
- `docs/integrations.md` with CodeGG first, then Zed/Codex/Claude/VS Code/Cursor/OpenCode;
- `docs/deployment.md` with client-spawned vs persistent topology;
- `docs/installation.md` with fleet one-liners;
- `docs/update.md` and `docs/release.md` closure truth;
- `AGENTS.md` if the new release/deployment contract becomes a repository invariant maintainers must preserve;
- architecture indexes for deployment/integration modules;
- `CHANGELOG.md`.

## Acceptance criteria

Phase 10/workstream closure is complete only when:

1. `eggsearch integrate list` accurately reports supported clients/capabilities;
2. every supported client has a correct print/render path for the transports it actually supports;
3. `--apply` exists only where native CLI or structure-preserving mutation is proven safe;
4. repeated apply is idempotent and preserves unrelated MCP/client config;
5. CodeGG stdio integration works using its existing search-backend bootstrap and required tools are discovered;
6. CodeGG can use the persistent endpoint through its explicit remote MCP path;
7. representative Codex/Claude/native-CLI integration succeeds against the exact candidate;
8. representative Zed or another config-file client render/apply flow is verified without destructive rewrite;
9. release workflow/installers/updater share one tested target/asset contract;
10. AArch64 SBC release installation is exercised on representative hardware/emulation and ARMv7 satisfies phase 6's runtime gate if published;
11. update + restart state matrix passes;
12. systemd, launchd, Windows, and cron paths satisfy phase 9 closure or any unresolved manager keeps the workstream non-complete;
13. stdio and HTTP MCP transport inventories remain equivalent;
14. README/docs accurately distinguish binary-only install from fleet `--service` install;
15. official MCP Registry metadata is validated if added, or a brief closure note records why it was deferred;
16. no package-manager pipeline or remote-auth scope was added opportunistically;
17. `make check` and the release/packaging smoke gates pass on the exact final candidate;
18. `registry.md` records phases 6-10 as implemented or explicitly superseded/blocked with evidence and this phase records final closure state.
