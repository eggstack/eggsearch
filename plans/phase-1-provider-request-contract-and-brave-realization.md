# Phase 1 — Provider Request Contract and Brave Capability Realization

Status: planned
Depends on: none
Baseline: `e645a3fe42090fb7b7e1ce8639681fe69878f57b`
Roadmap: `plans/roadmap.md`

## Objective

Make the existing provider capability model executable instead of descriptive-only, and prove the new contract by completing the already-present Brave Search API adapter.

At phase completion, provider engines receive a provider-neutral structured search request; `web_search` can express exact date/domain/language/region constraints; Brave natively enforces the capabilities its API actually supports; and direct web fan-out plus multi-subquery dispatch use the same engine contract.

## Current implementation evidence

At the audited baseline:

- `src/core/query.rs::WebSearchRequest` exposes query, providers, safe search, timeout, intent, and coarse freshness.
- `src/core/provider.rs::ProviderCapabilities` already contains flags for safe search, freshness, language, region, domain filters, news, and result timestamps.
- `src/meta/engines/mod.rs::SearchEngine::search` accepts only `query`, `max_results`, and `timeout`.
- `src/meta/adapter.rs::web_search` calls that trait directly from a `JoinSet`.
- `src/meta/dispatch.rs::dispatch_parallel` calls the same trait for repo/research/security multiquery work.
- `src/meta/engines/brave_api.rs` currently sends only `q` and `count` to `/res/v1/web/search` and discards the response `age` field.
- the `brave_api` descriptor currently advertises the relevant web capabilities as false.
- `CapabilityEnforcementTelemetry` exists for repo-oriented routing but web search currently relies mainly on advisory warnings.

## External contract being implemented

Brave's current API documentation confirms:

- Web Search: `GET/POST /res/v1/web/search`;
- News Search: `GET/POST /res/v1/news/search`;
- `safesearch=off|moderate|strict`;
- freshness `pd|pw|pm|py` or exact `YYYY-MM-DDtoYYYY-MM-DD`;
- `country`, `search_lang`, and `ui_lang` targeting;
- `extra_snippets=true` for additional excerpts.

References:
- https://api-dashboard.search.brave.com/api-reference/web/search/get
- https://api-dashboard.search.brave.com/api-reference/news/news_search/get

This phase does not enable Brave summaries, Goggles DSL, local-place enrichment, or other rich-result surfaces.

## Non-goals

- Do not add Exa, Tavily, or Firecrawl in this phase.
- Do not add generated summaries or answer endpoints.
- Do not make any credentialed provider a default.
- Do not redesign repo/research/security public request types merely because the engine trait changes.
- Do not add provider-specific public request parameters.
- Do not change SourceCard stable-ID inputs.

## Invariants

1. Existing `web_search` calls deserialize and behave as before when all new fields are omitted.
2. The SearchEngine trait has one request path; do not preserve a legacy overload that lets direct and parallel dispatch drift.
3. Global/per-provider deadlines, panic recovery, health accounting, partial failures, and deterministic ordering retain current semantics.
4. Constraint validation happens before network dispatch.
5. A capability is reported as natively enforced only when the corresponding provider request actually carried an equivalent upstream parameter/endpoint selection.
6. Local post-filtering may guarantee output-domain membership, but it is not provider-native filtering and must not be reported as such.
7. The keyless default provider set remains unchanged.

## Production changes

### 1. Introduce an internal structured engine request

Add an eggsearch-owned request type under the engine/meta layer, for example `src/meta/engines/request.rs`:

```rust
pub struct EngineSearchRequest {
    pub query: String,
    pub max_results: usize,
    pub timeout: Duration,
    pub intent: SearchIntent,
    pub safe_search: Option<SafeSearch>,
    pub freshness: Freshness,
    pub date_range: Option<SearchDateRange>,
    pub include_domains: Vec<String>,
    pub exclude_domains: Vec<String>,
    pub language: Option<String>,
    pub region: Option<String>,
}
```

