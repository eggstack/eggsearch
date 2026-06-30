# Final Phases 6–11 Corrective Closure Plan

## Purpose

Close the last two correctness issues found after the phases 6–11 hardening pass. The repository is now broadly in good shape: batch fetch, exact-error mode, GitLab/Gitea provider support, provider telemetry, quality metadata, security context, and research workflow scaffolding are implemented and mostly hardened.

This plan is intentionally narrow. Do not expand it into a broader feature phase. Fix the remaining correctness seams, add focused regression tests, update docs only where behavior changes, and run verification.

## Scope

This pass covers only:

1. Correct batch-fetch wave result association so concurrent task completion order cannot attach a result to the wrong input item.
2. Correct exact-error query length validation so `search.exact_error.max_error_chars` is the authoritative cap for `mode = exact_error`.
3. Fix the minor batch URL-scheme validation typo.
4. Add focused regression tests for the above.
5. Run final verification.

Do not change provider architecture, research workflows, quality scoring semantics, security-context schema, GitLab/Gitea provider surfaces, or MCP tool shapes unless a targeted test demonstrates a direct dependency on one of the issues above.

## Current state

### Batch fetch

`batch_fetch` now performs ordered bounded waves and uses `batch_concurrency` when `continue_on_error = true`. It forces effective concurrency to `1` when `continue_on_error = false`, preserving strict abort-on-first-failure semantics.

The remaining risk is result association inside a wave. The current implementation iterates expected `wave_indices` while calling `join_next()`. `JoinSet::join_next()` returns whichever task completes first, not the task corresponding to the current expected index. The code then rewrites `batch_result.index` to the expected index. This can attach a response from item B to item A when tasks complete out of order.

### Exact-error validation

`RepoSearchRequest::validate` now receives exact-error config and rejects exact-error mode when disabled. However, the effective cap is still computed as:

```rust
max_query_chars.max(ee_config.max_error_chars)
```

This uses the larger of the general query cap and the exact-error cap. The intended behavior is that `search.exact_error.max_error_chars` is the authoritative cap for exact-error mode.

### Minor typo

Batch web URL scheme validation currently reports `http orhttps`. Fix to `http or https`.

## Task 1: Fix batch wave result association

### Required behavior

Batch fetch must preserve both:

- result vector order matching input order, and
- each `BatchFetchResult` payload matching the actual item that produced it.

Concurrent completion order must not alter either property.

### Recommended implementation

Replace the current `for idx in &wave_indices { join_next().await ... }` association pattern with an explicit map keyed by returned result index.

Sketch:

```rust
let mut wave_results: std::collections::BTreeMap<usize, BatchFetchResult> = BTreeMap::new();
let mut wave_errors: std::collections::BTreeMap<usize, BatchFetchResult> = BTreeMap::new();

while let Some(joined) = join_set.join_next().await {
    match joined {
        Ok(Ok(batch_result)) => {
            wave_results.insert(batch_result.index, batch_result);
        }
        Ok(Err(tool_err)) => {
            // The future should preferably never return Err without an index.
            // If possible, change make_batch_fetch_future to always return
            // BatchFetchResult so error association remains exact.
        }
        Err(join_err) => {
            // Same issue: without an index, cannot know which item panicked.
        }
    }
}

for idx in wave_start..wave_end {
    match wave_results.remove(&idx) {
        Some(batch_result) => push it,
        None => synthesize a stable failure for idx,
    }
}
```

Even better, make the spawned future return an always-indexed outcome:

```rust
struct IndexedBatchOutcome {
    index: usize,
    result: BatchFetchResult,
}
```

or simply ensure `make_batch_fetch_future` returns `BatchFetchResult` for all expected failure cases. For semaphore acquisition failure or tool error, construct a `BatchFetchResult` with the original index rather than returning `Err(ToolError)`. Reserve actual `JoinError` for task panics/cancellation, then synthesize a failure for any missing index after collection.

### Requirements

- Do not mutate a returned result's `index` to match an expected index.
- Use the returned `BatchFetchResult.index` as the source of truth.
- Reorder collected results after the wave completes using input indices.
- Preserve `continue_on_error = false` behavior.
- Preserve total-budget accounting after pushing results in input order.
- If a task panics and the index cannot be recovered, synthesize a failure for any missing index in that wave.

### Budget note

The current wave scheduling gives every item in a wave the same pre-wave `remaining_budget` cap. That means total returned chars can exceed the total cap if multiple concurrent items each return up to the same remaining budget. Fixing this perfectly under concurrency is harder.

For this pass, choose one of these options and document/test it:

Preferred conservative option:

- Before scheduling a wave, divide remaining budget across remaining items in the wave:

```rust
let wave_len = wave_end - wave_start;
let per_wave_item_budget = remaining_budget / wave_len.max(1);
let item_max_chars = per_item_cap.min(per_wave_item_budget.max(1));
```

