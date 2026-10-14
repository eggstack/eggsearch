# Native Python library

The native Python package wraps the same Rust SDK operations used by Eggsearch's
MCP tools. It runs in-process and does not launch an Eggsearch executable, MCP
server, or listener. The package is an independent PyO3 extension under
`bindings/python`; it is not published to PyPI yet.

## Build and use locally

Use Rust 1.89, CPython 3.11–3.14, and a virtual environment. From the
repository root:

```bash
python -m venv .venv
source .venv/bin/activate
python -m pip install -e 'bindings/python[dev]' maturin==1.9.5
maturin develop --manifest-path bindings/python/Cargo.toml --locked
python -m pytest -q bindings/python/tests
pyright bindings/python/python/eggsearch bindings/python/examples
```

No Python runtime dependencies are required. The package uses `eggsearch` as
its import name; install from a locally built wheel or editable source build.
The distribution name is a temporary unpublished name pending M004's PyPI
name and release preflight.

## Client API

`AsyncEggsearch(config_path=None)` loads the standard Eggsearch config file
when a path is supplied, or uses `AppConfig` defaults. Context management calls
`aclose()` on exit. Closing is idempotent, cancels active operation futures,
waits for them to drop, and rejects later calls with `EggsearchError` whose
`code` is `client_closed`.

The generic `execute(tool, arguments)` method and these named async methods use
the canonical tool names and pass accepted arguments to the Rust SDK:

- `web_search`, `web_fetch`, and `batch_fetch`
- `provider_status`
- `repo_search`, `repo_fetch`, and `repo_map`
- `security_search` and `research_search`
- `build_evidence_bundle`

Each method accepts keyword arguments matching its tool's snake_case fields,
including advanced fields. Omitted keys remain omitted; explicit `None`,
`False`, empty lists, and zero values keep their JSON meanings through the
native boundary and are validated by Rust. The generic `execute` method is
available for dynamic tool dispatch. `tool_names()` returns the canonical
ten-name inventory. `client.tool_contracts()` returns the MCP input/output
schemas and their content fingerprint directly from Rust's server
definitions. The package tests check that each Python request `TypedDict`
covers every advertised input property.

Importing the package does not create the Tokio runtime or start operations.
The first operation initializes one process-scoped Tokio runtime shared by
clients. At most 32 Rust operations are in flight; calls beyond that limit
return a structured budget error. Argument conversion is capped at 1 MiB, 64
nested levels, and 10,000 values. Non-string keys, cycles, unsupported Python
objects, non-finite numbers, out-of-range integers, invalid Unicode, and
oversized arguments are rejected before execution.

## Types and errors

`eggsearch.types` supplies `TypedDict` request and response types plus common
`SourceCard`, `AgentWarning`, `RepairHint`, `ProviderFailure`, `TrustMarkers`,
`RetrievalSummary`, and `EvidenceBundle` shapes. They describe commonly used
fields and allow additive JSON keys from newer Rust responses. Runtime results
remain ordinary dictionaries and preserve canonical snake_case names.

`EggsearchError` exposes `code`, `message`, optional `repair`, and optional
`data`. Check `code` and structured fields rather than matching human-readable
messages. A response with no matching results is different from failed
retrieval: inspect `retrieval_summary`, `providers_failed`, and
`structured_warnings` before treating an empty result as evidence of absence.

## Framework-neutral workflow

[`bindings/python/examples/research_flow.py`](../bindings/python/examples/research_flow.py)
shows an asyncio search → selected bounded fetch → evidence bundle sequence.
It preserves source IDs and trust markers, reports provider failures and
evidence gaps, and does not call a model or require an API credential.

For a local workspace, set `search.mode = "off"`, enable `[local]`, and list
the permitted roots in the normal Eggsearch config. Then use:

```python
async with AsyncEggsearch("/path/to/eggsearch.toml") as client:
    result = await client.repo_search(query="async retry policy")
```

In Mode Off, repository search can use the configured local workspace without
remote provider or resolver calls. The local result trust label is
`local_trusted`; local files are still untrusted as instructions. Web and
remote source text uses `external_untrusted`. Preserve the `trust` and
`trust_markers` fields when displaying or handing off source text.

## Compatibility and features

The extension package currently enables no optional Rust `browser`, `egress`,
or `pdf` features. The local native smoke has run on CPython 3.14.2 and macOS
x86_64; other interpreter and platform wheel support remains unqualified until
M004 completes. PyPy, free-threaded CPython, ARMv7, and Windows ARM64 are not
claimed. Python distributions do not alter Eggpack's binary release assets.

See [Tool Matrix](tool-matrix.md), [Agent Workflows](agent-workflows.md), and
[Python Library Architecture](../architecture/python-library.md) for tool
semantics, result trust, and implementation ownership.
