# Phase 2 — Extractive Evidence and Fetch/Cache Controls

Status: planned
Depends on: phase 1
Baseline for planning: `e645a3fe42090fb7b7e1ce8639681fe69878f57b`
Roadmap: `plans/roadmap.md`

## Objective

Add the common context-efficiency primitives needed by modern search providers without violating eggsearch's compact-discovery model:

1. preserve small, source-derived search excerpts and usable result timestamps;
2. add deterministic query-focused extraction to `web_fetch` over eggsearch's own document chunks;
3. expose the existing fetch cache policy and a caller-controlled maximum cache age.

This phase should make later Firecrawl/Exa/Tavily adapters useful without turning `web_search` into search-plus-scrape or introducing LLM-generated summaries.

## Current implementation evidence

At the audited baseline:

- `SearchResult` carries title, URL, one optional snippet, source engine, and specialist metadata; it has no generic timestamp or excerpt fields.
- `aggregate_rrf` deduplicates provider results before `convert_aggregated` produces compact `SourceCard` values.
- `SourceCard` is explicitly documented as discovery-only, with full-page retrieval delegated to `web_fetch`.
- `FetchDocument` already contains deterministic blocks/chunks and stable chunk IDs suitable for post-extraction ranking.
- `FetchCache` already has raw and derived tiers, validators, fetched timestamps, `CacheFreshness`, and LRU byte/entry limits.
- `FetchCachePolicy::{Default, Bypass, Refresh}` already exists in `src/core/fetch.rs`, but `WebFetchRequest` has no cache-policy field.
- fetch/browser/PDF paths already enforce SSRF, redirect, byte, character, and timeout policy and must remain authoritative.

## External capability rationale

Modern retrieval APIs converge on bounded source-derived passages rather than only one SERP description:

- Brave `extra_snippets` can return up to five alternate excerpts per result.
- Exa can return query-relevant `highlights` and `highlightScores`.
- Tavily returns one to three direct source chunks, each capped at 500 characters, for its normal search depths.
- Firecrawl Developer returns matched markdown `passages`.

References:
- https://api-dashboard.search.brave.com/app/documentation/web-search
- https://exa.ai/docs/reference/search
- https://docs.tavily.com/documentation/api-reference/endpoint/search
- https://docs.firecrawl.dev/features/developer

The common semantic is extractive evidence, not generated answer text.

## Non-goals

- Do not return full page bodies from `web_search`.
- Do not call an LLM for excerpt selection or focused fetch.
- Do not enable Brave/Exa/Tavily generated summaries.
- Do not add recursive subpage fetching.
- Do not add persistent/disk cache storage.
- Do not add `cache_only` in this phase.
- Do not weaken existing HTTP cache-control, Vary, profile-scope, or SSRF semantics.

## Invariants

1. Search excerpts are source-derived and remain `external_untrusted`.
2. Search output has hard per-result and aggregate excerpt bounds.
3. Optional timestamp/excerpt metadata must not alter existing SourceCard stable IDs.
4. Focused fetch is a deterministic projection of already-extracted document content; it never causes extra URL traversal.
5. Cache controls affect reuse/revalidation only; they never bypass target validation, redirect checks, origin concurrency/circuit breakers, browser-profile isolation, content limits, or sanitization.
6. Default fetch behavior is unchanged when new cache/focus fields are omitted.

## Production changes

### 1. Add provider-neutral extractive excerpt types

Introduce a compact type in core, for example:

```rust
pub struct SourceExcerpt {
    pub text: String,
    pub score: Option<f64>,
    pub provenance: ExcerptProvenance,
}

pub enum ExcerptProvenance {
    ProviderSnippet,
    ProviderHighlight,
    ProviderPassage,
}
```

Do not include provider-specific field names in the public enum. If provenance detail is unnecessary for callers, a smaller stable vocabulary is preferred over carrying opaque vendor metadata.

Recommended hard limits:

- maximum 3 excerpts per SourceCard;
- maximum 500 characters per excerpt;
- maximum 1,200 characters total excerpt text per card after sanitization/bounding.

These limits are intentionally below the upstream maxima. They should be constants with tests, not provider-specific configuration knobs.

### 2. Add optional excerpt demand to the internal engine request

Extend the phase-1 `EngineSearchRequest` with a bounded excerpt count or equivalent provider-neutral hint.

