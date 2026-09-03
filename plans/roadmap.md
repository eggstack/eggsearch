# Search Capability Expansion Roadmap

Status: planned
Updated: 2026-09-03
Audited repository baseline: `e645a3fe42090fb7b7e1ce8639681fe69878f57b` (`eggsearch` 0.3.7)
Primary downstream consumer reviewed: `dbowm91/codegg` main

## Objective

Adopt the highest-value retrieval capabilities demonstrated by Brave Search, Firecrawl, Exa, and Tavily without changing eggsearch into an autonomous browsing/research agent.

The target architecture remains:

`discover -> rank -> select -> bounded fetch -> deterministic evidence handoff`

The work should improve query constraint enforcement, semantic/provider diversity, context efficiency, freshness handling, and coding-source discovery while preserving the existing trust model and keyless baseline.

## Current implementation evidence

The audited baseline already has most of the structural pieces required for this work:

- `src/core/query.rs` exposes `WebSearchRequest` with `query`, provider selection, `safe_search`, `intent`, and coarse `freshness`.
- `src/core/provider.rs` already models provider capabilities for safe search, freshness, language, region, domain filters, news, result timestamps, code/repository operations, scholarly search, and other specialist functions.
- `src/meta/engines/mod.rs` does not currently carry those capabilities into provider calls. `SearchEngine::search` accepts only `query`, `max_results`, and `timeout`.
- `src/meta/adapter.rs` performs direct `web_search` fan-out, RRF aggregation, sanitization, intent/freshness reranking, provider health accounting, and capability warnings.
- `src/meta/dispatch.rs` independently calls the same `SearchEngine::search` trait for multi-subquery `repo_search`, `research_search`, and `security_search`, so any trait migration must cover both dispatch paths atomically.
- `src/meta/engines/brave_api.rs` currently sends only `q` and `count`; its descriptor currently advertises none of Brave's richer search capabilities.
- `src/core/source_card.rs` intentionally defines `SourceCard` as compact discovery output rather than fetched page content.
- `src/core/fetch.rs` already defines `FetchCachePolicy::{Default, Bypass, Refresh}`, but the policy is not part of `WebFetchRequest` and is not an agent-visible control.
- `src/fetch/cache.rs` already maintains raw and derived caches with freshness metadata and validators.
- CodeGG's `src/search_backend/eggsearch.rs` translates stable agent-facing wrappers to eggsearch MCP calls and currently forwards `intent`, `freshness`, and `safe_search`. Additive eggsearch fields therefore remain backward-compatible, but they will require an explicit downstream wrapper update before CodeGG models can request them directly.

## External research baseline

Research was refreshed on 2026-09-03 from vendor documentation.

### Brave Search

References:
- https://api-dashboard.search.brave.com/api-reference/web/search/get
- https://api-dashboard.search.brave.com/api-reference/news/news_search/get
- https://api-dashboard.search.brave.com/app/documentation/web-search

Relevant capabilities:
- safe-search modes `off`, `moderate`, `strict`;
- relative freshness `pd`, `pw`, `pm`, `py` and exact `YYYY-MM-DDtoYYYY-MM-DD` ranges;
- country and search-language targeting;
- dedicated `/v1/news/search` index;
- up to five `extra_snippets` per result;
- Goggles and generated summary/context products also exist, but raw provider DSL and generated answers are intentionally out of scope here.

### Firecrawl

References:
- https://docs.firecrawl.dev/features/developer
- https://docs.firecrawl.dev/features/search
- https://docs.firecrawl.dev/features/research
- https://docs.firecrawl.dev/api-reference/endpoint/map

