# M007 — Real-World Discovery Quality Qualification and Bounded Corrections

Status: proposed
Draft: yes — corrective plan, implementation not started
Date: 2026-11-20
Subsystem: knowledge-delta
Milestone class: capability
Planning baseline: `16b4c2888206caeb6654ffebaa039862fc143712` on `feat/auto-20261117-044c4885`; reconciled after M004 closed on `380a55cd7ea7c0b37dc6eff53851926e96e894d0`
Source addendum: `plans/subsystems/knowledge-delta-corrective-addendum.md` M007
Closed predecessors: M003 (`plans/archive/implementation/knowledge-delta/003-source-driven-topic-discovery.md`), `plans/closure/knowledge-delta/003-status.md`; also M001/M002 closures; candidate `c0f71909cfe2e6016512423e55b9d00fdb7220c5`. M004, the closed corrective to M001 (`plans/closure/knowledge-delta/004-status.md`, candidate `380a55cd7ea7c0b37dc6eff53851926e96e894d0`), supplies the date-accounting and empty-state fields this evaluation attributes against.
Authority: accepted ADR-0006/ADR-0020/ADR-0021; `plans/000-long-term-specification.md` §§3–4, 7–9; `plans/003-planning-process.md` §7.

## Corrective finding and why earlier gates missed it

M003 closure proves one *synthetic* omitted-name recovery and related negative controls. It explicitly did not measure field precision/recall, and `src/meta/knowledge_delta/plan.rs` uses one exact topic plus a few fixed English lexical hints. A local co-occurrence edge is not a verified causal, package-compatibility or release relationship. The prior tests established correct bounded behavior, but did not measure whether the resulting briefs are useful on ordinary public releases, whether the added results are credible, or how much noise expansion introduces.

This is a **missing evaluation and possible ranking/coverage correction**, not a plan to introduce the crawler or LLM loop rejected in the original design.

## Objective and readiness

Publish reproducible evidence of real-world source-backed discovery, including after-cutoff date classification, novel related-topic recall, precision/no-change controls and network/context costs. Apply only small deterministic domain-specific changes proven to improve performance without altering the existing metasearch engine, global RRF scoring, URL safety or public MCP contracts.

**Hard:** M001–M003 closed, ADRs accepted — satisfied. **Interface:** the existing `knowledge_brief` result and cutoff/refresh/related schema, including the M004 coverage fields this evaluation measures — `coverage.empty_state_reason`, `coverage.selected_cards`, `coverage.dated`, `coverage.undated`, `coverage.at_or_before_cutoff`, `coverage.not_after_cutoff`, `coverage.selected_providers`, `coverage.timestamp_capable_providers` and `coverage.index_status`. **Soft:** incorporate M005 and M006 when their changes are ready, or explicitly document that M007 was tested on the older exact candidate. This milestone has an independent quality gate.

## Evaluation contract (write fixtures and pass/fail rule before tuning)

Construct a manifest-driven corpus of at least **80 dated cases** across software releases, dependency compatibility, public infrastructure and scientific/public-health updates. Include at least 20 withheld-name cases (the query omits a relevant package/event), 20 no-material-change/negative controls, and 15 ambiguous, conflicting, month/year/unknown date-basis cases. Source anchors must be independently traceable to stable official releases/advisories or clearly labelled secondary reporting; do not invent source URLs or use only new random search result counts as ground truth. Store original publication and event-date evidence separately. Keep evaluation sample selection independent of the eventual tuning. Every dated case requires a selected provider set that advertises `result_timestamps`: M004's live evidence established that the shipped defaults (`duckduckgo`, `brave`, `yahoo`) advertise none, so a corpus run against them returns `empty_state_reason: no_dated_sources` with `dated: 0` for every case. Pin the provider set per run, record `coverage.timestamp_capable_providers`, and treat an empty set as an invalid run rather than a zero score.

For each case record cutoff source/precision, expected temporal classification, topic and withheld entities, relevant source IDs/URLs, gold relevant claims, corpus seed/evidence state, source availability, expected false-positive controls and adjudication method. Use **frozen snapshots** in CI; live results cannot be replayed reliably later without timestamps. Measure precision@5, recall@5, evidence-linked novel hits@5, false temporal claims, source URL/span validity, duplicate yield, provider operations, fetched bytes, wall time and emitted characters. Report denominators and confidence intervals where useful, not marketing scores.

Compare equal-budget variants: M001 direct live, M002 indexed lexical, M003 one-step related expansion, and `research_search` as a general-research reference (label its different cutoff semantics). Seed each index variant with the **same approved public evidence** and apply identical max_items/max_chars, source/provider/fetch and deadline budgets. Do not credit one method for extra inputs or previously unseen gold labels. Record a separate prospective sample of **at least 15** contemporary public topics with fetch dates, source links, independently adjudicated relevance, and provider failure outcomes; if upstream access prevents this, keep the milestone open or explicitly mark only the offline qualification completed.

