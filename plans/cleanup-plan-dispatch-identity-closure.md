# Cleanup Plan: Dispatch Timeout and Identity Closure

## Objective

Close the final remaining seams after the Phase 1–6 corrective pass. This is a narrow cleanup handoff, not a new feature phase. The repo is already substantially improved, but one important corrective item remains incomplete: the parallel dispatch layer still passes a hardcoded 30-second timeout into provider calls while enforcing the true request deadline externally. There is also a smaller consistency issue around slash-form repository identity in `resolved_hints()`.

This pass should make the implemented behavior match the documented corrective-plan acceptance criteria, then run and document verification.

## Scope

In scope:

- Remove the hardcoded 30-second provider timeout from `src/meta/dispatch.rs`.
- Pass a real per-job timeout derived from the remaining request budget into every `SearchEngine::search` call.
- Improve deadline accounting so interrupted/skipped counters describe subqueries, not raw pending jobs.
- Clarify total versus partial provider failure semantics under multiquery dispatch.
- Normalize slash-form repo identity consistently in `RepoSearchRequest::resolved_hints()`.
- Add focused tests proving the above.
- Update docs only where they currently overclaim closure or describe stale behavior.

Out of scope:

- Provider health memory or cooldowns.
- New providers.
- Evidence bundles.
- Package ecosystem expansion.
- Security applicability parsing.
- Code-host raw-transform expansion.
- Any changes to the public MCP tool surface unless required for compatibility-safe telemetry.

## Current problems

### 1. Dispatch still passes a hardcoded provider timeout

`dispatch_parallel` enforces a global timeout by aborting jobs when the deadline expires, but each spawned provider call still receives `candidate_limit_duration()`, which returns `Duration::from_secs(30)`. This means provider implementations do not receive the real remaining request budget.

This is operationally survivable because the outer join timeout cancels pending jobs. However, it weakens timeout correctness:

- provider-level timeout classification can be inaccurate;
- providers may perform internal work under a timeout longer than the real request budget;
- tests cannot assert that provider calls respect request-level budget;
- the code contradicts the corrective-plan acceptance item that no hardcoded provider timeout should remain.

### 2. Deadline accounting conflates jobs and subqueries

`RequestDeadlineStats` exposes `subqueries_interrupted` and `subqueries_skipped`, but the current implementation increments `subqueries_interrupted` by the number of pending jobs. A single subquery can fan out to multiple providers, so this can overcount interrupted subqueries.

### 3. Partial provider failure semantics are not explicit enough

Under parallel multiquery dispatch, a provider can succeed on one subquery and fail on another. The response should not classify that provider as wholly failed if at least one job succeeded. It should be clear whether failures are total provider failures or partial job-level/provider-level failures.

### 4. Slash-form repo identity is canonical in validation but not fully normalized in hints

`ResolvedRepoIdentity` now centralizes identity for validation and local matching. However, `resolved_hints()` still overlays `self.repo` directly into `RepoQueryHints`. If `repo = "owner/name"` and `owner` is absent, hints can retain `repo = "owner/name"` rather than `owner = "owner"`, `repo = "name"`. This can produce odd planner/grouping/query behavior even if local matching is fixed.

## Workstream 1: Real per-job timeout propagation

### Desired behavior

Every provider job should receive a timeout derived from the actual remaining request budget.

At job execution time:

1. Acquire the global semaphore.
2. Acquire the provider semaphore.
3. Compute `remaining = overall_deadline.saturating_duration_since(Instant::now())`.
4. If `remaining.is_zero()`, return a timeout-like failure without calling the provider.
5. Call `provider.search(&query, candidate_limit, remaining)` or `min(remaining, configured_per_job_timeout)` if a per-job cap is later added.

Do not use a hardcoded 30-second timeout in dispatch.

### Implementation guidance

`tokio::time::Instant` is not directly portable into every closure unless captured before spawn or reconstructed. Capture `overall_deadline` into each task and compute remaining inside the task after semaphore acquisition. The dispatch code can continue to use the outer `timeout(remaining, join_next())` as a safety net.

Suggested shape:

