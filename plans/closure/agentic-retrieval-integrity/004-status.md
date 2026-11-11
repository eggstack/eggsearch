# M004 Candidate Snapshot Ranking Benchmark — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/004-candidate-snapshot-ranking-benchmark.md`

Corrects: `plans/implementation/deterministic-retrieval-quality/004-deterministic-relevance-and-selection.md`
and `plans/closure/deterministic-retrieval-quality/004-status.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `cd81f4d5e966364918fe2be25485b8ebbf424e71` (origin/main, 2026-11-10 UTC)

Implementation candidate: `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab`, the implementation commit immediately preceding this
closure commit. The closure record, roadmap status, and registry entry moved in the same closure
commit.

## Result

Ranker changes are now measurable offline. The post-dispatch sequence moved into
`src/meta/adapter/selection.rs::select()`, and both production and the harness call it:

`aggregate_rrf_with(rrf_k, dedup)` → `convert_aggregated` → local domain filter →
`apply_intent_reranking` → truncate.

`SelectionParams::production()` is the only value production passes: `RRF_K = 60`, dedup on,
truncate after rerank. The three seeded negative controls pass explicit alternatives through the
same body — never a copy.

`src/meta/adapter/selection_test_harness.rs` is `mock`-gated and re-exported from `src/lib.rs` as
`selection_test`; nothing is exposed in a default build. `aggregate_rrf_with` now drains its dedup
map in ascending normalized-key order, so exact ties cannot depend on hash iteration; the
production path already had a total order on `(score, title, url)`, so the pre-sort is unreachable
as a tie-break there.

## Corpora

| Tier | File | State |
|---|---|---|
| 1 — structured synthetic | `tests/fixtures/retrieval_quality/candidate_snapshots.json` | 11 cases, schema v2, committed |
| 2 — recorded real | `tests/fixtures/retrieval_quality/recorded_snapshots.json` | `"cases": []` with recorder instructions; deliberately not fabricated |
| 3 — recorded, untracked | `target/snapshots/` | `#[ignore]`d recorder, path-guarded (rejects any path outside `target/snapshots/` and any `.`/`..` component) |

Tier-1 coverage: rank inversion, duplicate cluster across three hosts, domain concentration,
provider agreement, provider disagreement, documentation-window promotion, code-intent promotion,
and four lexical-intent cases (security, releases, docs, code) added by M005's gate slice. Two
cases deliberately fail under the neutral prior, so the corpus is not rigged. Gold judgments were
authored before the provider lists.

New metrics: `duplicate_slots`, `distinct_domains_topk`, `authoritative_survival` (grade ≥ 2 kept
in the top k).

## Negative controls

| Control | Metric | Production | Seeded | Delta |
|---|---|---:|---:|---:|
| `rrf_k = f64::INFINITY` | authoritative survival | 0.8182 | 0.4545 | −0.3636 |
| | MRR@5 | 0.7429 | 0.3095 | −0.4333 |
| | distinct domains (`domain-concentration-001`) | 2.0 | 1.0 | −1.0 |
| `dedup = false` | duplicate slots | 0 | 9 | +9 |
| | duplicate slots (`duplicate-cluster-001`) | 0 | 2 | +2 |
| `truncate_before_rerank = true` | authoritative survival | 0.8182 | 0.5455 | −0.2727 |
| | authoritative survival (both promotion cases) | 1.0 | 0.0 | −1.0 |

Each control asserts the specific metric it must move. The degenerate-RRF control leaves
`duplicate_slots` at 0 by construction, so its diversity signal is asserted on MRR/nDCG/authoritative
survival plus the per-case domain count; the dedup control raises duplicate slots while slightly
raising nDCG on this corpus, which is why `duplicate_slots` is the asserted metric there.

## Legacy equivalence

`cases.json` and every existing judgment are untouched. The aggregate report kept the corpus
fingerprint `fnv1a64:b882ce3d844e8ace` and identical result hashes; a recursive diff over the whole
report showed only additive keys (`authoritative_survival`, `duplicate_slots`,
`duplicate_slot_rate`, `distinct_domains_topk`, and their denominators). No `pre-existing`
key/value changed.

## Determinism

`make eval-retrieval-quality` run twice produced byte-identical
`retrieval-quality-candidate-snapshot-report` and `retrieval-quality-derived-intent-rollout` blocks,
including row order, plus an in-test replay comparison of every result vector and the aggregate.

## Verification (combined candidate)

- `make eval-retrieval-quality`: 13 passed, 2 ignored, 0 failed, repeated twice.
- `cargo test --locked --features mock --test retrieval_quality`: 13 passed, 2 ignored.
- `cargo test --locked --features mock --test web_search_integration`: 70 passed.
- `cargo test --locked --all-features --test mcp_tools --test static_guards`: 5 + 60 passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `make check`: passed through the full `--all-features` suite (5,848 passed, 26 ignored, 0 failed)
  and hygiene; stops at the pre-existing `dependency-policy` advisories. `Cargo.lock` unchanged.

## Explicit statement

**No production ranking change was made.** RRF constant, dedup, domain filtering, intent and
freshness boosts, tie ordering, and truncation are unchanged; only their implementation moved and
they gained explicit parameters.

## Findings and residual risk

- `meta::engines::normalizer::normalize` does not strip a leading `www.`, while
  `core::identity::canonicalize_url` and the judgment matcher do. A `www.`/bare alias therefore
  occupies two production slots. The first fixture draft reproduced this and was moved to aliases
  `normalize()` actually merges; the gap is recorded here and needs its own corrective plan rather
  than a silent fixture workaround.
- Tier 2 is empty. Populating it requires the live recorder in an environment with provider access.
- The recorder's "no candidate rows → refuse to write" branch is code-asserted but not executed
  here.
- `distinct_domains_topk` is insensitive to the intent promotions in the current cases: each
  promotion displaces one filler on a different host, so the count is invariant.