The public `WebSearchRequest` may expose an optional `excerpt_count` capped to the same small maximum. Default must be zero so existing search output remains compact.

Provider engines may still use a better source-derived passage as the primary `snippet` without explicit excerpt demand, but additional excerpts should not be fetched/emitted by default.

### 3. Preserve generic result timestamps

Add a provider-neutral timestamp field to `SearchResult`/aggregation metadata, for example `published_at: Option<String>` or a small typed `ResultTimestamp` structure if source semantics need distinction.

Requirements:

- accept only parseable RFC3339/ISO date evidence from providers;
- do not infer publication timestamps from arbitrary snippet text;
- RRF merge chooses deterministic timestamp evidence and does not let provider completion order decide;
- surface the timestamp additively in `SourceMetadata` (or another compact metadata structure), not as part of SourceCard identity;
- update freshness reranking to consume the generic timestamp before falling back to specialist issue/release metadata.

For Brave, parse only fields whose response semantics are documented/usable. Do not set `supports_result_timestamps=true` until tests prove a stable field is preserved and freshness reranking consumes it.

### 4. Merge excerpts deterministically during RRF aggregation

Extend `SearchResult`/`AggregatedResult` merge logic so excerpts from duplicate URLs are:

- sanitized/bounded through the normal untrusted-text path;
- deduplicated by normalized text;
- ordered deterministically, preferring explicit upstream relevance score when comparable within the same provider and otherwise preserving deterministic provider/rank order;
- truncated to the common hard limits.

Do not average or compare vendor relevance scores across providers unless the scale is demonstrably compatible. Cross-provider RRF remains the URL/card ranking mechanism.

All excerpt text must contribute to trust-marker/injection-marker accounting in the returned card/response.

### 5. Populate Brave alternate excerpts

When the request asks for excerpts, set Brave `extra_snippets=true` and convert the returned alternate excerpts into the provider-neutral type.

Do not request Brave summaries. Keep the ordinary `description` as the primary snippet unless a deterministic relevance rule establishes a better source excerpt.

### 6. Add deterministic query-focused fetch projection

Extend `WebFetchRequest` with optional focus controls, for example:

- `focus: Option<String>`;
- `focus_max_chunks: Option<usize>` with a low hard cap;
- optionally `focus_max_chars` if the existing `max_chars` cannot cleanly serve as the total focused-output budget.

Do not overload `extract_mode` with query semantics.

After the normal fetch/extraction/cache pipeline has produced a `FetchDocument`, rank its existing chunks against the focus query using deterministic local logic. A suitable first implementation is lexical and dependency-free:

- normalized token overlap;
- exact phrase boost;
- title/heading proximity boost;
- code-token/exact-symbol boost for code-like queries;
- modest adjacency expansion so a high-scoring chunk can retain immediately neighboring context;
- stable tie-break by original document order/chunk ID.

Avoid embeddings or model calls in this phase. The goal is reproducibility and token reduction, not semantic perfection.

Expose a bounded focus result additively in `WebFetchResponse`, for example:

```rust
pub struct FocusedFetchSelection {
    pub chunks: Vec<DocumentChunk>,
    pub truncated: bool,
    pub total_chars: usize,
}
```

Do not erase the ordinary extracted response fields unless an existing output budget requires it. If duplication would make MCP responses too large, choose one documented contract: when `focus` is present, the ordinary `text` field may be the assembled focused projection while the full derived document remains internal/cacheable. The implementation plan must keep one unambiguous behavior and test it.

### 7. Keep focus selection outside the raw cache key

Focus selection is a request-time projection of a fetched/extracted document. Do not create one raw-cache entry per focus query.

Preferred architecture:

1. obtain/revalidate raw cache entry under existing URL/scope rules;
2. obtain/build derived document under extraction parameters;
3. apply focus ranking to the derived chunks for this request;
4. bound/sanitize the focused response.

If derived cache currently stores only a response shape insufficient for focus ranking, minimally extend the cached derived document so stable chunks are available. Do not cache arbitrary focus strings unless profiling demonstrates a need.

### 8. Wire `FetchCachePolicy` into `WebFetchRequest`

Add:

```rust
pub cache_policy: Option<FetchCachePolicy>
```

with omitted/default preserving today's behavior.

Implement precise semantics:

- `default`: use a fresh eligible cache entry; otherwise revalidate/fetch according to current cache rules;
- `bypass`: do not read raw or derived cache for this request; network/browser fetch still may populate cache afterward unless origin response forbids storage;
- `refresh`: do not serve an entry solely because it is locally fresh; revalidate using validators when possible, otherwise fetch, then update cache.

Do not conflate `bypass` and `refresh`: refresh should retain conditional-request efficiency.

### 9. Add caller maximum cache age

Add an optional bounded field such as `max_cache_age_seconds` to `WebFetchRequest`.

Semantics:

- it is an upper bound on acceptable age for cache reuse, not a request to extend origin freshness;
- effective acceptable age is no greater than both origin/cache metadata and caller maximum;
- `0` forces revalidation/fetch behavior equivalent to “do not serve without checking freshness,” but it must not disable cache storage;
- it must not make `no-store`, `no-cache`, `private`, unsupported `Vary`, or profile-scope entries more reusable than current policy permits.

A `bypass` request ignores cached content regardless of max age.

### 10. Preserve cache status/telemetry clarity

If `WebFetchResponse` already exposes cache status, extend it only as needed so callers can distinguish cache hit, revalidated, refreshed/miss, and bypassed behavior without leaking internal keys.

Do not report `fresh` when a caller's stricter max-age caused revalidation.

### 11. Update batch fetch deliberately

`batch_fetch` currently normalizes web items and forwards a limited field set. Decide explicitly whether focus/cache controls are supported per web item in this phase.

Preferred approach:

- allow `cache_policy` and `max_cache_age_seconds` per web item because they affect retrieval semantics;
- defer `focus` in batch mode unless response-budget handling is clearly bounded and tested.

Document whichever choice is made. Do not silently accept and discard fields.

### 12. Documentation and CodeGG compatibility

Update `docs/tool-matrix.md`, `docs/safety.md`, `architecture/fetch.md`, and agent workflow examples for focused fetch/cache behavior.

CodeGG currently does not forward these new fields from its stable wrapper. That is acceptable for eggsearch closure. Record the additive contract so a later CodeGG plan can copy only the generic fields it wants to expose.

## Focused tests

Add deterministic tests for:

1. excerpt per-item/count/aggregate bounds;
2. excerpt sanitization, prompt-injection marker accounting, and text deduplication;
3. deterministic excerpt merge under permuted provider completion order;
4. timestamp parsing/merge and freshness-reranking use;
5. Brave only sends `extra_snippets=true` when requested;
6. focus scoring exact phrase, token overlap, code-like symbol, heading proximity, adjacency expansion, and stable tie-break behavior;
7. focus output never exceeds chunk/character caps;
8. focus projection does not cause additional URL fetches;
9. `cache_policy=default` preserves current fresh-hit behavior;
10. `bypass` skips reads but does not bypass URL/SSRF/origin policy;
11. `refresh` performs conditional revalidation when validators exist;
12. caller max-age can make an otherwise locally fresh entry require revalidation;
13. caller max-age cannot override origin `no-store`/`no-cache`/Vary/profile restrictions;
14. omitted new fields preserve existing response fixtures;
15. batch-fetch behavior for new cache fields is explicit and bounded.

Normal tests must use mocks/in-memory cache and require no network.

## Broad verification

```bash
make check
```

Also run focused feature combinations if cache/fetch tests are feature-gated, including no-default-features and all-features paths as required by `AGENTS.md`.

## Acceptance criteria

Phase 2 is complete only when:

- search results can carry a small provider-neutral bounded excerpt set without becoming fetched-page responses;
- generic result timestamps are preserved and used by freshness ranking where valid;
- every returned excerpt passes the existing sanitization/trust pipeline;
- `web_fetch` supports deterministic focused chunk selection with no additional traversal or model call;
- `FetchCachePolicy` is agent-visible and wired to actual cache behavior;
- caller maximum cache age is correctly enforced as a stricter upper bound;
- default/omitted behavior remains backward-compatible;
- CodeGG's existing calls continue to work unchanged;
- docs are synchronized;
- `make check` passes.

## Stop condition

Do not add provider-generated summaries or full search-result bodies to solve relevance gaps discovered during this phase. If deterministic focused selection proves inadequate, record evidence for a later design decision rather than expanding phase scope.