```rust
let overall_deadline = overall_deadline;
join_set.spawn(async move {
    let _global_permit = global_sem.acquire().await.expect("...");
    let _provider_permit = provider_sem.acquire().await.expect("...");

    let remaining = overall_deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        return (..., Err(EngineError::Timeout { engine: provider_id_for_error }));
    }

    let result = provider.search(&query, candidate_limit, remaining).await;
    (..., result)
});
```

If `EngineError::Timeout` requires a static engine name and cannot accept `String`, use the provider engine’s `name()` where available, or add a helper that maps this condition into the closest existing timeout error. Keep this compile-clean rather than overfitting the pseudo-code.

### Tests

Add a mock engine that records the timeout passed to `search()`.

Required tests:

- With `global_timeout = 200ms`, provider receives a timeout no greater than 200ms.
- A job that waits behind semaphores receives a smaller timeout than the original global budget.
- No test or code path references `candidate_limit_duration()` after cleanup.
- Deadline cancellation still returns partial results.

### Acceptance criteria

- `candidate_limit_duration()` is deleted.
- No hardcoded 30-second provider timeout remains in `dispatch.rs`.
- Provider calls receive remaining budget or a bounded value derived from it.
- Tests assert timeout propagation behavior.

## Workstream 2: Accurate deadline accounting

### Desired behavior

The public fields `subqueries_interrupted` and `subqueries_skipped` should count unique subquery IDs, not provider jobs.

Definitions:

- `subqueries_interrupted`: unique subquery IDs with at least one pending/in-flight job when the deadline fired and no complete accounting was available for all jobs.
- `subqueries_skipped`: unique subquery IDs whose jobs never began provider execution before the deadline.

If tracking never-started precisely is too invasive, prefer conservative naming or internal fields:

- Keep public fields but ensure they are not raw job counts.
- Add internal `jobs_interrupted`/`jobs_skipped` only if useful for tests.

### Implementation guidance

Track job lifecycle explicitly. Extend `DispatchJob` or internal job state with stable IDs:

```rust
struct JobLifecycle {
    subquery_id: String,
    started: bool,
    completed: bool,
}
```

A lower-friction approach is to have each job return a started marker through a channel when it acquires both semaphores. The dispatcher can then compute:

- all subquery IDs;
- completed subquery IDs from results/failures;
- started subquery IDs from lifecycle messages;
- pending subquery IDs from all minus completed;
- skipped subquery IDs from all minus started.

Keep this simple. If lifecycle tracking adds too much complexity, at minimum change the deadline stats calculation to build unique subquery IDs from pending jobs before spawning consumes job metadata.

### Tests

Required tests:

- One subquery with three provider jobs times out and increments interrupted by one, not three.
- Two subqueries where one finishes and one times out reports exactly one interrupted subquery.
- Deterministic output order is unchanged.

### Acceptance criteria

- Public deadline counters no longer overcount provider fan-out as subquery count.
- Deadline warning text remains compatible but is more accurate.
- Tests cover multi-provider fan-out timeout behavior.

## Workstream 3: Total versus partial provider failure

### Desired behavior

A provider should appear in `providers_failed` only when all attempted jobs for that provider failed or timed out. If a provider has at least one successful job and at least one failed job, the response should emit a partial failure warning or telemetry entry, but it should not be classified as a total provider failure.

### Implementation guidance

Audit the helper that builds `providers_failed` from `queried_ids`, `dispatch.raw_results`, and `dispatch.raw_failures`.

Expected algorithm:

```rust
success_by_provider = providers with at least one raw_results entry
failure_by_provider = providers with at least one raw_failures entry
for provider in queried_ids:
    if provider in failure_by_provider and provider not in success_by_provider:
        providers_failed.push(total failure)
    if provider in failure_by_provider and provider in success_by_provider:
        warnings.push("provider_partial_failure: ...")
```

If the helper already behaves this way, add tests and document the behavior. If it does not, fix it.

### Tests

Required tests:

