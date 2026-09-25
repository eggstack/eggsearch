# Phase 3 — Firecrawl Developer Index Provider

Status: implemented
Depends on: phases 1-2
Baseline for planning: `e645a3fe42090fb7b7e1ce8639681fe69878f57b`
Roadmap: `plans/roadmap.md`

## Objective

Add Firecrawl's dedicated Developer Index as a specialist eggsearch provider for coding-agent evidence: issues, merged pull requests, repository READMEs, and curated documentation with matched source passages.

The integration should improve `repo_search` recall and primary-source discovery without pretending the Developer Index is source-code search, without coupling search to Firecrawl scraping, and without making an API key mandatory.

## Vendor contract being implemented

Research refreshed 2026-09-03 from:
- https://docs.firecrawl.dev/features/developer

Current documented behavior:

- dedicated `GET`/`POST https://api.firecrawl.dev/v2/search/developer`;
- natural-language `query`;
- `k` result count and `passages` matched passage count;
- result artifact kinds encoded by stable prefixes `doc:`, `issue:`, `pull_request:`, `readme:`;
- results carry URL and matched markdown passages; documentation titles may be absent;
- filters include `types`, `repos`, `sources`, `skills`, language/topic/license/star/archive/fork repository attributes;
- scoped `repos`/`sources` are echoed with `indexed` state so clients can distinguish an unindexed scope from a successful query with zero matches;
- no API key is required to get started; an Authorization bearer key raises rate limits;
- dedicated developer search costs credits per result batch, so requests must remain bounded.

The generic `/v2/search` `categories:["developer"]` surface is intentionally not the primary integration because it drops matched passages and richer index filters.

## Current eggsearch fit

Eggsearch already has:

- `repo_search` planners and grouped evidence roles;
- provider health/cooldown and bounded multiquery dispatch;
- `SourceCard` URL/source-kind classification for issues, PRs, repositories, and docs;
- phase-2 bounded extractive excerpts suitable for Firecrawl passages;
- retrieval-summary semantics designed to distinguish retrieval failure from evidence absence.

The main mismatch is optional authentication and Firecrawl's provider-level scope/index metadata.

## Non-goals

- Do not use Firecrawl Crawl, Scrape, Map, Interact, Agent, or Extract endpoints.
- Do not use general Firecrawl Search as another generic SERP provider in this phase.
- Do not advertise `supports_code_search`; the Developer Index does not provide arbitrary repository source-file search.
- Do not make Firecrawl part of default profiles until empirical quality/latency evidence justifies it.
- Do not require a Firecrawl key for provider construction.
- Do not surface Firecrawl credit/account metadata to agents unless needed for a stable provider error classification.

## Invariants

1. The keyless eggsearch baseline remains keyless; enabling the Firecrawl Developer provider can work without a credential under the vendor's documented allowance.
2. An optional configured key is read from an environment variable and never logged or serialized.
3. Every Firecrawl passage is treated as external untrusted content and bounded/sanitized before exposure.
4. Firecrawl artifact IDs are not allowed to replace eggsearch's stable-ID system; they may be preserved as provider metadata if useful.
5. Zero results from an explicitly scoped repository must not be conflated with “repository was indexed and contained no match” when the upstream says the scope is unindexed.
6. Provider failure remains local and participates in existing health/cooldown handling.

## Production changes

### 1. Add a provider ID and descriptor

Add `firecrawl_developer` to the built-in provider inventory.

Descriptor intent:

- provider kind: JSON API / remote search service;
- requires API key: false;
- enabled by default: false;
- default provider: false;
- native capabilities should reflect only implemented semantics.

Likely capability flags after implementation:

- issue search: true;
- repo filter: true when `repos` scope is used;
- result excerpts/query passages: if phase 2 introduces an explicit capability, true;
- code search: false;
- release search: false;
- scholarly search: false;
- repo indexing: do not reuse this flag unless its documented meaning matches Developer Index; it currently describes file-tree/symbol indexing and therefore should probably remain false.

