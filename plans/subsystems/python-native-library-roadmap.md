# Python Native Library Roadmap

Status: closed (M001-M007 closed; unpublished by design, publication separately authorized)
Planning baseline: `eggstack/eggsearch` `main` at `f386b07d7adaa06bbb4a51a9d7e2eecf971c238b` (2026-10-13, Cargo package version 0.4.3)
Primary outcome: in-process async Python library consumable by coding and research agents.

References: `plans/000-long-term-specification.md` sections 1-4, 6-9; `plans/001-terminology-and-domain-model.md` sections 1-5; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md` and `plans/adrs/ADR-0005-python-native-library-boundary.md`.

## Purpose and ownership

Provide a native Python library wrapping Eggsearch's Rust search/fetch/evidence engine. Python users must be able to `pip install` a compatible wheel, instantiate an asyncio client, execute any of the ten established tools without spawning the MCP server, share search/cache/provider state across concurrent calls, and receive structured, trustworthy results and repairable errors.

Ownership: `src/sdk.rs` owns the narrow direct-operation/application API and shared client lifecycle, while `src/mcp/tools/` remains the single semantic tool execution owner. `bindings/python/` owns PyO3 integration and Python-only ergonomics; it MUST NOT implement its own provider selection, policy, SSRF, fetch, sanitization, result projection, stable-ID algorithm, or ranking. Root default binary/deployment packaging remains owned by existing release and Eggpack paths.

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

At the planning baseline, `src/lib.rs` promised only MCP/CLI stability, `src/mcp/tools/mod.rs` exported the ten `run_*` functions, `src/mcp/state.rs` constructed shared adapter/fetch/cache/local state from `AppConfig`, and `src/mcp/projection.rs` owned compact/standard/diagnostic projections. The baseline `Cargo.toml` had no PyO3 or maturin and no `bindings/python` package. Its version 0.4.3 is a source baseline, not proof of PyPI availability or publication.

## Dependency graph and milestones

`M001 (infrastructure: shared SDK contract and parity)`
  -> hard closed
`M002 (capability: native PyO3 asyncio execution and lifecycle)`
  -> closed at `plans/closure/python-native-library/002-status.md`
`M003 (capability: ergonomic Python agent API, types, examples, semantic parity)`
  -> closed at `plans/closure/python-native-library/003-status.md`
`M004 (infrastructure + operational: wheel/sdist/release qualification)`
  -> closed at `plans/closure/python-native-library/004-status.md`
`M005 (invariant: in-flight cancellation/close and CLI-config opt-in corrective)`
  -> closed at `plans/closure/python-native-library/005-status.md`
`M006 (infrastructure: M004 exact-SHA artifact and closure reconciliation)`
  -> closed at `plans/closure/python-native-library/006-status.md`
`M007 (invariant: Windows safe-read corrective for local content access)`
  -> closed at `plans/closure/python-native-library/007-status.md`

M001-M003 are closed. M004 has substantial preliminary hosted evidence: Python-native run `38522548258` passed on main `1434c1e` across five build targets, 20 interpreter/platform smokes, extracted sdist, and dependency audit, but lacks the required `004-status.md`. M005 is closed. M006 qualification of post-M005 bytes exposed a Windows-only product defect (`File::open` on a directory root fails on Windows, so all jail-checked content reads fail there); M007 repairs it and M006 must qualify **the M007 repair candidate** before recording M004 closure. No cross-repository or Mode-Off release blocker is waived. Publication remains separately authorized.

| Milestone | Class | Status | Plan | User-facing exit |
|---|---|---|---|---|
| M001 direct execution facade | infrastructure | **closed** | `plans/implementation/python-native-library/001-shared-sdk-facade-and-parity.md` | Closure `plans/closure/python-native-library/001-status.md`; candidate `5f686d02e15daa2c5db7ad6d889c06eaff3ceeee`; ten handler parity cases, Mode-Off zero-dispatch counter, and `make check` passed |
| M002 native asyncio bridge | capability | **closed** | `plans/implementation/python-native-library/002-pyo3-async-execution-and-lifecycle.md` | Closure `plans/closure/python-native-library/002-status.md`; candidate `2392e72178aa249445a64bd629caac8219353499`; ten-operation asyncio smoke, bounded conversion, cancellation and close |
| M003 Python agent ergonomics and safety | capability | **closed** | `plans/implementation/python-native-library/003-agent-api-types-and-compatibility.md` | Closure `plans/closure/python-native-library/003-status.md`; candidate `ca4d0d6`; typed methods, schema coverage, no-network Mode-Off agent flow |
| M004 Python distribution and qualification | infrastructure | **closed** | `plans/implementation/python-native-library/004-wheel-sdist-and-release-qualification.md` | Closure `plans/closure/python-native-library/004-status.md`; candidate `faa1da2`; 5/5 builds, 20/20 smokes, offline sdist rebuild, digests recorded |
| M005 lifecycle and configuration corrective | invariant | **closed** | `plans/implementation/python-native-library/005-inflight-cancellation-and-default-config-corrective.md` | Closure `plans/closure/python-native-library/005-status.md`; candidate `baf00cfc8e72f5975010f62e40d5d56985f77c78`; deterministic confirmed-in-flight cancellation/close + permit recovery; explicit default-config opt-in and backwards-compatible no-arg semantics |
| M006 M004 exact-SHA closure reconciliation | infrastructure | **closed** | `plans/implementation/python-native-library/006-m004-exact-sha-distribution-closure.md` | Closure `plans/closure/python-native-library/006-status.md`; qualifying run `38617598636` on `faa1da2`; unpublished unless separately approved |
| M007 Windows safe-read corrective | invariant | **closed** | `plans/implementation/python-native-library/007-windows-safe-read-corrective.md` | Closure `plans/closure/python-native-library/007-status.md`; repaired `safe_open_relative` non-unix root handling; proven by 4/4 Windows smokes on `faa1da2` |

## Cross-cutting design requirements

**Configuration:** currently `AsyncEggsearch()` uses `AppConfig::default()` rather than the CLI's `default_config_path()`. M005 must preserve that documented no-arg behavior, add an explicit typed opt-in for Rust-owned default-config-path loading, and test parity/security with the CLI and with an explicit config path. All forms must use `AppConfig` validation, keep credentials environment-indirected and avoid mutable global overrides.

**Lifecycle and async:** single shared Tokio runtime/bridge is reused, never constructed once per operation. Ref-counted state may be shared by clients; per-instance close rejects new work, drains/cancels bounded in-flight work and is idempotent. Failure to cancel at the Python layer must not imply unbounded Rust work.

**Result model:** one primary `execute(tool, arguments) -> dict` path; wrappers share it. No MCP-specific `CallToolResult` / JSON-RPC envelopes in return values. Preserve JSON wire-shaped canonical keys with `TypedDict` types and predictable Python exceptions. No Pydantic requirement.

**Packaging:** maintain independent extension package/version policy and optional sdist; verify PyPI name availability and sdist's root path dependency portability. Qualify CPython 3.11-3.14 on Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64; unsupported targets fail clearly or use explicit source-build fallback. ARMv7, Windows arm64 and free-threaded Python deferred unless separately proven. Decide `abi3-py311` on compatibility evidence rather than assumption.

**Security:** equal authority for explicit Mode-Off no-remote behavior, local filesystem root allowance, provider routing, dynamic fetch direct route, URL validation, bounded output and per-provider failures. Agent display of source text never changes trust classification.

**Observability:** structured warnings/repair metadata and no hidden network fallback; small bounded logging/diagnostics, no leak of provider tokens. Performance benchmarks for repeated client search/fetch and baseline process footprint are diagnostic, not excuse to change ranking/budgets.

**Documentation/ops:** add `docs/python-library.md` and short README entry only after implementation, package README/examples and type stubs; keep `docs/test-inventory.md`, `architecture/` ownership and `AGENTS.md` index synchronized. Python artifacts do not enter the canonical seven-target binary asset set.

## Verification and closure

Each milestone adds deterministic no-network fixtures and the `make check` root gate; package milestones additionally use Python pytest/type checks and clean installation. M005 adds a barrier-controlled *running Rust future* cancellation/close regression, and M006 requalifies all wheels and the extracted sdist at the final exact SHA, recording long-lived hashes and native consumer evidence. Track contract parity for all ten tools plus failure/warning/Mode-Off cases, not only `web_search` happy-path. Run `cargo package --list`/`cargo publish --dry-run --locked` for root after packaging changes. Every milestone creates `plans/closure/python-native-library/NNN-status.md` with source SHA, completed evidence matrix, tests and outcomes, open findings, compatibility/security evidence and final status, and updates this roadmap + registry in the same closure commit. M001-M007 are closed by their corresponding closure records at `faa1da2` (`004-status.md`, `006-status.md`, `007-status.md` joining `001-status.md`–`003-status.md` and `005-status.md`).

Do not declare Python availability merely because a PyO3 extension compiled; user-visible capability requires a real Python consumer smoke. Do not treat the green M004 baseline run as proof for later bytes or as a closure record. M004/M006/M007 closed together on the M007 repair candidate `faa1da2` with qualifying run `38617598636`. Do not publish wheels until exact-SHA qualification and separate publisher approval. Branch and PR handoff is acceptable; do not merge planning into unrelated repository-hardening or Mode-Off release work.
