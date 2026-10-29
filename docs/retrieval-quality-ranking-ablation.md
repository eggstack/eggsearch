# Deterministic Relevance and Selection Experiment

M004 used the frozen M001 evaluator and existing ranking/workflow regression suites to determine whether a ranking change had support. No ranking or selection code changed.

## Baseline

The synthetic corpus is `retrieval-quality-v1`, fingerprint `fnv1a64:b882ce3d844e8ace`, evaluated by `retrieval-quality-evaluator-v1`. The corpus has 15 cases (9 train, 6 held out). In the held-out partition, 4 ranking cases contain 5 relevant documents and 5 judged passages; two additional cases cover absence and provider failure.

| Held-out metric | Result | Denominator |
|---|---:|---:|
| Recall@5 | 1.000 | 5 relevant documents across 4 ranking cases |
| MRR@5 | 1.000 | 4 ranking cases |
| nDCG@5 | 1.000 | 4 ranking cases |
| Passage hit at 1,200 characters | 1.000 | 5 passages |
| Status correctness | 1.000 | 6 cases |
| Precision@5 | 0.250 | 20 result slots across 4 ranking cases |

The corpus is self-authored synthetic truth, not independent adjudication. Perfect held-out retrieval on this six-case fixture leaves no credible basis to claim a general ranking lift. M003 also has no real provider-attributed snapshot series, and M002 has no real browser-rendered passages to use as comparable ranking inputs. A candidate tuned against these tiny fixtures would risk overfitting.

## Decision

Keep existing RRF, intent and focus behavior. Do not add score normalization, per-provider diversity, lexical/BM25 re-ranking, candidate truncation changes, or a persistent index. The no-change result is the accepted outcome: available evidence does not establish a reliable quality lift, and current held-out metrics are already at ceiling. No held-out labels were added to production heuristics.

Future ranking changes require a new corrective plan with independently adjudicated held-out tasks, a declared pre-candidate threshold, per-category non-regression checks, deterministic ties, and bounded time/memory. The existing regression suites continue to protect IDs, trust, exact provider selection, failure/absence distinctions, workflow authority, and deterministic local selection.

## Verification

- `make eval-retrieval-quality`: 8 passed, 1 opt-in live test ignored; synthetic result/corpus fingerprints stable.
- `cargo test --locked --features mock --test web_search_integration --test repo_workflow --test security_workflow --test research_workflow --test local_workspace_integration`: 310 passed.
- `cargo test --locked --all-features --test schema_identity_registry --test retrieval_attempt_ledger --test provider_routing --test structured_local_code_intelligence`: 228 passed.
- Property and full repository gates are recorded in the M004 closure report.
