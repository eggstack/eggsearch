# Plan 003 — rmcp HTTP Verification on Eggfetch

Status: ready

Source roadmap:
`plans/subsystems/repository-hardening-roadmap.md`

Milestone: M003 retire rmcp's reqwest HTTP client in favor of eggfetch

Primary class: infrastructure + maintenance

Planning baseline: `c49c600b76e690bb1bc52f641554cca6bf79f36f`

Hard dependencies: none.

Relevant long-term requirements:

- `plans/000-long-term-specification.md#4`
- `plans/000-long-term-specification.md#7`

Relevant ADR:

- `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`

## Objective

Remove rmcp's reqwest-backed Streamable HTTP client from eggsearch while
preserving `integrate --apply --transport http` verification behavior and
retaining rmcp's client/child-process support for stdio verification.

This milestone should make the default/non-browser dependency graph reqwest-free.
It does not claim all-features zero-reqwest closure because chromiumoxide 0.9.1
still declares reqwest unconditionally.

## Current evidence and research conclusion

`Cargo.toml` currently enables rmcp features:

- `server`
- `client`
- `transport-io`
- `transport-child-process`
- `transport-streamable-http-server`
- `transport-streamable-http-client-reqwest`
- `macros`

`src/integrations/common.rs::verify_stdio()` genuinely needs the rmcp client +
`TokioChildProcess` transport.

`verify_http()` first makes a bounded five-second eggfetch request to
`/healthz`, then switches to
`StreamableHttpClientTransport::from_uri()`, which selects rmcp's reqwest
backend for MCP initialize/`tools/list`.

`tests/mcp_http.rs` already proves that eggfetch can perform the server's
Streamable HTTP request/response contract directly, including session handling
and modern/legacy protocol cases.

rmcp 3.2.0 separates:

- `transport-streamable-http-client` — generic client transport;
- `transport-streamable-http-client-reqwest` — reqwest convenience backend.

A custom eggfetch implementation of rmcp's `StreamableHttpClient` is
technically feasible. It is not the preferred current solution because a
correct general adapter must own SSE parsing, legacy open-stream cancellation,
reconnection, custom headers, and raw SSE-event-size enforcement. Eggsearch's
integration verifier only needs a bounded local initialize + `tools/list`
proof and should not create a second general HTTP abstraction.

Research references:

- https://docs.rs/crate/rmcp/3.2.0
- https://docs.rs/rmcp/latest/rmcp/transport/streamable_http_client/trait.StreamableHttpClient.html
- `tests/mcp_http.rs`
- `src/integrations/common.rs`
- https://github.com/eggstack/eggfetch

## Invariants

- HTTP integration verification still proves a ready eggsearch server exposes
  the required ten-tool contract before reporting success.
- Stdio integration verification remains rmcp client + child-process based and
  behaviorally unchanged.
- Verification targets only the fixed loopback eggsearch endpoint; this is not a
  new general fetch API.
- HTTP redirects remain disabled and response size/time are bounded.
- Session IDs are never accepted without the same bounds the MCP server itself
  enforces.
- The normal MCP HTTP server remains rmcp's server transport; this milestone
  changes only the local verification client.
- No client registration JSON/commands change.

## Non-goals

- Reimplementing rmcp's general Streamable HTTP client.
- Adding remote/non-loopback MCP verification.
- Removing rmcp's `client` or `transport-child-process` features needed by
  stdio verification.
- Replacing chromiumoxide in this milestone.
- Changing MCP protocol version support.
- Changing eggfetch API or adding an eggsearch-specific adapter upstream.

## Required work

### 1. Extract a bounded HTTP MCP verification helper

Create one internal integration-verification owner, rather than embedding a
second protocol implementation in `common.rs`.

The helper must use the existing eggfetch client policy:

- loopback-only fixed endpoint;
- redirects disabled;
- five-second phase/total timeout or the existing equivalent;
- explicit decoded-body and per-message bounds;
- no inherited proxy/egress route;
- bounded headers/session ID.

Perform the MCP verification sequence required by the currently supported
Streamable HTTP protocol:

1. send `initialize` with a supported protocol version and client identity;
2. parse a bounded response and capture/validate `Mcp-Session-Id` when issued;
3. send the initialized notification required by the negotiated protocol;
4. send `tools/list`;
5. extract tool names and run the existing `check_tools()` contract;
6. best-effort DELETE/close of a created session when applicable.

