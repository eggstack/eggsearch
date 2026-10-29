# Retrieval quality benchmark

The retrieval-quality harness measures ranked evidence relevance. It is
separate from `tool_surface_evaluation`, which measures whether a query points
to the right MCP tool.

## Offline contract

`tests/fixtures/retrieval_quality/cases.json` is schema version 1 and contains
15 synthetic cases across coding errors, repository symbols/docs/issues/releases,
security advisories and applicability, package registries, multi-source
research, generic web, freshness/date constraints, and adversarial injection.
All result envelopes and passages were authored for this fixture. No provider
text, credentials, or personal queries are tracked. The corpus is auditable but
not an unbiased estimate of live relevance: it has one annotation author and
no independent adjudication. Six cases are explicitly held out from tuning.

The Rust evaluator reports macro precision@5, recall@5, MRR@5, nDCG@5,
passage hit within a 1,200-character budget, source diversity, evidence
coverage, and expected-state accuracy. Passage-hit and status metrics include
Wilson 95% intervals and trial counts; small corpus categories are descriptive.
Precision and nDCG exclude unjudged
results when judgments are incomplete; denominators are included. URL aliases
are canonicalized for scheme case, `www`, query, fragment, and trailing slash.
Grades are 0–3, with zero reserved for judged irrelevant results. Per-category
metrics are available from the evaluator. A deliberately degraded ranking
must fail its MRR/nDCG check.

Run the offline suite with:

```bash
cargo test --locked --features mock --test retrieval_quality
cargo test --locked --features mock --test retrieval_quality -- --nocapture
make eval-retrieval-quality
```

The corpus fingerprint and metric output are deterministic. The fixture replay
does not perform HTTP, call an inference service, or use Python. The fixtures
characterize evaluator behavior; they are not provider search snapshots and
must not be reported as a measured Eggsearch quality baseline.

### Frozen synthetic evaluator baseline

Reference implementation SHA: `7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b`.
Corpus: `retrieval-quality-v1`, fingerprint `fnv1a64:397f0c320097dbe5`.
Evaluator: `retrieval-quality-evaluator-v1`, schema version 1, mock feature.
The emitted JSON report includes per-case result hashes and per-category
denominators. Aggregate oracle-fixture metrics:

| Measure | Value | Denominator |
|---------|------:|------------:|
| Precision@5, macro | 0.3385 | 57 judged rank slots across 13 ranking cases |
| Recall@5, macro | 0.9615 | 13 ranking cases / 15 gold documents |
| MRR@5, macro | 0.9615 | 13 ranking cases |
| nDCG@5, macro | 0.9552 | 13 ranking cases |
| Passage hit@1,200 characters | 0.9333 | 14 of 15 judged passages |
| Source diversity@5, macro | 0.8462 | 13 nonempty-result cases |
| Expected-state accuracy | 1.0000 | 15 labeled outcomes |

This baseline freezes evaluator and fixture behavior only. It establishes no
live retrieval quality, cross-provider winner, latency, cost, or superiority
claim. The original implementation SHA is unchanged in production code from
the specified M001 planning baseline; intervening commits only registered the
roadmap and plans.

## Opt-in live observations

The ignored `retrieval_quality_live_observation` test calls the existing
`web_search` handler. It sends only explicitly selected corpus queries and
provider IDs, with ceilings of 20 queries, 8 providers, 160 provider-query
attempts, and a 10-second tool timeout. No live test runs in routine CI. It
scores returned cards against the corpus and prints bounded metadata, result
hashes, response byte counts, timestamps, p50/p95 latency, and aggregate
metrics; result content, credentials, and raw URLs are not put in the report.

An operator must set all of the following explicitly:

```bash
EGGSEARCH_RETRIEVAL_LIVE=1 \
EGGSEARCH_BENCH_SOURCE_SHA=<40-hex-source-sha> \
EGGSEARCH_RETRIEVAL_PROVIDERS=searxng \
EGGSEARCH_RETRIEVAL_PAID_PROVIDERS= \
EGGSEARCH_RETRIEVAL_LOCALE=en-US \
EGGSEARCH_RETRIEVAL_CASES=generic-web-001,freshness-date-001 \
  cargo test --locked --features mock --test retrieval_quality -- --ignored --nocapture
```

Paid providers must appear in both `EGGSEARCH_RETRIEVAL_PROVIDERS` and
`EGGSEARCH_RETRIEVAL_PAID_PROVIDERS`, and require the separate
`EGGSEARCH_RETRIEVAL_ALLOW_BILLING=1` acknowledgement. Ambient credentials
alone never select or bill a provider. Each provider must already be configured
in Eggsearch. Reference providers are observed one provider at a time through
the production adapter; SearXNG is supported when configured as a provider.
To measure a direct provider path, select that provider alone in Eggsearch;
to measure the credentialed Eggsearch path, select its configured credentialed
provider and declare it in the paid-provider list. The runner adds no providers.
Use separate runs and preserve the same case IDs and task constraints.

The live tool response does not expose a cache-state control, so the report
records that limitation rather than claiming a cache condition. It also does
not assert source-level passage equivalence where the live response omits an
excerpt. Record the exact source SHA, provider configuration, machine, locale,
time window, and whether an operator repeated a cohort alongside any retained
report. Do not commit raw upstream content. A 429, timeout, provider error, or
zero-result outcome is an observed provider outcome and never evidence of
expected absence.

## Handoff contract

M002–M005 can add versioned cases and separate recorded envelopes only when
redistribution rights are documented. Keep the held-out partition fixed;
candidate heuristics must not use held-out judgments. Keep retrieval metrics
separate from latency, bytes, estimated token cost, and paid request cost.
Missing annotations, unavailable connectors, and unsupported task classes are
reported as unscorable or skipped with denominators, never assigned a zero
quality score.
