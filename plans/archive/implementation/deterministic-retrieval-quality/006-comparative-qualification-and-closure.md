# M006 — Cross-Variant Comparative Qualification, Findings and Closure

Status: closed
Source: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` M006
Primary class: infrastructure and polish
Baseline: `7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b` (rebase exact candidate before qualification)
References: `plans/003-planning-process.md` ``2.5, 4, 8-10; ADR-0006; `architecture/testing.md`; `docs/release.md`; `docs/features.md`.
Dependencies: hard M001-M005 acceptance/closure or explicitly documented no-change conclusions; operational native browser availability for a meaningful enabled-browser run, and optional live provider credentials/quotas for relevant reference comparisons. Missing paid credentials do not block keyless/default or offline qualification.

## Objective

Publish a defensible, reproducible record of whether Eggsearch's deterministic metasearch, source selection, optional PDF/browser extraction and context projections improve retrieval quality compared with its own baseline and appropriate competitors. Close the workstream *without* claiming unaudited live superiority or unintentionally changing the default release/API surface.

## Work packages

1. Reconcile M001 corpus, source permissions, annotation definitions, held-out partition, fingerprints and frozen baseline. Audit each of M002-M005 for landed changes, negative/no-change results and outstanding blockers; ensure every decision has an independently reproducible acceptance record.
2. Execute **four source/runtime feature arms** (default, `pdf`, `browser`, `pdf browser`) using common controlled fixture truth and equal relevance/time budgets, plus current `--all-features` regression gates (not a fifth baseline). Record Chrome and `lopdf` versions, feature build/test flags, runtime enablement, input/case IDs, errors/skips, relevance scores, file sizes, startup latency, RSS/CPU and p50/p95 under bounded repetitions. Browser-required cases must be executed on a real configured browser runner or remain honestly blocked.
3. Run a controlled **keyless live observation cohort** for default DDG/Startpage/Yahoo and opt-in configured keyless alternatives (including SearXNG where available), with all evidence failures and duplicate/unique source contributions. When explicitly authorized and budgeted, add direct API references and the equivalent Eggsearch-backed configuration; label cost, native filtering, source/index differences and latency separately. Never promote one snapshot to an eternal guarantee, score unavailable sources as absent, or publish uncontrolled source bodies.
4. Produce a multi-axis comparison for all benchmark cases: retrieval source recall@k/precision/nDCG and passages; quality of evidence handoff; provider failure/absence status; extractions (HTML, JS, PDF, combined); agent context bytes/tokens/turns; latency, resource use and per-request billable API cost. Report mean/median/uncertainty and denominators; do not compress this into one marketing score.
5. Reconcile documentation discrepancies (only if verified): current DNS address pinning description in `docs/safety.md` vs `src/fetch/client.rs`; currently parsed-but-no-op `respect_robots_txt` / user agent settings; test inventory Python 3.15 and release-version claims. These are docs/operational evidence tasks, not approval to change unrelated production policies.
6. Run focused tests and canonical gates on the **exact candidate SHA**. Where changes warrant a release, hand off to the existing Eggpack and PyPI processes with ordinary exact-SHA qualification; do not publish as part of this plan or promise Python browser/PDF availability. Register actual closure evidence in the new `plans/closure/deterministic-retrieval-quality/` directory and reconcile the roadmap and registry status; any regressions become separate corrective plans.

## Required qualification

- `cargo fmt --check`; `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `cargo test --locked --no-default-features`; `cargo test --locked --features pdf`; `cargo test --locked --features browser`; `cargo test --locked --features "pdf browser"` (narrow runs first when expensive), `cargo test --locked --all-features`.
- `make check`; `make bench-check`; `make docs-check`; `make packaging-check`.
- M001 deterministic quality test, M002 PDF/browser matrix, M003 provider-failure/cost suite, M004 held-out ranking gates, M005 context-trace tests. Optional live evaluations must be separate, manually gated, bounded and provenance-logged.
- Verify installed default CLI/Python wheel still function with no API key, and explicit Mode-Off repo local path makes zero remote calls; optional source builds remain available.

## Acceptance criteria, blockers and report

Publish `docs/retrieval-quality-comparison.md`, dataset/method version, per-category metrics, full feature-arm status, resource tradeoffs and clearly separated economic/latency costs. Provide exact commit/test logs and live trial time windows. Report what was actually observed; make no unsupported universal superiority claims. The comparative evidence is from candidate `10b06ab76434983cd9942e81bf2090fc9d6e31cc`; the discovered macOS symlink containment and root-path test failures were corrected and qualified at `99316f31caf37ad2165cd5e822316942faf20c6f` under repository-hardening M011. The corrected candidate passes `make check`. No ranking or provider change was justified.

Stop on attempts to require inference, remove keyless HTML engines, strip PDF/browser support, bypass challenge protections, add automatic paid calls, silently alter the ten tools/CodeGG policy or weaken published release compatibility. The terminal closure record `plans/closure/deterministic-retrieval-quality/006-status.md` must include requirement-to-evidence table, commands and outcomes, release compatibility, critical/medium/low residuals, recommendation, and handoff of any independent cross-repo work.
