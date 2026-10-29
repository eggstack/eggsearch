# M001 Closure — Benchmark Contract and Reference Corpus

Status: closed
Implementation candidate: `eggstack/eggsearch@310611b733c2067ddeab184618156ef072934a6e`
Production baseline: `eggstack/eggsearch@7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b`
Corpus: schema 1, `retrieval-quality-v1`, fingerprint `fnv1a64:b882ce3d844e8ace`
Evaluator: `retrieval-quality-evaluator-v1`
Plan: `plans/implementation/deterministic-retrieval-quality/001-benchmark-contract-and-reference-corpus.md`

## Requirement evidence

| Requirement | Evidence | Outcome |
|-------------|----------|---------|
| Versioned task manifest with search dimensions, judgments, URL aliases, temporal window, provider constraints, expected outcomes, and uncertainty | `tests/fixtures/retrieval_quality/cases.json`; corpus provenance and uncertainty; `tests/retrieval_quality.rs` schema validation | 15 synthetic cases across coding error, repository symbol/docs/issue/release, security/advisory applicability, package registry, multi-source research, generic web, freshness/date, adversarial injection, expected absence, and provider failure. Six tasks are held out. |
| Rights-safe reference data and annotation notes | `tests/fixtures/retrieval_quality/README.md` | Original synthetic text only. No copied upstream text, secrets, or personal queries. One author; no independent adjudication. |
| Offline replay through current adapter/tool seam | `synthetic_reference_envelope_replays_through_search_adapter_and_tool_handler` | A synthetic envelope traverses `MockEngine`, `MetadataSearchAdapter`, and `run_web_search`; no runtime network, inference, or Python dependency. Fixture scoring remains explicitly labeled an evaluator contract, not a search-quality measurement. |
| Ranked retrieval and passage metrics with denominators and uncertainty | `tests/retrieval_quality.rs`; `docs/retrieval-quality-benchmark.md` | Precision@5, recall@5, MRR@5, nDCG@5, passage hit@1,200 characters, source diversity, evidence coverage, and expected-state correctness are reported separately. Incomplete labels exclude unjudged results. Wilson intervals are included for passage and status rates. |
| Status, ambiguity, deterministic behavior, and a known degraded case | `tests/retrieval_quality.rs` | Result/absence/failure are distinct; URL aliases use core canonicalization and preserve query strings; ambiguous aliases are rejected; out-of-window results do not match temporal gold; two offline runs match; a missing relevant rank fails the MRR/nDCG gate. |
| Opt-in live observation path and ceilings | Ignored `retrieval_quality_live_observation` in `tests/retrieval_quality.rs`; operator instructions in `docs/retrieval-quality-benchmark.md` | Calls the existing `web_search` handler with explicit query and provider allowlists, separate paid-provider acknowledgement, 20-case/8-provider/160-attempt ceilings, and a per-search timeout. Reports hashes and bounded metadata only. Not run in this closure because no explicit provider/query/billing cohort was supplied. |
| Exact reference report and M002–M005 handoff | `docs/retrieval-quality-benchmark.md`; updated roadmap, registry, and implementation plan statuses | Frozen synthetic report is keyed to this candidate and baseline. M002–M005 are now ready; M006 remains blocked on their completion and operational evidence. |

## Frozen synthetic report

The report was emitted from the exact candidate with
`EGGSEARCH_BENCH_SOURCE_SHA=310611b733c2067ddeab184618156ef072934a6e`.
It covered 15 tasks (13 ranking cases, 15 gold documents, 57 precision rank
slots, 15 passages, 15 state labels). Aggregate fixture metrics were
precision@5 0.3385, recall@5 0.9615, MRR@5 0.9615, nDCG@5 0.9552, passage
hit@1,200 characters 0.9333, source diversity@5 0.8462, and state accuracy
1.0000. The low fixture precision reflects the deliberately short synthetic
result lists under fixed precision@5; these metrics verify the evaluator and
are not live provider quality scores. Category denominators and result hashes
are emitted in the machine-readable test report and summarized in the benchmark
documentation.

Partition report: training had 9 ranking cases (precision@5 0.3778, recall@5
0.9444, MRR@5 0.9444, nDCG@5 0.9353, passage hit 9/10); held-out had 4
ranking cases (precision@5 0.2500, recall@5 1.0000, MRR@5 1.0000, nDCG@5
1.0000, passage hit 5/5). The held-out set is tiny and not evidence of
generalization; its purpose is to pin partition separation.

## Verification

| Command | Result |
|---------|--------|
| `cargo test --locked --features mock --test retrieval_quality` | 7 passed, 1 ignored |
| `cargo test --locked --all-features --test tool_surface_evaluation --test retrieval_attempt_ledger` | 50 passed |
| `cargo test --locked --all-features --test provider_routing --test web_search_integration --test repo_workflow --test security_workflow --test research_workflow` | 383 passed |
| `make check` | Passed on the exact implementation tree; formatting, clippy, no-default check, all-features tests, hygiene, dependency policy, and packaging contract passed |
| `make bench-check` | Passed; Criterion harness compiled with all features and was not run as a threshold |
| `make docs-check` | Passed with rustdoc warnings denied |
| `git diff --check` | Passed |

The ignored live test was not run and no external query or billable request was
made. The report records cache state as unavailable through the current search
contract and retries as unavailable telemetry rather than inferring values.

## Compatibility, safety, and limitations

- No production ranking, provider routing, default feature, MCP, CLI, SDK,
  Python, source identity, cache, or release artifact changed.
- Keyless HTML providers remain first-class; no inference service, model, paid
  provider, network call, or new runtime dependency is needed by CI.
- The live path uses existing provider and fetch policy, does not bypass
  CAPTCHA/rate limits, and redacts content by emitting only bounded counts and
  hashes. Billable providers require both a per-provider list and a separate
  explicit billing acknowledgement.
- The small synthetic corpus is not statistically representative. Its
  annotations are not independently adjudicated. Real live relevance, paid
  costs, repeat-cohort variance, p50/p95 provider comparisons, retry counts,
  and cache-state controls remain operational observations for opt-in runs.
- No unresolved correctness or security findings. The live observations are
  deliberately unrun, not treated as zero-result evidence.

## Recommendation

Closed. The offline evaluation contract, synthetic reference corpus, baseline
report, regression gate, and bounded opt-in live observation entry point are in
place. M002–M005 may begin against the frozen corpus and exact production
baseline; no retrieval-quality improvement is claimed by M001.
