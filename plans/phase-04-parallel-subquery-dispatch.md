# Phase 4 Plan: Parallel Subquery Dispatch and Latency Control

## Objective

Refactor specialized search dispatch so `repo_search`, `security_search`, and `research_search` use request deadlines efficiently. The current pattern runs subqueries sequentially while each subquery fans out to providers concurrently. This can starve later evidence categories when early subqueries are slow. This phase should introduce priority-aware bounded parallel dispatch across `(subquery, provider)` jobs while preserving deterministic output and partial-result semantics.

## Rationale

Coding agents value the first high-quality evidence more than exhaustive search. For example:

- Exact-error search should quickly run exact phrase, error-code, issues, and changelog queries.
- Repository investigation should not let generic docs queries starve source/issues queries.
- Research search should start primary sources and specifications early.
- Security search should prioritize native advisories before generic exploit discussion.

The repo already has deadline telemetry and partial-result warnings. This phase should improve the scheduler behind that telemetry rather than changing the overall response model.

## Scope

In scope:

- Add a shared dispatch scheduler for specialized multi-subquery searches.
- Dispatch `(subquery, provider)` jobs with bounded concurrency.
- Add subquery priority and provider priority.
- Preserve global request deadline behavior.
- Preserve per-provider failure accounting.
- Preserve deterministic aggregation and result ordering.
- Track interrupted/skipped jobs and subqueries accurately.
- Add tests using mock fast/slow/failing providers.

Out of scope:

- Persistent provider health memory; that is Phase 7.
- Network retry policy beyond current provider behavior.
- New providers.
- Changing ranking semantics from Phase 3.
- Streaming partial results over MCP.

## Current shape to replace

The existing helper effectively performs:

1. For each planned subquery in order:
2. Spawn one task per engine for that subquery.
3. Wait until all engines finish or the global deadline expires.
4. Move to the next subquery.

This preserves simple ordering but wastes the global deadline if an early subquery/provider combination is slow.

## Proposed scheduler model

Introduce a dispatch abstraction, likely in `src/meta/dispatch.rs`:

```rust
pub struct DispatchJob {
    pub subquery_id: String,
    pub subquery_label: String,
    pub query: String,
    pub provider_id: String,
    pub provider: Arc<dyn SearchEngine>,
    pub priority: i32,
    pub subquery_order: usize,
    pub provider_order: usize,
}

pub struct DispatchConfig {
    pub candidate_limit: usize,
    pub global_timeout: Duration,
    pub max_concurrent_jobs: usize,
    pub max_concurrent_per_provider: usize,
}

pub struct DispatchOutput {
    pub raw_results: Vec<DispatchedProviderResults>,
    pub raw_failures: Vec<DispatchedProviderFailure>,
    pub deadline: RequestDeadlineStats,
    pub telemetry: DispatchTelemetry,
}
```

Do not expose these exact types publicly unless needed. Keep them internal to `meta` if possible.

## Priority model

Subqueries should carry priority. Start with simple per-tool mappings.

### Repo search normal mode

Priority order:

1. Source query when symbol/path/file/language hints exist.
2. Docs/README/package registry.
3. Examples/tests.
4. Issues/pull requests.
5. Releases/changelog/migration.

If no symbol/path hints exist, docs/README and source should be close in priority.

### Exact-error mode

Priority order:

1. Exact quoted phrase.
2. Error code.
3. Tool/language-specific issue search.
4. Changelog/release/migration.
5. Docs.
6. Generic source.

### Security search

Priority order:

1. Native advisory provider jobs.
2. Vendor advisory/source-quality queries.
3. Package registry advisory queries.
4. Fix release/changelog queries.
5. Defensive guidance.
6. Exploit/community discussion if requested.

### Research search

Priority order should derive from workflow and source type:

- Specifications, official docs, reference implementations: high.
- Benchmarks: high for performance workflow.
- Security considerations: high for security workflow.
- Case studies and discussions: medium.
- Counterpoints: medium, but ensure at least one runs when requested.

