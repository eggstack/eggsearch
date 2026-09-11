# Tool-Surface Evaluation Corpus

Deterministic Layer 1 (retrieval/discovery) fixtures for the agentic
tool-surface evaluation plan. Offline, no model or network access.

## Layout

- `cases.json` — 43 labeled tool-selection fixtures.
- Runner: `tests/tool_surface_evaluation.rs` (routine CI).
- Opt-in live comparison: `tests/tool_surface_live.rs` (`#[ignore]`d).

## Fixture shape

Each fixture records `id`, `category`, `query`, `expected_primary`,
`acceptable` (always includes the primary), `expected_followups`,
`forbidden_primary`, and `why` (documents the preferred tool and which
alternatives are acceptable).

## Categories

| Category | Cases | Expected primary |
|----------|-------|------------------|
| `generic_web` | 5 | `web_search` |
| `repo` | 6 | `repo_search` (5), `repo_map` (1) |
| `fetch` | 5 | `web_fetch`, `repo_fetch` (2), `batch_fetch` (2) |
| `exact_error` | 4 | `repo_search` (`exact_error` path) |
| `security` | 5 | `security_search` (4), `web_search` (1 generic how-to) |
| `research` | 5 | `research_search` |
| `evidence` | 2 | `build_evidence_bundle` |
| `diagnostics` | 4 | `provider_status` (2), negatives (2) |
| `ambiguity` | 7 | preferred tool plus documented acceptable alternatives |

Every stable tool appears as `expected_primary` at least once, so the
runner can prove all capabilities remain discoverable. Negatives pin
`provider_status` and `build_evidence_bundle` out of ordinary research.

## Metrics and gates

The runner scores each query against live tool definitions (real
description-token overlap plus curated domain vocabulary), then reports
top-1 accuracy (rank 1 in `acceptable`), recall@3, MRR, compact
discovery bytes, and serialized definition bytes with estimated tokens
(bytes/4, ceiling). CI gates:

- each description at most 1000 chars;
- total advertised definitions at most 86000 bytes;
- server instructions at most 6000 bytes;
- compact top-3 discovery at most 512 bytes;
- top-1 accuracy at least 0.90, recall@3 at least 0.95, MRR at least 0.90;
- every `acceptable`/`expected_followups`/`forbidden_primary` reference
  and every `next_actions` hydration target is a known stable tool.

## Baseline (v0.3.8)

- total definition bytes: 77952 (~19488 estimated tokens);
- largest single tool: `build_evidence_bundle` (39683 bytes of schema);
- longest description: `repo_fetch` (825 chars);
- server instructions: 5275 bytes;
- compact top-3 discovery: at most 273 bytes;
- deterministic discovery: 43/43 top-1, recall@3 1.0, MRR 1.0;
- fingerprint:
  `eggsearch-0.3.8|tools=batch_fetch:433,build_evidence_bundle:445,provider_status:94,repo_fetch:825,repo_map:559,repo_search:667,research_search:753,security_search:543,web_fetch:572,web_search:501|bytes=77952`.

Re-run after disclosure or schema changes and record the new fingerprint
with the delta; context reduction must be measured end to end, so bytes
moving from definitions into discovery output do not count as success.

## Live comparison (manual only)

```bash
EGGSEARCH_EVAL_MODEL=vendor/model/version EGGSEARCH_EVAL_SURFACE=compact-topk3 \
  cargo test --locked --all-features --test tool_surface_live -- --ignored --nocapture
```

Never commit transcripts containing secrets or uncontrolled fetched
content; only the metric report is kept.
