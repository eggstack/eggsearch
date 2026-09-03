# Phase 5 — Tavily Search Provider and Workstream Closure

Status: planned
Depends on: phases 1-2; may execute after phase 4 or in parallel with phase 4 once shared primitives are stable
Baseline for planning: `e645a3fe42090fb7b7e1ce8639681fe69878f57b`
Roadmap: `plans/roadmap.md`

## Objective

Add Tavily as an opt-in credentialed search provider using the provider-neutral constraints and extractive-evidence primitives from phases 1-2, then perform a portfolio-level closure pass across Brave, Firecrawl Developer, Exa, and Tavily.

This phase is also the explicit decision gate for two researched but deferred capabilities: bounded site discovery and Firecrawl Research Index integration. The phase must record whether either has enough demonstrated value to justify a separate follow-on plan; it must not implement them opportunistically.

## Vendor contract being implemented

Research refreshed 2026-09-03 from:
- https://docs.tavily.com/documentation/api-reference/endpoint/search

Current Tavily Search endpoint:

`POST https://api.tavily.com/search`

Authentication:

`Authorization: Bearer <token>`

Relevant request capabilities include:

- `query`;
- `search_depth` (`advanced`, `basic`, `fast`, `ultra-fast`);
- `chunks_per_source` from 1 to 3 for `advanced`, `basic`, and `fast`;
- `max_results`;
- `topic`, including general/news-oriented routing;
- `time_range`, `start_date`, `end_date`;
- `include_domains`, `exclude_domains`, and include-domain filter/boost behavior;
- country and language controls;
- `safe_search`.

Tavily documents source chunks as direct source snippets of at most 500 characters each. It also exposes `include_answer`, `include_raw_content`, image, auto-parameter, and other features that are not part of this integration.

## Non-goals

- Do not enable Tavily-generated answers.
- Do not request Tavily raw page content; eggsearch `web_fetch` remains the fetch owner.
- Do not expose `search_depth`, `chunks_per_source`, `include_domains_mode`, or other Tavily-specific knobs through MCP.
- Do not add Tavily to default providers automatically.
- Do not implement Tavily Crawl/Map or other endpoint families.
- Do not implement Firecrawl Map/Research Index in this phase.

## Invariants

1. Tavily is opt-in and credentialed; missing credentials are provider-local.
2. Search-derived Tavily chunks are bounded excerpts, not fetched documents.
3. Generic domain/date/language/region/safe-search semantics are mapped only where a true upstream equivalent exists.
4. Generated answer and raw-content response fields remain disabled.
5. Provider defaults/keyless baseline and the existing MCP tool count remain unchanged during Tavily implementation.
6. Closure evidence is collected against the exact final candidate, not inferred from phase-local tests.

## Production changes

### 1. Register `tavily` as a required-credential provider

Add `tavily` to provider inventory, descriptor, required credential handling, construction, docs, and inventory tests.

Recommended configuration:

```toml
[search.api.tavily]
enabled = true
api_key_env = "TAVILY_API_KEY"
```

Allow the existing generic `base_url` override for deterministic HTTP-mock testing if consistent with other API providers. Production default: `https://api.tavily.com/search`.

Do not enable or select it by default.

### 2. Define provider capabilities conservatively

Expected capabilities when implemented:

- safe search: true;
- freshness/date filtering: true;
- language: true if `language`/filtering semantics are mapped exactly;
- region: true if the generic region value maps safely to Tavily country semantics;
- domain filters: true;
- news: true when `SearchIntent::News` maps to Tavily's news topic;
- result timestamps: only true if the response provides reliable per-result timestamps and eggsearch preserves them; do not infer from query dates;
- code/repo/issues/releases/security/scholarly: false unless a future dedicated mapping exists.

If phase 2 has an excerpt capability, Tavily supports it for `basic`/`fast`/`advanced` modes.

### 3. Implement `src/meta/engines/tavily.rs`

Use a typed request struct and the shared bounded HTTP-response reader.

Choose one stable provider-internal search depth for v1. Prefer `basic` as the quality/latency-balanced vendor mode unless focused benchmarking before implementation demonstrates a better default. Do not make this an agent-facing parameter in this phase.

Minimal request should explicitly keep generated/fetch-heavy features disabled:

```json
{
  "query": "...",
  "search_depth": "basic",
  "max_results": 10,
  "include_answer": false,
  "include_raw_content": false,
  "include_images": false,
  "auto_parameters": false
}
```

Add only mapped provider-neutral constraints.

### 4. Map generic date/freshness semantics

Map:

- exact eggsearch date range -> `start_date` / `end_date` in Tavily's documented date form;
- relative freshness -> `time_range` when a semantic equivalent exists.

Do not send both exact dates and relative time range after phase-1 validation has declared them mutually exclusive.

