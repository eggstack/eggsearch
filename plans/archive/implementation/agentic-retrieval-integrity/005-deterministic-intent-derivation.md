# M005 — Deterministic Intent Derivation

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M005
Primary class: infrastructure
Baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71`
References: `src/core/query.rs` (`SearchIntent`); `src/meta/planner.rs`; `src/core/repo_query.rs` (`RepoQueryHints::parse`); `src/core/error_query.rs`; `src/meta/adapter/normalization.rs` (`apply_intent_reranking`); `architecture/meta.md` § 3.
Dependencies: soft M004 — M004 supplies the snapshot lane used to accept an applied (non-observe-only) rollout.

## Objective

Derive the caller's retrieval intent from the query itself with fixed precedence and reported reasons, so intent-aware reranking and later routing work for agents that omit the `intent` argument, without adding a required field or a new failure mode.

## Why derivation rather than a new argument

The local-inference qualification recorded repeated invalid `research_domain`/`goal` values and repair turns from small models. Adding an `objective` or `goal` argument to `web_search` would add a new required decision, a new enum to violate, and a new repair path. The retrieval layer already knows how to classify queries (`RepoQueryHints`, exact-error parsing); the missing piece is a single documented classification with observable reasons.

## Non-goals

- No new MCP tool argument. `web_search` keeps its current schema; `intent` remains optional and authoritative when supplied.
- No change to `build_generic_query`. Derived intent must not trigger the existing code/issues/releases term-stuffing rewrites; that is a separate measured decision.
- No query expansion, synonym generation, or multi-query fan-out.
- No provider routing change — that is M006 and depends on M004 evidence.
- No inference, model, or embedding classifier.

## Invariants that cannot regress

- An explicitly supplied `intent` always wins; existing callers see unchanged behaviour.
- Query text sent to providers is unchanged by this milestone.
- Stable IDs, RRF ordering, dedup, and truncation are unchanged.
- Classification is a pure function: no clock, no network, no randomness, bounded input length.
- The response stays additive; missing or unclassifiable input falls back to the existing default.

## Expected production changes

1. **Pure classifier.** `src/meta/intent.rs` with `derive_intent(query, &RepoQueryHints) -> DerivedIntent { intent, reasons: Vec<&'static str>, confidence }`. Fixed precedence, evaluated once, first match wins:
   1. explicit caller intent (handled by the caller, not the classifier);
   2. error shapes — compiler codes, panic/traceback markers, stack-frame syntax;
   3. security advisory identifiers and vulnerability vocabulary;
   4. dependency/version syntax;
   5. migration and upgrade vocabulary;
   6. comparison vocabulary;
   7. release/changelog vocabulary;
   8. issue/pull-request vocabulary;
   9. code-shaped tokens — identifiers, file paths, extensions, symbol syntax;
   10. documentation vocabulary;
   11. news and recency vocabulary;
   12. fallback `web`.
2. **Optional threading.** `WebSearchArgs.intent` is already `Option<SearchIntent>`; pass the option to the adapter instead of collapsing it with `unwrap_or_default`, so "omitted" is distinguishable from "explicitly web".
3. **Consumption.** Reranking consumes the derived intent when the caller omitted one. The planner continues to consume only the caller's value, preserving query text.
4. **Reporting.** Additive response fields `derived_intent` and `derived_intent_reasons`, present in standard and diagnostic detail; compact carries the intent string only.
5. **Rollout gate.** Land with derivation reported but observe-only. Apply it to reranking only when M004's snapshot lane shows no regression against `intent = web` on the tier-1 corpus; otherwise keep observe-only and record why in the closure record.

## Storage and compatibility effects

- Additive response fields only; no MCP schema change, no storage, no migration.
- No change to planner output, provider selection, or warnings.

## Ordered work packages

1. Implement the classifier with a table-driven test corpus covering each precedence class, boundary cases, and near-miss negatives.
2. Add determinism and bounded-input tests, including a 512-character maximum query.
3. Thread the optional intent through `run_web_search` and the adapter without changing planner inputs.
4. Add response fields and projection tests for compact, standard, and diagnostic.
5. Run the observe-only comparison over the M004 tier-1 corpus and record either non-regression or the reason for staying observe-only.
6. Update `architecture/meta.md` § 3 (planner inputs and derived intent), `docs/agent-workflows.md`, and `docs/test-inventory.md`.

## Verification

- `cargo test --locked --features mock --test web_search_integration`
- `cargo test --locked --features mock --test retrieval_quality`
- `cargo test --locked --features mock --test mcp_projection --test mcp_tools`
- `cargo test --locked --all-features --test property_retrieval --test retrieval_attempt_ledger`
- `make check`; `make docs-check`

## Acceptance and stop conditions

- Every precedence class has at least one positive and one near-miss negative case.
- An explicit `intent` produces byte-identical output to baseline.
- Query text sent to engines is byte-identical to baseline for every corpus query.
- Observed-only mode adds fields and changes no ranking; applied mode requires snapshot non-regression evidence.
- Stop if classification requires a model, changes planner rewrites, or needs a new argument.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/005-status.md` records the candidate SHA, the classifier table, the observe-only comparison result, the explicit observe-only-or-applied decision with its evidence, and confirmation that planner output and query text are unchanged.
