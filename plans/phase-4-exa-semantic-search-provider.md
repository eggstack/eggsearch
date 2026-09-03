# Phase 4 — Exa Semantic Search Provider

Status: planned
Depends on: phases 1-2
Baseline for planning: `e645a3fe42090fb7b7e1ce8639681fe69878f57b`
Roadmap: `plans/roadmap.md`

## Objective

Add Exa as an opt-in credentialed search provider to give eggsearch a retrieval path that is materially different from its existing HTML/SERP sources: semantic/neural discovery with native date/domain constraints, publication timestamps, and source-derived highlights.

The integration must remain a search provider. It must not enable Exa's generated summaries, agent runs, output-schema generation, system prompts, subpage crawling, or full-text search-and-fetch behavior.

## Vendor contract being implemented

Research refreshed 2026-09-03 from:
- https://exa.ai/docs/reference/search

Current Exa Search endpoint:

`POST https://api.exa.ai/search`

Authentication:

`x-api-key: <EXA_API_KEY>`

Relevant request capabilities include:

- `query`;
- `numResults`;
- `includeDomains` / `excludeDomains`;
- `startPublishedDate` / `endPublishedDate`;
- `startCrawlDate` / `endCrawlDate`;
- search `type` including `auto`;
- optional content `highlights`;
- optional `livecrawl` and `maxAgeHours` when content retrieval is requested.

Relevant result fields include:

- title, URL, `publishedDate`, author;
- `highlights` and `highlightScores` when requested.

The same endpoint also exposes generated summary/output/context/systemPrompt/subpage features. Those are explicitly out of scope.

## Current eggsearch fit

After phases 1-2, eggsearch should have:

- structured provider-neutral search requests;
- exact date/domain constraints;
- generic result timestamps;
- bounded source excerpts;
- provider health/cooldown and deterministic RRF aggregation.

Exa therefore fits as a conventional new engine rather than a new MCP tool or workflow subsystem.

## Non-goals

- Do not call Exa Agent or Answer endpoints.
- Do not send `systemPrompt`, `outputSchema`, `additionalQueries`, or provider-generated summary requests.
- Do not request full result text by default.
- Do not request subpages.
- Do not use live crawl as a substitute for `web_fetch`.
- Do not expose Exa search-type/provider knobs through MCP in this phase.
- Do not add Exa to default providers or profiles automatically.

## Invariants

1. Exa is disabled unless explicitly configured with a credential.
2. Missing/invalid Exa credentials produce provider-scoped skip/failure state, not server-wide failure.
3. Exa results remain external untrusted evidence and pass the same sanitization path as every other remote provider.
4. Only source-derived highlights may become `SourceExcerpt`; generated summary/output fields are never requested or surfaced.
5. Publication timestamps are preserved only when parseable; they do not affect stable IDs.
6. Exact/relative date and domain semantics use the generic phase-1 request fields, not Exa-specific MCP options.

## Production changes

### 1. Register `exa` as a required-credential provider

Add `exa` to:

- `KNOWN_PROVIDER_IDS`;
- required API-provider credential inventory;
- provider descriptor builder;
- engine construction/registration;
- provider setup docs and inventory tests.

Recommended configuration:

```toml
[search.api.exa]
enabled = true
api_key_env = "EXA_API_KEY"
```

`base_url` may remain overridable through the existing API-provider config for tests/proxies, with the production default `https://api.exa.ai/search`.

Do not enable the provider by default.

### 2. Provider capability descriptor

Set capabilities only when implemented and tested.

Expected native capabilities:

- freshness/date filtering: true once generic relative/exact mapping is implemented;
- domain filters: true;
- result timestamps: true;
- safe search: do not claim unless Exa exposes and eggsearch maps a semantic equivalent; `moderation` is not automatically identical to the existing SafeSearch contract;
- language/region: only mark true if a generic field maps cleanly and is actually sent;
- news: do not claim solely because search can find news pages unless a native/category route with tested semantics is implemented;
- code/repo/issue/release: false.

If phase 2 adds an explicit excerpt/highlight capability, advertise it.

### 3. Implement `src/meta/engines/exa.rs`

Use the shared reqwest client and bounded body reader.

Send a minimal JSON body. Default request should resemble:

```json
{
  "query": "...",
  "numResults": 10,
  "type": "auto"
}
```

Add only generic constraint fields supported by the request:

- include/exclude domains;
- publication date start/end;
- optional bounded highlight request when excerpts are requested.

Keep these absent/false:

- summary;
- context;
- output schema;
- system prompt;
- subpages;
- full text.

Do not rely on undocumented defaults when an explicit `false`/omission is safer. Prefer a serialized request struct over ad hoc JSON values so tests can lock the outbound contract.

### 4. Map exact and relative freshness to publication dates

Eggsearch's generic exact date range maps naturally to Exa `startPublishedDate`/`endPublishedDate` as UTC boundaries.

For relative `Freshness::{Day,Week,Month,Year}`, compute a bounded publication-date lower bound using the request execution time and send the corresponding start date. Use one helper shared/tested for all exact-date API providers where possible.

Requirements:

- avoid local-timezone ambiguity; use UTC;
- define whether the end bound is omitted or set to the current timestamp and test the decision;
- do not use Exa crawl-date fields to represent publication freshness;
- do not advertise crawl-date filtering in the public generic API in this phase.