The exact ownership/lifetime shape may differ, but it must be safe to move into spawned tasks and remain dyn-compatible through `SearchEngine`.

Prefer:

```rust
fn search<'a>(
    &'a self,
    request: &'a EngineSearchRequest,
) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>>;
```

over adding more positional arguments.

Provide a minimal constructor for multiquery jobs that currently have only query/budget information. Avoid duplicating default semantics in every engine implementation.

### 2. Migrate every engine and both dispatch paths atomically

Update all `SearchEngine` implementations in `src/meta/engines/mod.rs` to read query/max-results/timeout from the structured request while initially ignoring unsupported optional fields.

Update:

- direct `MetadataSearchAdapter::web_search` fan-out in `src/meta/adapter.rs`;
- `DispatchJob`/`dispatch_parallel` in `src/meta/dispatch.rs`;
- test/mock engine implementations;
- any live-smoke helpers or doctor/probe code that invokes engine search.

Do not land a state where one dispatcher uses the new request and another still calls the positional trait.

### 3. Extend the public web-search constraint model

In `src/core/query.rs`, add provider-neutral optional fields:

- `date_range` with documented ISO `YYYY-MM-DD` start/end values;
- `include_domains`;
- `exclude_domains`;
- `language`;
- `region`.

Recommended validation rules:

- exact date range must contain both bounds, each parseable as a calendar date, with `start <= end`;
- exact `date_range` and non-`any` relative `freshness` are mutually exclusive to avoid provider-precedence ambiguity;
- domain entries are normalized lowercase hostnames only: reject schemes, ports, credentials, paths, query strings, fragments, empty labels, or wildcard syntax in this phase;
- cap domain list count and per-host length to prevent query/schema abuse; a reasonable initial hard cap is 32 entries per list and DNS-compatible hostname length;
- reject the same normalized hostname appearing in both include and exclude sets;
- language/region strings must be bounded and syntactically conservative; do not invent a full locale database in core. Document that provider support is best-effort unless capability-enforced.

Keep validation pure and unit-testable.

### 4. Carry constraints through the planner without provider leakage

`src/meta/planner.rs::SearchPlan` already owns generic and provider-specific query strings. Extend the plan or the adapter assembly step so all generic constraints survive planning and become an `EngineSearchRequest`.

Do not encode exact dates/language/safe-search into opaque query text when a selected provider has a native parameter. Query-string rewriting remains appropriate for existing code-host textual operators, not for portable web constraints.

### 5. Add deterministic local domain enforcement

Because not every provider supports native domain filters, enforce requested include/exclude domains on normalized result URLs after provider aggregation and before final truncation.

Requirements:

- filter against parsed hostnames, not substring matches;
- decide and document whether a filter entry matches only the exact host or the host plus subdomains. Prefer exact host plus subdomains, with label-boundary matching (`example.com` matches `docs.example.com` but not `notexample.com`);
- increase the pre-filter candidate pool by a bounded factor when domain filters are present so local filtering does not trivially starve final results; keep the existing configured `max_results_cap` as the absolute ceiling;
- if filtering yields fewer results, return fewer results rather than issuing unbounded follow-up searches;
- preserve retrieval-attempt evidence so zero returned cards is not treated as proof that providers had no candidates.

Represent local filtering as approximation/local enforcement in web capability telemetry; do not flip a provider's `supports_domain_filters` flag merely because the adapter post-filters it.

### 6. Extend capability-enforcement telemetry to web search

Reuse or extend `CapabilityEnforcementTelemetry` rather than inventing provider-specific warning strings for every new constraint.

For a web request, record requested capabilities and distinguish at least:

- provider-native enforcement;
- locally enforced/approximated behavior;
- not enforced.

The serialized response change must be additive. Existing warnings can remain for compatibility, but they should be derived from the same enforcement decision so telemetry and human-readable warnings cannot disagree.

