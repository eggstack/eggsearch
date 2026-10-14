# Python Native Library M003 — Agent API, Types and Semantic Compatibility Closure

Status: closed
Candidate: `ca4d0d6`
Implementation plan: `plans/implementation/python-native-library/003-agent-api-types-and-compatibility.md`
Predecessor: `plans/closure/python-native-library/002-status.md`

## Delivered

- Added async context management, `aclose()`, generic `execute()`, and all ten canonical named keyword methods to `AsyncEggsearch`.
- Added `TypedDict` request and response shapes, trust/warning/failure/evidence types, an extension `.pyi` stub, and the `py.typed` marker. Explicit `None`, false, empty-list, and zero values pass through unchanged; runtime dictionaries retain additive response keys.
- Added `tool_contracts()` to expose input/output schemas and the MCP contract fingerprint from the Rust server tool definitions. A Python contract test verifies that all ten method names match and each request type covers all advertised input properties.
- Added a framework-neutral search/fetch/evidence bundle example, package README instructions, and `docs/python-library.md` covering trust, partial failure, Mode Off, config, and feature limits.
- Added a no-network Python consumer journey that searches a configured local workspace in Mode Off, preserves `local_trusted` trust markers, and packages the selected source into an evidence bundle.
- Updated the root README, architecture index, test inventory, testing guide, contributor skill, and Python architecture handoff.

## Compatibility and parity evidence

| Capability | Evidence |
|---|---|
| Ten named operations | Python delegation test covers the exact ten canonical names; the Python smoke invokes all ten through the native SDK |
| Typed request parity | Python test reads the live Rust MCP input schemas and asserts every schema property is present on the corresponding request `TypedDict` |
| Semantic execution parity | M001's exact-candidate SDK tests compare all ten SDK results/errors against their existing handlers; M003 named methods only forward unchanged kwargs to generic `execute()` |
| Trust and local policy | No-network Mode-Off Python journey returns `local_trusted` workspace evidence and preserves that label in the evidence bundle; M001's counted Rust fixture proves zero remote dispatch |
| Structured failures | Public error test checks `EggsearchError.code`, `.message`, `.repair`, and `.data` |
| Response details and unknown keys | Generic JSON dictionaries are returned without projection or key filtering; named wrappers do not transform SDK results |

## Verification on the exact candidate

| Check | Result |
|---|---|
| Rust 1.89 locked binding check | Passed |
| CPython 3.14.2 macOS x86_64 maturin editable build | Passed |
| `python -m pytest -q bindings/python/tests` | 17 passed |
| `python -m compileall -q bindings/python/python/eggsearch bindings/python/examples` | Passed |
| Pyright 1.1.414 on package API and examples | 0 errors, 0 warnings |
| Root `make check` | Passed, including all-feature tests, static guards, hygiene, dependency policy, and packaging contract |

## Compatibility limits and handoff

The native wheel was exercised only on CPython 3.14.2, macOS x86_64, and Rust
1.89.0. CPython 3.11–3.13 and other targets remain unqualified. The package
still uses the unpublished temporary distribution name `eggsearch-native`;
M004 owns PyPI name preflight and source/wheel qualification. No package was
published. Optional `browser`, `egress`, and PDF features remain excluded.

M003 is closed. M004 is ready to qualify local wheel and sdist builds, verify
clean extraction and native smoke, add the supported platform/interpreter CI
matrix, and document exact artifact provenance. Publication remains outside
this plan and requires separate authorization.
