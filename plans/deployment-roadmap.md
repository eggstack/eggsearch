# Binary Distribution and Deployment Roadmap

Status: implemented — phases 6–10 implemented; registry metadata deferred
Updated: 2026-09-04
Audited repository baseline: `f595683b8ebdec0afb13363ec9e8ad7654f9824b` (`eggsearch` 0.3.8)
Primary downstream consumer reviewed: `dbowm91/codegg` main at `b3faf5050a1813bdef9e58d760f758318cb3dfcd`
Reference implementation reviewed: `eggstack/gregg` main

## Objective

Make eggsearch cheap to deploy across developer machines and small fleets without introducing an OS package-manager program yet.

The target user experience is:

1. publish verified prebuilt executables for the common desktop and SBC targets as GitHub Release assets;
2. provide a copy/paste Unix installer and a PowerShell installer that prefer a matching release binary and compile from crates.io only when no release binary exists for the host;
3. provide binary-first self-update with crates.io as the stable-version authority and the exact matching GitHub release as the preferred payload source;
4. preserve the existing stdio MCP mode for clients that should own the server lifecycle;
5. add a persistent loopback Streamable HTTP MCP mode before introducing daemon/service supervision;
6. add system-specific startup, health-watchdog, restart, and manual-instruction commands suitable for systemd, launchd, Windows, and cron-style environments;
7. make registration with CodeGG and common MCP-capable agents/IDEs a deterministic CLI-assisted operation.

The work should optimize for reproducible fleet bootstrap, old/SBC Linux compatibility, conservative privilege behavior, and a small maintenance surface. It must not turn into apt/Homebrew/MSI/Winget package maintenance in this workstream.

## Current implementation evidence

At the audited baseline:

- `Cargo.toml` publishes one `eggsearch` binary at version 0.3.8, requires Rust 1.88, and uses default features that do not include optional PDF/browser support.
- `src/main.rs` exposes search/fetch/provider/update commands and `eggsearch mcp stdio`; there is no persistent MCP transport, service manager, or restart command yet.
- `.github/workflows/ci.yml` remains the routine-verification job, and `.github/workflows/release-binaries.yml` now owns the tagged binary matrix and draft-release assembly.
- `docs/release.md` documents the manual crates.io-first sequence followed by the release-binary workflow; GitHub release publication remains a maintainer-controlled draft review step.
- README installation is binary-first, with verified release assets and a narrow exact-version Cargo fallback for Raspberry Pi/Le Potato class systems.
- `packaging/release-targets.txt`, the Unix/PowerShell installers, and `make packaging-check` enforce the shared target, checksum, and fallback contract.
- the existing code already depends on async HTTP/JSON primitives (`tokio`, `reqwest`, `serde_json`), so a self-updater does not need to shell out to curl for its network operations.
- CodeGG already treats eggsearch as its external search backend. `src/search_backend/bootstrap.rs` defaults to a local stdio launch when `[search.eggsearch]` is selected and can also consume an explicitly configured MCP server through CodeGG's existing local/remote MCP transport layer. This means eggsearch must preserve stdio even after persistent HTTP is added.

Gregg provides a useful reference for release asset naming, binary-first install semantics, glibc portability, checksum verification, startup method selection, `croncheck`, and self-update. Reuse those policies where they fit, but do not copy daemon-specific assumptions that do not fit MCP stdio lifecycle.

## Architectural decisions

### 1. Keep stdio and persistent service lifecycle separate

`eggsearch mcp stdio` remains the primary zero-daemon integration. MCP clients such as CodeGG, Zed, Codex, Claude Code, Cursor, VS Code, and OpenCode may spawn the process on demand.

Systemd/launchd/Windows/cron supervision applies only to a new persistent HTTP transport. A service manager must never supervise the stdio command because there would be no client-owned stdin/stdout session to serve.

The persistent command should be explicit, with the intended shape:

```text
eggsearch mcp serve --bind 127.0.0.1:11320 --path /mcp
```

The exact rmcp API/version must be re-audited before implementation. Prefer the current rmcp-supported Streamable HTTP server transport and current MCP protocol semantics; do not introduce a legacy SSE-only transport.

### 2. Publish architecture/libc assets, not board-branded assets

Do not publish separate `raspberrypi` or `le-potato` binaries. Publish Rust target artifacts with one stable naming contract.

Initial required target matrix:

| Host family | Rust target | Release requirement |
|---|---|---|
| Linux x86-64 | `x86_64-unknown-linux-gnu` | required |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | required; primary 64-bit SBC artifact |
| Linux ARMv7 hard-float | `armv7-unknown-linux-gnueabihf` | required if portability/smoke gate is proven; otherwise phase must remain open with explicit blocker |
| macOS Intel | `x86_64-apple-darwin` | required |
| macOS Apple Silicon | `aarch64-apple-darwin` | required |
| Windows x86-64 | `x86_64-pc-windows-msvc` | required |
| Windows ARM64 | `aarch64-pc-windows-msvc` | required when the current dependency graph compiles and native runner smoke passes; otherwise record a concrete blocker rather than silently source-building forever |

Linux GNU assets must have an intentional minimum glibc floor rather than inheriting the GitHub runner's current libc. Reuse Gregg's `cargo-zigbuild`/Zig approach where possible. The ARMv7 path is a specific feasibility gate: use a pinned release-only cross toolchain (`cargo-zigbuild`, `cross`, or equivalent), and do not publish the asset until the executable can be exercised under an appropriate runner/QEMU or equivalent architecture smoke.

Default release binaries use eggsearch default features. Optional `pdf`/`browser` feature combinations are not separate binary SKUs in this workstream.

### 3. Make the GitHub release tag the version namespace

Stable asset names:

```text
eggsearch-x86_64-unknown-linux-gnu
eggsearch-aarch64-unknown-linux-gnu
eggsearch-armv7-unknown-linux-gnueabihf
eggsearch-x86_64-apple-darwin
eggsearch-aarch64-apple-darwin
eggsearch-x86_64-pc-windows-msvc.exe
eggsearch-aarch64-pc-windows-msvc.exe
```

Every executable has `<asset>.sha256`. The release should also contain `install.sh` and `install.ps1` using the exact same mapping table.

Do not put `vX.Y.Z` into each asset filename. The tag path already namespaces the release and gives both exact and `/latest/download/` URLs a stable contract.

### 4. Binary first; Cargo fallback is narrow

Installer and updater fallback policy is normative:

- matching release asset exists -> download, checksum, identify/version-check, then install/replace;
- asset is intentionally unsupported or the exact release asset returns HTTP 404 -> compile the exact crates.io version if Cargo is available;
- network timeout, TLS failure, GitHub 5xx, checksum failure, malformed checksum, or wrong candidate identity/version -> hard failure; never reinterpret these as permission to start a long source build.

This distinction is especially important on SBCs, where accidental compilation defeats the purpose of this work.

### 5. crates.io is the update version authority

`eggsearch update` obtains the latest stable crates.io version (prefer `crate.max_stable_version` from the crates.io API), compares it semantically with `env!("CARGO_PKG_VERSION")`, and then requests the exact `v<version>` GitHub asset.

GitHub `releases/latest` is convenient for the bootstrap installer but is not authoritative for self-update version selection. Equal versions are a no-op; local development versions newer than crates.io are never automatically downgraded.

### 6. Do not hide privilege escalation

Neither installer nor CLI invokes `sudo` automatically.

A privileged operation either succeeds under the current effective identity or returns/prints the exact command the operator can rerun with elevation. If systemd or another preferred manager is detected but installation lacks privilege, do not silently install a cron duplicate instead.

The normal copy/paste installer remains binary-only. Fleet/service installation is explicit:

```text
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | sudo bash -s -- --service
```

This distinction is intentional: stdio users should not receive an unused background service. `--service` delegates startup policy to `eggsearch startup install` rather than duplicating service-manager logic in shell.

### 7. Persistent service is loopback-only in this workstream

The persistent HTTP transport binds loopback by default and service templates use loopback. Non-loopback/LAN serving requires a separate authentication/exposure design and is not enabled opportunistically by this workstream.

The HTTP implementation must validate local Host/Origin expectations as appropriate for the selected MCP SDK/transport and expose a small bounded `/healthz` endpoint used by supervisors. Health must identify eggsearch and process readiness without calling external search providers.

### 8. Supervision uses one owner at a time

Intended auto-selection:

```text
Windows                  -> Windows SCM when implemented
macOS                    -> launchd
Linux + running systemd  -> systemd
other Unix/Linux         -> cron
```

The CLI owns manager detection/rendering/install/uninstall/status/restart. The shell installer merely delegates to it.

`croncheck` is a non-systemd watchdog. It probes the configured local health endpoint and starts the persistent command only when eggsearch is definitely absent. Timeout, malformed response, or a different listener on the port is an error, not permission to spawn a duplicate.

