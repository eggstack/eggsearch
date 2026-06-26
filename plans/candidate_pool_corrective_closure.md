# Candidate Pool Corrective Closure Plan

## Context

The agent-facing tool surface cleanup is now mostly aligned with the intended Codegg boundary. The latest implementation fixed several items from `plans/agent_tool_surface_corrective_closure.md`:

- `SearchIntent` and `Freshness` now support controlled weak-model aliases.
- `FreshnessMatch` is no longer emitted without date evidence.
- MCP initialization no longer says crawling is conditionally allowed.
- README fetch-safety wording again states no JavaScript and no crawling.

However, one important correctness gap remains in the candidate-pool reranking implementation. The code computes a larger candidate pool after provider results are returned, but each provider is still asked for only the final `max_results`. In practice, real providers may never return enough candidates for intent-aware reranking to rescue a docs/security/release result just outside the final window.

There is also a safety issue in the current hard-coded candidate-pool cap: `candidate_pool_size(final_max_results)` uses `.clamp(final_max_results, 50)`, which can panic when `final_max_results > 50`. This conflicts with the server-configured cap model and should be removed.

This plan is intentionally narrow. It closes the remaining candidate-pool bug and cleans up tests so future regressions are caught accurately.

## Goals

1. Make provider fan-out request the candidate limit, not the final return limit.
2. Make candidate-pool sizing config-aware and impossible to panic for large `effective_max_results`.
3. Preserve final response truncation to the caller's effective `max_results`.
4. Fix tests so mock engines respect the `max_results` argument and catch the original bug.
5. Keep `eggsearch` a bounded retrieval substrate. Do not add research-agent behavior.
6. Keep freshness behavior honest: no `FreshnessMatch` without date evidence.

## Non-goals

Do not add a research workflow tool.

Do not add `include_content` to `web_search`.

Do not fetch pages inside `web_search`.

Do not add browser automation, JavaScript execution, crawling, recursive link following, caching, or summarization.

Do not add provider-specific date extraction in this pass.

Do not expand provider support in this pass.

## Current bug summary

Current high-level flow in `MetadataSearchAdapter::web_search`:

```rust
let max_results = effective_max_results;

for engine in &engines {
    join_set.spawn(async move {
        let result = engine.search(&query, max_results, engine_timeout).await;
        (engine.name().to_string(), result)
    });
}

let candidate_limit = candidate_pool_size(max_results);
let aggregated = aggregate_rrf(raw_results.clone(), candidate_limit);
apply_intent_reranking(&mut results, req.intent, req.freshness);
results.truncate(max_results);
```

The second half is correct in principle: aggregate a candidate pool, rerank, then truncate.

The first half is wrong: each provider is still called with `max_results`, so there may be no candidate pool larger than the final result count. The current regression test does not catch this because its mock engine ignores the `_max_results` argument and returns all results anyway.

## Required behavior

For a request with final return count `effective_max_results`, the adapter must:

1. compute a safe `candidate_limit` greater than or equal to `effective_max_results`;
2. pass `candidate_limit` to each provider's `search` call;
3. aggregate up to `candidate_limit` compact results;
4. attach deterministic metadata;
5. apply intent-aware reranking;
6. truncate final output to `effective_max_results`;
7. return no more than `effective_max_results` cards.

No page bodies are fetched. The extra candidate pool only changes how many compact search results are requested from providers.

## Candidate-pool sizing design

### Problem with current cap

Current helper:

```rust
fn candidate_pool_size(final_max_results: usize) -> usize {
    final_max_results
        .saturating_mul(3)
        .clamp(final_max_results, 50)
}
```

This is unsafe because `usize::clamp(min, max)` panics if `min > max`. If `final_max_results` is ever greater than 50, this becomes `.clamp(60, 50)` and panics.

It also hardcodes 50 rather than respecting the configured `max_results_cap`.

### Recommended helper

Use a helper that accepts the configured cap or an explicit candidate cap:

```rust
fn candidate_pool_size(final_max_results: usize, candidate_cap: usize) -> usize {
    if final_max_results == 0 {
        return 0;
    }

    let desired = final_max_results.saturating_mul(3);
    desired
        .max(final_max_results)
        .min(candidate_cap.max(final_max_results))
}
```

This guarantees:

- never less than `final_max_results`;
- never panics when `final_max_results > candidate_cap`;
- normally returns `min(final_max_results * 3, candidate_cap)`;
- if config permits a final count larger than the candidate cap, the final count wins so the candidate pool is at least the final window.

### Where should `candidate_cap` come from?

Preferred near-term implementation:

- Add a `candidate_results_cap` field to `MetadataSearchAdapter` initialized from search config.
- If adding config is too large for this pass, pass the already-configured `search.max_results_cap` into `web_search` alongside `effective_max_results`.

The cleanest API may be:

```rust
pub async fn web_search(
    &self,
    req: &WebSearchRequest,
    effective_max_results: usize,
    max_results_cap: usize,
) -> WebSearchResponse
```

Then:

```rust
let candidate_limit = candidate_pool_size(effective_max_results, max_results_cap);
```

Alternatively, store `max_results_cap` inside `MetadataSearchAdapter` at construction time. That avoids changing the adapter API but requires wiring config through adapter construction and mock constructors.

Given this repo already computes `resolution` in `run_web_search`, passing `state.config.search.max_results_cap` into `adapter.web_search` is likely the smaller change.

### Acceptance criteria

- `candidate_pool_size(1, 50) == 3`
- `candidate_pool_size(5, 50) == 15`
- `candidate_pool_size(10, 50) == 30`
- `candidate_pool_size(20, 50) == 50`
- `candidate_pool_size(50, 50) == 50`
- `candidate_pool_size(60, 50) == 60`, no panic
- `candidate_pool_size(100, 50) == 100`, no panic
- `candidate_pool_size(0, 50) == 0` if the helper supports zero; production validation should still prevent zero final results.

## Adapter flow changes

### Required change

Compute `candidate_limit` before spawning provider tasks and pass it to `engine.search`:

```rust
let final_max_results = effective_max_results;
let candidate_limit = candidate_pool_size(final_max_results, max_results_cap);

for engine in &engines {
    let query = req.query.clone();
    let engine_timeout = effective_timeout;
    let per_provider_limit = candidate_limit;
    join_set.spawn(async move {
        let result = engine.search(&query, per_provider_limit, engine_timeout).await;
        (engine.name().to_string(), result)
    });
}

let aggregated = aggregate_rrf(raw_results.clone(), candidate_limit);
let mut results = convert_and_sanitize(...);
apply_intent_reranking(&mut results, req.intent, req.freshness);
results.truncate(final_max_results);
```

### Important naming cleanup

Avoid using `max_results` ambiguously for both candidate and final result counts. Use explicit variable names:

```rust
let final_max_results = effective_max_results;
let candidate_limit = candidate_pool_size(final_max_results, max_results_cap);
```

Then update logging:

```rust
debug!(
    query = %req.query,
    providers = ?queried_ids,
    final_max_results,
    candidate_limit,
    timeout_ms = effective_timeout.as_millis(),
    "dispatching metasearch"
);
```

### Acceptance criteria

- Providers receive `candidate_limit`, not `final_max_results`.
- Final response truncation still uses `final_max_results`.
- Logs distinguish candidate limit from final return limit.
- `web_search` remains discovery-only.

## Test corrections

### Problem with current mock

The mock engine currently ignores its `_max_results` parameter:

```rust
fn search(..., _max_results: usize, ...) -> BoxFuture<...> {
    let results = self.results.clone();
    Box::pin(async move { Ok(results) })
}
```

This makes the candidate-pool regression test pass even if production providers still receive the wrong limit.

### Required mock behavior

Update mock engine to respect `max_results`:

```rust
fn search(..., max_results: usize, ...) -> BoxFuture<...> {
    let mut results = self.results.clone();
    results.truncate(max_results);
    Box::pin(async move { Ok(results) })
}
```

This makes tests reflect real provider behavior.

### Candidate-pool regression test

