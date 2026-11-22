# eggsearch

[![Crates.io](https://img.shields.io/crates/v/eggsearch.svg)](https://crates.io/crates/eggsearch)
[![PyPI](https://img.shields.io/pypi/v/eggsearch.svg)](https://pypi.org/project/eggsearch/)
[![PyPI Downloads](https://static.pepy.tech/personalized-badge/eggsearch?period=total&units=INTERNATIONAL_SYSTEM&left_color=BLACK&right_color=GREEN&left_text=downloads)](https://pepy.tech/projects/eggsearch)
[![docs.rs](https://docs.rs/eggsearch/badge.svg)](https://docs.rs/eggsearch)
[![License](https://img.shields.io/crates/l/eggsearch.svg)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/eggsearch.svg)](https://crates.io/crates/eggsearch)

A lightweight MCP (Model Context Protocol) search and fetch server for AI agents:
live web metasearch with RRF dedup, bounded fetch, and deterministic evidence
bundling, over client-owned stdio or explicit loopback-only Streamable HTTP.

**No API keys are required for the default installation.** The default install is
keyless across web, fetch, advisory, package-registry, and scholarly paths.
Credentialed providers are opt-in.

## Install

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash
```

```powershell
irm https://github.com/eggstack/eggsearch/releases/latest/download/install.ps1 | iex
```

Pin a release version when reproducibility matters. `cargo install eggsearch --locked`
installs the published crate. To build the current checkout, run
`cargo build --release`. The installer verifies its SHA-256 checksum and
version, and never elevates. See [Installation](docs/installation.md) for
supported targets and fallback rules.

Install the native asyncio Python package separately:

```bash
python -m pip install eggsearch
```

On ARMv7 SBCs, require a prebuilt wheel so unsupported targets fail without
attempting a source build:

```bash
python -m pip install --only-binary=:all: eggsearch
```

## Quickstart

```bash
eggsearch doctor          # look for "healthy": true
eggsearch providers       # per-provider enabled / configured / routable
```

```bash
$ eggsearch search "rust tokio" --max-results 1
# Results for 'rust tokio' (1 items, 1 failed)

1. <<<EXTERNAL_UNTRUSTED field=title id=src_9778070202700542>>>
Tokio - An asynchronous Rust runtime
<<<END>>>
   https://tokio.rs/
   [duckduckgo]
   <<<EXTERNAL_UNTRUSTED field=snippet id=src_9778070202700542>>> Tokio is a library for
   writing fast, reliable, and easy network applications with Rust. … <<<END>>>

Failed providers:
  - yahoo: engine 'yahoo' returned status 500 (http_status)
```

```bash
$ eggsearch fetch "https://example.com" --metadata-only
# Fetch: https://example.com

Final URL: https://example.com/
Title: <<<EXTERNAL_UNTRUSTED field=title id=https://example.com/>>>
Example Domain
<<<END>>>
Status: 200
Content-Type: text/html; charset=utf-8
Fetched: true
Truncated: false
```

Every card carries a content-derived stable ID and an `<<<EXTERNAL_UNTRUSTED>>>`
frame: treat fetched text as data, never instructions. Results merge across
providers, so one provider failing is reported as a warning rather than failing
the query. One URL per `web_fetch`, `batch_fetch` for explicit fan-out, `--json`
for machine output.

Preview client configuration first. Rendering is read-only; `--apply` is opt-in:

```bash
eggsearch integrate list
eggsearch integrate claude --transport stdio --apply
```

Or run the server yourself, as a client-owned child process:

```bash
eggsearch mcp stdio
```

or a persistent loopback endpoint:

```bash
$ eggsearch mcp serve --bind 127.0.0.1:11320 --path /mcp &
$ curl -s http://127.0.0.1:11320/healthz
{"service":"eggsearch","status":"ready","version":"0.4.4","protocol":"streamable-http"}
```

`mcp serve` accepts loopback binds only and does not register a service. See
[Deployment](docs/deployment.md) and [Managed service](docs/service.md).

## MCP tools

| Tool | Purpose |
|------|---------|
| `web_search` | Live metasearch over configured providers |
| `fetch` | Bounded fetch of selected web URLs or repository locators |
| `provider_status` | Diagnostic provider config, health, and recipes |
| `repo_search` | Structured repository evidence discovery |
| `repo_map` | Bounded repository structure discovery |
| `security_search` | Vulnerability and advisory search |
| `research_search` | Multi-source evidence discovery |
| `knowledge_brief` | Cutoff-aware candidate developments; not a claim that a model lacks a fact |
| `build_evidence_bundle` | Deterministic, non-summarizing evidence packaging |

`web_fetch`, `batch_fetch`, and `repo_fetch` remain callable compatibility aliases
with unchanged request and response shapes; they are not advertised in `tools/list`.

Start with the tool that matches the task: `web_search` for research, `repo_search`
for codebases, `security_search` for advisories, `research_search` for comparisons.
`provider_status` is for hosts and troubleshooting, not a first research step.
Responses include machine-readable `next_actions`, so keep the working set small
and load specialized tools only as needed. `web_search` returns source cards, not
a synthesized answer; read useful sources with `web_fetch`. See [Tool matrix](docs/tool-matrix.md)
and [Agent workflows](docs/agent-workflows.md) for details.

## Safety

- Fetched content is `external_untrusted`: data, not instructions. `sanitize_output` defaults to `true`, with injection markers reported as `injection_hits`.
- Fetch is bounded and explicit: one URL per `web_fetch`, no autonomous crawling, SSRF checks against blocked address ranges.
- Current source builds enable PDF extraction and browser rendering by default.
  Published `v0.4.4` binaries predate that change and include neither feature.
  Browser rendering requires an existing Chrome or Chromium installation.
  The opt-in `egress` proxy route is limited to provider upstreams and is
  excluded from prebuilt binaries.

See [Safety](docs/safety.md) and [Threat model](docs/threat-model.md).

## Documentation

**Using it:** [Quickstart](docs/quickstart-codegg.md) ·
[Configuration](docs/config.md) ·
[Tool matrix](docs/tool-matrix.md) ·
[Agent workflows](docs/agent-workflows.md) ·
[Provider setup](docs/provider-setup.md) ·
[Optional features](docs/features.md) ·
[Safety](docs/safety.md)

**Operating it:** [Installation](docs/installation.md) ·
[Deployment](docs/deployment.md) ·
[MCP integrations](docs/integrations.md) ·
[Managed service](docs/service.md) ·
[Update](docs/update.md)

**Developing it:** [Architecture overview](architecture/overview.md) ·
[MCP response contract](architecture/codegg-contract.md) ·
[CodeGG integration](docs/codegg-integration.md) ·
[Test inventory](docs/test-inventory.md) ·
[Release process](docs/release.md)

The in-process asyncio Python package is published separately on PyPI. See the
[Python library guide](docs/python-library.md) for its API, platform limits,
and experimental ARMv7 SBC support.

## Build

```bash
cargo build --release        # target/release/eggsearch
make check                   # fmt, clippy, tests, hygiene, dependency policy, packaging
```

The routine gate is network-free. Current source builds include PDF extraction
and browser rendering by default. `--no-default-features` omits them, and
`--features egress` enables the source-build-only provider proxy route.

Native forge smoke tests call provider APIs directly with operator tokens. They
are **maintainer-only** diagnostics, not user-facing or release evidence.

## License

MIT. See [LICENSE](LICENSE).
