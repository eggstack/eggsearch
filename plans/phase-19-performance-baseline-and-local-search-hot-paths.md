# Phase 19 — Performance Baseline and Local Search Hot Paths

Status: planned
Depends on: phases 17-18 implemented
Baseline for planning: `205ab26fb03c6769035a1c05bb9b1f41c2a9ead1` (`main`)
Governing roadmap: `performance-optimization-roadmap.md`

## Why this phase exists

Local workspace search is one of eggsearch's highest-value paths for CodeGG, and the audited implementation currently performs work proportional to the entire cached inventory even when the requested result set is small.

Two behaviors are specifically targeted:

1. the warm search path deep-clones `WorkspaceInventory` despite the cache already storing an `Arc<WorkspaceInventory>`;
2. candidate sorting repeatedly recomputes allocation-bearing scores inside the comparator.

These are internal inefficiencies. The user-visible local-search contract, result ordering, freshness semantics, and public Rust API do not need to change.

This phase also establishes benchmark conventions for the wider performance workstream. Optimization should be supported by production-shaped benchmarks before later phases use performance claims as closure evidence.

## Objective

Make warm local workspace search operate on shared inventory snapshots and score/filter candidates once per entry, while preserving exact observable behavior.

The desired end state is:

- cached inventories are shared internally by Arc rather than recursively cloned per search;
- rebuilding an inventory allocates the snapshot once and publishes that same snapshot to the cache;
- candidate filtering pre-normalizes query/path inputs outside per-entry closures;
- scoring is performed once per candidate;
- only the bounded top-K set is fully ordered;
- ties remain deterministic and equivalent to the existing path-ordered stable sort;
- existing public owned-return inventory APIs remain source-compatible;
- benchmarks exercise the actual production-shaped candidate selection path.

## Work item 1 — Add production-shaped local-search benchmarks first

Extend `benches/perf.rs` before or in the same commit as the optimization.

The current inventory benchmarks call `score_inventory_entry` once per entry. Keep those microbenchmarks, but add higher-level cases that model the actual candidate-selection work.

At minimum cover:

- 1,000 inventory entries;
- 4,096 entries, matching the current near-cap characterization case;
- a larger configured-cap characterization case if it can run quickly enough for local benchmarking;
- a selective language/path filter;
- a query that produces many equal or near-equal scores so deterministic tie ordering is exercised;
- warm cached-inventory acquisition separately from cold inventory construction.

Prefer a pure helper around candidate selection so the benchmark can avoid filesystem noise when measuring scoring/sorting. Add a separate warm-search benchmark only if its filesystem setup is deterministic and bounded.

Record the pre-change benchmark output in the implementation record. Criterion timing must not become a pass/fail CI threshold.

## Work item 2 — Introduce an internal shared inventory acquisition path

The cache currently stores:

~~~text
Arc<RwLock<Option<Arc<WorkspaceInventory>>>>
~~~

but `search_sync` obtains an owned `WorkspaceInventory` by cloning through the Arc.

Add an internal helper that returns:

~~~text
Option<Arc<WorkspaceInventory>>
~~~

or equivalent shared ownership.

Requirements:

- acquiring a fresh cached inventory performs only an Arc clone, not a deep clone;
- freshness probing and `needs_rebuild` behavior remain unchanged;
- a rebuild constructs one `WorkspaceInventory`, wraps it in one Arc, stores an Arc clone in the cache, and searches through the same shared snapshot;
- no cache lock is held while scanning candidates or reading files;
- a concurrent rebuild cannot mutate a snapshot already being searched;
- poisoning/error behavior remains no worse than today.

Do not change the public `get_or_build_inventory() -> Option<WorkspaceInventory>` signature. If external callers require an owned value, retain it as a compatibility wrapper that clones intentionally at the API boundary. Eggsearch's internal hot path should use the shared helper.

Add tests proving:

- two warm acquisitions refer to the same logical snapshot until invalidation/rebuild;
- a rebuild publishes a new snapshot;
- search results are unchanged when using the shared path;
- public owned-return behavior still works.

## Work item 3 — Normalize filter inputs once

In `LocalWorkspaceBackend::search_sync`, normalize values that are currently recomputed per entry.

At minimum:

- compute lowercase query once as today;
- compute lowercase path hint once when a path hint exists;
- avoid repeated `ph.to_lowercase()` inside the inventory filter;
- avoid reconstructing equivalent normalized values in both warm-cache and newly-built-inventory branches.

Prefer extracting the duplicated inventory-search branch into a private helper if doing so makes the warm and cold paths share exactly the same filter/ranking implementation. Do not create a generic framework solely for this phase.

## Work item 4 — Score each viable inventory entry once

Replace comparator-time scoring with a decorated candidate representation.

Conceptually:

~~~text
filtered entry
  -> compute score once
  -> retain original deterministic ordinal/path identity
  -> select bounded top K
  -> order retained K deterministically
~~~

The retained K remains the current effective candidate budget, approximately `max_results * 2`, with overflow-safe arithmetic.

The comparator for the decorated form must include the existing deterministic tie behavior. The current inventory is sorted by `relative_path`, and the current stable score sort preserves that order for equal scores. A replacement using `select_nth_unstable_by` or another partial-selection primitive must therefore include an explicit tie key such as original ordinal or `relative_path`, then sort the retained K by the same composite comparator.

