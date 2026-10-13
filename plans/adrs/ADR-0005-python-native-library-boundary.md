# ADR-0005: Python-Native Library Boundary over Eggsearch's Existing Execution Semantics

Status: accepted
Date: 2026-10-13
Decision owners: project maintainers
Scope: Python only

Related: `plans/000-long-term-specification.md` ``1-4, 6-9; `plans/001-terminology-and-domain-model.md`; `plans/002-long-term-roadmap.md`; `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`; `plans/003-planning-process.md`.
Affected subsystem: `plans/subsystems/python-native-library-roadmap.md`.

## Context and problem

The main crate is application-first: the CLI and exactly ten MCP tools are the stable user contract, while the public-looking Rust module tree has no general semver stability guarantee (`src/lib.rs`). The existing ten Rust tool handlers already perform policy checks, canonical argument interpretation, structured warning and error production, projections, and bounded operations using a shared `ServerState`. Python agents require in-process, asyncio-compatible access without an MCP subprocess, daemon, HTTP listener, or reimplemented search engine.

This direction is an **intentional additive extension** to the product's application-first stance, not a promise of stability for every Rust module. The canonical `plans/000-long-term-specification.md` language currently says the stable contract consists only of MCP and CLI, and its single-library-plus-binary description must be reconciled explicitly in M001 rather than silently contradicted.

## Decision drivers

1. One execution authority for provider routing, Mode-Off local-only behavior, authorization, sanitization, SSRF checks, content bounds, provenance, and deterministic identities.
2. A predictable Python-facing compatibility contract and native wheel installation experience, independently of any agent framework.
3. No PyO3, Python interpreter, or extension dependency in the root crate's normal binary/default build; no changes to the seven-target Eggpack binary-release contract.
4. Async-first integration that does not block the Python asyncio event loop or create a Tokio runtime for each request.
5. Minimal maintenance and a strict ownership boundary between Rust/domain and language ergonomics.

## Considered options

- **A — dedicated in-process Rust SDK facade and PyO3 extension (chosen):** new narrow `src/sdk/` facade and independent `bindings/python` extension crate, reusing all ten existing tool execution paths.
- **B — pure-Python MCP/subprocess client:** simpler packaging and immediate tool parity, but violates the requested in-process/no-extra-daemon library boundary and adds process/session maintenance.
- **C — direct bindings to `meta`, `fetch`, and internal state/engine types:** less initial wrapper code, but duplicates policy/response semantics and freezes internal implementation details.
- **D — C ABI first:** unnecessary unsafe ownership and async marshalling for Python's first-class native binding.

## Decision and ownership

Adopt option A. `src/sdk/` is a narrow Rust application facade with an owned shared state, explicit lifecycle and a canonical tool dispatch function. It must delegate to the existing `mcp::tools::run_*` seams (or refactor those seams into one shared owner with MCP delegating) and preserve each tool's validation, repair hints, policy decisions, response projection, and transport-independent JSON result semantics. It is not an alternative provider engine or a newly exposed unbounded fetch client.

The facade is intentionally public for binding compilation but **does not make the remainder of the root crate a semver-stable Rust SDK**. The Python package is the supported consumer API: `AsyncEggsearch`, `execute(tool, arguments)`, and ergonomic named tool methods. No C ABI, TypeScript/Node bindings, browser bundle, framework dependency, or service distribution is approved by this decision.

Root remains a single library + binary *package* with its current public binary assets; the Python extension may be a separate non-workspace Cargo package under `bindings/python/` with a path/version dependency on the root package. The integration must prove `cargo package`, `cargo publish --dry-run`, baseline `make check`, and lockfile hygiene still work. Using a workspace would require a separate justified decision because it can modify root packaging and lockfile semantics.

M001 must reconcile normative product description (`plans/000-long-term-specification.md` ``1, 6, 8 as applicable), `src/lib.rs`, and `architecture/maintenance.md` to express the precise SDK exception. Do not opportunistically rewrite the other canonical architectural guarantees.

## Contract and compatibility

The ten MCP names remain the canonical operation identifiers. A direct client executes operation arguments with the same canonical value and omission/default semantics as MCP; it returns the **tool payload**, not JSON-RPC or MCP content blocks. The client must expose structured failures with existing machine-readable `ToolErrorCode`, message, bounded repair metadata, and optional bounded diagnostics. Result trust labels, content-derived IDs, `next_actions`, partial/failed/absent distinctions, `response_detail`, and evidence bundle identities must survive byte-equivalent normalization of operation payloads. Python wrappers may provide convenience defaults but must not silently alter underlying semantics.

Do not claim automatic cancellation of every upstream transport just because a Python Future was cancelled. The async bridge must document whether the Rust future is dropped, how in-flight work is stopped, and the deterministic bounded fallback. Close and interpreter-shutdown behavior requires tests.

Semantic evolution of Python's exported methods/types and wire-shaped dictionaries is additive within a compatible release; contract changes require an explicit migration. Exact distribution version and PyPI project name are selected at release-preflight, not assumed.

## Security and cross-cutting consequences

Existing application policy MUST apply in-process, including Mode-Off zero remote dispatch for `repo_search`, explicit local permissions, URL/redirect/SSRF checks, fetch and fan-out budgets, sanitized source content, and external-untrusted markers. No Python credential or filesystem override may bypass config validation; opt-in provider secrets remain environment-indirected. Extension-only FFI code stays outside root `#![deny(unsafe_code)]`. No implicit network traffic from importing the package.

Positive: one native reusable engine, no MCP transport requirement, improved agent integration.
Costs: Python runtime lifecycle, wheel matrix, sdist completeness, API compatibility and dedicated security/CI ownership.

## Verification and exit

M001: deterministic ten-tool parity tests including Mode-Off no-remote counters, all-provider failures, warnings, projection, and evidence identity.
M002/M003: direct Python async smoke, concurrency, cancellation, shutdown, agent-facing callable examples, no MCP child/process/listener.
M004: exact-SHA wheel and sdist builds, clean-environment installation, ABI/platform correctness, supply-chain and release policy. No PyPI publication until separately authorized.

Supersession: none. This decision supplements, and does not supersede, ADR-0001.