## Ordered work packages

1. **Baseline and reproducibility.** Freeze a manifest and ground-truth rubric *before* tuning. Capture exact branch SHA, model-cutoff snapshot, index seed set, provider profile with a non-empty `coverage.timestamp_capable_providers`, config, dates, query budgets and evaluator version. Implement an offline runner in existing corpus/test tooling or a focused `tests/knowledge_delta_quality.rs` suite with deterministic fixtures; avoid adding a runtime evaluator or new MCP tool.
2. **Failure taxonomy.** Classify missed evidence as (a) no source discovered, (b) no dated field, (c) wrong date basis, (d) source not fetched, (e) source fetched but selected out, (f) generic/co-occurrence expansion noise, (g) misleading duplicate/stale index hit, (h) provider unavailable. Separate source-quality errors from model-cutoff-coverage errors. Attribute failures through existing `RetrievalAttemptSet` and source/edge IDs, without leaking raw user queries to logs by default. Attribute the dated-source outcomes through `coverage.empty_state_reason` (`no_dated_sources`, `all_sources_at_or_before_cutoff`, `no_matching_index_hits`) instead of a parallel vocabulary, and keep a provider-capability gap distinct from a retrieval failure.
3. **Bounded deterministic improvements.** Only if ablation supports it, adjust `src/meta/knowledge_delta/plan.rs` for domain-specific lexical query patterns, `src/meta/knowledge_delta/discovery.rs` for source-anchored subject extraction, and postselection in `src/meta/knowledge_delta/execute.rs`/`index.rs` for dated evidence and source-authority weighting. Keep a hard cap of existing request fan-out and fetched documents; preserve stable IDs and deterministic tie order, original RRF weights, strict temporal comparison, and one-step local expansion. No arbitrary domain/host acquisition from untrusted page links.
4. **Adversarial and negative controls.** Test generic package terms, ambiguous aliases, typo collisions, false co-occurrence, out-of-date publication of old event, month/year precision, multiple conflicting sources, truncated HTML, injected source instructions, and empty/offline indexes. Verify coverage/warnings distinguish zero findings from policy-denied or provider-unavailable retrieval.
5. **Live and consumer qualification.** Run prospective live probes in an isolated, explicitly permitted environment. Re-run a representative CodeGG/eggui-shaped query with model ID passed explicitly, compare index disabled vs explicit retained and related mode, audit total requests/latency/context size, and separately state whether it is source-backed evidence rather than a model answer. Keep failures visible.
6. **Docs and regression.** Update `architecture/research.md`, `architecture/evidence-workflow.md`, `architecture/testing.md`, `docs/agent-workflows.md`, `docs/test-inventory.md`, and dedicated evaluation docs with seed manifest and counts. Any contract change requires SDK/Python and `mcp_tool_contract` parity; do not quietly change tool-list names or global ranking.

## Verification and quantitative gate

Run on the exact candidate:
```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_discovery
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_brief
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test research_evidence_corpus
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test property_knowledge_delta
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```
Also run the new reproducible corpus runner and record source manifests, before/after results, budgets, and the live-study notes. CI's required qualification is network-free; the live study is an **additional operational evidence gate**, not a flaky CI prerequisite.

For any changed ranking/expansion implementation, pre-register these qualification criteria: at least a **10 percentage-point** absolute recall@5 gain versus the matched baseline on the withheld-name subset, **zero false post-cutoff classifications** in the dated regression corpus, **100% traceable source IDs/spans for emitted claims**, and a no-change-control false-positive rate that does not rise by more than **3 percentage points**. Query/fetch/character maxima and total deadline must never exceed existing configured caps. If the data do not support the gain, retain the existing behavior, publish the negative result, and keep the claimed improvement unclosed rather than cherry-picking the corpus. If no code changes are warranted, a measurement-only closure requires explicit maintainer approval and must not claim improved recall.

## Stop, rollback and closure

**Stop:** the approach needs autonomous crawling, page-followed new hosts, an internal LLM/embedding service, unchecked HTML fact extraction, changed cutoff inference semantics, unbounded provider calls, altered canonical tool inventory, or hidden provider failures. A materially new evidence protocol requires a separate proposed ADR before implementation.

If a tuning change increases false positives or weakens date/provenance validity, revert that part while retaining the measurement harness and report the failure. A no-improvement result is a valid research finding, not an excuse to overstate closure.

Write `plans/closure/knowledge-delta/007-status.md` with candidate SHA, frozen manifest hashes, corpus composition, metrics tables including baseline and ablations, prospective live-study results and failures, guard/test outcomes, request/latency/context costs, security findings, and remaining limits. Update addendum and registry together. Do not reopen M003's closed fixture evidence.
