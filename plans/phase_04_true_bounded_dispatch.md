# Phase 4: True Bounded Multiquery Dispatch

## Objective

Replace spawn-all semaphore-gated multiquery dispatch with a true bounded queue executor. Repo, security, and research searches should execute `(subquery, provider)` jobs with configured global and per-provider concurrency limits while avoiding one spawned task per pending job. Deadline telemetry should distinguish jobs skipped before start from jobs interrupted after start.

The current design already preserves deterministic output order and applies concurrency gates, but it still creates all tasks up front. This phase makes the execution model match the documented bounded behavior more closely and prepares eggsearch for larger search plans.

## Current problem statement

The multiquery dispatcher sorts jobs by priority and then spawns every job into a `JoinSet`. Each task waits on global and provider semaphores before executing. This bounds active provider calls, but it does not bound task count. For modest job counts this is fine; for deeper research/security/repo plans, it creates unnecessary scheduler overhead and weaker deadline accounting.

A queue-based executor should keep only the active job set in flight. When a job completes, the executor starts the next eligible job if deadline and per-provider limits permit.

## Scope

In scope:

- Refactor the dispatcher internals to a queue-based executor.
- Preserve public request/response shapes unless telemetry gains additive fields.
- Preserve deterministic output order independent of completion order.
- Preserve global deadline and per-provider concurrency semantics.
- Improve deadline telemetry.
- Add stress and determinism tests.

Out of scope:

- Changing planner subquery generation.
- Changing ranking or aggregation semantics.
- Changing provider-specific search logic.
- Introducing a persistent job scheduler.

## Design requirements

### Job lifecycle states

Track jobs in lifecycle states:

- `queued`: not started.
- `running`: provider call in progress.
- `succeeded`: provider returned results.
- `failed`: provider returned an error.
- `skipped_deadline`: deadline expired before the job started.
- `interrupted_deadline`: deadline expired while the job was running or pending completion.
- `join_failed`: task panicked or was cancelled unexpectedly.

Not all states need to be exposed publicly, but tests and telemetry should be able to distinguish skipped vs interrupted.

### Deterministic scheduling

Jobs should be sorted by:

1. priority, lower is higher priority;
2. subquery order;
3. provider order.

The executor should start jobs in that order subject to per-provider concurrency availability. If the next job is blocked only because its provider is at capacity, the executor may scan forward to find another runnable job for a different provider. If scanning forward is implemented, final output order must still be sorted deterministically by the original ordering keys.

### Concurrency semantics

- `max_concurrent_jobs` bounds the number of active provider calls.
- `max_concurrent_per_provider` bounds active calls for a single provider.
- Both values must be clamped or validated to at least 1.
- Deadline expiration stops new dispatch and aborts/races running work according to current behavior.

### Deadline semantics

At deadline expiration:

- queued jobs become `skipped_deadline`;
- running jobs become `interrupted_deadline` if they do not complete before cancellation;
- completed jobs remain available and are returned;
- partial-result behavior is preserved.

### Output semantics

Raw results and failures should be sorted by `(subquery_order, provider_order)` before returning, as today. The aggregation layer should not observe nondeterministic completion order.

## Implementation steps

1. Add characterization tests around current deterministic ordering and partial-result behavior.
2. Introduce an internal job state struct that stores the existing dispatch metadata plus status.
3. Replace spawn-all loop with an executor loop: maintain a sorted pending queue, maintain per-provider active counts, maintain active `JoinSet` entries only for started jobs, start jobs while global capacity and provider capacity permit, await the next completion or deadline, update active counts and collect result/failure, and start more jobs if time remains.
4. Implement optional runnable-job scanning. Keep it simple: if scanning risks complexity, execute strictly in sorted order for the first pass.
5. Add skipped/interrupted accounting to `RequestDeadlineStats`.
6. Preserve or adapt existing `DispatchOutput` fields. Add fields only if needed for richer telemetry.
7. Run existing repo/security/research tests and adjust only where telemetry has intentionally improved.

## Required tests

Add tests for:

- No more than `max_concurrent_jobs` provider calls are active at once.
- No more than `max_concurrent_per_provider` calls are active for a provider.
- Output ordering is deterministic despite varied provider delays.
- Priority ordering is respected.
- Short deadline returns partial completed results.
- Queued jobs at deadline are counted as skipped.
- Running jobs at deadline are counted as interrupted.
- Provider failures are preserved and sorted deterministically.
- Zero config values are rejected or clamped before dispatch.
- Large job list does not spawn all jobs immediately. This can be tested with instrumentation around mock engine call start counts.

## Acceptance criteria

- The dispatcher no longer spawns one task per job up front.
- Active provider calls are bounded by global and per-provider caps.
- Results are deterministic across repeated runs with the same mock delays.
- Deadline telemetry distinguishes skipped and interrupted work.
- Existing search behavior remains compatible except for improved telemetry.
- Stress tests cover large job plans.

## Risks and mitigations

Risk: A strict queue can underutilize global concurrency when the next job is blocked by provider capacity.

Mitigation: Implement limited scanning for runnable jobs by provider capacity, or accept strict behavior in the first pass and document it.

Risk: Refactor changes output ordering.

Mitigation: Sort all results/failures by original order keys before returning and snapshot tests.

Risk: Cancellation behavior leaks permits or active counts.

Mitigation: Use RAII-style active-count decrement in completion handling and explicit abort handling at deadline.

## Handoff notes

Keep the executor internal to the dispatch module. Do not change planners or aggregation in this phase. Add instrumentation in tests rather than production counters unless telemetry already has a natural place for it.
