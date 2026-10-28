# Native Python library

The native Python package wraps the same Rust SDK operations used by Eggsearch's
MCP tools. It runs in-process and does not launch an Eggsearch executable, MCP
server, or listener. The package is an independent PyO3 extension under
`bindings/python`; it is not published to PyPI yet.

## Build and use locally

Use Rust 1.89, CPython 3.11–3.15, and a virtual environment. From the
repository root:

```bash
python -m venv .venv
source .venv/bin/activate
python -m pip install -e 'bindings/python[dev]' maturin==1.15.0
maturin develop --manifest-path bindings/python/Cargo.toml --locked
python -m pytest -q bindings/python/tests
pyright bindings/python/python/eggsearch bindings/python/examples
```

No Python runtime dependencies are required. The candidate distribution and
import name are both `eggsearch`, and the Python package version tracks the root
Eggsearch version as a starting point for synchronized candidate artifacts.
PyPI's project JSON endpoint returned 404 for `eggsearch` on 2026-10-14. The
package remains unpublished, and name availability must be rechecked
immediately before any separately authorized publication.

## Client API

`AsyncEggsearch(config_path=None)` uses `AppConfig` defaults when no path is
supplied and loads the given TOML file otherwise; it never reads the standard
installed config on its own. `AsyncEggsearch.from_default_config()` opts into
the standard installed config using Rust-owned `default_config_path()` and
`AppConfig::load` policy, matching the CLI fallback to defaults when the file
is absent and reporting invalid TOML as a deterministic `ValueError` without
leaking credentials. `AsyncEggsearch.default_config_path()` exposes the
resolved standard path without loading credentials or raw config. Context
management calls `aclose()` on exit. Closing is idempotent, cancels active
operation futures, waits for them to drop, and rejects later calls with
`EggsearchError` whose `code` is `client_closed`.

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

## Cancellation and close

`asyncio.Future.cancel()` on a pending `execute` drops the corresponding Rust
future promptly via `pyo3-async-runtimes` 0.29 `Cancellable` polling; the
shared 32-permit and per-client active count are released by RAII guards, and a
subsequent operation succeeds. `aclose()` cancels the per-client
`CancellationToken`, makes the `tokio::select!` close branch win over a running
`EggsearchClient::execute`, drains active calls, and returns `client_closed`
for close-triggered operations while user cancellation still surfaces as
`asyncio.CancelledError`. Dropping the SDK future also drops a blocked provider
engine future, verified by a counted barrier engine in `src/sdk.rs` and a
debug-only bridge barrier in `bindings/python`. Underlying TCP/TLS teardown
remains bounded by Tokio and `eggfetch-core` timeouts; cancellation is a prompt
future drop with permit recovery, not a proven instantaneous socket reset. New
calls after close fail with `client_closed`, independent clients keep their own
lifecycle, and the runtime survives sequential `asyncio.run` loops.

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

The binding enables PyO3's `abi3-py311` stable ABI, targeting GIL-enabled
CPython 3.11 and newer. The active PyPI distribution qualification targets
CPython 3.11 through 3.15 on each supported platform; only candidates that pass
the exact 25-cell workflow matrix qualify. Linux wheels use the manylinux2014
baseline; native targets are Linux x86_64 and aarch64, macOS x86_64 and arm64,
and Windows x86_64. CI builds on each target and installs each exact wheel on
all five interpreters without installing Rust in the consumer job. These
claims remain conditional on the corresponding exact-candidate jobs passing.

PyPy, free-threaded CPython, and Windows ARM64 are not claimed. ARMv7 is
planned as experimental SBC support only after its separate build and emulated
installed-wheel qualification passes. The release candidate uses a pinned
manylinux cross toolchain and targets the `manylinux_2_28_armv7l` compatibility
tag; its glibc floor and hard-float ABI must pass artifact audit and emulated
installation before publication. That support is limited to ARMv7 hard-float
Linux with glibc 2.28 or newer. ARMv6, soft-float, Windows ARM64, and Alpine or
other musl systems are not included. After the first release appears on PyPI,
SBC users must require a wheel so pip fails on an unsupported target instead
of trying to compile from source:

```bash
python -m pip install --only-binary=:all: eggsearch
```

Do not build the source distribution on an SBC. The source distribution is a
developer fallback for environments outside the published wheel matrix. The
extension package enables no optional Rust `browser`, `egress`, or `pdf`
features. The native library uses the same Eggsearch configuration, provider
credential environment variables, bounded operations, sanitization, and trust
labels as the Rust engine. It does not probe credentials during import.
Python distributions do not alter Eggpack's binary release assets.

An sdist build requires Rust 1.89, a C toolchain, and the Cargo dependencies in
`bindings/python/Cargo.lock`. It can build offline when those locked crate
sources are already in Cargo's local cache; a fresh build must be able to fetch
them from Cargo registries. Native wheels avoid those build requirements at
install and runtime. After the M003 first publication is verified, upgrade
with `python -m pip install --upgrade eggsearch` and remove with
`python -m pip uninstall eggsearch`.

## Post-publication SBC benchmark

M004 runs only on native ARMv7 or aarch64 SBC hardware after publication. Install
the exact public wheel with `--only-binary=:all:`, then run the standard-library
harness outside the repository checkout:

```bash
python -m pip install --only-binary=:all: --report install-report.json eggsearch==VERSION
python -m pip download --only-binary=:all: --no-deps --dest wheelhouse eggsearch==VERSION
python /path/to/eggsearch/bindings/python/scripts/smoke_installed_wheel.py
python /path/to/eggsearch/bindings/python/benchmarks/sbc_runtime.py \
  --version VERSION --wheel /path/to/wheelhouse/eggsearch-*.whl \
  --pip-report install-report.json
```

The harness verifies that the wheel matches the public PyPI URL and SHA-256 in
the pip install report, including the installed native extension. It uses a
local Mode-Off search fixture and a loopback-only fetch fixture. It records cold
import, first operation, repeated sequential event loops, search latency at
1–32 concurrent calls, fetch latency and output near configured character
bounds, cancellation and close, memory, thread and descriptor counts, wheel
size, OS, CPU, RAM, libc, storage, and available thermal or throttling
information. It performs no wheel build and calls no remote providers. Keep
the JSON output with the exact published wheel hash and hardware inventory for
the M004 closure record.

See [Tool Matrix](tool-matrix.md), [Agent Workflows](agent-workflows.md), and
[Python Library Architecture](../architecture/python-library.md) for tool
semantics, result trust, and implementation ownership.