Tests should lock exact mapping and ensure unsupported values are omitted rather than guessed.

### 5. Map domain constraints natively

Send normalized host lists to `include_domains`/`exclude_domains`.

Use Tavily's strict filtering semantics, not `boost`, because eggsearch's generic include-domain contract means restrict results to the specified hosts. Do not surface `include_domains_mode` publicly.

Retain adapter local post-filtering as defense-in-depth if phase 1 established it, but report native enforcement through capability telemetry when Tavily receives the filter.

### 6. Map safe search, language, region, and news intent

- generic safe-search request -> Tavily `safe_search` only if the boolean Tavily control can represent the requested eggsearch mode without collapsing a meaningful three-state distinction incorrectly. If Tavily supports only on/off, document the mapping: e.g. `Off -> false`, `Moderate|Strict -> true`, and classify `Strict` as approximate unless vendor semantics prove equivalence;
- language -> Tavily language/filter control only under documented semantics;
- region/country -> Tavily country when the generic field is a supported country value;
- `SearchIntent::News` -> `topic="news"`; otherwise use general.

Capability telemetry must reflect exact vs approximate safe-search enforcement if modes differ.

### 7. Convert source chunks into bounded excerpts

For `basic` search, request only the number of chunks needed by the phase-2 excerpt demand, bounded to Tavily's 1-3 range.

Tavily returns source-derived chunk content. Convert it to provider-neutral excerpts while enforcing eggsearch's lower common caps.

If no additional excerpts are requested, use the smallest Tavily chunk count compatible with obtaining the ordinary result snippet and discard excess provider content before SourceCard construction.

Do not set `include_raw_content=true` to improve excerpts.

### 8. Keep answer/raw-content fields impossible by construction

Prefer typed request structs whose defaults/serialization omit or explicitly set:

- `include_answer=false`;
- `include_raw_content=false`;
- images/image descriptions/favicon unless there is an independently approved need;
- auto-parameter behavior false so provider-side query rewriting does not silently change eggsearch semantics.

Add a regression test that serializes a representative request and asserts forbidden fields are absent or false.

### 9. Error and health integration

Map authentication, quota/payment, rate-limit, validation, and 5xx/network failures through the common provider error path established by prior phases.

Do not parse arbitrary error bodies unboundedly. A Tavily failure must not suppress results from other selected engines.

### 10. Provider/profile policy

Explicit selection should work immediately after configuration.

Do not modify default providers. Profile inclusion, if considered, belongs in the closure comparison below and should only change when there is evidence that the provider adds enough unique quality/recall to justify extra latency/cost.

## Portfolio closure pass

After Tavily implementation, audit the complete workstream rather than treating five green phase tests as sufficient.

### A. Provider inventory and contract audit

Verify at the exact final commit:

- Brave API capabilities reflect implemented safe-search/freshness/language/region/news behavior;
- Firecrawl Developer is keyless-routable with optional credential enhancement and does not claim source-code search;
- Exa/Tavily are required-credential, disabled by default, and fail locally when not configured;
- provider counts in README/docs/architecture/tests agree;
- `provider_status` routability/skip codes/capabilities match actual construction behavior.

### B. Constraint matrix

Create or update a test matrix covering each generic web constraint across providers:

| Constraint | Brave API | Exa | Tavily | keyless HTML providers |
|---|---|---|---|---|
| relative freshness | native | native date lower bound | native time range | unsupported/local rerank only |
| exact date range | native | native publication dates | native start/end | unsupported |
| include/exclude domains | local unless a true Brave-native implementation was added | native | native | local post-filter |
| safe search | native | only if exact Exa mapping exists | native/approximate per mode semantics | existing support declaration |
| language | native | only if mapped | native if mapped | provider-dependent |
| region | native | only if mapped | native if mapped | provider-dependent |
| news intent | dedicated endpoint | generic unless explicit mapping added | native news topic | generic intent rerank |
| excerpts | Brave alternate snippets | Exa highlights | Tavily source chunks | ordinary snippet only |

The exact table in code/docs should reflect implementation reality, not this planning expectation.

### C. Determinism and budget audit

Test provider completion-order permutations and duplicate URLs so:

- RRF/card order is stable;
- excerpt order/deduplication is stable;
- aggregate excerpt limits hold;
- local domain filtering happens before final truncation under bounded candidate expansion;
- provider-specific result metadata never changes stable IDs;
- request/response byte and result caps remain in force.

### D. Trust and prompt-injection audit

Verify every new remote text field is sanitized:

- Brave alternate snippets;
- Firecrawl passages and any provider artifact labels;
- Exa highlights;
- Tavily chunks;
- provider error messages/scoped metadata that can reach MCP output.

No field sourced from a provider should become instruction-trusted because it is structured JSON.

### E. CodeGG compatibility audit

Against current CodeGG behavior:

- existing `websearch` forwarding of query/max-results/provider/intent/freshness/safe_search still works;
- existing `webfetch` calls still work with omitted focus/cache fields;
- raw eggsearch MCP tools remain additive from CodeGG's perspective;
- no provider-specific field is required from CodeGG to obtain baseline functionality.

If exposing new generic domain/date/focus/cache controls in CodeGG would materially improve agent behavior, write a separate CodeGG plan after eggsearch closes. Do not modify CodeGG as an unplanned side effect here.

### F. Live-smoke evidence

Normal CI remains network-free. For release/closure evidence, optional ignored smoke tests may be run when credentials are available for Brave/Exa/Tavily and keyless Firecrawl Developer.

A provider is not considered broken merely because a maintainer lacks credentials. Mock-contract tests are normative; live smoke is supporting evidence for upstream compatibility.

Record live-smoke date/provider/result succinctly if performed.

## Deferred-extension decision gate

Research found two plausible follow-on capabilities:

### Bounded site discovery

Firecrawl Map can return very large URL sets and supports subdomains/sitemap/cache behavior. Eggsearch's current safety contract emphasizes explicit bounded fetch and no crawling.

Only promote a `site_map`-style tool if implementation experience shows a recurring failure mode where search engines cannot locate a known site's relevant documentation pages and repo/docs link extraction is insufficient.

Any future plan must require:

- same-origin by default;
- hard URL count well below vendor limits;
- explicit timeout/aggregate response-size budget;
- no page-content retrieval;
- SSRF validation on seed and returned URLs;
- no automatic recursive fetch;
- separate MCP/tool-count compatibility review.

Reference: https://docs.firecrawl.dev/api-reference/endpoint/map

### Firecrawl Research Index

Firecrawl Research currently exposes ~43M paper abstracts dominated by PubMed/bioRxiv/medRxiv plus arXiv, paper passage reads, and related-paper modes (`similar`, `citers`, `references`). Eggsearch already has OpenAlex/Crossref/Semantic Scholar plus a research evidence workflow.

Promote this only if evidence shows one of these gaps remains material:

- biomedical/arXiv recall not met by current scholarly providers;
- query-focused paper passages materially reduce PDF fetch cost;
- citation-neighborhood expansion has enough agent value to justify new provider-neutral paper-followup types.

Do not squeeze passage reads/citation traversal into the ordinary `SearchEngine` trait without a provider-neutral contract.

Reference: https://docs.firecrawl.dev/features/research

At phase closure, update `plans/registry.md` with one of:

- `deferred — insufficient demonstrated need`;
- `follow-on plan required — <specific evidence>`.

## Focused Tavily tests

Use `httpmock`; no normal test requires a Tavily key.

Required cases:

1. missing credential -> expected non-routable skip code;
2. configured key -> correct bearer header, never leaked;
3. request explicitly disables answer/raw-content/auto-parameter behavior;
4. exact date and relative freshness mappings;
5. strict include/exclude domain mapping with no boost mode;
6. news intent -> Tavily news topic;
7. safe-search mode mapping and telemetry for any approximation;
8. language/region mapping only when representable;
9. chunk count bounded 1-3 and converted to common excerpts;
10. chunk/excerpt sanitization and deterministic deduplication;
11. auth/quota/rate-limit/5xx errors map to common provider health semantics;
12. provider remains absent from defaults unless operator configuration says otherwise;
13. provider/status/docs inventories update correctly.

## Broad and closure verification

Run on the exact final candidate:

```bash
make check
```

Also run focused mock-contract suites for Brave, Firecrawl Developer, Exa, Tavily, structured engine requests, excerpt aggregation, focused fetch/cache controls, and provider-status inventory.

If ignored live-smoke tests are run, record them separately; do not make them part of `make check`.

## Acceptance criteria

Phase 5 and the overall workstream are complete only when:

- Tavily is an opt-in credentialed provider using generic date/domain/news/language/region/safe-search semantics where exact or explicitly documented approximate mappings exist;
- Tavily source chunks become bounded excerpts and answer/raw content remain disabled;
- all four researched provider integrations/capability improvements share one provider-neutral request/evidence model;
- keyless defaults and existing MCP tools remain backward-compatible;
- provider capabilities/routability, docs, and inventory tests match reality;
- all new untrusted text is sanitized and bounded;
- deterministic aggregation/budget tests pass;
- current CodeGG wrapper requests remain compatible;
- `make check` passes on the exact final candidate;
- `plans/registry.md` records phase closure and the explicit decision for bounded site discovery and Firecrawl Research Index follow-on work.

## Stop condition

Once the acceptance criteria above are met, stop this workstream. Do not use the closure pass as authorization to add site mapping, recursive crawl, Firecrawl Research citation traversal, provider-generated answers, or CodeGG wrapper changes. Those require separate plans with their own acceptance criteria.
