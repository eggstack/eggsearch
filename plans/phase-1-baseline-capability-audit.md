# Phase 1: Baseline Preservation and Capability Audit

## Objective

Preserve eggsearch's current generic MCP search/fetch behavior while preparing the codebase for specialized codegg retrieval workflows. This phase is deliberately conservative. It should not add `repo_search`, `security_search`, or `research_search` yet. It should make the existing contract explicit, verify capability reporting, strengthen warnings when provider semantics are approximate, and add regression coverage so later phases do not quietly break generic search.

The end state is a stable baseline: `web_search`, `web_fetch`, and `provider_status` remain useful general-purpose tools, while codegg can inspect provider/tool capabilities before deciding whether to use generic fallback paths or future specialized tools.

## Non-goals

Do not introduce new specialized MCP tools in this phase.

Do not remove or rename existing request or response fields.

Do not change `web_search` from discovery-only behavior.

Do not make `web_fetch` crawl linked pages, execute JavaScript, or fetch more than one explicit URL.

Do not infer stronger provider guarantees than the provider can actually enforce.

## Relevant existing areas

Likely files to inspect and modify:

- `src/core/query.rs`
- `src/core/provider.rs`
- `src/core/result.rs`
- `src/core/source_card.rs`
- `src/core/config.rs`
- `src/meta/adapter.rs`
- `src/meta/planner.rs`
- `src/meta/response.rs`
- `src/meta/engines/*`
- `src/mcp/*`
- `src/fetch/*`
- `tests/*`
- `README.md`
- `CHANGELOG.md`

The current baseline already has `SearchIntent`, `Freshness`, `ProviderCapabilities`, RRF aggregation, provider status, search warnings, source-card metadata, and untrusted-content sanitization. This phase should harden those primitives and document their exact meaning.

## Workstream 1: Public contract documentation

Update documentation to define the stable public behavior of the existing tools.

For `web_search`, document that it:

- Performs live provider fan-out over selected or configured providers.
- Returns compact source cards only.
- Does not fetch full page bodies.
- Does not summarize or synthesize content.
- Treats all external result text as untrusted data.
- Preserves partial results when providers fail or time out.
- Applies intent/freshness only as retrieval/ranking hints, not as hard semantic guarantees unless provider support exists.

For `web_fetch`, document that it:

- Fetches one explicit HTTP(S) URL.
- Follows only bounded validated redirects.
- Does not crawl links.
- Does not execute JavaScript.
- Applies byte and character bounds.
- Preserves document structure where supported.
- Labels content as `external_untrusted`.

For `provider_status`, document that it:

- Reports known providers, enabled state, configured state, default state, provider kind, and capabilities.
- Is diagnostic/non-probing unless current implementation states otherwise.
- Must not claim capability support for fields that are merely approximated locally.

Add a compatibility section for codegg. It should describe the current fallback behavior that later phases will rely on:

- Generic search: call `web_search` with `intent = web`.
- Documentation search: call `web_search` with `intent = docs`.
- Code/repo fallback: call `web_search` with `intent = code` and repo hints.
- Security fallback: call `web_search` with `intent = security` and expect source cards, not normalized advisory facts.
- Research fallback: call `web_search` with `intent = web`/`docs`/`news` as appropriate, then explicitly fetch selected URLs.

## Workstream 2: Provider capability audit

Review all built-in provider descriptors in `src/core/provider.rs` and make capability flags exact.

Audit at least these capabilities for every known provider:

- `supports_safe_search`
- `supports_freshness`
- `supports_language`
- `supports_region`
- `supports_domain_filters`
- `supports_news`
- `supports_code_search`
- `supports_repo_filter`
- `supports_org_filter`
- `supports_path_filter`
- `supports_language_filter`
- `supports_symbol_hint`
- `supports_issue_search`
- `supports_release_search`
- `supports_result_timestamps`

Rules:

- If a provider accepts a native parameter and upstream enforces it, set the provider-side capability to true.
- If eggsearch only rewrites the query string, do not mark a provider-side filter capability as true.
- If structured timestamp fields are returned and used by eggsearch, set `supports_result_timestamps = true`.
- If a provider can return relevant results through generic text search but has no native semantics, leave the semantic capability false.

Special attention:

- HTML scrape providers should usually have conservative capability flags.
- `github_code`, `github_issues`, and `github_releases` should reflect the actual API implementation, not intended future behavior.
- `brave_api` should distinguish generic web search support from code/security/repo semantics.
- `searxng` should only claim configured features that the local adapter actually passes through.

## Workstream 3: Capability warning behavior