- Provider succeeds on one subquery and fails on another: not in `providers_failed`; warning contains `provider_partial_failure`.
- Provider fails on every attempted subquery: appears in `providers_failed`.
- Provider has no attempted jobs due to empty provider set or empty subqueries: no false failure.
- Multiple providers preserve deterministic warning order.

### Acceptance criteria

- Total failure and partial failure are distinguishable.
- `providers_failed` remains backward-compatible as total failures only.
- Partial failures are visible to agents.

## Workstream 4: Normalize slash-form identity in resolved hints

### Desired behavior

All supported repository locator forms should produce equivalent hints:

1. Explicit `owner = "tokio-rs"`, `repo = "axum"`.
2. Slash-form `repo = "tokio-rs/axum"` with no owner.
3. Query hint `query = "repo:tokio-rs/axum Router"`.

For identity-sensitive planning and grouping, all three should effectively resolve to owner `tokio-rs` and repo `axum`. Explicit fields must still override query hints.

### Implementation guidance

Update `RepoSearchRequest::resolved_hints()` so it consults `resolved_repo_identity()` and overlays the normalized owner/repo into `RepoQueryHints` unless explicit fields already require different behavior.

Be careful with precedence:

- Explicit `owner` + `repo` wins.
- Slash-form `repo` with no owner splits into owner/repo.
- Query hint is used only when explicit/slash-form fields do not provide identity.
- Explicit `repo` without slash and without owner should remain a repo-name hint, not be discarded.

Suggested behavior:

```rust
let identity = self.resolved_repo_identity();
if let Some(id) = identity {
    hints.owner = Some(id.owner);
    hints.repo = Some(id.repo);
} else {
    // preserve existing explicit field overlays for non-identity hints
}
```

Then overlay non-identity fields: host, org, path, file, language, symbol.

### Tests

Required tests:

- `resolved_hints()` explicit owner/repo returns owner and repo separately.
- `resolved_hints()` slash-form repo returns owner and repo separately.
- `resolved_hints()` query-hint repo returns owner and repo separately.
- Explicit owner/repo overrides conflicting query hint.
- Explicit bare repo without owner remains a bare repo hint if that behavior is currently supported.
- Repo-only planner emits equivalent owner/repo-scoped subqueries for explicit and slash-form identity.

### Acceptance criteria

- Slash-form `repo = "owner/name"` does not leak as a raw repo name in planning/grouping when owner is absent.
- All locator forms behave equivalently in tests.
- Existing explicit-field precedence is preserved.

## Workstream 5: Verification and documentation cleanup

### Required verification commands

Run and record:

```bash
cargo fmt --check
cargo clippy --all-features --all-targets -- -D warnings
cargo clippy --features mock --all-targets -- -D warnings
cargo test --all-features
cargo test --features mock
```

If any command cannot run in the environment, document the exact reason in the final commit message.

### Documentation updates

Update only what is stale after this cleanup:

- Remove any references to dummy or hardcoded dispatch timeout behavior.
- Clarify that dispatch passes remaining request budget to providers.
- Clarify that `providers_failed` means total provider failure, while partial provider failures are warnings/telemetry.
- Clarify that `resolved_repo_identity()` is the canonical identity path and that `resolved_hints()` normalizes slash-form identities.

### Acceptance criteria

- Commit message includes the exact verification commands and outcomes.
- README/AGENTS do not overclaim unimplemented behavior.
- Tests prove the specific cleanup issues are closed.

## Final checklist

- [ ] `candidate_limit_duration()` removed.
- [ ] No hardcoded `Duration::from_secs(30)` provider timeout remains in dispatch.
- [ ] Provider `search()` receives remaining request budget.
- [ ] Tests record and assert provider timeout values.
- [ ] Deadline stats count unique subqueries, not provider jobs.
- [ ] Partial provider failure is visible but does not populate `providers_failed` as total failure.
- [ ] Slash-form repo identity is normalized in `resolved_hints()`.
- [ ] Explicit identity overrides query-hint identity.
- [ ] Repo-only planner tests cover explicit, slash-form, and query-hint identity equivalence.
- [ ] Documentation matches final dispatch and identity behavior.
- [ ] Formatting, clippy, and tests pass for the relevant feature sets.