### 7. Complete Brave Web Search parameter mapping

Refactor `src/meta/engines/brave_api.rs` to accept `EngineSearchRequest` and map:

- `SafeSearch` -> `safesearch`;
- `Freshness::{Day,Week,Month,Year}` -> `pd|pw|pm|py`;
- `SearchDateRange` -> Brave exact date syntax;
- language -> `search_lang`;
- region/country -> `country` when the generic value can be represented safely;
- `max_results` -> `count` under Brave's documented maximum.

If generic language/region cannot be represented exactly, omit rather than silently transforming to an unrelated locale.

Do not set `summary=true`.

### 8. Use Brave's dedicated News Search endpoint for news intent

When `SearchIntent::News`, dispatch `brave_api` to `/res/v1/news/search` rather than the generic web endpoint.

Keep the provider ID `brave_api`; endpoint selection is an implementation detail, not a new provider. Parse the news response into the same provider-neutral `SearchResult` model.

Tests must prove endpoint selection is intent-driven and that non-news requests still use the web endpoint.

### 9. Correct Brave provider capabilities

Update the `brave_api` descriptor in `src/core/provider.rs` only for capabilities actually implemented in this phase.

Expected native flags after implementation should include:

- safe search: true;
- freshness: true;
- language: true if mapped;
- region: true if mapped;
- news: true;
- domain filters: false unless a true provider-side implementation is added rather than local post-filtering.

Do not set `supports_result_timestamps` until phase 2 preserves usable result-level timestamps.

### 10. Keep provider config and docs synchronized

No new provider is added in this phase, but update:

- `docs/tool-matrix.md` for new web_search fields/semantics;
- `docs/provider-setup.md` for Brave capabilities;
- `docs/agent-workflows.md` where exact-domain/date examples improve agent guidance;
- architecture docs for the new engine request contract;
- schema/inventory snapshot tests if request schemas are asserted.

## Focused tests

Add deterministic tests for:

1. Engine request migration: a mock engine receives query, budgets, intent, and constraints unchanged.
2. `dispatch_parallel` uses the same structured request path and preserves deadline/panic accounting.
3. Date parsing accepts valid leap/calendar dates and rejects invalid/reversed ranges.
4. Relative freshness plus exact date range is rejected.
5. Domain normalization and exact/subdomain matching are correct; deceptive suffixes do not match.
6. Domain post-filtering occurs before final truncation and never exceeds the configured candidate cap.
7. Brave web request contains the expected safe-search/freshness/date/language/region parameters.
8. Brave news intent hits `/res/v1/news/search`.
9. Unsupported generic constraints are omitted rather than guessed.
10. Capability telemetry reports Brave-native vs local-domain enforcement correctly.
11. Existing request fixtures with no new fields still deserialize and preserve prior outputs.
12. Existing provider health/timeout tests continue to pass after the trait migration.

Use `httpmock` or the existing mock feature; no test in the normal suite may require Brave network access or credentials.

## Broad verification

Run the repository-prescribed broad gate on the exact candidate:

```bash
make check
```

If provider/schema snapshots are maintained outside that target, run the focused snapshot tests explicitly as well.

## Acceptance criteria

Phase 1 is complete only when:

- every SearchEngine implementation compiles against one structured request contract;
- both direct web fan-out and multiquery dispatch use it;
- `WebSearchRequest` supports validated exact date/domain/language/region constraints additively;
- Brave enforces safe search, relative/exact freshness, supported locale hints, and news intent through native API parameters/endpoints;
- domain output constraints are deterministic even for providers without native filters and are not mislabeled as native enforcement;
- Brave provider capabilities match the implementation;
- no generated Brave answer/summary path is reachable;
- keyless defaults are unchanged;
- documentation is synchronized;
- `make check` passes.

## Stop condition

Do not start phase 2 until this phase is closed. Excerpts and new provider adapters depend on a trustworthy request/capability contract; implementing them on the positional trait would create migration debt immediately.
