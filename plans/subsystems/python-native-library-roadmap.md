# Python Native Library Roadmap

Status: active (M001 closed; M002 ready; M003-M004 blocked by predecessor closure)
Planning baseline: `eggstack/eggsearch` `main` at `f386b07d7adaa06bbb4a51a9d7e2eecf971c238b` (2026-10-13, Cargo package version 0.4.3)
Primary outcome: in-process async Python library consumable by coding and research agents.

References: `plans/000-long-term-specification.md` sections 1-4, 6-9; `plans/001-terminology-and-domain-model.md` sections 1-5; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md` and `plans/adrs/ADR-0005-python-native-library-boundary.md`.

## Purpose and ownership

Provide a native Python library wrapping Eggsearch's Rust search/fetch/evidence engine. Python users must be able to `pip install` a compatible wheel, instantiate an asyncio client, execute any of the ten established tools without spawning the MCP server, share search/cache/provider state across concurrent calls, and receive structured, trustworthy results and repairable errors.

Ownership: `src/sdk/` owns the narrow direct-operation/application API and shared client lifecycle, while `src/mcp/tools/` remains the single semantic tool execution owner. `bindings/python/` owns PyO3 integration and Python-only ergonomics; it MUST NOT implement its own provider selection, policy, SSRF, fetch, sanitization, result projection, stable-ID algorithm, or ranking. Root default binary/deployment packaging remains owned by existing release and Eggpack paths.

## Invariants (must hold in every milestone)

- All ten existing MCP tools and CLI behavior continue unchanged. Provider/tool inventory and advertised MCP schemas remain authoritative. No additional MCP tools.
- Identical direct/MCP effective request semantics, budgets, sanitized content, trust markers, structured warnings, `next_actions`, provenance, and evidence identity. Failures/partial results must not be silently flattened to absence.
- `search.mode=off` local-repo requests never remotely dispatch (including resolver/forge helpers); test explicit zero remote calls. Do not equate network isolation with an empty result or unavailable credentials.
- All outbound HTTP remains in `eggfetch-core`, with bounded fetch/SSRF/pinning/redirect policy in Eggsearch. No reqwest path introduced and no egress on dynamic fetch.
- Python import is side-effect-free (no network, MCP child, listener, updater, browser spawn or credential probing). Rust/PyO3 failures do not panic/unwind into CPython.
- Root `#![deny(unsafe_code)]` preserved; PyO3 only in separate extension crate, default root dependency graph and binary-release inventory unchanged. Preserve Rust 1.89 MSRV or explicitly prove any binding-only higher MSRV is isolated.
- Do not edit closed workstreams or historical closure records to claim capability. Record exact commands and SHA in new closure records.

## Non-goals

TypeScript, Node, C ABI, standalone C shim, WASM, universal CPython/PyPy support, no-GIL/free-threaded CPython guarantees, compiled browser bundling, agent framework dependencies, an embedded LLM/summarizer, Python provider implementations, local package index, background service, a new Rust general-purpose stable module tree, auto-publish to PyPI, or reworking the existing Eggpack binary workflow.

## Baseline evidence

`src/lib.rs` promises only MCP/CLI stability. `src/mcp/tools/mod.rs` exports the ten `run_*` functions; tools accept typed args and return structured `serde_json::Value` with `ToolError`, alongside one synchronous evidence bundler. `src/mcp/state.rs` constructs shared adapter/fetch/cache/local state from `AppConfig`. `src/mcp/projection.rs` has compact/standard/diagnostic projections. `Cargo.toml` describes one root library+binary with no PyO3 or maturin and no existing `bindings/python` package. `plans/registry.md` currently has no Python library workstream. Current `Cargo.toml` version 0.4.3 is source baseline, not proof of PyPI availability or publication.

## Dependency graph and milestones

`M001 (infrastructure: shared SDK contract and parity)`
  -> hard closed
`M002 (capability: native PyO3 asyncio execution and lifecycle)`
  -> hard closed
`M003 (capability: ergonomic Python agent API, types, examples, semantic parity)`
  -> hard closed
`M004 (infrastructure + operational: wheel/sdist/release qualification)`

Interfaces after M001 may inform parallel exploratory packaging, but the official M002-M004 handoff remains blocked until predecessor closure. No cross-repository or Mode-Off release blocker is waived: develop against current main and re-run the exact Mode-Off regressions, with separate operational gating for publication if existing immutable-release qualification remains blocked.