### 9. Integration favors native client commands over config surgery

Provide an `eggsearch integrate` surface that can print or apply configuration for CodeGG, Zed, Codex, Claude Code, Cursor, VS Code, and OpenCode.

When a client has a stable native MCP registration command, call that command rather than editing its config file. Otherwise render the exact config and only support `--apply` when an atomic, structure-preserving edit can be tested. Never overwrite an entire client config.

CodeGG is first-class: preserve its default stdio bootstrap, and support an explicit remote/persistent MCP declaration through its existing MCP transport configuration without adding provider-specific search behavior to CodeGG.

### 10. Keep future packaging metadata cheap

Prepare official MCP Registry metadata (`server.json` or the then-current schema) during integration/closure if the schema can accurately describe eggsearch's package/transport forms.

Do not make MCPB, apt, Homebrew, Winget, MSI, Debian/RPM packaging, Docker images, or auto-update daemons prerequisites for this workstream. They can be added later without changing the GitHub release asset contract.

## Cross-phase invariants

1. `eggsearch mcp stdio` remains backward-compatible for CodeGG and existing clients.
2. The keyless default configuration remains usable.
3. Release artifacts and Cargo fallback build the same crate version and default feature surface.
4. Checksums are verified before a downloaded candidate is executed or installed.
5. Candidate identity/version is verified before replacing an installed executable.
6. No transient network failure silently becomes a source compile.
7. No installer or CLI silently invokes `sudo`.
8. One startup manager owns a persistent instance; permission failure never creates a second manager.
9. Persistent HTTP is loopback-only unless a later explicit security plan expands exposure.
10. Normal repository tests remain deterministic/network-free. Release workflow and ignored/smoke paths may exercise produced artifacts.
11. Every phase updates user/operator documentation for the behavior it makes real; docs must not advertise later planned phases as already available.
12. `make check` remains the broad repository closure gate unless a phase adds a packaging-specific gate in addition to it.

## Ordered phases

### Phase 6 — Release binaries and bootstrap installers

The release-only GitHub Actions matrix, stable asset/checksum contract, Unix and PowerShell binary-first installers, Cargo fallback, target/installer verification, and README copy/paste installation path are implemented. This phase provides immediate value to SBC deployments without daemon support.

### Phase 7 — Binary-first self-update

Shared target/version/download/replacement logic and `eggsearch update`/`update --check` are implemented. crates.io is authoritative; exact GitHub assets are preferred; exact-version Cargo compilation is fallback only for absent/unsupported assets. The updater now restarts only a previously healthy registered persistent service through phase 9 startup state.

### Phase 8 — Persistent Streamable HTTP MCP

Qualify the current rmcp dependency/API against the current MCP specification, add loopback Streamable HTTP service mode and `/healthz`, preserve stdio, and add protocol-level smoke/conformance coverage.

### Phase 9 — Startup supervision, croncheck, and restart

Add startup manager detection/install/instructions/status/uninstall, embedded systemd/launchd/Windows definitions, cron management, identity-safe `croncheck`, restart semantics, update/restart integration, and installer `--service` delegation.

### Phase 10 — Agent/IDE integration and deployment closure

Client registration render/apply support for CodeGG and the major MCP clients,
deployment/integration/release documentation, local CI/packaging verification,
and the final client/transport contract are implemented. Registry metadata is
deferred pending a schema that can represent all release assets accurately.

## Deferred by design

- apt, RPM, Homebrew, Winget, Chocolatey, Scoop, MSI/PKG installers;
- containers as the primary deployment path;
- unattended/background auto-update scheduling;
- LAN/public MCP serving or authentication design;
- multiple feature-specific binary variants;
- board-specific CPU tuning or Raspberry Pi/Le Potato branded artifacts;
- browser/PDF feature bundles as separate release assets;
- MCPB until its architecture-selection behavior is sufficient for this multi-architecture binary matrix.

## Workstream closure rule

The workstream is complete when phases 6-10 meet their acceptance criteria against an exact candidate, the required release targets are published or any explicitly conditional target has a documented technical blocker, Unix/Windows installers and updater share the same target/asset contract, stdio and persistent HTTP both pass MCP initialization/tool-list smoke, service installation is idempotent and privilege-safe on supported managers, CodeGG still works through its default stdio bootstrap, representative external clients can be configured from generated/native commands, documentation matches reality, and `make check` plus the packaging/release gates pass.