### 5. Map generic domain constraints natively

Send normalized phase-1 host lists to `includeDomains`/`excludeDomains`.

If Exa permits path-like entries but eggsearch's generic contract deliberately accepts hostnames only, preserve eggsearch's narrower semantics. Provider capability should mean the provider enforces eggsearch's documented domain rule, not every feature Exa supports.

The adapter's local post-filter may remain as defense-in-depth but capability telemetry should record native enforcement for Exa when the fields were sent successfully.

### 6. Parse and preserve `publishedDate`

Map parseable `publishedDate` into the generic result timestamp added in phase 2.

Do not synthesize timestamps when absent. Invalid timestamps should be ignored with bounded diagnostic behavior rather than failing the entire result set unless the provider response is structurally invalid.

Update `supports_result_timestamps` and freshness-reranking tests accordingly.

### 7. Request bounded highlights only when needed

When `EngineSearchRequest` asks for excerpts, request Exa highlights and convert:

- `highlights[i]` -> provider-neutral excerpt text;
- `highlightScores[i]` -> optional provider-local excerpt score when aligned.

Apply phase-2 count and character caps regardless of upstream response length.

Do not compare Exa highlight scores numerically against Brave/Tavily/Firecrawl scores. They may only order Exa's own excerpts before deterministic cross-provider merging.

If no excerpts are requested, avoid paid/extra content extraction when possible.

### 8. Keep live crawl out of the normal search path

Exa exposes `livecrawl` and `maxAgeHours` under content retrieval. Eggsearch already owns explicit URL retrieval through `web_fetch`, with SSRF, cache, browser, PDF, and trust controls.

Therefore:

- do not enable live crawl in normal Exa search;
- do not return Exa full text;
- do not route `web_fetch` through Exa in this phase.

If later evidence shows Exa's crawl cache is valuable as an optional fetch fallback, that requires a separate plan because it changes fetch provenance and safety semantics.

### 9. Error and quota handling

Exa documents 400, 401, 402, 429, and 5xx responses.

Map them into existing provider error classes without leaking response bodies unboundedly:

- 401 -> credential/auth failure;
- 402 -> quota/payment provider failure;
- 429 -> rate-limited so health cooldown uses the existing rate-limit path;
- 400 -> bounded provider/request error;
- 5xx/network -> transient provider failure.

If phase 3 introduced a reusable quota/payment error classification, reuse it. Otherwise avoid adding an Exa-specific public error type.

### 10. Profiles and routing

Register Exa so explicit provider selection works immediately once configured.

Do not add it to default providers. Do not modify built-in research/coding profile ordering in this phase unless a benchmark or live-smoke comparison demonstrates a clear benefit and the change is separately documented.

A later profile-tuning pass can use actual quality/latency evidence rather than vendor positioning.

### 11. Documentation

Update:

- `docs/provider-setup.md` with `EXA_API_KEY` configuration;
- provider inventory/count tests;
- `provider_status` capability examples;
- architecture engine inventory;
- `docs/agent-workflows.md` with optional semantic-provider selection examples if useful.

Explicitly state that eggsearch uses Exa only for search metadata/highlights and not Exa-generated summaries/agents.

## Focused tests

Use `httpmock`; normal tests require no Exa network/key.

Required cases:

1. missing required credential yields the correct non-routable skip code;
2. configured key sends `x-api-key` and never appears in diagnostics;
3. default request contains query/result count/type but no summary/context/text/subpages/systemPrompt/outputSchema;
4. exact date range maps to publication-date fields correctly;
5. relative freshness uses UTC and the expected bounded date window;
6. include/exclude domains map to native fields;
7. capability telemetry reports native domain/date enforcement;
8. `publishedDate` maps to generic timestamp and participates in freshness reranking;
9. invalid optional published date does not corrupt an otherwise valid result;
10. highlights are absent unless requested, then bounded/sanitized/deduplicated correctly;
11. highlight score ordering is provider-local and deterministic;
12. 401/402/429/5xx responses map to expected provider failure/health classes;
13. oversized JSON body is rejected by the shared response cap;
14. provider is never selected by defaults when merely configured unless the operator explicitly changes defaults/profile config;
15. docs/provider count snapshots are updated.

An ignored live-smoke test may validate one Exa search when `EXA_API_KEY` is set, but it is not part of routine CI.

## Broad verification

```bash
make check
```

## Acceptance criteria

Phase 4 is complete only when:

- `exa` is a normal opt-in credentialed engine under the existing provider/router/health system;
- exact/relative publication freshness and domain filters use the generic search contract and are enforced natively;
- parseable publication timestamps reach SourceCard metadata and freshness ranking;
- bounded Exa highlights integrate through the common excerpt type;
- no Exa-generated summary, output, agent, subpage, or full-text path is requested;
- missing credentials and quota/rate failures remain provider-local;
- defaults/keyless baseline are unchanged;
- documentation and inventory tests agree with implementation;
- `make check` passes.

## Stop condition

Do not expose provider-specific Exa tuning merely because the API supports it. If later benchmarking shows a stable need for search-type selection, model it as a provider-neutral quality/latency request in a separate plan rather than leaking Exa enums through MCP.