Relevant capabilities:
- Developer Index dedicated to issues, merged pull requests, READMEs, and curated documentation, with matched markdown passages and stable artifact IDs;
- Developer Index supports repository/source/type filters and echoes scoped repositories/sources with an `indexed` flag, allowing callers to distinguish an unindexed scope from a successful zero-result query;
- Developer Index is usable without an API key at baseline; a key raises rate limits;
- general Search supports domain and time filters, but search-plus-scrape is not a good fit for eggsearch's separation of discovery and fetch;
- Research Index provides a separate paper corpus with passage reads and citation-neighborhood expansion; useful, but outside this workstream's initial provider scope;
- Map can discover very large URL sets, which conflicts with eggsearch's bounded-explicit-fetch posture unless wrapped in a much stricter future tool.

### Exa

Reference:
- https://exa.ai/docs/reference/search

Relevant capabilities:
- semantic/neural web retrieval through a single search endpoint;
- include/exclude domain constraints;
- published-date and crawl-date ranges;
- `publishedDate` result metadata;
- query-relevant `highlights` with scores;
- optional live crawl, summaries, output schemas, and agent features exist but should not be enabled by eggsearch's provider adapter.

### Tavily

Reference:
- https://docs.tavily.com/documentation/api-reference/endpoint/search

Relevant capabilities:
- domain filters, exact start/end dates, relative time windows, language/country controls, safe search, and news topic routing;
- `basic`, `fast`, and `advanced` search depths return one to three source-derived chunks, each bounded to 500 characters;
- generated answers and raw-content retrieval are optional and should remain disabled in eggsearch.

## Architectural decisions

### 1. Make the engine request provider-neutral and capability-bearing

Introduce an eggsearch-owned internal request object and migrate `SearchEngine::search` to accept it. The object should carry only provider-neutral semantics: query text, result/timeout budgets, intent, safe-search request, relative or exact freshness, domain constraints, language/region hints, and bounded excerpt intent as later phases require it.

Do not expose Brave, Exa, Tavily, or Firecrawl parameter names through MCP.

The migration must cover direct `web_search` fan-out and `dispatch_parallel` together. Multi-subquery workflows may initially construct a minimal request containing only query/budgets when their public request types do not expose the new constraints.

### 2. Exact search constraints belong in `WebSearchRequest`

Add provider-neutral fields for:
- exact date range;
- include domains;
- exclude domains;
- language;
- region/country.

Validation must be deterministic and conservative. Date strings use one documented ISO form. Host filters are hostnames, not arbitrary URL prefixes. Contradictory inputs must fail rather than relying on provider precedence.

Provider-native enforcement should be used when available. Deterministic post-filtering may be used for domain constraints when a provider lacks a native filter, but it must be represented as approximation/local enforcement rather than falsely advertised as provider-native filtering.

### 3. Prove the abstraction with Brave before adding new providers

Brave is already implemented and therefore provides the lowest-risk vertical slice. Phase 1 completes safe search, relative/exact freshness, language/region targeting, dedicated news routing, and capability descriptors before Exa/Tavily are added.

### 4. Keep search cards compact

Do not turn `SourceCard` into fetched page content. Provider highlights/passages may be preserved only as a small, bounded set of extractive excerpts with explicit provenance and hard per-result/response caps.

Query-focused full-page retrieval belongs in `web_fetch`, implemented as deterministic chunk selection over eggsearch's own extracted `FetchDocument` rather than a provider-generated summary.

### 5. Preserve the keyless baseline

Exa and Tavily are optional credentialed providers and remain disabled unless explicitly configured. Firecrawl Developer should preserve its vendor-supported keyless path; an optional credential may raise limits but must not become required for provider construction.

No provider added by this work may become a default provider automatically. Operators opt in.

### 6. Do not import nested agency

The following stay out of scope:
- Brave Answers or generated summaries;
- Exa Agent/deep-reasoning/output-schema/system-prompt behavior;
- Tavily `include_answer` or raw page retrieval as part of search;
- Firecrawl Agent, Interact, Crawl, or search-and-scrape coupling.

CodeGG is already the orchestration agent. Eggsearch should remain a retrieval/evidence service with inspectable behavior.

## Cross-phase invariants