| Milestone | Class | Status | Plan | User-facing exit |
|---|---|---|---|---|
| M001 direct execution facade | infrastructure | **closed** | `plans/implementation/python-native-library/001-shared-sdk-facade-and-parity.md` | Closure `plans/closure/python-native-library/001-status.md`; candidate `5f686d02e15daa2c5db7ad6d889c06eaff3ceeee`; ten handler parity cases, Mode-Off zero-dispatch counter, and `make check` passed |
| M002 native asyncio bridge | capability | **ready** | `plans/implementation/python-native-library/002-pyo3-async-execution-and-lifecycle.md` | M001 hard dependency closed; Python asyncio client invokes ten tools concurrently without MCP process, honors typed errors and shutdown |
| M003 Python agent ergonomics and safety | capability | **blocked (hard: M002 closure)** | `plans/implementation/python-native-library/003-agent-api-types-and-compatibility.md` | Typed, documented Python tool wrappers and representative working agent workflow |
| M004 Python distribution and qualification | infrastructure | **blocked (hard: M003 closure; operational: publisher approval/platform qualification)** | `plans/implementation/python-native-library/004-wheel-sdist-and-release-qualification.md` | Clean wheel/sdist installation and reproducible native consumer smoke on qualified targets |

## Cross-cutting design requirements

**Configuration:** default and explicit config-file loading must use `AppConfig` and its validation. JSON/dict overrides are optional and MUST not allow privileged policy bypass. Credentials remain referenced by environment names. Python client owns or borrows an immutable effective configuration; avoid global mutable knobs.

**Lifecycle and async:** single shared Tokio runtime/bridge is reused, never constructed once per operation. Ref-counted state may be shared by clients; per-instance close rejects new work, drains/cancels bounded in-flight work and is idempotent. Failure to cancel at the Python layer must not imply unbounded Rust work.

**Result model:** one primary `execute(tool, arguments) -> dict` path; wrappers share it. No MCP-specific `CallToolResult` / JSON-RPC envelopes in return values. Preserve JSON wire-shaped canonical keys with `TypedDict` types and predictable Python exceptions. No Pydantic requirement.

**Packaging:** maintain independent extension package/version policy and optional sdist; verify PyPI name availability and sdist's root path dependency portability. Qualify CPython 3.11-3.14 on Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64; unsupported targets fail clearly or use explicit source-build fallback. ARMv7, Windows arm64 and free-threaded Python deferred unless separately proven. Decide `abi3-py311` on compatibility evidence rather than assumption.

**Security:** equal authority for explicit Mode-Off no-remote behavior, local filesystem root allowance, provider routing, dynamic fetch direct route, URL validation, bounded output and per-provider failures. Agent display of source text never changes trust classification.

**Observability:** structured warnings/repair metadata and no hidden network fallback; small bounded logging/diagnostics, no leak of provider tokens. Performance benchmarks for repeated client search/fetch and baseline process footprint are diagnostic, not excuse to change ranking/budgets.

**Documentation/ops:** add `docs/python-library.md` and short README entry only after implementation, package README/examples and type stubs; keep `docs/test-inventory.md`, `architecture/` ownership and `AGENTS.md` index synchronized. Python artifacts do not enter the canonical seven-target binary asset set.

## Verification and closure

Each milestone adds deterministic no-network fixtures and the `make check` root gate; package milestones additionally use Python pytest/type checks and clean installation. Track contract parity for all ten tools plus failure/warning/Mode-Off cases, not only `web_search` happy-path. Run `cargo package --list`/`cargo publish --dry-run --locked` for root after packaging changes. Every milestone creates `plans/closure/python-native-library/NNN-status.md` with source SHA, completed evidence matrix, tests and outcomes, open findings, compatibility/security evidence and final status, and updates this roadmap + registry in the same closure commit. M001 is closed by `plans/closure/python-native-library/001-status.md`; M002 is now dependency-ready.

Do not declare Python availability merely because a PyO3 extension compiled; user-visible capability requires a real Python consumer smoke. Do not publish wheels until exact-SHA qualification and separate publisher approval. Branch and PR handoff is acceptable; do not merge planning into unrelated repository-hardening or Mode-Off release work.
