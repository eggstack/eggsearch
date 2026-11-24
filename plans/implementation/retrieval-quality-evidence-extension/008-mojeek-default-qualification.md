# M008 — Mojeek Generic-Default Provider Qualification

Status: blocked
Blocker: held-out Mojeek and current-default provider arms did not meet the predeclared reliability, paired-control, and relevance gates.
Source: `plans/subsystems/retrieval-quality-evidence-extension-roadmap.md` M008
Primary class: capability
Baseline source SHA: `7ec251df2ba715705468f2eb1e5392fa87fc4d32`; rebuild and record the exact release binary identity before trials
References: retrieval-quality M007 plans 001 and 002; `docs/retrieval-quality-cross-domain-live.md`; external runner `scripts/run_retrieval_quality_external_live.py`; fixture provenance `tests/fixtures/retrieval_quality/README.md`; ADR-0006; `plans/003-planning-process.md`.
Dependencies: Interface — frozen M007 SciFact, NQ, and FEVER test query/qrel fixtures and runner report contract, already present in the repository. M007's separate hosted-CI closure remains open and is not claimed here. Operational — reachable keyless Mojeek, DuckDuckGo, Brave HTML, and Yahoo endpoints. No paid provider, credential, or configured external service is allowed.

## Objective and non-goals

Determine whether Mojeek has sufficiently reliable and distinct held-out retrieval value to join Eggsearch's generic shipped defaults. Use the same fixed query IDs and repetition numbers for Mojeek and each currently shipped generic default provider. Change production defaults, tests, and user-facing configuration documentation only if every predeclared qualification gate below passes.

Non-goals: do not amend M007 or its historical reports; do not change ranking, provider adapter/parser/challenge behavior, routing, or MCP contracts; do not add paid providers or credentials; do not change qrels or select queries after observing results; do not retain raw search responses or upstream page bodies; and do not claim general provider superiority from this small positive-qrel sample. A failed, incomplete, or control-incomparable trial leaves the defaults unchanged.

## Current implementation evidence

- `src/core/config.rs` ships `default_providers = [duckduckgo, brave, yahoo]` and has Mojeek disabled in `search.providers`.
- `src/core/config/tests.rs` and `tests/config_validation.rs` assert the disabled/default-provider contract; `docs/provider-setup.md`, `docs/config.md`, and `docs/codegg-integration.md` contain user-facing provider/default examples.
- A prior three-query Mojeek-only release-profile smoke returned five unique sources per query with no reported provider errors. That is functional evidence only, not held-out relevance or a default-admission decision.
- The runner scores only URL or normalized-title matches against positive qrels. Unmatched results are unjudged, not irrelevant; cache state is uncontrolled.

## Frozen trial design

- Candidate source SHA: `7ec251df2ba715705468f2eb1e5392fa87fc4d32`. Build a release `eggsearch` binary with `cargo build --release --locked --no-default-features --bin eggsearch` and record the binary SHA-256. Do not use the test harness binary.
- Dataset arms: `scifact`, `nq`, and `fever`, using the checked-in frozen test query/qrel fixtures. Exclude Touché 2020 because the runner cannot safely match its opaque qrel IDs.
- Fixed seed: `m008-mojeek-default-qualification-v1`; three deterministically selected queries per dataset, two repetitions, ten results per query, eighteen-second per-attempt timeout.
- Provider arms, isolated by the existing runner: `duckduckgo`, `brave`, `yahoo`, and `mojeek`. Each arm runs the exact same selected query IDs and repetition numbers. Total budget: 72 provider attempts (4 × 3 × 3 × 2); no retries or extra queries. The runner's 80-attempt per-provider ceiling remains in force.
- Build a temporary, credential-free qualification config that enables only these four keyless HTML providers while retaining the shipped three-provider default list; every trial explicitly selects exactly one provider. Record the config fingerprint. Do not read or alter the user's runtime config.
- Run the external runner once per provider/dataset with `--query-count 3 --repetitions 2 --seed m008-mojeek-default-qualification-v1 --max-results 10 --timeout-seconds 18 --max-provider-queries 80`. Preserve complete JSON reports in `plans/closure/retrieval-quality-evidence-extension/evidence/mojeek-<provider>-<dataset>-m008.json`.

