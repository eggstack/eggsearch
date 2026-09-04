# eggsearch

[![Crates.io](https://img.shields.io/crates/v/eggsearch.svg)](https://crates.io/crates/eggsearch)
[![docs.rs](https://docs.rs/eggsearch/badge.svg)](https://docs.rs/eggsearch)
[![License](https://img.shields.io/crates/l/eggsearch.svg)](https://github.com/eggstack/eggsearch#license)
[![Downloads](https://img.shields.io/crates/d/eggsearch.svg)](https://crates.io/crates/eggsearch)

Lightweight MCP (Model Context Protocol) search and fetch server for AI agents. Combines live web metasearch, repo-oriented search, bounded fetch, and deterministic evidence bundling over stdio or explicit loopback-only Streamable HTTP.

**No API keys are required for the default installation.** eggsearch ships with keyless web, fetch, advisory, registry, and scholarly paths. Credentialed forge and search adapters are optional enhancements.

## Install

On supported desktop and SBC targets, the installer downloads the matching
GitHub Release binary, verifies its SHA-256 checksum, and checks its version
before installation:

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash
```

Pin an exact published version with `--version`:

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash -s -- --version 0.3.8
```

On Windows PowerShell:

```powershell
irm https://github.com/eggstack/eggsearch/releases/latest/download/install.ps1 | iex
```

The installer never elevates. It uses `/usr/local/bin` or
`%ProgramFiles%\\Eggsearch` only when already privileged, and otherwise uses
`$HOME/.local/bin` or `%LOCALAPPDATA%\\Eggsearch`. Unsupported targets and a
confirmed missing release asset fall back to an exact Cargo install; download,
checksum, and version failures stop without falling back. See
[Installation](docs/installation.md) for the target matrix and verification
rules.

After installation, check for a newer stable release or update the currently
running binary:

```bash
eggsearch update --check
eggsearch update
```

The updater uses crates.io for version discovery, verifies the exact release
checksum and candidate version, and uses an isolated exact-version Cargo build
only when the host is unsupported or the exact asset returns HTTP 404. It never
downgrades or elevates itself. See [Update](docs/update.md) for the policy.

For a source/manual installation, use Cargo:

```bash
cargo install eggsearch --locked
```

## Run

For a client-owned local process, use stdio:

```bash
eggsearch mcp stdio
```

For a persistent foreground endpoint, use Streamable HTTP:

```bash
eggsearch mcp serve --bind 127.0.0.1:11320 --path /mcp
```

Persistent mode accepts loopback binds only, serves MCP at `/mcp`, and exposes
`GET /healthz` for local readiness checks. The ordinary installer does not
start or register a service. See [Deployment](docs/deployment.md) for the
transport choice and safety boundaries.

## MCP Tools

| Tool | Purpose |
|------|---------|
| `web_search` | Live metasearch over configured providers (opt-in bounded excerpts, result timestamps) |
| `web_fetch` | Bounded fetch of one explicit HTTP(S) URL (deterministic focus reads, cache controls) |
| `batch_fetch` | Bounded batch fetch over explicit URLs or repo locators (per-item cache controls) |
| `provider_status` | Diagnostic provider/capability report with workflow recipes |
| `repo_search` | Structured repository evidence discovery with grouped bundles |
| `repo_fetch` | Fetch a specific repo file span or symbol |
| `repo_map` | Bounded repository structure discovery |
| `security_search` | Vulnerability and advisory retrieval |
| `research_search` | Multi-source evidence discovery |
| `build_evidence_bundle` | Deterministic, non-summarizing evidence packaging |

Search tools return machine-readable `next_actions` hints. See [tool-matrix.md](docs/tool-matrix.md) for full tool reference.

## Safety

- Web and remote results are `external_untrusted`
- `sanitize_output` defaults to `true`
- Fetch is bounded and explicit — no crawling, no JavaScript execution, one URL per call
- Fetch targets validated against blocked address ranges (private networks, loopback, link-local, multicast, reserved, documentation)
- Provider errors are bounded before exposure

See [safety.md](docs/safety.md) and [threat-model.md](docs/threat-model.md) for full details.

## Build From Source

```bash
cargo build --release
```

The binary is written to `target/release/eggsearch`.

## Development

```bash
make check
```

Runs formatting, clippy, feature compilation, and the deterministic test suite. Native forge smoke tests exercise the adapter path directly with configured API tokens — these are **maintainer-only** diagnostics, not user-facing. See [release.md](docs/release.md) for the full release process.

## Documentation

- [Configuration](docs/config.md) — config file reference, profiles, defaults
- [Provider Setup](docs/provider-setup.md) — all 37 providers, skip codes, health
- [Optional Features](docs/features.md) — PDF extraction, browser rendering, browser profiles
- [Tool Matrix](docs/tool-matrix.md) — compact tool reference with trust semantics
- [Agent Workflows](docs/agent-workflows.md) — recommended tool call sequences, evidence roles
- [Safety and Fetch Behavior](docs/safety.md) — fetch boundaries, blocked ranges, sanitization
- [Threat Model](docs/threat-model.md) — trust boundaries, prompt injection, escape hatches
- [Architecture Overview](architecture/overview.md) — component index with deep dives
- [MCP Response Contract](architecture/codegg-contract.md) — trust model, warnings, deterministic IDs
- [Release Process](docs/release.md) — preparation, verification, publication
- [Installation](docs/installation.md) — binary targets, installers, checksums, and fallback rules
- [Deployment](docs/deployment.md) — stdio versus persistent loopback Streamable HTTP
- [Update](docs/update.md) — binary-first self-update, verification, and fallback rules
