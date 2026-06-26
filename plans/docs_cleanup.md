# Eggsearch Final Documentation Cleanup Plan

## Purpose

This plan covers the final documentation cleanup pass before integrating `eggsearch` into Codegg. The implementation is considered functionally ready for first integration. This handoff should not add new search providers, alter MCP behavior, change fetch security policy, or introduce new architecture. The goal is to make the public README, module docs, examples, and tool descriptions accurately match the current implementation.

## Current State

`eggsearch` is now a flattened single-crate Rust MCP server exposing three MCP tools:

- `web_search`
- `web_fetch`
- `provider_status`

The intended product boundary is:

- lightweight metasearch MCP server
- no hard SearXNG dependency
- no local full-text index
- no crawler
- no browser automation
- no persistent cache requirement
- bounded single-URL fetch/extract support
- optional API-backed provider support, currently including `brave_api`

The remaining issue is stale or imprecise documentation. Some docs still reflect earlier phases where `web_fetch` was not exposed through MCP, and some config examples still use the deprecated `max_results` field instead of the preferred `default_max_results` field.

## Non-Goals

Do not implement new functionality in this cleanup pass.

Out of scope:

- adding Tavily, Exa, Kagi, SerpAPI, or other API providers
- changing provider aggregation behavior
- changing `web_fetch` SSRF policy
- changing MCP tool schemas except documentation text/descriptions if needed
- adding local indexing or Tantivy
- changing config semantics beyond docs/examples
- changing default provider set
- changing Codegg itself

## Required Cleanup Items

### 1. README: Update Search Config Example

Find the primary TOML config example in `README.md`.

Replace deprecated/global `max_results` usage with `default_max_results`.

Preferred example:

```toml
[search]
default_max_results = 10
max_results_cap = 50
max_query_chars = 512
timeout_ms = 8000
```

If the README mentions `max_results` in config, clarify that it is accepted only as a backwards-compatible alias for `default_max_results`.

Suggested wording:

> `default_max_results` controls the default number of results when a client does not pass `web_search.max_results`. `max_results_cap` is the server-enforced upper bound. The legacy config key `max_results` is still accepted as an alias for `default_max_results`, but new configs should use `default_max_results`.

Do not remove request-side `max_results` from the MCP tool docs. That field remains correct as a per-call override.

### 2. README: Correct MCP Tool List Everywhere

Search `README.md` for references to MCP tools or project structure.

Every user-facing MCP tool list should include exactly:

```text
web_search
web_fetch
provider_status
```

Fix stale text such as:

```text
mcp/ — web_search + provider_status
```

Replace with:

```text
mcp/ — MCP server and tool adapters for web_search, web_fetch, and provider_status
```

Also check any “available tools,” “tool surface,” “usage,” or “MCP integration” sections for stale claims that fetching must be done out of band.

### 3. README: Clarify Search vs Fetch Workflow

Ensure the README clearly distinguishes source discovery from content retrieval.

Recommended wording:

```text
Use web_search to discover candidate sources. It returns compact SourceCards with titles, URLs, snippets, provider metadata, and trust labels. It does not fetch full page contents.

Use web_fetch only for an explicit HTTP(S) URL selected by the user or by a host after reviewing search results. web_fetch retrieves one URL, follows bounded validated redirects, extracts bounded text from HTML/text responses, and marks the result as external_untrusted.
```

Avoid wording that implies `web_fetch` is a crawler, browser, or general browsing environment.

### 4. README: Ensure Current Provider List Is Accurate

The README provider list should match the current implementation.

Expected provider categories:

No-key/default or no-key-capable providers:

- `duckduckgo`
- `brave`
- `startpage`
- `yahoo`
- `mojeek`
- `searxng` when configured with a base URL

API-backed provider:

- `brave_api`, opt-in, requires API key through environment variable

The README should distinguish between:

- known/supported provider IDs
- enabled providers
- default providers queried when the client does not specify a provider list

Suggested wording:

> `providers` controls which providers are available to the server. `default_providers` controls which enabled providers are queried when a `web_search` request does not specify providers explicitly.

### 5. README: Tighten SSRF / DNS-Rebinding Claims

Do not overclaim complete DNS rebinding protection.

Accurate wording:

