# eggsearch

[![Crates.io](https://img.shields.io/crates/v/eggsearch.svg)](https://crates.io/crates/eggsearch)
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

Pin a version, or install from source with `cargo install eggsearch --locked`. The
installer verifies a SHA-256 checksum and the version, and never elevates — see
[Installation](docs/installation.md) for targets and fallback rules.

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

Register with a client — rendering is read-only, `--apply` is opt-in:

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
{"service":"eggsearch","status":"ready","version":"0.4.1","protocol":"streamable-http"}
```

`mcp serve` accepts loopback binds only and registers no service on its own — see
[Deployment](docs/deployment.md) and [Managed service](docs/service.md).

## MCP tools

| Tool | Purpose |
|------|---------|
| `web_search` | Live metasearch over configured providers |
| `web_fetch` | Bounded extraction of one HTTP(S) URL |
| `batch_fetch` | Bounded batch fetch over URLs or repo locators |
| `provider_status` | Diagnostic provider config, health, and recipes |
| `repo_search` | Structured repository evidence discovery |
| `repo_fetch` | Repository file fetch by locator, line range, or symbol |
| `repo_map` | Bounded repository structure discovery |
| `security_search` | Vulnerability and advisory search |
| `research_search` | Multi-source evidence discovery |
| `build_evidence_bundle` | Deterministic, non-summarizing evidence packaging |

Start with the tool that matches the task: `web_search` for research, `repo_search`
for codebases, `security_search` for advisories, `research_search` for comparisons.
`provider_status` is for hosts and troubleshooting, not a first research step.
Responses carry machine-readable `next_actions` — keep the working set small and
hydrate specialists on demand. See [Tool matrix](docs/tool-matrix.md) and
[Agent workflows](docs/agent-workflows.md).

## Safety

- Fetched content is `external_untrusted`: data, not instructions. `sanitize_output` defaults to `true`, with injection markers reported as `injection_hits`.
- Fetch is bounded and explicit: one URL per `web_fetch`, no autonomous crawling, SSRF checks against blocked address ranges.
- Optional `pdf` and `browser` features add PDF extraction and bounded headless Chrome. The `egress` proxy route is provider-upstream only and is excluded from prebuilt binaries.

See [Safety](docs/safety.md) and [Threat model](docs/threat-model.md).

## Documentation

**Using it** — [Quickstart](docs/quickstart-codegg.md) ·
[Configuration](docs/config.md) ·
[Tool matrix](docs/tool-matrix.md) ·
[Agent workflows](docs/agent-workflows.md) ·
[Provider setup](docs/provider-setup.md) ·
[Optional features](docs/features.md)

**Operating it** — [Installation](docs/installation.md) ·
[Deployment](docs/deployment.md) ·
[MCP integrations](docs/integrations.md) ·
[Managed service](docs/service.md) ·
[Update](docs/update.md) ·
[Dependency security](docs/dependency-security.md) ·
[Release process](docs/release.md) ·
[Release checklist](docs/release-checklist.md)

**Building it** — [Architecture overview](architecture/overview.md) ·
[MCP response contract](architecture/codegg-contract.md) ·
[CodeGG harness integration](docs/codegg-integration.md) ·
[Test inventory](docs/test-inventory.md)

## Build

```bash
cargo build --release        # target/release/eggsearch
make check                   # fmt, clippy, tests, hygiene, dependency policy, packaging
```

The routine gate is network-free. Optional features: `--features pdf`,
`--features browser`, `--features egress`.

Native forge smoke tests call provider APIs directly with operator tokens. They
are **maintainer-only** diagnostics, not user-facing or release evidence.

## License

MIT — see [LICENSE](LICENSE).
