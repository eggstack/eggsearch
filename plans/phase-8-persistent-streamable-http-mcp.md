# Phase 8 — Persistent Streamable HTTP MCP

Status: planned
Depends on: none for protocol work; should land after or alongside phase 6 so release artifacts exercise the final CLI surface
Baseline for planning: `f595683b8ebdec0afb13363ec9e8ad7654f9824b` (`eggsearch` 0.3.8)
Roadmap: `plans/deployment-roadmap.md`

## Objective

Add a persistent, supervisor-friendly MCP server transport without weakening or replacing the existing stdio path.

The new transport is the prerequisite for systemd/launchd/Windows/cron startup work. It should implement current MCP Streamable HTTP semantics on loopback, expose a small identity-safe health endpoint, shut down cleanly, and reuse the same eggsearch MCP service/tool implementation as stdio so transport choice does not create divergent behavior.

Intended CLI shape:

```text
eggsearch mcp stdio
eggsearch mcp serve --bind 127.0.0.1:11320 --path /mcp
```

The exact option names may adjust to repository conventions, but `mcp stdio` must remain valid and persistent service mode must remain explicit.

## Current implementation evidence

At the planning baseline:

- `src/main.rs` exposes `McpCmd::Stdio` only;
- `commands::mcp::run_stdio` owns the current MCP server startup;
- `Cargo.toml` uses `rmcp = "1"` with `server`, `transport-io`, and `macros` features only;
- the server/tool implementation already exists and should not be duplicated for HTTP;
- CodeGG defaults to spawning eggsearch over stdio, but its MCP service also has a remote transport path for explicitly configured servers;
- current third-party clients commonly support stdio and/or Streamable HTTP, making those the two useful transports to preserve.

## Research/qualification prerequisite

Before editing the transport code, re-check the current versions of:

- MCP specification, especially Streamable HTTP session/request requirements;
- `rmcp` server transport features/API;
- any `axum`/HTTP integration required by the selected `rmcp` version.

Do not assume the baseline `rmcp = "1"` API is the current recommended HTTP-server API. Upgrade `rmcp` only as far as required for a supported/current Streamable HTTP implementation, and treat that upgrade as part of this phase's compatibility work.

If an rmcp upgrade changes stdio tool schemas/serialization/protocol behavior, resolve those regressions inside this phase before service work proceeds.

## Non-goals

- No legacy SSE-only server as the primary persistent transport.
- No public/LAN bind by default.
- No OAuth/authentication design for remote internet/LAN use.
- No reverse proxy, TLS certificate management, discovery service, or multi-tenant server.
- No systemd/launchd/SCM/cron installation until phase 9.
- No separate tool implementation for HTTP.

## Invariants

1. `eggsearch mcp stdio` remains backward-compatible and uses the same tool service as before.
2. HTTP and stdio expose the same MCP server identity, tool inventory, schemas, and tool behavior.
3. Persistent service binds loopback by default.
4. Service health never probes external providers and therefore remains useful when the internet/providers are unavailable.
5. HTTP request/session handling is bounded and follows the selected current MCP transport contract.
6. Remote request text retains existing untrusted/sanitization semantics; transport does not bypass tool-layer validation.
7. Normal tests remain network-free except loopback-only deterministic integration tests.
8. A malformed/non-MCP HTTP client cannot panic the process or create unbounded response/request state.

## Production changes

### 1. Refactor MCP service construction away from transport startup

If `commands::mcp::run_stdio` currently combines service creation and stdio transport binding, split it so both transports consume one canonical service/tool factory.

Suggested conceptual shape:

```text
build_mcp_service(config) -> server/service handler
run_stdio(config)         -> attach stdio transport to canonical service
run_http(config, opts)    -> attach Streamable HTTP transport to canonical service
```

Do not duplicate tool registration, server metadata, provider construction, or configuration loading.

### 2. Qualify/update rmcp

Update `Cargo.toml`/`Cargo.lock` to the current compatible rmcp release/features needed for Streamable HTTP server support.

Requirements:

- retain stdio transport feature;
- enable only the HTTP/server features actually needed;
- avoid `all-features` in production merely for convenience;
- inspect transitive server framework/runtime additions for footprint and MSRV compatibility with Rust 1.88, or intentionally revise MSRV only with repository-wide evidence and documentation;
- run existing deterministic MCP/schema tests before changing server behavior.

If current rmcp requires a Rust version above the project's MSRV, decide explicitly between a compatible rmcp release and an intentional MSRV bump; do not let CI discover this accidentally after implementation.

### 3. Add `McpCmd::Serve`

Suggested options:

```text
eggsearch mcp serve
  --bind 127.0.0.1:11320
  --path /mcp
```

Use typed socket/path validation. Default bind is loopback and a stable documented port; `11320` is the planned default because it follows nearby eggstack service conventions, but confirm no repository/service collision before landing.

Do not accept an arbitrary URL string and reparse it differently across service managers.

### 4. Loopback-only exposure policy

For this workstream, reject non-loopback bind addresses with a clear message rather than silently exposing the MCP server:

```text
non-loopback MCP serving is not enabled by this release; use stdio/local loopback or implement the remote-auth deployment plan
```

If implementation evidence requires an escape hatch for development, it must be an unmistakable `--insecure-*` style opt-in, excluded from service templates, and documented as unsafe. Prefer no escape hatch in this phase.

IPv4 and IPv6 loopback should both be considered; the chosen default can remain `127.0.0.1` for simple manager/cron health probes.

### 5. Host/Origin and HTTP request hardening