## Concurrency controls

Add config fields with safe defaults. Keep caps conservative.

Possible config additions under `[search]`:

```toml
multiquery_concurrency = 8
multiquery_provider_concurrency = 2
```

Rules:

- `multiquery_concurrency` caps total in-flight `(subquery, provider)` jobs.
- `multiquery_provider_concurrency` prevents one provider from receiving too many concurrent requests.
- The global timeout still bounds the whole request.
- A per-job timeout should be the remaining global timeout or a smaller configured provider timeout if already present.

If config changes are too broad for this phase, start with internal constants and document a later config pass.

## Determinism requirements

Parallel dispatch must not make responses flaky.

- Assign stable `subquery_order` and `provider_order` before spawning jobs.
- Collect results with metadata about their planned order.
- Before aggregation, sort raw result batches by `(subquery_order, provider_order)` or another stable key.
- Preserve existing aggregation behavior after sorting.
- Provider failure ordering should also be stable.
- Telemetry should be deterministic except for actual timeout/failure timing.

## Provider failure accounting

The current provider failure model assumes providers queried per request. With multiple subqueries, failure semantics need care.

Recommended response behavior:

- Preserve existing `providers_failed` as a per-provider aggregate for compatibility.
- Add internal or optional telemetry for job-level failures if useful.
- A provider should count as failed globally only if all attempted jobs for that provider fail or time out.
- If some jobs succeed and some fail, report partial provider failure in telemetry/warnings rather than marking the provider as wholly failed.

If changing this behavior is too invasive, preserve current provider-level behavior but add TODOs/tests that document partial semantics.

## Affected modules

Likely files:

- `src/meta/adapter.rs`
- `src/meta/repo_planner.rs`
- `src/meta/research_planner.rs`
- `src/meta/security_search.rs`
- new `src/meta/dispatch.rs`
- `src/core/config.rs` if config fields are added
- response telemetry types in `src/core/repo_search.rs`, `src/core/research.rs`, and security types if expanded
- tests using mock engines

## Implementation steps

1. Extract current `dispatch_subqueries` behavior behind a new dispatch module with equivalent behavior.
2. Add stable job construction from subqueries and engines.
3. Add priority field to planned subqueries or wrap them during dispatch.
4. Implement bounded concurrent job execution with `JoinSet` or `FuturesUnordered` plus a semaphore.
5. Enforce per-provider concurrency.
6. Preserve global deadline and cancel remaining jobs when exceeded.
7. Sort results deterministically before aggregation.
8. Wire repo/research/security search through the new dispatcher.
9. Add telemetry fields only if needed for acceptance; otherwise keep public responses stable.
10. Remove the old sequential helper once tests pass.

## Tests

Add tests for:

- A slow first subquery does not prevent a later high-priority subquery from returning results.
- Global deadline produces partial results and deadline warnings.
- Per-provider concurrency cap is respected with mock provider instrumentation.
- Deterministic output order across repeated runs with different completion orders.
- Provider failures are reported consistently.
- Exact-error priority schedules exact phrase/error-code jobs before generic docs.
- Research search still applies group diversity after parallel dispatch.
- Existing simple web search remains unaffected.

## Acceptance criteria

- Specialized search tools no longer execute subqueries strictly one at a time.
- Request-level deadlines are still enforced.
- Partial results are preserved.
- Output ordering is deterministic under mock completion reordering.
- Warnings/telemetry still report deadline interruption/skipping accurately.
- Provider failure accounting remains compatible or is explicitly extended with tests.
- No crawler behavior is introduced.
- `cargo test` passes.

## Handoff notes

Be conservative. This phase touches concurrency and can produce subtle nondeterminism. Prefer a small, well-tested dispatcher with stable ordering over an aggressive scheduler. Do not mix in provider health/cooldown memory yet; that belongs to Phase 7.
