# eggsearch Pre-Integration Cleanup Plan

## Context

`eggsearch` is close to ready for integration into Codegg as a lightweight MCP metasearch and bounded fetch server. The core architecture is now aligned with the intended use case: no local index, no crawler, no hard SearXNG dependency, no paid API requirement by default, and clear separation between `web_search` for discovery and `web_fetch` for explicit URL retrieval.

This pass is intentionally narrow. Do not add new search providers, new crawling behavior, local indexing, browser automation, or large architectural changes. The goal is to remove documentation/API inconsistencies, make behavior claims accurate, and verify the MCP contract before Codegg integration.

## Goals

1. Make public docs match the current implementation.
2. Make config naming around `default_max_results` and `max_results_cap` unambiguous.
3. Make `safe_search` behavior honest and non-misleading.
4. Remove or clearly mark no-op config fields before Codegg surfaces them.
5. Correct the README’s SSRF/DNS-rebinding wording to match the actual implementation.
6. Add small regression tests around the intended MCP tool surface.
7. Leave API provider expansion for after Codegg integration, except for preserving existing `brave_api` behavior.

## Non-Goals

Do not implement Tavily, Exa, Kagi, SerpAPI, Firecrawl, or other new API-backed providers in this pass.

Do not add Tantivy, SQLite FTS, persistent indexing, cached response databases, crawler behavior, or local corpus search.

Do not convert `web_fetch` into a browser-like tool. It should remain bounded single-URL HTTP(S) fetch plus text extraction.

Do not expose provider-specific raw responses in normal MCP output.

## Current State Summary

The current implementation appears broadly correct:

- `web_search`, `web_fetch`, and `provider_status` are exposed through MCP.
- `web_fetch` manually follows redirects and validates each redirect target before fetching.
- `web_fetch` enforces scheme, credential, host/IP, content-type, content-length, byte, timeout, and redirect limits.
- `max_results` is now a per-request value, while config owns `default_max_results` and `max_results_cap`.
- Old `search.max_results` is accepted as a compatibility alias for `default_max_results`.
- `brave_api` exists as an opt-in API-backed provider through environment-variable credentials.

The remaining issues are mainly stale docs and slightly overbroad claims.

## Task 1: Fix README Config Naming

### Problem

The README still shows config examples using:

```toml
[search]
max_results = 10
max_results_cap = 50
```

This obscures the intended distinction between default result count and hard result cap.

### Required Change

Update README examples to use:

```toml
[search]
default_max_results = 10
max_results_cap = 50
```

Add a short compatibility note:

```text
`search.max_results` is still accepted as a deprecated alias for `search.default_max_results` for older configs. New configs should use `default_max_results`.
```

### Acceptance Criteria

- README uses `default_max_results` in all primary examples.
- README explains that MCP request `max_results` is a per-call preference.
- README explains that `max_results_cap` is a server-side hard cap.
- README mentions the deprecated alias only as a migration note, not as the recommended spelling.

## Task 2: Update MCP Tool Surface Documentation

### Problem

Some documentation still describes the MCP surface as `web_search + provider_status`, omitting `web_fetch`.

### Required Change

Update all relevant docs/comments to list the current MCP tools:

```text
web_search
web_fetch
provider_status
```

Check at least:

- README project structure section.
- `src/lib.rs` crate-level docs.
- `src/mcp/mod.rs` docs.
- Any server initialization or instruction text that still says fetching should be done out of band.

### Acceptance Criteria

- Repository docs consistently describe all three MCP tools.
- Server initialization instructions tell agents to use `web_search` for discovery and `web_fetch` only for explicit URLs selected from search results or user input.
- No public doc says `web_fetch` is deferred or out of scope.

## Task 3: Correct `safe_search` Semantics

### Problem

The tool/schema currently accepts `safe_search`, but current HTML providers do not enforce it. Some comments imply it is mapped to providers, which overstates behavior.

### Required Change

Make the documentation precise:

- In request/core docs: describe `safe_search` as reserved/advisory unless the selected provider explicitly supports it.
- In MCP tool description: state that current HTML providers do not enforce safe search.
- Preserve existing warning behavior when a caller supplies `safe_search` and no selected provider can enforce it.

Suggested wording:

```text
Reserved for provider-specific safe-search support. Current HTML providers do not enforce this option; when supplied, eggsearch reports a warning unless a selected provider supports it.
```

### Acceptance Criteria

- No docstring says safe search is generally mapped/enforced by the adapter unless that is actually true.
- Tests still cover warning emission for unsupported `safe_search`.
- Existing API-provider capability machinery remains intact for future providers that can enforce safe search.

## Task 4: Clarify or Remove No-Op Config Fields

### Problem

`LiveConfig` contains fields such as `user_agent` and `respect_robots_txt` that are parsed but not currently used. If Codegg surfaces these settings, users may assume they are effective.

### Required Change

Pick one of two approaches.

Preferred approach for this pass: keep the fields but mark them as reserved/unused in docs and `doctor` output.

Alternative: remove them from public examples and leave them only as hidden serde-compatible fields if preserving config compatibility matters.