Do not change score weights or ranking reasons in this phase.

Do not cache lowercase copies of every path in `InventoryEntry` unless benchmarks show that the additional persistent memory is justified after the score-once change.

## Work item 5 — Remove duplicate warm/cold candidate-selection code where safe

The current warm cached-inventory branch and freshly-built-inventory branch contain substantially duplicated filtering, sorting, validation, reading, and match construction.

After the scoring semantics are locked by tests, extract the smallest useful private helper so both branches use one candidate-selection implementation.

The helper should not own freshness/build telemetry policy. Its responsibility should be limited to searching an already-selected immutable `WorkspaceInventory`.

This is a maintainability optimization as well as a performance guard: later changes should not accidentally optimize only one branch.

## Work item 6 — Consider config/root sharing only after measurement

`search()` currently clones `LocalConfig` and the roots vector before entering `spawn_blocking`. These copies are much smaller than the workspace inventory and may be negligible.

Do not expand Phase 19 merely to remove them.

If the new benchmarks or allocation profiling show these copies materially contribute to warm-search cost, it is acceptable to store internal immutable config/root state behind Arc and clone only handles into `spawn_blocking`, provided:

- `config() -> &LocalConfig` remains source-compatible;
- `roots() -> Vec<(usize, PathBuf)>` retains its public behavior;
- construction/validation semantics do not change.

Otherwise record them as measured/deferred.

## Correctness invariants

The optimization must preserve:

- identical matching behavior for path/language/file/symbol hints;
- identical score calculation;
- identical deterministic ordering, including equal-score ties;
- identical `max_results`, truncation, timeout, and `max_indexed_files` behavior;
- identical inventory freshness confidence and rebuild triggers;
- identical safe-open and root-containment validation;
- identical structured-symbol/regex-fallback behavior;
- identical telemetry field meanings.

## Required tests

Add or update deterministic tests for:

1. shared cached snapshot acquisition;
2. cache rebuild snapshot replacement;
3. warm and cold inventory paths producing the same ordered matches for a fixture;
4. equal-score candidate ordering;
5. top-K selection matching the legacy full-sort reference implementation over generated deterministic fixtures;
6. path-hint case behavior matching existing semantics;
7. small `max_results` and boundary/overflow-safe candidate budget behavior.

A property-style comparison against a simple reference full sort is preferred for the selector because it directly protects semantic equivalence.

## Performance evidence

At minimum record before/after for:

- candidate selection at 1,000 entries;
- candidate selection at 4,096 entries;
- warm cached inventory acquisition;
- one representative end-to-end warm local search if deterministic enough.

Useful secondary evidence includes allocation counts or heap profiling, but do not add a permanent allocator dependency merely for this phase.

Expected shape of improvement:

- cached snapshot acquisition should become O(1) shared-handle cloning rather than O(total inventory data);
- scoring work should become O(N) score evaluations plus bounded top-K ordering rather than O(N log N) score evaluations from comparator recomputation.

Do not encode a speculative percentage target as an acceptance criterion.

## Non-goals

Phase 19 does not:

- change ranking weights;
- introduce embeddings/vector search;
- add a persistent on-disk index;
- add background indexing;
- change structured symbol semantics;
- add filesystem watchers;
- make local search asynchronous internally beyond the existing `spawn_blocking` boundary;
- change public MCP or Rust response/request types.

## Suggested execution order

~~~text
1. capture baseline benchmark output
2. add selector/reference-equivalence tests
3. add internal shared inventory acquisition
4. publish one Arc snapshot on rebuild
5. pre-normalize filters
6. implement score-once bounded top-K selection
7. deduplicate warm/cold inventory search helper
8. run correctness gates
9. rerun benchmarks and record evidence
10. update plan + registry only when acceptance criteria are met
~~~

## Acceptance criteria

Phase 19 is complete only when:

1. production-shaped candidate-selection benchmarks exist.
2. warm inventory acquisition no longer deep-clones `WorkspaceInventory` in the internal search path.
3. a rebuilt inventory is allocated once and shared into the cache/search path.
4. public `get_or_build_inventory` compatibility is preserved.
5. path hints are normalized outside the per-entry filter.
6. each viable candidate is scored once for selection.
7. only the bounded retained candidate set is fully ordered, or an equivalent measured algorithm proves no worse while avoiding comparator recomputation.
8. equal-score deterministic ordering matches the legacy behavior.
9. warm and cold inventory paths share the optimized selector rather than maintaining divergent copies.
10. timeout, truncation, freshness, safe-open, symbol, and telemetry semantics remain unchanged.
11. targeted tests compare the optimized selector to a reference legacy full sort.
12. before/after Criterion evidence is recorded.
13. `make check` passes on the exact implementation candidate.
14. the implementation record names any contemplated micro-optimizations that were rejected after measurement.

## Handoff notes

Start with the inventory Arc clone bug and comparator scoring. Those are the high-confidence wins.

Avoid broad data-model changes until after those two corrections are measured. In particular, storing pre-lowercased paths can save CPU but permanently increases inventory memory; the score-once change may make that unnecessary.

The selector must protect tie ordering explicitly. Performance is not a reason to introduce nondeterministic local-search results.

## Implementation record

Not yet implemented. Record exact implementation SHA, commands, tests, benchmark environment/results, and any deviations here before changing status to `implemented`.