Keep the existing concept but make it fail under the old bug:

- mock engine has three results;
- result 1 is generic/unknown;
- result 2 is official docs;
- final max results is 1;
- candidate limit should be 3;
- provider receives 3, so docs result is present in candidate pool;
- docs intent reranking promotes docs result into final result;
- final response length is exactly 1.

If provider fan-out passes `final_max_results` instead of `candidate_limit`, the mock will truncate to 1 before aggregation and the test will fail.

### Add explicit provider-limit test

Add a test-only mock engine that records the `max_results` it was called with. For example:

```rust
struct RecordingMockEngine {
    name: &'static str,
    results: Vec<SearchResult>,
    seen_limit: Arc<Mutex<Option<usize>>>,
}
```

Test:

- call `adapter.web_search(&req, 2, 50)` or equivalent;
- assert provider saw 6, not 2;
- assert final response length is at most 2.

If changing the adapter signature is undesirable in tests, use whatever production signature exists after the implementation.

### Candidate-pool helper tests

Update existing `candidate_pool_size_scales_by_three` test to cover the config cap and no-panic cases:

```rust
assert_eq!(candidate_pool_size(1, 50), 3);
assert_eq!(candidate_pool_size(5, 50), 15);
assert_eq!(candidate_pool_size(10, 50), 30);
assert_eq!(candidate_pool_size(20, 50), 50);
assert_eq!(candidate_pool_size(50, 50), 50);
assert_eq!(candidate_pool_size(60, 50), 60);
assert_eq!(candidate_pool_size(100, 50), 100);
```

Also add a small-cap case:

```rust
assert_eq!(candidate_pool_size(5, 8), 8);
assert_eq!(candidate_pool_size(10, 8), 10);
```

### Neutral ordering test

Keep the neutral `SearchIntent::Web` test. Ensure it still passes after the mock starts respecting `max_results`.

### Freshness test

Keep the news/freshness test. Ensure no `FreshnessMatch` is emitted.

## Integration surface updates

If `MetadataSearchAdapter::web_search` changes signature, update all call sites.

Likely call sites:

- `src/mcp/tools.rs`
- adapter unit tests
- integration tests under `tests/`
- any CLI path that directly invokes the adapter

In `run_web_search`, pass the configured cap:

```rust
let resp = state
    .adapter
    .web_search(&req, resolution.effective, state.config.search.max_results_cap)
    .await;
```

If the adapter instead stores the candidate cap internally, ensure mock constructors and production constructors initialize it consistently.

## Documentation cleanup

Update AGENTS.md and any developer docs to clarify the candidate flow:

```text
Provider fan-out requests a bounded candidate pool larger than the final return count. RRF aggregation, deterministic metadata classification, and intent-aware reranking operate on that candidate pool. The response is then truncated to the caller's effective max_results.
```

Do not over-document internal helper constants in the README. README should stay focused on user-facing tool behavior.

If any README wording mentions the internal candidate cap, avoid hardcoding values unless they are configurable and stable.

## Validation commands

Run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

If no GitHub Actions workflows exist, record local command output in the commit message or follow-up note.

If clippy cannot run with all features due to repo-specific constraints, record the exact successful command and the reason.

## Final acceptance checklist

The closure is complete when:

- [ ] Provider `search` calls receive `candidate_limit`, not final result count.
- [ ] `candidate_limit` is computed before provider fan-out.
- [ ] Candidate-pool sizing is config-aware or otherwise impossible to conflict with `max_results_cap`.
- [ ] `candidate_pool_size` cannot panic when `effective_max_results > 50`.
- [ ] Final response truncation still uses the caller's effective `max_results`.
- [ ] Mock engines in tests respect `max_results`.
- [ ] There is a regression test that fails if providers receive the final count instead of the candidate count.
- [ ] Neutral `web` intent preserves RRF ordering.
- [ ] No `FreshnessMatch` is emitted without date evidence.
- [ ] No crawling, fetching, summarization, caching, or research workflow behavior is added.
- [ ] README and MCP instructions remain aligned with the strict search/fetch boundary.
