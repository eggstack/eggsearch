# M001 — Shared SDK Facade and Ten-Tool Parity

Status: ready
Primary class: infrastructure
Source: `plans/subsystems/python-native-library-roadmap.md` M001
Baseline: `main` `f386b07d7adaa06bbb4a51a9d7e2eecf971c238b` (2026-10-13, Cargo 0.4.3)
ADRs: `ADR-0001` (fixed MCP tool ownership) and `ADR-0005` (Python SDK boundary).
Long-term: `plans/000-long-term-specification.md` ``1-4, 6-9; `plans/001-terminology-and-domain-model.md` ``1-5.

## Objective, readiness and non-goals

Introduce a stable, narrow in-process Rust operation interface for future Python bindings while preserving the ten tool execution semantics verbatim. No hard dependency is outstanding. ADR-0005 is accepted as the intended additive Python product direction; reconcile the existing normative specification's MCP/CLI-only stability statement in this milestone. This is not a Python deliverable or a new general Rust semver guarantee.

Non-goals: PyO3, wheels, new MCP names, refactoring engines, changing rankings/fetch contracts, changing CLI/startup/installation, browser-profile operations, TypeScript/C ABI, native Python models, network-based tests.

## Baseline and ownership

`src/mcp/tools/mod.rs` exposes `run_web_search`, `run_web_fetch`, `run_batch_fetch`, `run_provider_status_async`, `run_repo_search`, `run_repo_fetch`, `run_repo_map`, `run_security_search`, `run_research_search`, and synchronous `run_build_evidence_bundle`. They take typed args and, where applicable, `Arc<ServerState>`, returning `Result<Value, ToolError>`. `src/mcp/server.rs` wraps these in `rmcp`, `mcp/projection.rs` projects output; `mcp/state.rs` creates all shared clients, local backends and caches. `src/lib.rs` explicitly disclaims general Rust API stability.

## Ordered work packages

**WP1 — Decide and document narrow contract.** Add `src/sdk/mod.rs` and an owned `EggsearchClient` with a single `ServerState::build(AppConfig)` construction; avoid exposing internal engine instances. Define a stable `ToolName` enum/string conversion mapped from authoritative `tool_contract::tool_names()` (exact ten; case-sensitive), `execute(name, serde_json::Value)`, explicit `config` construction and simple typed Rust convenience only where it delegates. Return actual tool JSON payloads, never `CallToolResult`/JSON-RPC blocks. Unknown operation and invalid argument shape get consistent structured errors, distinguish shape errors from semantic repairs. Pure bundler uses the same dispatch path.

**WP2 — Share one semantic execution owner.** Implement an exhaustive match forwarding each tool to its existing `run_*` handler. Parse input with serde once, using existing argument models; do not duplicate defaults, clamping, canonical goal resolution or provider selection. MCP routing must continue through those exact functions. If a shared execution module is extracted, both SDK and MCP must call it and all tool implementations remain the sole policy owner. Expose machine-readable `SdkError` mapping from `ToolErrorCode`, message, bounded repair and data; avoid relying on human message substrings to classify new failures.

**WP3 — Lifecycle and concurrency boundary.** The Rust facade must be `Send + Sync` where the underlying state permits. Construct once, cheap clone/shared state, concurrent `&self` calls. No new owned Tokio runtime in the facade; async caller supplies runtime. No added background updater, logger initialization, process or listener. Specify behavior for dropping a clone and for calls that outlive another clone. Keep all request budgets and default `response_detail` behavior unchanged.

**WP4 — Parity and security fixtures.** Introduce fixture-backed SDK-vs-existing-handler/MCP tests for all ten tools using `mock`/local fixtures, typed invalid requests, provider hard failure vs empty match, mixed provider successes, projectors (compact/standard/diagnostic), identical structured warnings, trust/ID/next actions, fetch truncation, evidence bundle byte/ID stability. Mode-Off `repo_search` tests must instrument provider/resolver/forge paths and assert zero external calls; compare live auto-provider behavior separately. No live network. Avoid test-only bypass that changes runtime semantics.

**WP5 — Documentation/reconciliation.** Update `src/lib.rs` statement, `architecture/maintenance.md` ownership table, new `architecture/python-library.md` (or coherent subsection) and `plans/000-long-term-specification.md` only to describe the explicitly approved additional Python native consumer, exact scoped SDK boundary, and separate binding package; do not rewrite old requirements or misstate Python as shipped. Preserve root single library+binary *package*, binary artifact and release rules. Update `AGENTS.md` as index, not duplicate rules; update test inventory and roadmap status only upon closure.

## Compatibility, failure and security

No change to MCP tool set, request acceptance, CLI exit status, schema fingerprints, or existing Rust `core` types. Config validation remains authoritative. Invalid args follow semantic error versus shape error categories; existing default values/advanced hidden arguments stay supported. All-provider failure is not an empty successful response. Mode-Off local-only remote exclusion is explicitly retested and no bypass via generic `execute` allowed. No network, subprocess or Python interpreter dependency introduced.

## Verification and acceptance

Focused (adapt names to actual existing tests, log commands):
- `cargo test --locked --all-features --lib sdk` (new SDK unit fixtures)
- `cargo test --locked --features mock --test mcp_tools` (where present; inspect suites before execution)
- `cargo test --locked --all-features --test evidence_contract`
- `cargo test --locked --all-features --test static_guards`
- `cargo test --locked --all-features --test provider_capability_contract`

Broad: `make check`; `RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps`; `cargo package --list` and `cargo publish --dry-run --locked` if root manifest/package list changes.

Pass requires a direct Rust consumer invoking all ten operations with observationally equivalent JSON results/errors versus existing handlers, accepted policy/Mode-Off zero-call evidence, no runtime or process management, no new MCP operation, and passing broad guards. If parity requires redefining default/Mode-Off dispatch or weakening existing safety gates, STOP and register a corrective/ADR before touching engine internals. Do not close on compilation alone.

## Closure handoff

Create `plans/closure/python-native-library/001-status.md` with exact SHA, changed owner paths, one case-per-tool parity matrix, failure/Mode-Off counters, command outputs, compat/security evidence and residual risks. Update roadmap and registry to M001 closed / M002 ready **in the same closure commit**. No M002 implementation before this evidence.