This guarantees the wave cannot exceed the remaining budget by much, assuming fetchers honor `max_chars`.

Alternative option:

- Keep current per-item cap but after the wave completes mark `batch_total_budget_exhausted` and stop further waves. This is simpler but allows `total_chars_returned > max_total_chars` in the final response. If this option is chosen, docs and tests must explicitly allow overrun within one wave. This is not recommended.

Recommendation: implement per-wave budget division.

### Tests

Add focused tests with delayed mock fetches or test-only futures so completion order is out of order:

1. `batch_fetch_preserves_result_payloads_when_wave_completes_out_of_order`
   - Item 0 returns URL/title/text `A` but delays longer.
   - Item 1 returns URL/title/text `B` but completes first.
   - Response `results[0]` must contain item 0 payload, not item 1 payload.
   - Response `results[1]` must contain item 1 payload.

2. `batch_fetch_preserves_order_and_indices_under_concurrency`
   - At least 3 items, concurrency 2 or 3.
   - Ensure vector position, `index`, label, and payload all match input.

3. `batch_fetch_concurrent_wave_budget_does_not_exceed_total_cap`
   - Use multiple items that each try to return more than their share.
   - Assert `total_chars_returned <= max_total_chars` if implementing per-wave budget division.

4. `batch_fetch_continue_on_error_false_still_aborts_in_order`
   - Verify sequential strict abort remains intact.

5. `batch_fetch_url_scheme_error_message_is_spaced_correctly`
   - Assert validation message contains `http or https`.

## Task 2: Fix exact-error max character cap

### Required behavior

For normal repo search:

```rust
cap = search.max_query_chars
```

For exact-error mode:

```rust
cap = search.exact_error.max_error_chars
```

If exact-error mode is disabled:

```rust
return validation error
```

Do not use `max_query_chars.max(exact_error.max_error_chars)`. The exact-error config is the source of truth for exact-error mode.

### Recommended implementation

In `RepoSearchRequest::validate`, change:

```rust
max_query_chars.max(ee_config.max_error_chars)
```

to:

```rust
ee_config.max_error_chars
```

Also consider renaming the method or adding a doc comment explaining that `max_query_chars` is ignored for exact-error mode when `exact_error_config` is present.

If `exact_error_config` is `None`, use `ExactErrorConfig::default()` as currently done, but tests through MCP should always populate it.

### Tests

Add tests for:

1. `repo_search_exact_error_uses_exact_error_cap`
   - `search.max_query_chars = 10000`.
   - `search.exact_error.max_error_chars = 100`.
   - Query length 101 with `mode=exact_error` must fail.

2. `repo_search_normal_uses_normal_query_cap`
   - Normal mode should still use `search.max_query_chars`.

3. `repo_search_exact_error_allows_larger_than_normal_when_configured`
   - `search.max_query_chars = 512`.
   - `search.exact_error.max_error_chars = 8000`.
   - Query length >512 and <=8000 with `mode=exact_error` should validate.

4. `repo_search_exact_error_disabled_rejects_mode`
   - Confirm existing behavior remains covered.

Prefer tests at the public MCP/tool layer where practical, because config propagation from `ServerState` to `RepoSearchRequest` is part of the behavior.

## Task 3: Documentation touch-up

Only update docs if needed.

Required docs if not already accurate:

- `batch_fetch` uses ordered bounded waves.
- `continue_on_error=false` forces sequential strict abort semantics.
- `batch_concurrency` applies only when `continue_on_error=true`.
- Total budget behavior under concurrent waves, especially if using per-wave budget division.
- Exact-error mode uses `search.exact_error.max_error_chars`, not the general search cap.

Keep changes minimal. Do not rewrite the full README.

## Task 4: Verification

Run:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Run targeted tests:

```bash
cargo test batch_fetch
cargo test exact_error
cargo test repo_search_exact_error
```

If CI status is unavailable, record local command results in the implementation summary.

## Acceptance criteria

This corrective closure is complete when:

- Concurrent `batch_fetch` cannot misassociate payloads with the wrong input index.
- Batch results are returned in input order with correct `index`, `label`, and payload.
- Batch budget behavior is deterministic and tested under concurrent waves.
- The URL-scheme validation message says `http or https`.
- Exact-error mode uses `search.exact_error.max_error_chars` as its cap.
- Exact-error disabled mode still rejects requests.
- Targeted regression tests cover both issues.
- `cargo fmt`, clippy, and tests pass.

## Suggested implementation order

1. Fix exact-error cap; it is a one-line semantic correction plus tests.
2. Fix batch URL-scheme typo.
3. Refactor batch wave collection to keyed result association.
4. Add per-wave budget division or explicitly document/test allowed wave overrun.
5. Add targeted tests.
6. Minimal docs.
7. Full verification.