## Predeclared admission gate

All conditions are required; otherwise do not change the defaults.

1. Trial integrity: all 12 reports identify the same exact source SHA, release binary SHA, runner fingerprint, selected query IDs for each dataset, seed, result limit, and timeout; no paid providers or credentials were used; all attempts and all non-success outcomes are represented.
2. Mojeek operations: at least 17/18 outcomes are valid (`results` or `zero_results`), at least 5/6 are valid in each dataset, and at least 4/6 return non-empty result sets in each dataset. Timeouts, provider errors, and command errors count as invalid.
3. Comparable controls: each existing default provider has at least 4/6 valid outcomes in each dataset. A Mojeek match counts as unique only on a query/repetition for which all three existing default-provider arms have valid outcomes and none matched that same relevant corpus document. If this paired coverage is missing, the gate does not pass.
4. Held-out contribution: Mojeek matches at least three distinct query/document pairs, across at least two dataset families, and at least one such pair is unique under condition 3. Repeated observations of the same query/document pair count once. This is a minimum evidence floor, not a superiority claim.
5. Record per-arm valid/error/empty counts, matched qrel pairs and denominators, hit@10, macro/micro recall@10, query-level unique contributions, latency distribution, and all limitations. No qrel match is not interpreted as proof of irrelevance.

## Ordered work packages

1. Validate fixture and runner identities, confirm the frozen selection, and build the exact release CLI candidate.
2. Create the minimal temporary qualification config and run the 72 bounded, keyless, paired provider attempts. Preserve every report, including empty responses and failures; do not retry.
3. Evaluate every gate condition from the raw JSON reports and write the qualification summary and closure record. If any condition fails or is unmeasurable, leave shipped defaults and provider enablement unchanged.
4. Only if all gates pass: add Mojeek to the shipped generic default list and enable it in default configuration; update exact-default tests and user-facing default lists/examples in `docs/provider-setup.md`, `docs/config.md`, and `docs/codegg-integration.md`. Do not alter parser or ranking behavior.
5. For a passing candidate, run focused default/config and search integration tests, `make check` and `make docs-check`, then a bounded three-query live default-fan-out smoke using the resulting release binary. Record selected providers, per-provider outcomes, result counts, and latency. If tests or the fan-out smoke fail, revert the default change and record the failure; do not claim qualification.

## Verification and closure evidence

- The plan's qualification decision is based on the exact candidate source SHA and binary hash; changes to the source or the frozen cohort invalidate the trial.
- Persist the 12 raw runner JSON reports, qualification summary, and `plans/closure/retrieval-quality-evidence-extension/008-status.md`. Include source/binary/runner/config/corpus fingerprints, exact query IDs, host, per-arm counts and metrics, gate-by-gate verdict, any default fan-out run, tests run, and residual limitations.
- If the gate fails, the closure recommendation is no default change and the evidence must identify the failed/incomplete condition. If it passes, closure additionally requires tests/docs/fan-out acceptance and registry/roadmap updates; hosted CI remains a separate qualification when required by the broader M007 process.

## Progress — 2026-11-23

The exact 72-attempt cohort ran once on the pinned source and release CLI; all 12 runner JSON reports are preserved. Mojeek produced five valid result outcomes, all on SciFact, plus 13 HTTP-status errors and zero matched held-out qrel pairs. The controls also failed the paired-coverage gate (Yahoo had 18 HTTP-status errors; DuckDuckGo/Brave had no valid FEVER outcomes). Trial integrity passed; operational reliability, control comparability, and held-out contribution failed. No defaults, production code, tests, or user-facing default docs changed; the live default-fan-out smoke was skipped. Summary: `plans/closure/retrieval-quality-evidence-extension/mojeek-default-qualification-008.md`; closure: `plans/closure/retrieval-quality-evidence-extension/008-status.md`. No network retries were issued.

## Stop conditions

Stop without retries if credentials or paid-provider access are required, if a provider arm cannot use the frozen test cohort, if the exact source/binary identity changes, or if the 18-second per-attempt bound is exceeded. Record a provider outage as an operational outcome, not a relevance score. Never compensate for missing paired controls by treating errors as empty results.