1. Existing ten MCP tools remain backward-compatible unless a later plan explicitly introduces a versioned contract change.
2. Existing requests that omit new fields retain current behavior.
3. No API key is required for the default installation.
4. Untrusted provider text passes through the existing sanitization/trust path; excerpts are not instruction-trusted.
5. All network reads remain bounded by byte, result, and timeout limits.
6. Provider failure isolation, health/cooldown behavior, partial results, retrieval-attempt accounting, and deterministic result ordering remain intact.
7. Stable IDs must not change merely because optional timestamp/excerpt metadata is added.
8. Normal tests are network-free. Provider HTTP behavior is exercised with mocks; real-provider checks are ignored/live-smoke only.
9. `make check` is the broad closure gate for every phase.
10. Documentation counts and provider inventories must be updated whenever provider/tool counts change.

## Ordered phases

### Phase 1 — Provider request contract and Brave capability realization

Refactor the `SearchEngine` request contract, add the generic search constraints, migrate both dispatch paths, extend web capability-enforcement telemetry, and fully exercise the existing Brave API adapter. This is the prerequisite for every later provider phase.

### Phase 2 — Extractive evidence and fetch/cache controls

Add bounded provider excerpts/timestamps, deterministic query-focused fetch projections, and wire the existing cache policy plus caller maximum-cache-age semantics into `web_fetch`. This phase creates the common result/fetch primitives consumed by Firecrawl, Exa, and Tavily.

### Phase 3 — Firecrawl Developer Index

Add a specialist `firecrawl_developer` provider oriented to `repo_search`/developer evidence. Preserve matched passages and scoped-index status under bounded provider-neutral types. Support keyless use with optional authentication enhancement.

### Phase 4 — Exa semantic search provider

Add opt-in Exa search as a semantic retrieval source. Use only retrieval, date/domain constraints, result timestamps, and bounded highlights. Keep generated summaries, full text, subpage crawling, and agent features disabled.

### Phase 5 — Tavily provider and workstream closure

Add opt-in Tavily search with provider-neutral constraints and bounded source chunks, then run a portfolio-level compatibility/closure pass. Record whether bounded site discovery or Firecrawl Research Index work has enough demonstrated value to justify a separate follow-on plan.

## Cross-cutting verification

Every phase should include focused unit/HTTP-mock tests plus the repository broad gate. The final closure pass should additionally verify:

- `provider_status` accurately reports new provider capability and routability state;
- missing Exa/Tavily credentials degrade provider-locally and never make the server unhealthy;
- Firecrawl Developer operates without a key in construction/config tests and optionally attaches a bearer token when configured;
- domain/date/safe-search/language/region constraints are never claimed as natively enforced when they were not sent upstream;
- search result and excerpt aggregation remains deterministic under provider completion-order variation;
- sanitization and injection-marker accounting includes every excerpt returned to callers;
- `web_fetch` cache bypass/refresh/max-age semantics do not bypass SSRF, redirect, browser-profile, origin-control, or content-size policy;
- CodeGG's existing wrapper requests continue to work unchanged.

## CodeGG handoff implications

No CodeGG change is required to land provider-internal improvements or optional providers in eggsearch. However, CodeGG currently forwards only `intent`, `freshness`, and `safe_search` for web search and does not forward focus/cache controls for fetch. After phases 1-2 stabilize, a separate CodeGG integration plan may expose selected generic controls through its stable wrappers.

Do not add provider-specific Exa/Tavily/Firecrawl knobs to CodeGG. The point of this work is to keep those details behind eggsearch's provider-neutral contract.

## Workstream acceptance and stop condition

Stop when phases 1-5 meet their individual acceptance criteria, `make check` passes on the exact final candidate, documentation and provider inventories match reality, the keyless baseline remains intact, and `registry.md` records all five phases as implemented or explicitly superseded.

Do not implement crawl, browser actions, provider-generated answers, a broad site mapper, or Firecrawl citation-graph operations as opportunistic additions. If implementation evidence justifies one of those extensions, write a new plan after this workstream closes.