Override `supports_role` conservatively so developer searches are selected for roles such as official documentation, repository README/primary project context, maintainer issue discussion, and pull-request/change evidence, but not arbitrary implementation/source-code roles.

### 2. Add clean optional-auth configuration

Do not force this provider through the existing “API provider means required credential” assumption.

Implement one reusable optional-credential path rather than a Firecrawl-only environment read buried in the engine. Acceptable designs include:

- extending provider credential metadata to `none | optional | required`;
- or adding a small optional-API-provider inventory used by config/provider construction.

The important semantics are:

- `[search.providers].firecrawl_developer = true` is sufficient for a routable keyless provider;
- an optional `[search.api.firecrawl_developer]` entry may name `FIRECRAWL_API_KEY` (or another env var) and attach the bearer header when present;
- a missing optional env var does not produce `missing_api_key` or make the provider unroutable;
- an explicitly empty/invalid configured optional credential should either fall back keyless with a precise warning or be treated as configuration-invalid. Pick one rule and test it; do not silently alternate.

Update `provider_status` so callers can understand routability without being told a key is required.

### 3. Implement a dedicated engine module

Add `src/meta/engines/firecrawl_developer.rs` using the shared reqwest client and bounded response-body reader.

Prefer POST JSON for filtering because array filters are first-class there.

Minimum request mapping:

- `query` <- engine request query;
- `k` <- bounded engine `max_results` under an eggsearch cap;
- `passages` <- bounded requested excerpt count, with a low default/maximum from phase 2;
- `types` derived from search intent/evidence role where safe;
- `repos` when an unambiguous `owner/repo` scope is available from `repo_search`;
- optional Authorization bearer header only when configured.

Do not forward repository-language/topic/license/star filters until eggsearch has provider-neutral public/internal equivalents. Do not add Firecrawl-specific fields to `RepoSearchRequest` solely to reach them.

### 4. Preserve repository scope structurally

The dedicated endpoint's `repos` filter is valuable and should be used when `repo_search` has unambiguous repository identity.

Avoid reparsing an already-rewritten opaque query if the structured repo hints are available earlier in the planner. Extend the phase-1 engine request or dispatch job with a small provider-neutral repository scope if necessary, for example owner/repo or normalized repository locator.

Do not make every engine parse `owner/repo` out of free text independently.

If this requires a small extension to the structured engine request, update all constructors with safe defaults and retain one dispatch contract.

### 5. Map artifact kinds conservatively

Convert Firecrawl artifact IDs/types as follows:

- `issue:` -> issue-thread evidence;
- `pull_request:` -> pull-request evidence;
- `readme:` -> repository root/readme/official project documentation as URL classification supports;
- `doc:` -> documentation/reference URL classification, falling back to URL heuristics.

Title fallback for doc results must be deterministic because upstream titles may be absent. Prefer a sanitized URL-derived label or URL itself over fabricated prose.

Preserve the upstream artifact ID in optional provider metadata only if it is useful for diagnostics or future follow-up. It must remain bounded and untrusted-data-safe.

### 6. Map matched passages into phase-2 excerpts

Each returned passage is markdown source evidence. Convert only the top bounded passages into `SourceExcerpt` values with passage provenance.

Requirements:

- apply the common excerpt count, per-excerpt, and total-character caps;
- preserve code/table markdown only within those caps;
- sanitize and injection-scan passage text exactly like other remote evidence;
- do not promote the passage into a fetched document or set `SourceCard.fetched=true`;
- the primary snippet may use the first matched passage when it is more useful than an absent/generic description, but keep that rule deterministic.

### 7. Preserve scope-index status without provider leakage into SourceCard

Firecrawl echoes requested `repos`/`sources` with `indexed` booleans. This is retrieval-state evidence, not source-card content.

Add a provider-neutral mechanism to preserve it at the response/retrieval layer. Preferred approaches:

- extend the internal engine response from bare `Vec<SearchResult>` to a small `EngineSearchBatch { results, retrieval_metadata }` if phase implementation shows this is the cleanest route;
- or attach provider execution metadata to dispatch output in a way that `repo_search` can translate into its existing retrieval summary/gaps.

Do not add `firecrawl_indexed: bool` to every SourceCard.

The final `repo_search` response should be able to distinguish at least:

- provider succeeded, scope indexed, zero query matches;
- provider succeeded, requested scope not indexed;
- provider request failed/timed out/rate-limited.

If introducing a new retrieval outcome is too invasive, emit a stable provider warning/attempt metadata field and document it; still avoid classifying “not indexed” as ordinary evidence absence.

### 8. Integrate with repo-search routing and profiles conservatively

Register the engine through the normal provider construction path and health registry.

Do not add it to default providers. Consider adding it to the built-in coding/research profile only if profile semantics allow disabled/unconfigured optional providers to be skipped cleanly and tests prove no surprising degradation. Otherwise document it as an opt-in provider and leave profile modification to a later evidence-based change.

When `repo_search` intent/subquery is source-code-specific and no issue/docs/README role is requested, capability partitioning should be allowed to skip Firecrawl rather than sending irrelevant calls.

### 9. Error classification

Map HTTP behavior into existing `EngineError` categories:

- 429 -> rate limited;
- 4xx auth/config responses -> HTTP/provider error with bounded message, never leaking key;
- 5xx/network -> normal transient provider failure;
- oversized/invalid JSON -> bounded parse failure.

Credit exhaustion/payment responses should be classified consistently and should not trigger global server failure. If current `EngineError` lacks a useful stable category, prefer a small generic “quota/payment” extension only if other provider phases can reuse it.

### 10. Documentation

Update:

- provider inventory/counts in `docs/provider-setup.md`, README, architecture docs, and tests;
- `provider_status` examples for keyless optional-auth state;
- `docs/agent-workflows.md` with a repo-search example where Developer Index finds the issue/PR behind a behavior;
- trust/safety docs to clarify that matched passages are still search-result evidence, not fetched/instruction-trusted content.

## Focused tests

Use `httpmock` and no live network for normal tests.

Required cases:

1. provider builds and is routable keyless when enabled;
2. optional API key adds the expected Authorization header and is never rendered in Debug/error output;
3. absent optional key does not produce `missing_api_key`;
4. result parsing handles all four artifact prefixes;
5. missing doc title uses deterministic fallback;
6. `owner/repo` scope maps to `repos` filter;
7. issue intent restricts types appropriately without pretending source code support;
8. passages respect common excerpt bounds and sanitization;
9. scoped `indexed=false` is preserved distinctly from zero matches;
10. 429 enters the normal rate-limit failure/health path;
11. oversized response body is rejected by the shared cap;
12. capability partitioning skips Developer Index for unsupported evidence roles;
13. default provider resolution remains unchanged when provider is disabled/unconfigured;
14. provider/status inventory tests reflect the new count.

Add one ignored/live-smoke test only if maintainers find it useful; it must support keyless operation and optional keyed operation without being required by `make check`.

## Broad verification

```bash
make check
```

## Acceptance criteria

Phase 3 is complete only when:

- `firecrawl_developer` is an opt-in built-in provider using the dedicated Developer Index endpoint;
- it is routable without an API key and optionally authenticated when configured;
- repo scope and artifact types are mapped through provider-neutral structures;
- matched passages become bounded/sanitized excerpts rather than fetched content;
- unindexed scope is not mislabeled as ordinary zero evidence;
- provider capability flags do not claim arbitrary code search;
- provider failures remain isolated under existing health/cooldown semantics;
- default/keyless installation behavior is unchanged;
- docs/provider inventories are synchronized;
- `make check` passes.

## Stop condition

Do not broaden this phase into Firecrawl general search, crawl, scrape, map, research, or agent integration. Those are separate product surfaces with different trust/budget characteristics.