Do not implement full robots.txt handling in this pass.

### Acceptance Criteria

- README does not imply `user_agent` or `respect_robots_txt` currently affect behavior.
- `doctor` either does not display these as active protections, or labels them as reserved/not currently enforced.
- Code comments explain why these fields exist if retained.

## Task 5: Correct SSRF / DNS-Rebinding Wording

### Problem

The README currently claims DNS-rebinding-style attacks are mitigated by resolving up front and re-checking the connected address. The implementation validates the initial URL and each redirect target before request, but does not appear to verify the actual post-connect peer address.

### Required Change

Replace overbroad language with accurate wording.

Suggested README wording:

```text
eggsearch validates the initial URL and each redirected URL before fetching. It resolves hostnames before each request and rejects localhost, private, link-local, multicast, documentation, and other blocked address ranges unless explicitly allowed. This mitigates common SSRF and redirect-to-private-network cases, but it is not a complete DNS-rebinding defense because the post-connect peer address is not independently verified.
```

### Acceptance Criteria

- README no longer claims connected-address rechecking unless implemented.
- Security docs accurately distinguish pre-request DNS/IP validation from complete DNS-rebinding defense.
- Tests for redirect-to-private-network and credential-bearing redirects remain present or are added if missing.

## Task 6: Fix Stale Examples and Doctests

### Problem

At least one library example appears stale: `WebSearchRequest::validate` previously took additional arguments but now takes only `max_query_chars`.

### Required Change

Run:

```bash
cargo test
cargo test --doc
cargo clippy --all-targets -- -D warnings
```

Fix stale examples, doctests, and comments revealed by these commands.

### Acceptance Criteria

- `cargo test` passes.
- `cargo test --doc` passes, or doctests are intentionally disabled with a clear reason.
- `cargo clippy --all-targets -- -D warnings` passes, or the project documents why a specific lint is allowed.

## Task 7: Add MCP Tool Surface Regression Test

### Problem

The previous implementation had `web_fetch` in CLI but not MCP. Add a test to prevent recurrence.

### Required Change

Add a test that verifies the MCP server exposes exactly or at least the expected tools:

```text
web_search
web_fetch
provider_status
```

Depending on how MCP tool listing is implemented, this can be either:

- a unit test over the tool registry/helper, or
- an integration test against the MCP server over stdio/test transport.

Prefer the least brittle test that still catches accidental unregistration.

### Acceptance Criteria

- Test fails if `web_fetch` is removed from MCP registration.
- Test does not require live network.
- Test is included in normal `cargo test`.

## Task 8: Add MCP-Level `web_fetch` Test With Local HTTP Server

### Problem

Fetch logic is tested at lower levels, but Codegg will call it through MCP. Add at least one MCP-facing test to verify tool argument handling and response shape.

### Required Change

Use a local test HTTP server such as `wiremock`, `httpmock`, `axum` test server, or a minimal Tokio TCP listener.

Test case:

1. Start local HTTP server on loopback.
2. Configure fetch policy to allow private/localhost for test only.
3. Call MCP `web_fetch` with a URL served by the local server.
4. Assert response includes extracted text, final URL, content type, byte count or equivalent metadata, and `external_untrusted`/trust marker behavior.

### Acceptance Criteria

- Test does not require internet.
- Test explicitly enables localhost/private fetch only for the test config.
- Test fails if MCP `web_fetch` wiring breaks.
- Test verifies sanitized/framed output behavior at least minimally.

## Task 9: Keep API Provider Expansion Deferred

### Problem

There is a temptation to add several API-backed providers before Codegg integration. This would increase surface area and delay validating the actual harness workflow.

### Required Change

Do not add more API providers in this cleanup pass.

Preserve and lightly document the current `brave_api` provider:

- disabled by default.
- requires an environment variable credential.
- appears in `provider_status` with credential status but never prints the secret.
- can be enabled in config.

### Acceptance Criteria

- No Tavily/Exa/Kagi/SerpAPI/Firecrawl implementation in this pass.
- `brave_api` remains opt-in.
- README clearly states API-backed providers are optional and not required for default no-key use.

## Task 10: Pre-Codegg Integration Smoke Checklist

Before handing back for Codegg integration, verify:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --doc
cargo run -- --help
cargo run -- doctor
```

Also manually verify:

```bash
eggsearch mcp
```

can be launched by an MCP host and tool discovery includes:

```text
web_search
web_fetch
provider_status
```

If possible, run a local no-network MCP test path:

- `provider_status` works without network.
- `web_fetch` works against a local HTTP test server when localhost fetch is explicitly allowed.
- `web_search` validation rejects empty/overlong query without network.

## Expected Final State

After this pass, eggsearch should be ready for Codegg-side integration with the current provider set. The project should present a stable, narrow MCP contract:

```text
web_search       discovery only; returns compact source cards
web_fetch        bounded single-URL fetch/extract; returns untrusted text data
provider_status  diagnostic provider/config/capability status
```

The default search path remains no-key metasearch. `brave_api` remains an optional API-backed provider, but adding more API providers is deferred until after Codegg integration validates the workflow and identifies concrete result-quality gaps.

