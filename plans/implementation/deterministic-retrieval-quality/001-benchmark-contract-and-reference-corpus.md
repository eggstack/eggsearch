# M001 — Comparative Retrieval Benchmark Contract and Gold Corpus

Status: closed
Source: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` M001
Primary class: infrastructure
Baseline: `eggstack/eggsearch@7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b`
References: `plans/000-long-term-specification.md` ``3-5, 7-9; `plans/001-terminology-and-domain-model.md` ``1-3; ADR-0001, ADR-0006; `plans/003-planning-process.md`.
Dependencies: no open hard/interface prerequisites. Prior tool-surface M006 (tool-selection evaluation) is closed, and must remain a separate measurement.

## Objective

Create a benchmark **of retrieval quality**, not tool selection. Ship a deterministic offline evaluator, a rights-safe gold dataset, and a strictly opt-in live observation runner that can compare keyless Eggsearch, credentialed Eggsearch, SearXNG, and direct supported providers without requiring external inference or billing for regular tests. Establish an immutable reference baseline before M002-M005 changes.

Closure evidence: `plans/closure/deterministic-retrieval-quality/001-status.md`.

## Current evidence

`tests/fixtures/tool_surface/cases.json` has 43 curated tool-discovery cases; it does not label retrieved source relevance. `tests/corpus/scenarios/` already holds controlled workflow fixtures. `benches/perf.rs` covers throughput/components. `tests/provider_routing.rs`, `tests/web_search_integration.rs`, `tests/research_workflow.rs` and `tests/retrieval_attempt_ledger.rs` test contract behavior rather than comparative live quality. `docs/tool-matrix.md` specifies compact/standard/diagnostic responses and failure semantics. Add benchmark data and code without duplicating these ownership surfaces.

## Scope and work packages

1. **Freeze methodology before tuning.** Define a versioned task manifest (IDs, query, language/region, search intent, task category, retrieval scope, max_results, relevance judgments at document and passage level, accepted URL/identity aliases, optional temporal validity window, provider restrictions, expected absence/failure distinction, annotation provenance and uncertainty). Include coding errors, repo symbol/doc/issue/release, security advisory plus applicability, package registry, multi-source research, generic web, freshness/date and adversarial injection. Supply explicit held-out partition and independent annotations/adjudication where feasible; do not use live upstream texts as unlicensed tracked fixtures.
2. **Build offline replay.** Reuse `mock` and the existing adapter/tool handlers; capture normalized input and bounded provider envelopes under explicit redistribution policy; synthetic fixtures always permissible. Emit stable machine-readable run metadata with source SHA, corpus/algorithm revision, build features, config fingerprint (secret-free), cache state, tie-break seed = none, and result hashes. No runtime HTTP, model, or Python dependencies.
3. **Implement metrics.** Precision@k, recall@k, MRR/nDCG, passage hit@character budget, document/source diversity, evidence coverage and status correctness. Score gold relevant IDs/URL aliases *after* canonicalization with explicit ambiguity handling. Penalize missing relevant results but exclude unlabelled items from metrics where judgments are incomplete; report denominators and uncertainty. Track latency, deadlines, retries, unique source count, bytes and estimated token budget separately, never conflate ranking score with cost or truthfulness.
4. **Create opt-in live benchmark command.** Use current Rust CLI/SDK/MCP execution path; allow an operator-configured SearXNG and direct API reference adapter only as test harness baselines, not new production providers. Enforce request/cost/time ceilings, user agent/terms compliance, no CAPTCHA bypass, credential redaction, bounded logs, and no live query from default CI. Require explicit flags per paid reference and never autodetect/bill based on ambient credentials. Record source version, trial timestamps and configured upstream; transient 429/timeout is an observed outcome, not absence.
5. **Set reproducibility gates.** Compare paired cases to an exact pre-change `main` baseline with identical features/config/snapshots; record p50/p95 where sample size permits and the sample count. Split training/tuning vs held-out sets. Publish benchmark schema, annotation guide, and baseline report with unanswered or unscorable tasks disclosed. Add a minimal make target or documented cargo test command; no mandatory external services.

## Hard invariants, exclusions, and security

Preserve keyless HTML providers as the principal metasearch baseline; do not call them unreliable as a justification for removal. No inference, embeddings, learned model, generated answers, crawler, new MCP tools, or default behavior changes. No secret-bearing telemetry or personal user queries in committed fixtures. Respect source permissions/terms and the existing bounded fetch, local scope, SSRF, trust, and Mode-Off zero-remote-dispatch rules. Benchmark code must not turn test-only reference clients into production network paths. No persistent user search history.

## Storage, API, and migrations

Add isolated `tests/fixtures/retrieval_quality/` (manifest + corpus + expected evidence), `tests/retrieval_quality.rs` or analogous evaluator modules, plus `docs/retrieval-quality-benchmark.md`. An optional `benches/retrieval_quality` or `tests/retrieval_quality_live.rs` may implement opt-in runs. Internal data schema is versioned; existing MCP/CLI/Python APIs, source IDs, output fields, release binaries, and caches do not change. Build scripts must be portable to Linux/macOS/Windows; dev-only tools must not be added to default dependencies without a measured need.

## Required verification

- `cargo test --locked --features mock --test retrieval_quality` (new offline suite; select actual test filename at implementation).
- `cargo test --locked --all-features --test tool_surface_evaluation` and `cargo test --locked --all-features --test retrieval_attempt_ledger`.
- `cargo test --locked --all-features --test provider_routing --test web_search_integration --test repo_workflow --test security_workflow --test research_workflow`.
- `make check`; `make bench-check`; `make docs-check`.
- Opt-in live run separately with explicit budget and provider enablement; report skipped baselines and counts, never claim a missing credential as a failed test.

## Acceptance and stop conditions

Offline benchmark is deterministic across two runs of the same commit (canonical IDs, ordering, exact metrics) and produces per-category denominators, judged source/passage relevance, error/absence labels and an independent baseline report. A small deliberately degraded ranking fixture must fail a known quality gate. No external requests are made by routine CI. Stop if gold labels cannot be audited, code needs an unauthorized network/credential dependency, baseline is being changed while establishing truth, or competitor terms preclude an intended live fixture. Closure file `plans/closure/deterministic-retrieval-quality/001-status.md` must attach source SHA, schema version and fingerprint, per-category metrics, commands/results, limitations, data provenance, and M002-M005 handoff contract.
