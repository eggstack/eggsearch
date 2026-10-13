# M003 — Python Agent API, Types and Semantic Compatibility

Status: blocked (hard dependency: M002 closed)
Primary class: capability
Source: `plans/subsystems/python-native-library-roadmap.md` M003
Planning baseline: `main` `f386b07d7adaa06bbb4a51a9d7e2eecf971c238b`; refresh after M002.
ADRs `ADR-0001`, `ADR-0005`; relevant long-term ``1-5, 7-9 of `plans/000-long-term-specification.md`.

## Objective and non-goals

Turn the working native extension into a Python-idiomatic, minimally dependent library suitable for a generic research or coding agent. Preserve full advanced tool access, trust interpretation, and truthful failures. No publication before M004. Hard gate: M002 closure record and stable native `execute` semantics.

Non-goals: Pydantic as required dependency, SDK-client HTTP/MCP fallback, framework-specific integrations, LLM invocation, generated answers, hidden crawling/autonomous fetch, synchronous wrapper, C ABI, TypeScript, compatibility with free-threaded Python or PyPy without qualification.

## Ordered work packages

**WP1 — Ergonomic async client.** Implement public Python `AsyncEggsearch` wrapper with `async with`, `aclose`, `execute(tool, arguments)`, and keyword-style `web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, and `build_evidence_bundle`. All named methods delegate to `execute` and retain full accepted advanced args, defaults and request projection. Prefer `**kwargs`/typed-options or narrow typed args over reimplementing Rust validation. Provide `tool_names` and authoritative tool input/output schema/fingerprint discovery if cleanly supported by the M001 facade, not a second hand-maintained registry.

**WP2 — Data model and types.** Return Python dictionaries with canonical snake_case keys. Supply `TypedDict` (including optional fields) for the main result/request envelopes, nested `SourceCard`, `AgentWarning`, `RepairHint`, provider failures, trust markers, retrieval/coverage information, selected fetches and evidence bundle types. Include `py.typed`; optional stdlib dataclass views are fine but must not discard unknown future JSON keys. Maintain one source of truth for ten method names/types and parity checks so hand-edited stubs do not silently diverge from tool schemas. Make custom `EggsearchError` and subclasses or code-based dispatch for invalid shape, semantic/repairable, policy/capability, upstream, internal and closed client; `code` stable and meaningful, no arbitrary exception-string matching.

**WP3 — Agent consumption examples.** Add `docs/python-library.md`, package README and examples showing: install from local wheel; async search->choose candidate->bounded fetch->evidence bundle; typed interpretation of `structured_warnings`, `trust`, `next_actions` and failure vs absence; concurrent research/repo/security operations with explicit budgets; local workspace `repo_search` under Mode-Off with no remote calls. Example should be framework-neutral (a callable adapter function taking a tool name and dict) and require no model-provider/API credentials. No CLI registration or service needed.

**WP4 — Compatibility/error hardening.** Snapshot/compare each of ten operation outputs across Rust facade, MCP structured content, and Python consumer after normalizing transport-specific shells. Include response detail `compact`/`standard`/`diagnostic`, empty provider vs all-provider failures, mixed partial results, deterministic stable IDs, injection markers, collision/duplicate handling, truncation, local-trusted vs external untrusted, evidence gap/coverage and bundle identity. Ensure Python optional keyword names don't change the semantics of omitted/null/false/empty arrays, and errors preserve repair hints and policy denial. Reject invalid objects and use stable predictable Python `TypeError`/`EggsearchError` policy.

**WP5 — Usability qualification.** Type-check examples with pyright or mypy under documented versions; run test-only no-network Python agent simulation calling the public wrapper, report wheel import and type metadata, CPython versions tested, observed cold-start/idle-memory compared with binary subprocess only as characterization. No new required runtime Python package dependencies unless measured and justified.

## Compatibility and security

Do not rewrite `SourceCard` fields, FNV IDs, advertised tool schemas or root structured error taxonomy. Caller-provided config/paths cannot escape server policy, origin safety, local workspace confinement or Mode-Off zero outbound dispatch. Do not convert fetched content to executable code or misrepresent content as trusted. Expose all hard failures, repaired arguments, provider skips and partial retrieval attempts. No background MCP process, auto-update, network on import or credential export.

## Verification and acceptance

From root: `make check`, `cargo test --locked --all-features --test evidence_contract`, `cargo test --locked --all-features --test static_guards`. From installed editable wheel/venv: `python -m pytest -q`, `python -m compileall python/eggsearch`, configured `pyright`/`mypy`, runnable examples using deterministic local fixtures. Maintain explicit 10/10 parity table and full wrapper-to-generic method test, independent of online search uptime.

Accept only when at least one realistic Python agent integration consumes the native library via asyncio without subprocess, names all ten capabilities, interprets trust/partial failures correctly, and no semantic drift is observable. STOP for binding-only special cases that bypass validated tool semantics; create a corrective if the typed surface loses diagnostics.

Closure: `plans/closure/python-native-library/003-status.md` with exact SHA, example transcript/test output, static type-check findings, per-tool parity matrix, error/Mode-Off/security evidence and remaining known limitations; roadmap+registry status update M003 closed / M004 ready in same commit.
