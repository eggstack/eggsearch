# eggsearch

[![Crates.io](https://img.shields.io/crates/v/eggsearch.svg)](https://crates.io/crates/eggsearch)
[![docs.rs](https://docs.rs/eggsearch/badge.svg)](https://docs.rs/eggsearch)
[![License](https://img.shields.io/crates/l/eggsearch.svg)](https://github.com/eggstack/eggsearch#license)

eggsearch is a lightweight MCP (Model Context Protocol) search and fetch server for AI agents. It combines live web metasearch, repo-oriented search, bounded fetch, and deterministic evidence bundling over stdio.

**No API keys are required for the default installation.** eggsearch ships with keyless web, fetch, advisory, registry, and scholarly paths. Credentialed forge and search adapters are optional enhancements.

Generic search uses the server's configured default provider list. The shipped defaults favor DuckDuckGo, Startpage, and Yahoo; other providers such as Brave, SearXNG, GitHub/GitLab/Gitea code and issue search, OSV, local workspace search, security advisory databases (GitHub Advisory, NVD, CISA KEV, RustSec), package registries (crates.io, PyPI, npm, Go Proxy, Maven Central, NuGet, RubyGems, Packagist), scholarly search (OpenAlex, Crossref, Semantic Scholar), and Sourcegraph code search are available when configured.

## Stable MCP Surface

eggsearch exposes ten stable MCP tools:

- `web_search` - live metasearch over configured providers
- `web_fetch` - bounded fetch of one explicit HTTP(S) URL
- `batch_fetch` - bounded batch fetch over explicit URLs or repo locators
- `provider_status` - diagnostic provider/capability report with routability info plus workflow recipes
- `repo_search` - structured repository evidence discovery with grouped bundles
- `repo_fetch` - fetch a specific repo file span or symbol
- `repo_map` - bounded repository structure discovery with native remote tree retrieval
- `security_search` - vulnerability and advisory retrieval
- `research_search` - multi-source evidence discovery
- `build_evidence_bundle` - deterministic, non-summarizing evidence packaging

Search tools return machine-readable `next_actions` hints. `web_fetch` supports `extract_mode: "text"`, `"markdown"`, and `"metadata_only"`.

## Safety Defaults

- Web and remote results are `external_untrusted`.
- Local workspace results are `local_trusted`, but they are still not instruction-trusted.
- `sanitize_output` defaults to `true` for both search and fetch.
- `web_fetch` is bounded and explicit: it does not crawl, does not execute JavaScript, and only fetches one requested URL.
- Fetch targets are validated against blocked address ranges (private networks, loopback, link-local, multicast, reserved, and documentation addresses). Redirect targets are revalidated before being followed.
- `provider_status` is diagnostic only; it reports configured providers, routability, skip reasons and codes, capabilities, cached health, and workflow recipes.

Retrieval responses expose provider-scoped attempts. A zero-result attempt means the
provider completed successfully; `provider_failed`, `deadline_prevented_completion`,
`provider_capability_unavailable`, and `provider_skipped_by_policy` are distinct
states. A candidate limit reached without proof of more results is reported as
`limit_reached_unknown`, not confirmed truncation.

For the full operator threat model, including fetch network boundaries, trust classes, prompt-injection handling, local workspace caveats, provider disclosure notes, and escape-hatch risks, see [`docs/threat-model.md`](docs/threat-model.md).

## Install

```bash
cargo install eggsearch
```

## Run

```bash
eggsearch mcp stdio
```

## Build From Source

```bash
cargo build --release
```

The binary is written to `target/release/eggsearch`.

## Development

```bash
make check
```

That runs the full local CI gate: formatting, clippy, all feature matrix tests, schema-corpus checks, documentation contract tests, release build, docs build, and publish dry-run.

Documentation contract tests verify that code snippets in docs stay in sync with the codebase.

Native forge smoke tests (`tests/native_forge_smoke.rs`) are separate from
fallback repository search. They exercise the adapter path directly with
configured API tokens. These tests verify optional adapters and are
**maintainer-only** — users do not need these credentials. Missing adapter
credentials limit adapter-specific release claims but do not invalidate
keyless-core release evidence. Scheduled smoke runs are diagnostics and do
not promote a release. See [`docs/release-verification.md`](docs/release-verification.md)
for the core vs. adapter evidence model.

## Docs

- [Provider setup](docs/provider-setup.md)
- [Configuration](docs/config.md)
- [Safety and fetch behavior](docs/safety.md)
- [Threat model](docs/threat-model.md)
- [Tool matrix](docs/tool-matrix.md)
- [Agent workflows](docs/agent-workflows.md)
- [Architecture contract](docs/architecture/codegg-contract.md)
- [Release checklist](docs/release-checklist.md)
- [Release verification protocol](docs/release-verification.md)