Implement the local host/origin validation recommended by the selected MCP transport/SDK to reduce localhost DNS-rebinding/cross-origin risk.

At minimum:

- reject unexpected Host targets when bound to the documented local endpoint if the framework does not do this safely itself;
- apply current MCP Origin validation guidance where relevant;
- cap request body size;
- cap relevant header/session identifier sizes/counts;
- use bounded server timeouts consistent with long MCP tool calls without allowing idle connection state to grow forever;
- do not reflect arbitrary malformed request data into large error bodies.

Document any validation delegated to rmcp/framework rather than reimplementing it twice.

### 6. Implement current Streamable HTTP protocol semantics

Use rmcp's supported Streamable HTTP service rather than hand-rolling JSON-RPC framing.

Tests must exercise the current protocol flow, including where applicable:

- initialize request/response;
- protocol-version negotiation;
- session identifier behavior;
- required request headers/content types;
- initialized notification if required by the current spec;
- `tools/list`;
- one deterministic local tool call that does not need internet/provider credentials;
- session/error behavior for malformed/missing identifiers according to current spec;
- clean client disconnect/reconnect.

Do not preserve obsolete transport behavior merely because an older client once accepted it.

### 7. Add `/healthz`

Expose a separate bounded HTTP health endpoint on the same listener, outside MCP session state.

Suggested response:

```json
{
  "service": "eggsearch",
  "status": "ready",
  "version": "0.4.0"
}
```

Exact schema may add a small protocol field, but must remain stable enough for `croncheck` and manager verification.

Health rules:

- HTTP 200 only when the process is ready to accept MCP initialization;
- response has a small hard byte cap;
- no provider/API calls;
- no search/fetch execution;
- identify eggsearch explicitly so `croncheck` can distinguish another listener on the port from the intended server;
- avoid secrets/config paths/provider credentials in output.

### 8. Add graceful shutdown

Handle supported termination signals/control paths so systemd/launchd/Windows can stop the persistent server without abrupt state loss.

Unix: SIGTERM and Ctrl-C should trigger graceful listener shutdown with a bounded drain period.

Windows: structure server shutdown so phase 9's SCM service control handler can request the same shutdown path rather than terminating the process externally.

Stdio lifecycle remains client/EOF driven as today.

### 9. Logging contract

Persistent mode logs to stderr in foreground operation and must be safe for manager capture/journald.

Do not write normal logs to stdout in stdio mode because stdout is protocol transport. Preserve the current stderr tracing discipline.

Avoid logging full provider credentials, authorization headers, raw MCP tool arguments containing secrets, or untrusted fetched content by default.

### 10. Configuration interaction

The global `--config` behavior must work identically for stdio and serve modes.

Service mode should not invent a second configuration format. Phase 9 may choose canonical service config paths and pass them explicitly through `--config`.

The planned health bind/path settings should be either CLI-owned or one clearly documented config surface; do not create multiple conflicting precedence chains just for service management.

## Deterministic tests

Add unit/integration coverage for:

- `mcp serve` CLI parsing/defaults;
- rejection of non-loopback binds;
- path normalization/validation;
- `/healthz` identity/status/version and byte bound;
- health does not trigger provider construction/network operations beyond normal config load;
- HTTP initialize + `tools/list`;
- HTTP and stdio tool-name/schema equivalence;
- current session/header requirements;
- malformed JSON/body/content-type/session handling;
- request-size cap;
- Host/Origin rejection rules;
- graceful loopback shutdown;
- repeated connect/disconnect does not leak persistent session entries beyond transport policy;
- existing stdio MCP tests continue unchanged where possible.

Use ephemeral loopback ports in tests; do not require port 11320 to be free in routine CI.

## Release smoke integration

Extend phase 6's release smoke once this phase lands:

For every native runnable release target:

1. start staged `eggsearch mcp serve --bind 127.0.0.1:<ephemeral>`;
2. poll `/healthz` until ready with a short upper bound;
3. perform MCP initialize + `tools/list` over Streamable HTTP;
4. request graceful termination;
5. verify clean exit.

Continue the stdio smoke as well. Both transports are supported products.

## Documentation changes

Add/update:

- `README.md`: clearly distinguish local client-spawned stdio from persistent service mode;
- `docs/deployment.md`: transport selection, loopback-only policy, default bind/path, health endpoint, foreground operation;
- `docs/installation.md`: no service is started by the ordinary installer yet;
- `docs/release.md`: release smoke now covers both transports;
- `architecture/` MCP transport documentation;
- `CHANGELOG.md`.

Do not document systemd/launchd/cron commands until phase 9 lands.

## Acceptance criteria

Phase 8 is complete only when:

1. the selected rmcp version/features are documented and compatible with the repository MSRV policy;
2. all existing stdio MCP tests still pass or have an explicitly justified protocol update;
3. `eggsearch mcp stdio` remains usable by CodeGG's current default bootstrap;
4. `eggsearch mcp serve` starts on loopback with documented defaults;
5. non-loopback bind is rejected by default;
6. `/healthz` returns bounded eggsearch identity/readiness without external provider activity;
7. a current-protocol HTTP client can initialize, list tools, and call a deterministic tool;
8. HTTP and stdio expose equivalent tool inventory/schema;
9. malformed/oversized/invalid-origin requests fail safely;
10. graceful termination succeeds on Unix and the shutdown seam is usable by Windows SCM work;
11. native release smoke can exercise both stdio and HTTP modes;
12. CodeGG can connect to the persistent endpoint through an explicit remote MCP declaration without changing its default stdio behavior;
13. `make check` passes on the exact final candidate;
14. `registry.md` and this phase status are updated in the closure commit.
