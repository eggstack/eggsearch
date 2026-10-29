# M004 Deterministic Relevance and Selection — Closure Status

Status: closed (no-change experiment)

Source plan: `plans/implementation/deterministic-retrieval-quality/004-deterministic-relevance-and-selection.md`

Roadmap: `plans/subsystems/deterministic-retrieval-quality-roadmap.md`

Implementation/evaluation candidate: `84472dfe010c498ab95a60b08153e6209c9ec5e5`

Baseline production commit: `7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b`

Report: `docs/retrieval-quality-ranking-ablation.md`

## Result

The frozen M001 synthetic corpus has 15 cases (9 train, 6 held out). On the held-out split, four relevance cases contain 5 relevant documents and 5 judged passages. Recall@5, MRR@5, nDCG@5, passage hit@1,200 characters and status correctness all score 1.0. Precision@5 is 0.25 over 20 top-result slots. Two other held-out tasks explicitly cover absence and provider failure.

These self-authored judgments are not independent truth. The ceiling scores and small denominators do not support a robust ablation claim. M002 lacks browser-rendered passages and M003 lacks real provider-attributed response snapshots. No candidate algorithm was promoted; RRF, intent weighting, focus extraction and selection remain unchanged. No inference, index, dependency, default feature or schema change was added.

## Verification

- `make eval-retrieval-quality`: 8 passed; 1 opt-in live observation ignored. Corpus fingerprint `fnv1a64:b882ce3d844e8ace` and evaluator revision `retrieval-quality-evaluator-v1` remained stable.
- `cargo test --locked --features mock --test web_search_integration --test repo_workflow --test security_workflow --test research_workflow --test local_workspace_integration`: 310 passed.
- `cargo test --locked --all-features --test schema_identity_registry --test retrieval_attempt_ledger --test provider_routing --test structured_local_code_intelligence`: 228 passed.
- `cargo test --locked --all-features --test property_sanitize --test property_local_fs`: 37 passed.
- `make check`, `make bench-check`, and `make docs-check` passed at the preceding candidate; this milestone changes only its evaluation report and planning closure. Exact evaluator and regression runs above passed for this candidate.

## Decision

Close with no ranking or selection change. A future candidate needs a new plan, independently adjudicated held-out tasks, thresholds declared before candidate comparison, per-category non-regression, and bounded time/memory evidence.
