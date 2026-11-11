# M004 — Candidate Snapshot Ranking Benchmark

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M004
Primary class: infrastructure (corrective to deterministic-retrieval-quality M004)
Baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71`
References: `plans/implementation/deterministic-retrieval-quality/004-deterministic-relevance-and-selection.md`; `plans/closure/deterministic-retrieval-quality/004-status.md`; `docs/retrieval-quality-ranking-ablation.md`; `tests/retrieval_quality.rs`; `tests/fixtures/retrieval_quality/`; `src/meta/adapter/normalization.rs`; `src/core/identity.rs`.
Dependencies: none hard.

## Objective

Make ranker changes measurable offline by replaying per-provider candidate lists through the production selection path, so a fusion, dedup, diversity, or rerank change produces a deterministic metric delta instead of an unanswerable claim.

## Why this is corrective

DRQ M004 closed with "no reliable lift established" because `tests/retrieval_quality.rs` scores `Case::results`, a single pre-merged list authored in the fixture. It never invokes `aggregate_rrf`, `convert_aggregated`, domain filtering, or `apply_intent_reranking`. The corpus can validate metric arithmetic but cannot distinguish two rankers. Improving the ranker was therefore untestable, not unnecessary.

The failed verification was structural: there was no candidate-generation record to replay, and no negative control proving the harness would notice a worse selector.

## Non-goals

- No production ranking change in this milestone. The harness must land and prove itself before any selector changes.
- No live network calls in routine CI.
- No embedding, model, or hosted reranker.
- No change to existing case judgments or to the legacy single-list scoring path.

## Invariants that cannot regress

- Existing corpus cases and their metrics continue to pass unchanged.
- `make eval-retrieval-quality` remains network-free and deterministic across repeated runs.
- Fixtures contain no page bodies, no credentials, and no personal data: titles, normalized URLs, ranks, and provider IDs only.
- Stable IDs and RRF tie ordering are untouched.
- The harness exercises production functions, not a copy; a copy would drift and reintroduce the original defect.

## Expected production changes

1. **Test-harness exposure.** Expose the selection functions to integration tests under the existing `mock` feature, following the `bounded_command_test` precedent in `src/lib.rs`. Nothing is exposed in a default build.
2. **Fixture schema extension.** Add an optional `provider_lists` array to retrieval-quality cases: `[{ provider_id, results: [{ rank, title, url }] }]`. When present, the harness constructs the real `Vec<(String, Vec<SearchResult>)>` and runs the production path; when absent, the legacy path is used unchanged.
3. **Tier-1 corpus.** A committed structured-synthetic snapshot set with controlled structure: rank inversions between providers, duplicate clusters across distinct hosts, domain concentration, provider agreement and disagreement, and adjudicated grades. Authored in-repo and fully redistributable.
4. **Tier-2 corpus.** A small committed recorded-real set with provenance in the fixture README; normalized URL and title only.
5. **Tier-3 recorder.** An opt-in script that performs a bounded live run and writes a larger snapshot under `target/snapshots/` (never tracked), for release qualification.
6. **Negative controls.** Three seeded regressions the harness must detect: a degenerate RRF constant that collapses rank diversity, dedup disabled so duplicates occupy slots, and truncation before rerank so pool expansion stops working.
7. **Metrics.** Reuse precision, recall, MRR, and nDCG. Add duplicate-slot count, distinct-domain count in the top k, and authoritative-survival rate (grade ≥ 2 documents retained in the top k) because fusion defects show up there first.

## Storage and compatibility effects

- New fixture files under `tests/fixtures/retrieval_quality/`; size bounded and documented. No production storage.
- `src/lib.rs` gains one `#[cfg(feature = "mock")]` re-export.
- No MCP, schema, or CLI change.

## Ordered work packages

1. Extract the production selection sequence into a callable function without changing behaviour; prove equivalence by asserting existing corpus metrics are identical before and after the refactor.
2. Add the `mock`-gated test harness export.
3. Extend the fixture schema and implement the replay path alongside the legacy path.
4. Author tier-1 cases; verify each case's expected values are derived, not hand-written to match the ranker.
5. Add the three negative controls and assert each one degrades the expected metric.
6. Author tier-2 cases and the tier-3 recorder script; document provenance and licensing in the fixture README.
7. Wire into `make eval-retrieval-quality`, update `docs/retrieval-quality-benchmark.md`, `docs/retrieval-quality-ranking-ablation.md`, `architecture/testing.md`, and `docs/test-inventory.md`.

## Verification

- `make eval-retrieval-quality` twice; metric output must be identical.
- `cargo test --locked --features mock --test retrieval_quality`
- `cargo test --locked --features mock --test web_search_integration` (unchanged ranking behaviour)
- `make check`; `make docs-check`

## Acceptance and stop conditions

- Replaying the same snapshot twice yields identical metrics and ranking.
- Each of the three seeded regressions is detected with a recorded metric drop; a harness that cannot fail is rejected.
- Legacy cases produce byte-identical metrics before and after the refactor.
- The harness runs fully offline in CI.
- Stop if the work requires changing production ranking to make tests pass, or if fixtures cannot be made redistributable.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/004-status.md` records the candidate SHA, case counts per tier, the negative-control table with observed deltas, two-run determinism evidence, the equivalence proof for legacy metrics, and an explicit statement that no ranking change was made.
