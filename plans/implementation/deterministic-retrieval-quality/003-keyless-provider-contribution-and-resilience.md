# M003 — Keyless Provider Contribution, Fault Isolation and Bounded Routing

Status: ready
Source: `plans/subsystems/deterministic-retrieval-quality-roadmap.md` M003
Primary class: infrastructure; capability only if measured routing improvement lands
Baseline: `7ad8e81b2d8f7036d35dcb2772a1778c73d08c3b`
References: `plans/000-long-term-specification.md` ``3, 5, 7-9; ADR-0006; `architecture/engines.md`; `src/meta/dispatch/`; `src/meta/provider_diagnostics.rs`.
Dependencies: hard M001 (closed at `310611b733c2067ddeab184618156ef072934a6e`); no paid provider or external service hard prerequisite.

## Objective

Measure *how much* independent keyless engines contribute to discovery, overlap, recall and latency under partial upstream failure, preserving their intentional SearXNG-derived HTML adapter design. Investigate deterministic, bounded scheduling/failover only where the benchmark establishes a benefit without material recall loss.

## Current evidence

`src/meta/engines/` contains one provider-specific engine per upstream, including HTML engines; `src/meta/dispatch/execution.rs` already runs sorted bounded jobs with per-provider limits; `OriginController` and provider diagnostics record failures, cooldown, skips and source detail. `tests/provider_routing.rs`, `tests/dispatch_fault_injection.rs` and `tests/provider_probe_conformance.rs` cover many synthetic states but not independent live incremental relevant-result contribution.

## Work packages

1. Extend M001 evaluation to measure each keyless default (DuckDuckGo, Startpage, Yahoo), optional HTML Brave/Mojeek, and configured SearXNG independently and in combinations. Measure unique relevant hits, overlap, marginal recall@k, failure modes, timeouts, stable response identity and latency by category. Compare to paid/API baselines **only when explicitly opted in**, reporting cost and capability differences.
2. Add controlled HTML drift fixtures: changed markup, empty SERP, generic challenge page, 403/429/503, decoded-size breach, DNS/connect/TLS failure, slow upstream and timed-out subqueries. Verify no accidental zero-results inference after failure; record normalized attempt outcome, skips, cooldown, and partial evidence.
3. Define evaluation of candidate *deterministic* scheduling under identical provider sets and cost budgets (e.g. existing parallel fan-out vs bounded staged dispatch). Honor requested provider order/selection and explicit filters; prohibit arbitrary suppression of an enabled keyless provider or changes to default providers. No implicit conversion to paid or authenticated routes; any fallback stays within the explicitly configured eligible set and existing cost limits.
4. If staged scheduling wins on held-out quality-with-budget measures, implement as **opt-in** internal or additive configuration only, with a canonical stable ordering, no randomization, bounded maximum probes, deterministic stop/coverage predicates and full ledger visibility for deferred/not-attempted providers. If advantage is not convincingly shown, **do not implement it**; record a no-change result.
5. Add tests for exact configured-provider behavior, quota/cooldown isolation, tie/dedup correctness, deliberate empty vs failed providers, all-provider failures, keyless no-key bootstrap, and stability under varied task completion order. Document terms/rate-limit expectations without encouraging anti-bot circumvention.

## Invariants, effects and exclusions

No inference, learning-to-rank, auto-billing, auto-CAPTCHA, credential/region bypass, unbounded fan-out, changes to existing output semantics or silent provider omissions. Explicit `providers` must not be expanded unexpectedly. Existing response cards, RRF, capability-skip/attempt ledgers, optional credentials and keyless configuration remain compatible. No new persistent provider health DB or background crawler. Use existing engine/dispatch/diagnostic seams rather than introducing a parallel router.

## Verification

`cargo test --locked --features mock --test dispatch_fault_injection --test provider_routing --test provider_probe_conformance --test web_search_integration` and new retrieval-quality/routing fixtures; `cargo test --locked --all-features --test provider_request_contract`; `make check`; `make bench-check`. Any live run uses opt-in M001 limits and reports 429/403 as observed failures rather than benchmark corruption.

## Acceptance and closure

Provider marginal-value reports exist for independent and combined keyless paths, with exact query/task categories, valid denominators, labels, and failure conditioning. Existing keyless startup and explicitly selected providers are unchanged. Optional routing code lands **only** when held-out relevance, latency, and call-cost evidence supports it without regression in failure/absence correctness; otherwise M003 closes with a documented negative experiment. Record code/config effects, test outcomes, comparisons and any deferred ideas under `plans/closure/deterministic-retrieval-quality/003-status.md`. Stop if a proposal makes an API key mandatory or masks upstream failure.
