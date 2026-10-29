# Deterministic Retrieval Quality and Comparative Evaluation Roadmap

Status: active; M001-M005 closed; M006 conditionally closed pending repository-hardening M011
Planning baseline: `7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b` (main, 2026-10-28 UTC)

References: `plans/000-long-term-specification.md` ``1, 2, 3, 4, 5, 7, 8, 9; `plans/001-terminology-and-domain-model.md` ``1-5; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; accepted ADR-0001, ADR-0002, ADR-0006.
Prior closed evidence: `plans/subsystems/search-capability-roadmap.md`; `plans/subsystems/performance-optimization-roadmap.md`; `plans/subsystems/tool-surface-consolidation-roadmap.md`; `plans/closure/mcp-tool-surface-consolidation/006-status.md`.

## 1. Purpose and ownership

Establish *measured* comparative retrieval quality, optional-extraction fidelity, provider contribution, and agent-context efficiency. Then implement only deterministic optimizations that demonstrate value. Eggsearch owns discovery/fusion/selection, bounded fetch, evidence metadata, benchmark methodology, and MCP/SDK projections. CodeGG owns model decisions, policy, LSP semantic answers, the working directory, and frontend disclosure/hydration. This workstream does not reimplement a crawler or agent.

### Work classification

- **Invariant:** keyless HTML metasearch first-class and no-key functional; inference-free deterministic results; ten existing tools and SDK/Python parity; stable IDs, trust/provenance, explicit failures, deadlines, Mode-Off, source scopes and local budgets.
- **Capability:** improved relevant-evidence yield and extraction fidelity when optional features are enabled; improved usable evidence per agent context budget when supported by measured results.
- **Infrastructure:** repeatable gold datasets and source snapshots; feature-variant benchmark harness; labeled metrics and cost/latency telemetry; reproducible comparisons.
- **Polish:** reporting, documentation corrections and developer ergonomics only after correctness.

## 2. Current state and gaps

The baseline has 44 provider IDs, keyless default DuckDuckGo/Startpage/Yahoo, RRF/intent reranking, query-focused lexical extraction, provider-specific capability enforcement, separate `pdf`/`browser` source features, 10 MCP tools, compact/standard/diagnostic response modes, and extensive offline contract tests. Existing `tests/fixtures/tool_surface/` evaluates **tool choice**, not search recall or passage relevance. `benches/perf.rs` primarily benchmarks component performance rather than comparative end-to-end retrieval. `tests/browser_transport.rs`/`tests/browser_live_smoke.rs` and PDF fuzz/contract coverage exist, but not a controlled default-vs-feature extraction matrix with reference truth. No objective comparative web/repo/security/research quality baseline or provider marginal-contribution series is established.

## 3. Invariants and non-goals

- The SearXNG-derived/keyless HTML design is intentional. Do not replace keyless defaults with paid API dependencies, or penalize providers merely because they use HTML. Failures remain observable with bounded retry/cooldown; no CAPTCHA solving or rate-limit circumvention.
- No Eggsearch-side inference, generative answer synthesis, model-generated snippets, embeddings, vector index, or LSP dependency. Existing opt-in Exa/Tavily etc. integrations remain unchanged.
- No new MCP tools, schema-breaking changes, default features, compulsory Chrome/`lopdf`, new binary SKU, or Python wheel feature expansion.
- No recursive crawling, unbounded exploration, automatic billing, credential tests in routine CI, or on-board SBC compilation.
- Avoid changing existing ranking weights or provider selection solely to improve microbenchmarks; candidate algorithms require independently recorded comparative evidence and deterministic equivalence/safety review.

## 4. Measurement architecture and acceptance

Maintain two strictly separate lanes:

1. **Offline/controlled lane (routine CI, no network, no inference):** checked-in licenses/rights-cleared fixtures, stable synthetic HTML/PDF/JS cases, recorded permitted provider envelopes where appropriate, labeled source/passage truth, deterministic replay, unit-level metrics and security invariants. This lane gates semantic regressions and budgets without expecting live upstream reproducibility.
2. **Opt-in live observation (manual/dispatch, never required for normal CI):** real keyless queries and separately authorized credentialed/direct competitor baselines. Record exact revision, feature build, machine, browser executable/version, provider configuration, locale/time, anonymized task ID, retrieval outcomes, request/byte/token and time budgets, redirects, cache state, billable requests, and raw/sanitized evidence hashes. Restrict secrets and third-party text from committed reports. Use time-window cohorts and repeat observations; never treat a transient upstream failure as stable comparative truth.

Primary measures: source recall@k, precision@k, nDCG@k/MRR (where labels exist), gold-document coverage, relevant passage hit@budget, evidence sufficiency/unsupported-claim risk, status correctness (failure vs absence), time-to-first-useful-evidence, p50/p95 latency, provider marginal contribution, bytes/tokens per useful evidence, memory/CPU and feature-build size. Separately report paid query cost when known. Coverage and score definitions must be frozen before tuning and holdout labels must not be incorporated in candidate heuristics. Where no gold labels exist, classify as diagnostics, not quality score.

Comparisons should separate: Eggsearch keyless default; each independently selected keyless provider; explicitly configured keyless aggregators (e.g. SearXNG); configured credentialed Eggsearch path; and **direct** API/provider baseline using the same underlying provider where applicable. Never compare a free/default and paid/API-backed path without explicitly labeling capabilities, cost and query equivalence. Pin corpus version and record unavailable or unsupported reference connectors without inventing scores.

