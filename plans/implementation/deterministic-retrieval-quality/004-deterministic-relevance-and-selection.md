# M004 — Deterministic Relevance, Passage Selection and Ranking Experiments

Status: blocked on M001 baseline contract
Source: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` M004
Primary class: capability if measured lift is accepted; otherwise infrastructure evaluation
Baseline: `7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b`
References: `plans/000-long-term-specification.md` ``3, 4, 7-9; ADR-0001 and ADR-0006; `architecture/core.md`; `architecture/local-workspace.md`; `src/meta/fetch_ranking.rs`; `src/core/focus.rs`.
Dependencies: hard M001; soft M002 (feature-extraction quality) and M003 (marginal provider observations). No LSP/CodeGG changes are prerequisites.

## Objective

Measure reproducible ranking and evidence-selection quality improvements **without inference**. Evaluate whether bounded lexical/scoring refinements improve retrieved evidence for web, repository and specialist workflows, while preserving deterministic identity, provenance, failure/absence semantics, RRF behavior, and the default no-key/no-model path.

## Current implementation evidence

`src/meta` combines provider rankings with RRF and typed research/repo/security postprocessing. `src/meta/fetch_ranking.rs` scores suggested fetches with provenance, confidence, role, hints; `src/core/focus.rs` and fetch span/render modules implement lexical chunk selection with heading/symbol and adjacency signals. `src/meta/local_backend.rs` has bounded git-aware inventory and `src/meta/local_symbols.rs` supplies Rust/Python/JS-TS/Go structural parsing with lexical fallback. Existing perf suites lack independently labeled relevance holdouts. The CodeGG daemon owns LSP authority; Eggsearch must not create a second semantic language service.

## Work packages

1. **Establish controlled ablations** against M001 frozen labels and baseline commit. Evaluate existing RRF + intent + focus, score normalization, candidate truncation, and per-provider diversity separately. Keep evidence roles and applicability labels distinct from a relevance judgment.
2. **Prototype pure bounded algorithms only where needed**: BM25-style per-document/passage lexical ranking, exact error/phrase and symbol/path boosts, structural heading matching, or local-index candidate selection. Compare individually and jointly against existing selection, never treat BM25 as automatically superior or import an embedding runtime.
3. **Time and memory bound every scorer.** Use caps on tokens, documents, term postings, temporary allocations, and file parses; overflow and malformed/Unicode input must fail or fall back predictably. Keep same caching and sanitized source text; no unbounded full-corpus preprocessing or persistent index migration. On cheap devices, a single shared lexical index or existing inventory may be preferable; measure before adding storage.
4. **Preserve behavioral equivalence at authority boundaries**: source stable IDs, canonical URL dedup, exact explicitly selected providers, native-vs-local constraints, deterministic tie ordering, `local_trusted` provenance vs instruction trust, warning ledger, query budgets, partial/absent distinctions, Mode-Off remote denial. Any intentional ranking policy change must be explicit, tested, documented and classified as an accepted contract amendment; do not silently rewrite `plans/000-long-term-specification.md` `7.
5. **Use holdout selection and cost gates.** Compare relevant-source recall@k, nDCG/MRR, passage hit@budget, inference-free task sufficiency, latency, throughput, RSS, output characters/tokens and per-category failures. Decide retain/rollback from fixed thresholds recorded **before** reading holdout outputs; report confidence intervals where meaningful, not one lucky corpus improvement.
6. **Prefer no-change closure when appropriate.** If no reliable quality lift exists, retain baseline ranking and close the experimental milestone with transparent negative results rather than forcing a new dependency.

## Constraints and compatibility

No neural model, hosted rerank inference, mandatory vector DB, LSP dependency, extra MCP tool, content summarization, or provider-specific heuristic that unfairly excludes HTML engines. No source-execution path or new broad filesystem authority. Existing Python SDK and MCP response schemas remain stable; rankings may vary only behind an explicit accepted change and refreshed corpus fixtures. Keep release binaries and optional features unchanged unless a separate future release plan authorizes changes.

## Verification

- M001 offline labeled retrieval harness and held-out selection report; repeat exact-SHA run to prove stable ordering.
- `cargo test --locked --features mock --test web_search_integration --test repo_workflow --test security_workflow --test research_workflow --test local_workspace_integration`.
- `cargo test --locked --all-features --test schema_identity_registry --test retrieval_attempt_ledger --test provider_routing --test structured_local_code_intelligence`.
- `cargo test --locked --all-features --test property_sanitize --test property_local_fs` plus targeted new tie/budget/Unicode tests.
- `make check`; `make bench-check`; `make docs-check`.

## Acceptance and stop conditions

A comparable baseline vs candidate report records each algorithm, corpus split, metric denominators, per-category regressions, execution time and resource impact. Accepted changes must beat predeclared held-out thresholds, have byte-bounded allocation/timeout tests and retain all safety invariants; an evidence-backed rejection is equally valid closure. Stop if candidate requires inference, changes source authority/IDs, shifts compiled default feature dependencies, or overfits fixtures. Closure `plans/closure/deterministic-retrieval-quality/004-status.md` records exact baseline/candidate SHAs, algorithm/threshold version, accepted/no-change rationale and tests.