Improve warnings when a request asks for behavior that selected providers cannot enforce.

Add or harden warnings for these cases:

- `safe_search` supplied but selected providers do not enforce safe search.
- `freshness != any` supplied but no selected provider supports provider-side freshness and no returned result-level timestamps can be used.
- `intent = code` with only providers that lack native code/repo/path/language support.
- `intent = issues` with no selected issue provider.
- `intent = releases` with no selected release provider.
- `intent = security` with no advisory-native provider available, making the output only generic search results.
- Unknown provider IDs are rejected or surfaced consistently according to the current MCP boundary.

Warning language should be concise and machine-readable enough for codegg to display. Prefer structured warning records where available. Avoid long prose.

Do not block generic fallback by default. The goal is to make approximation visible, not to make generic search brittle.

## Workstream 4: Intent-neutral generic search regression tests

Add tests proving `intent = web` preserves generic search behavior.

Recommended test cases:

- `SearchIntent::Web` leaves the query trimmed but otherwise not expanded by repo/security/news-specific suffixes.
- `Freshness::Any` produces no freshness warning.
- `web_search` with mock engines returns source cards with expected title, URL, snippet, providers, score, trust, fetched=false, and metadata.
- RRF aggregation deduplicates normalized URLs and preserves provider lists.
- Candidate-pool expansion does not change final `max_results` semantics.
- Provider failures produce `providers_failed` and warnings without discarding successful provider results.
- Sanitization behavior remains stable for search result title/snippet fields.

## Workstream 5: Existing intent regression tests

Add focused tests for existing intent behavior so future specialized phases can change internals without breaking current fallback behavior.

Recommended test cases:

- `intent = docs` promotes official docs/package registry kinds but does not fetch content.
- `intent = code` promotes source repository/source file/package registry kinds.
- `intent = issues` promotes issue/PR kinds.
- `intent = releases` promotes release/tag kinds.
- `intent = security` promotes `SecurityAdvisory` kinds if detected.
- `intent = news` promotes news kinds if detected.
- Freshness boost only applies when result-level timestamp evidence exists.
- Rank reasons remain deterministic enum-like values rather than generated prose.

## Workstream 6: Provider status tests

Add tests for `provider_status` output. These should use both default/static provider descriptors and configured API-provider cases.

Recommended test cases:

- Known providers are all represented.
- Enabled providers are marked enabled.
- Default providers are marked default.
- SearXNG configured state reflects presence/absence of base URL.
- API providers are configured only when enabled and the configured env var resolves.
- Capability summaries match the detailed capability booleans.
- Unknown API provider IDs do not accidentally appear as known semantic providers unless explicitly supported.

## Workstream 7: Version/capability discovery preparation

Prepare the codebase for later codegg integration without necessarily adding new tools yet.

Options:

- Extend `provider_status` to include a stable `server_capabilities` object if that fits current architecture.
- Or add documentation and tests around current `provider_status` as the only capability discovery endpoint for this phase.

If adding `server_capabilities`, keep it conservative:

- `generic_search = true`
- `explicit_fetch = true`
- `repo_search = false`
- `security_search = false`
- `research_search = false`
- `document_fetch = true` if current `web_fetch.document` support is enabled in the build
- `pdf_fetch = true/false` depending on feature availability if this can be detected cleanly

Do not overfit this to codegg. It should be generally useful to any MCP client.

## Workstream 8: README and changelog updates

Update README to clarify that generic search remains first-class and specialized codegg workflows are planned layered improvements.

Update CHANGELOG with a concise unreleased entry:

- Capability reporting/audit hardening.
- More explicit warnings for approximate provider semantics.
- Regression coverage for generic search/fetch behavior.
- Compatibility groundwork for future specialized retrieval tools.

## Testing requirements

Run or add equivalent coverage for:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test
```

If live-network tests exist, they should remain opt-in. This phase should rely on deterministic mock-provider tests.

## Acceptance criteria

This phase is complete when:

- `web_search`, `web_fetch`, and `provider_status` are documented as stable baseline tools.
- Generic `intent = web` behavior is covered by regression tests.
- Existing non-web intent reranking behavior is covered by regression tests.
- Provider capability flags are audited and conservative.
- Requests for unsupported semantics produce visible warnings or documented fallback behavior.
- No new specialized tool has been introduced prematurely.
- Full local test suite passes.

## Handoff notes

Implement this phase before Phase 2. Later repo/security/research tools need a stable baseline and accurate provider capability model. The most important principle is to avoid turning provider query rewriting into a false claim of provider-side semantic support. If the server approximates a behavior through query terms or local reranking, surface that approximation to clients.