Extraction builds: default HTTP-only; `--features pdf`; `--features browser`; `--features "pdf browser"`. Each needs runtime configuration enabled separately; compiled availability is not equivalent to attempted or successful extraction. Include HTML, JS shells, static and text PDFs, scanned/image-only PDFs with explicit unsupported/low-quality result, and mixed batches. Run source feature builds on appropriately provisioned desktop/CI, not SBC; browser absence is a recorded skip, never a pass. `--all-features` is a separate regression gate, not an extraction baseline (it also enables `egress`).

## 5. Milestone dependency graph

```text
M001 benchmark contract, corpus and baselines (ready)
  +--> M002 optional PDF/browser extraction comparison (hard)
  +--> M003 keyless provider resilience/contribution (hard)
  +--> M004 deterministic relevance experiment (hard)
  +--> M005 MCP/SDK and CodeGG context-economics handoff (hard)
             M002 + M003 + M004 + M005
                         |
                         v
          M006 integrated comparative qualification (hard)
```

M002 and M003 are soft dependencies for M004's experimental code; no ranking/dispatch change can be accepted until their relevant differential measurements are available. M005 has an interface dependency on the existing CodeGG progressive-disclosure contract and may produce a downstream handoff without changing CodeGG. M006 is also operationally dependent on provisioned browser and explicitly opted-in live-reference access; missing optional paid credentials must not block closure of the offline baseline.

## 6. Milestones and exit boundaries

| Milestone | Class | Handoff | Dependencies | User-visible exit |
|---|---|---|---|---|
| M001 — evaluation contract and reference corpus | infrastructure | `plans/implementation/deterministic-retrieval-quality/001-benchmark-contract-and-reference-corpus.md` | ready; closed historical tool-surface baseline | Reproducible offline gold metrics and opt-in live runner with exact provenance |
| M002 — PDF/browser feature comparison | infrastructure/capability evaluation | `002-pdf-browser-extraction-matrix.md` | hard M001 | Four independently qualified builds, page/passage fidelity, cost/footprint comparison, no degradation of feature security |
| M003 — provider resilience/contribution evaluation | infrastructure/capability if justified | `003-keyless-provider-contribution-and-resilience.md` | hard M001 | Controlled failover/overlap and first-class keyless evidence, bounded optional routing changes only if justified |
| M004 — deterministic relevance/reranking experiment | capability if justified | `004-deterministic-relevance-and-selection.md` | hard M001; soft M002/M003 | Measured gain on held-out labels, or explicit no-change conclusion if gain cannot be established |
| M005 — agent context and CodeGG handoff | capability/infrastructure | `005-agent-context-efficiency-and-codegg-handoff.md` | hard M001; interface closed tool-surface contract | Lower evidence/context cost without loss of task-relevant evidence; consumer-owned changes tracked separately |
| M006 — comparative qualification and closure | infrastructure/polish | `006-comparative-qualification-and-closure.md` | hard M002-M005 | Repeatable feature/provider results and recorded decisions, stop/go evidence, fully reconciled roadmap |

## 7. Cross-cutting concerns

All experiments preserve explicit per-provider failure/absence ledger and capability-skip states, FNV identities, sanitized and untrusted framing, scoped authenticated local access, read-only MCP, eggfetch address pinning, browser public-network-only restrictions, profile partitioning, PDF page/char limits, and bound/timeout/circuit ownership. Only reproducible offline metrics enter routine CI. Any new on-disk benchmark artifact must avoid live source content and credentials; work with bounded metadata/hashes. Schema evolution is additive; runtime defaults and published artifacts remain unchanged unless a separate release plan is approved. Consult `architecture/testing.md` for test placement; use `make check`, `make bench-check`, `make docs-check` and narrow new suite runners.

## 8. Risks and decisions

HTML provider flux, dynamic search-result drift, changing competitor cost/terms, query-label bias, fixture overfitting, optional Chromium availability, PDF extraction limits, and local metadata trust need explicit reporting. Do not infer a global winner from one query class or one upstream outage. An experimentally worse variant is a valid result; record it rather than changing the baseline. Full OCR, wider browser automation, persistent semantic indexing, paid service mandate, default feature changes, and changing CodeGG LSP ownership require separate explicit decisions.

## 9. Closure and status

Every milestone needs its own `plans/closure/deterministic-retrieval-quality/NNN-status.md` with implementation SHA, tests and outputs, quality deltas by task, unchanged safety/policy evidence, failures, resource budgets, and release/consumer implications. M001 is closed at `310611b733c2067ddeab184618156ef072934a6e` with closure `plans/closure/deterministic-retrieval-quality/001-status.md`. M002 is closed with the browser rendering limitation and exact evidence in `plans/closure/deterministic-retrieval-quality/002-status.md`. M003 is closed with no routing change; live observations are an M006 input. M004 is closed with no ranking change because no independent held-out lift was established. M005 is closed with an Eggsearch-side CodeGG consumer handoff; downstream implementation remains consumer-owned. M006 is conditionally closed at candidate `10b06ab76434983cd9942e81bf2090fc9d6e31cc`: integrated Chrome/browser smoke, bounded keyless live observations, feature builds, compatibility checks, report, and focused exact-candidate suites are recorded in `plans/closure/deterministic-retrieval-quality/006-status.md` and `docs/retrieval-quality-comparison.md`. The canonical macOS test gate fails on a root-path alias assertion and an outside-root symlink traversal when following is enabled. Repository-hardening M011 is the corrective control point; close M006 fully only after M011 has regression evidence and the corrected macOS candidate passes the canonical gate. No ranking or provider-routing change was accepted. No later milestone is pre-closed.