Support both legal bounded JSON and SSE response forms that the local rmcp
server may emit. Do not assume arbitrary SSE is safe: raw event size and total
verification bytes must be capped before JSON decoding.

If the current local server can be configured/proven to deterministically
return JSON for this request sequence, document and test that invariant; do not
carry a general SSE parser solely as speculative code.

### 2. Reuse protocol fixtures instead of inventing a second contract

Align request headers/message shapes with `tests/mcp_http.rs` and
`tests/mcp_2026_protocol.rs`.

Move only genuinely shared constants/builders to an internal owner if doing so
reduces drift. Do not expose a new public Rust API merely so integration tests
can call private verification mechanics.

Add focused unit/integration tests for:

- ready happy path;
- missing required tool;
- malformed initialize response;
- oversized response/session ID;
- timeout/stalled response;
- non-success HTTP status;
- unexpected redirect;
- legal session cleanup.

### 3. Remove rmcp's reqwest client feature

After the helper is proven, remove
`transport-streamable-http-client-reqwest` from the rmcp feature list.

If no remaining code uses the generic
`transport-streamable-http-client` feature, do not enable it either.
Retain `client` and `transport-child-process` for stdio verification.

Regenerate `Cargo.lock`.

### 4. Add dependency-shape guards

Record and enforce:

```bash
cargo tree --locked -i reqwest
cargo tree --locked --all-features -i reqwest
```

Expected after M003:

- default graph: no reqwest package;
- all-features graph: reqwest is reachable only through chromiumoxide until M006.

Add a static/metadata guard that fails if rmcp becomes a reqwest reverse
dependency again. The temporary chromiumoxide exception must name M006 as its
exit condition.

### 5. Reconcile architecture documents

`ADR-0002` currently records rmcp's reqwest Streamable HTTP client as an
intentional transitive exception. Preserve that historical statement but append
a dated implementation note/addendum when M003 lands stating that the exception
was retired; do not rewrite the original decision as if the exception never
existed.

Update `architecture/maintenance.md`, which currently says the rmcp
Streamable HTTP client feature is required for `integrate --apply`
verification. After M003 only rmcp client + child-process remain required for
stdio.

Update transport/testing docs with the new verification ownership.

## Failure and recovery semantics

- A health success followed by MCP initialize/tools failure remains a failed
  integration verification.
- HTTP/SSE/body/session limits are hard failures.
- Session cleanup failure after successful verification should be surfaced if it
  indicates the local session remains live; define/test the same behavior on all
  platforms.
- Do not fall back to rmcp's reqwest transport if eggfetch verification fails.

## Focused verification

At minimum:

```bash
cargo tree --locked -i reqwest
cargo tree --locked --all-features -i reqwest
cargo test --locked --all-features --test mcp_http
cargo test --locked --all-features --test mcp_2026_protocol
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features integrations
make check
make docs-check
make bench-check
```

Exercise `eggsearch integrate ... --transport http --apply` against a local
persistent server using a disposable client configuration/fixture as current
integration policy permits.

## Acceptance criteria

- No rmcp Cargo feature activates reqwest.
- The default eggsearch dependency graph contains no reqwest.
- The all-features graph identifies chromiumoxide as the only remaining direct
  reqwest reverse dependency.
- HTTP integration verification still initializes MCP and validates required
  tools using eggfetch with bounded timeout/body/session behavior.
- Stdio verification is unchanged.
- No registration format, CLI argument, MCP tool/schema, or server behavior
  changes.
- ADR/architecture docs accurately describe the retired rmcp exception.

## Stop conditions

Stop if:

- the local server's supported response semantics require implementing a
  substantial general SSE/reconnection client merely for verification;
- manual bounded verification cannot provide behavior equivalent to the existing
  rmcp verifier;
- removing the reqwest feature disables required server functionality;
- a regression requires widening endpoint authority beyond fixed loopback.

If a full rmcp custom-client adapter becomes necessary, write a separate
bounded design/implementation plan rather than embedding it opportunistically.

## Closure evidence required

Create `plans/closure/repository-hardening/003-status.md` containing:

- before/after rmcp feature set;
- before/after `cargo tree -i reqwest` outputs for default/all-features;
- integration verification behavior matrix;
- focused/canonical tests and hosted CI;
- ADR/documentation reconciliation;
- explicit statement that chromiumoxide is the sole remaining reqwest blocker.