```text
web_fetch validates the initial URL and each redirected URL before making a request. It rejects unsupported schemes, embedded credentials, localhost/private-network targets by default, and hostnames that resolve to blocked address ranges during validation. This mitigates common SSRF and redirect-to-private-network cases, but it should not be described as complete DNS-rebinding protection.
```

Avoid wording like:

```text
re-checks the connected address
fully prevents DNS rebinding
complete SSRF protection
```

unless the implementation actually validates the connected peer address after connection.

### 6. README: Clarify No-Op / Reserved Config Fields

Check whether the README exposes any config fields that are currently parsed but not enforced or not wired through, such as:

- `safe_search` for HTML providers
- `user_agent` if still not applied to the underlying provider clients
- `respect_robots_txt` if parsed but not enforced

For each no-op/reserved field, either remove it from the main example or explicitly mark it as reserved/advisory.

Preferred principle:

- main config examples should show only fields that work today
- reserved fields may be documented separately under “reserved/future options”

### 7. Core Docs: Fix `src/mcp/mod.rs`

Update `src/mcp/mod.rs` module-level documentation so it reflects all MCP tools.

It should say the MCP module exposes:

- `web_search`
- `web_fetch`
- `provider_status`

Remove stale wording that says only `web_search` and `provider_status` are available.

### 8. Core Docs: Fix `src/lib.rs`

Update crate-level docs in `src/lib.rs`.

Required fixes:

- mention `web_fetch` as part of the MCP surface
- ensure examples use the current `WebSearchRequest::validate(...)` signature
- ensure examples use `default_max_results` terminology if discussing config
- avoid saying `web_fetch` is deferred or out of scope

If there are doctests, run them or ensure they compile against the current API.

### 9. Core Docs: Fix Query / Safe Search Wording

Check `src/core/query.rs` and any generated schema text around `safe_search`.

Current desired semantic:

```text
safe_search is reserved for provider-specific enforcement. Current HTML providers may not enforce it. When supplied but unsupported, the server should warn rather than silently claiming enforcement.
```

Avoid wording that implies all providers enforce safe search today.

### 10. SourceCard / Fetch Docs

Search for stale comments such as:

- `web_fetch deferred`
- `fetched is always false for MVP`
- `page fetching out of band`

Clarify the distinction:

- SourceCards returned by `web_search` are discovery-only and normally have `fetched = false`
- `web_fetch` returns a separate fetched-document response
- fetched text is marked `external_untrusted`

Do not imply search results themselves contain fetched page contents.

## Validation Checklist

After editing docs, run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --doc
```

If `cargo test --doc` is not currently part of CI, run it manually at least once for this cleanup pass because the previous stale `validate(...)` example risked doctest drift.

Also run:

```bash
eggsearch doctor
```

If available and safe for the environment, also run:

```bash
eggsearch doctor --probe
```

Do not require network-dependent probing in normal CI.

## Acceptance Criteria

This cleanup pass is complete when:

1. README config examples use `default_max_results`, not deprecated `max_results`.
2. README clearly explains the distinction between request-side `web_search.max_results`, server-side `default_max_results`, and server-side `max_results_cap`.
3. All README MCP tool lists include `web_search`, `web_fetch`, and `provider_status`.
4. README does not claim complete DNS-rebinding protection or post-connect peer-address verification unless implemented.
5. README accurately lists supported providers, including `mojeek`, optional `searxng`, and opt-in `brave_api`.
6. README distinguishes enabled providers from default providers.
7. `src/mcp/mod.rs` and `src/lib.rs` reflect the current MCP tool surface.
8. Query/safe-search docs do not imply unsupported enforcement.
9. Stale “web_fetch deferred/out-of-band” wording is removed.
10. Tests, clippy, formatting, and doctests pass.

## Codegg Integration Readiness After This Pass

After this cleanup pass, eggsearch should be considered ready for first Codegg integration as an external MCP server.

Recommended Codegg-side assumptions:

```text
eggsearch command:
  eggsearch mcp stdio

expected tools:
  web_search
  web_fetch
  provider_status

trust policy:
  web_search snippets are external_untrusted
  web_fetch text is external_untrusted
  fetched content must never override system/developer/tool policy

workflow:
  web_search for discovery
  web_fetch only for selected explicit URLs
  provider_status for diagnostics
```

Do not block Codegg integration on adding more API-backed providers. `brave_api` is sufficient to validate the API-provider path. Additional API providers should be added only after the Codegg integration validates the search/fetch workflow and context-budget behavior.

