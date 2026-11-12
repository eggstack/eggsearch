# M017 — Routing Shipped-Set Evidence and Keyword Predicate

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M017
Primary class: invariant (corrective to M006)
Baseline: `dace41c3a75307ea35b745d695a11630ec6495c0`
References: `plans/implementation/agentic-retrieval-integrity/006-intent-aware-provider-routing.md`; `plans/closure/agentic-retrieval-integrity/006-status.md`; `plans/implementation/agentic-retrieval-integrity/004-candidate-snapshot-ranking-benchmark.md`; `plans/closure/agentic-retrieval-integrity/004-status.md`; `plans/subsystems/agentic-retrieval-integrity-roadmap.md` §4 and §10 decision 2; `plans/003-planning-process.md`; `src/meta/routing.rs`; `src/meta/engines/rustsec.rs`; `src/meta/adapter/web.rs`; `src/meta/adapter/selection_test_harness.rs`; `src/meta/provider_diagnostics/decisions.rs`; `tests/retrieval_quality.rs`; `tests/fixtures/retrieval_quality/candidate_snapshots.json`; `tests/provider_routing.rs`; `tests/config_validation.rs`; `src/core/config/routing.rs`; `src/core/config.rs`; `src/core/provider.rs`; `docs/config.md`; `docs/provider-setup.md`; `architecture/config.md`.
Dependencies: hard M004, M005, and M006. M006 is `conditionally closed`; its single named condition is what this milestone discharges.

## Objective

Discharge M006's named condition by making the tier-1 snapshot lane measure the shipped default provider set, then let that measurement decide `[search.routing].intent_mode` instead of shipping an unmeasured policy change. M006 withheld `apply` because every routing case recorded a bespoke default set (`duckduckgo/startpage` or `duckduckgo/yahoo`), while the shipped `default_providers` are `duckduckgo`, `brave`, `yahoo` (`src/core/config.rs:292`). A routing case can only have a routing effect when the specialist can enter the candidate set, so a lane that does not model the shipped set cannot measure the shipped default. This milestone authors shipped-set cases for every intent that has a preference row, re-runs `make eval-retrieval-quality` twice, and pins the default to the measured decision. If the evidence does not support `apply`, the default stays `off`, M006's condition is restated with the measured reason and remains `conditionally closed`; it is never waived.

The milestone is also corrective for a defect exposed by the live review on 2026-11-12. Routing had added `rustsec` for the `security` intent because `rustsec` declares security capability, but `RustSecEngine::search` unconditionally rejects a non-ID query with `EngineError::Unsupported` (`src/meta/engines/rustsec.rs:210`), which `classify` maps to `error_class: unknown` (`src/meta/adapter/error.rs:55`). The routed call therefore always failed. `rustsec` is already absent from `INTENT_PROVIDER_PREFERENCES` and from the docs at this baseline, but nothing prevents it from being added back, and the existing predicate test still admits it because capability describes subject matter, not answerability. This milestone adds a durable keyword-query predicate to admission and a test that would have caught the regression.

## Non-goals

- No ranking, fusion, dedup, diversity, truncation, or intent-reranking change. The lane is a measurement surface, not a ranker change.
- No provider added, removed, renamed, or re-enabled; no change to `KNOWN_PROVIDER_IDS`, `default_providers`, or `[search.providers]` defaults.
- No change to the 24-field `ProviderCapabilities` contract (`src/core/provider.rs:180`), to `ProviderDescriptor`, or to `provider_status.routing.preferences`.
- No change to explicit-provider dispatch. A caller that explicitly requests `rustsec` keeps today's behaviour, including the `Unsupported` → `unknown` classification; reclassifying that error is a separate milestone.
- No new MCP tool, argument, or result field; no new warning code; the four `native_*_unavailable` strings and `intent_routing_capability_excluded` stay byte-identical.
- No change to `error_class` wire strings, stable IDs, sanitization, deadlines, or the search cache key.
- No new dependency, no `reqwest`, no live network in CI, no new fixture tier.
- No `docs`-intent routing entry and no provider added on a quality hunch.
- No blanket `apply` flip without the recorded lane decision; the default follows the measurement, not this plan.

## Invariants that cannot regress

- Routing remains add-only. It never removes, reorders, or displaces a caller-supplied provider, a profile's providers, or the shipped default list. `base` is preserved exactly and additions are an ordered, deduplicated suffix.
- Explicit `providers` and every profile produce byte-identical selected providers, warnings, `degraded`, `partial`, skip reasons, and attempt ledger.
- Mode-Off performs zero remote dispatch and is byte-identical to baseline in providers, warnings, ledger, and cache key, including the `rustsec` warning case: the keyword predicate is applied to native-coverage detection only under `apply`.
- A real capability gap is still reported. The change may add a true `native_*_unavailable` warning under `apply`; it never removes one.
- Determinism: the same query, config, and enabled/built sets produce an identical provider order. No clock, randomness, hash-map iteration, or fixture-order dependence in output. RRF tie ordering `(score, title, url)` is untouched and remains asserted on the corpus.
- The routed set stays bounded by `base + max_extra_providers` (`<= 4`), and the attempt ledger keeps one row per considered provider in table order.
- The lane replays the production selection and routing functions, never a copy; a routed provider with no recorded candidate list still fails closed.
- Ten MCP tools, FNV-1a IDs, trust framing, sanitization tiers, and deadline behaviour are unchanged.
- Ordinary source files stay under 1,600 lines / 80 KB and inside every ratchet ceiling in `architecture/maintenance.md`; no explanatory code comments; no new dependency.
- `[search.routing]` without the table still parses and validates, and an unknown `intent_mode` still names both valid values.

## Expected production changes

1. **Keyword-query eligibility predicate.** `src/meta/routing.rs` gains a positive, code-resident allowlist of the provider ids a default-path keyword fan-out may route:

   ```rust
   pub(crate) const KEYWORD_QUERY_PROVIDERS: &[&str] = &[
       "github_code", "gitlab_code", "gitea_code", "sourcegraph",
       "github_issues", "gitlab_issues", "gitea_issues", "firecrawl_developer",
       "github_releases", "gitlab_releases", "gitea_releases",
       "osv", "github_advisory", "nvd",
       "tavily", "brave_api",
   ];

   pub(crate) fn serves_keyword_query(id: &str) -> bool;
   ```

   `provider_supports(id, capability)` becomes `serves_keyword_query(id) && descriptor-capability-holds`, so the existing per-entry predicate test, `capable_built_providers`, and the exclusion warning all use one definition of "usable native provider". `select_additions` gains `RoutingDisposition::KeywordUnsupported`, wire string `keyword_unsupported`, evaluated after `cooling_down` and before `capability_missing`, so a provider that knows the subject but cannot answer a keyword query is reported precisely instead of as a capability gap.

   The mechanism choice is deliberate. **A new declared capability** was rejected because it would make a 25th flag in a contract consumed by `provider_status`, the native-enforcement matrix, and every descriptor literal, and it would repeat the original mistake: a declarative subject-matter flag that can disagree with the engine. **A per-engine trait method** is the most truthful at the implementation site, but routing, disclosure, and the offline lane decide from ids without engine instances; making it work would require constructing credentialed engines in tests and a static mirror for the lane, which reintroduces two sources of truth. **The positive allowlist with a behavioural test** is chosen: it is evaluable from ids alone, it fails closed for any new table entry that is not explicitly admitted, it changes no published capability contract, and the one known rejecting engine is pinned by executing it. The residual risk is a future engine that rejects keywords and is admitted to the allowlist without evidence; the test matrix below makes that a two-file review rather than a single-line table edit.

2. **Truthful native coverage under `apply`.** `native_capability_warning` (`src/meta/adapter/web.rs:644`) currently calls `any_engine_supports` on raw capabilities, so a built `rustsec` suppresses `native_advisory_search_unavailable` although it cannot serve the query. Under `apply`, the "does a native provider exist" check uses `built_provider_ids.iter().any(|id| provider_supports(id, capability))`, the same keyword-aware predicate routing uses; under `off`, it keeps `any_engine_supports` unchanged so Mode-Off behaviour is byte-identical. The helper therefore keeps `engines` for the `off` branch and adds the id-based check for `apply`; no other call site changes. Because the predicate is routing-scoped, a built provider that routing intentionally never uses (for example `local_workspace`) no longer suppresses or appears in the exclusion warning under `apply`; that is the intended truthful-coverage effect and is covered by the warning-matrix test.

3. **Shipped-set tier-1 cases.** `tests/fixtures/retrieval_quality/candidate_snapshots.json` gains seven cases, one per `SearchIntent` that has a preference row, all with `category: "intent_routing"`, `partition: "train"`, `max_results: 5`, `freshness: "any"`, `judgments_complete: true`, `expected_state: "results"`, and a `routing` block whose `default_providers` is exactly `["duckduckgo", "brave", "yahoo"]` and whose `max_extra_providers` is `1`.

   | Case | Query (must derive the annotated intent) | Intent | Specialist list | Judged document |
   |---|---|---|---|---|
   | `web-no-routing-shipped-001` | `shared rate limit implementation notes` | `web` | none (control) | none; routed set equals baseline |
   | `docs-no-routing-shipped-001` | `tower-http trace layer documentation` | `docs` | none (control) | none; routed set equals baseline |
   | `code-github-shipped-001` | `Router::layer middleware ordering` | `code` | `github_code` | `https://github.com/example/router/blob/main/src/layer.rs`, grade 3 |
   | `issues-github-shipped-001` | `auth token refresh bug report` | `issues` | `github_issues` | `https://github.com/example/auth/issues/42`, grade 3 |
   | `releases-github-shipped-001` | `axum release notes changelog` | `releases` | `github_releases` | `https://github.com/tokio-rs/axum/releases/tag/axum-v0.8.0`, grade 3 |
   | `security-osv-shipped-001` | `CVE-2024-1234 openssl advisory` | `security` | `osv` (and an `nvd` distractor list) | `https://osv.dev/vulnerability/CVE-2024-1234`, grade 3 |
   | `news-tavily-shipped-001` | `breaking news semiconductor supply chain` | `news` | `tavily` | `https://news.example.test/semiconductor-supply-chain`, grade 3 |

   Every query is chosen so the existing classifier derives the annotated intent with no earlier class firing; the implementation asserts `derive_intent(&case.query).intent == case.intent` for each case, exactly as the existing routing test does.

   Generic-list rule, applied identically to every shipped-set case: record three lists (`duckduckgo`, `brave`, `yahoo`), each exactly three rows. Rank 1 is the same URL on all three, so dedup collapses it; ranks 2 and 3 are distinct per provider and distinct from every specialist URL. The specialist list has the judged document at rank 1 and a non-judged distractor at rank 2. No generic list contains the judged document.

   The RRF arithmetic makes the outcome independent of a fragile tie-break: with `rrf_k = 60`, the generic-only baseline contributes one document at `1/61`, three at `1/62`, and three at `1/63`; the routed set adds the judged document at `1/61`. The top five therefore contains both `1/61` documents and three of the `1/62` documents, so the judged document is always present and the baseline never scores it. Expected per-case delta: `recall_at_5` and `authoritative_survival` `0 → 1`, `mrr_at_5` and `ndcg_at_5` increase, `distinct_domains_topk` increases by one (the specialist host), `duplicate_slots` stays `0`. The plan asserts the direction and records the measured value; it does not hardcode exact metric values.

   The `security-osv-shipped-001` case additionally records an `nvd` list with one non-judged advisory, so the negative control can replace the route. `SECURITY_PREFERENCE_ORDER` and `SECURITY_DISTRACTOR_ORDER` in `tests/retrieval_quality.rs:1840` are updated to the current table order (`osv`, `github_advisory`, `nvd`) and its `nvd`-first permutation; the stale `rustsec` entry is removed because it is no longer a routing candidate.

4. **Gate rework.** `intent_routing_rollout_gate_matches_the_recorded_decision` (`tests/retrieval_quality.rs:2106`) stops requiring every routing case to use the shipped defaults. It defines `shipped_cases` as routing cases whose `default_providers` equals `AppConfig::default().search.default_providers`, then:

   - requires a shipped-set case for every intent disclosed by the preference table, including the two empty rows; the harness exposes `intent_preferences()` from the constant table so coverage cannot drift from the code;
   - requires every shipped case's `enabled_providers` to contain every shipped default, and every routed provider to have a recorded candidate list;
   - computes `improved`/`worsened` from shipped cases only; the four bespoke routing cases remain mechanism coverage and are still checked for non-regression by their existing test;
   - keeps `pre_existing_changed` over every case without a `routing` block, byte-compared against plain replay;
   - re-points the negative control to `security-osv-shipped-001` and requires a `recall_at_5` drop of at least `0.25` for `Ordered(distractor, 1)` versus `Ordered(preference, 1)`, reproduced twice;
   - replaces `lane_measures_shipped_default_path` with `shipped_intent_coverage` equality, and adds per-case `shipped_default_path` and aggregate coverage fields to `retrieval-quality-routing-report`.

   `should_apply` becomes: no pre-existing change, no shipped-case regression, the negative control detected, coverage complete, and either at least one measured improvement or a non-empty recorded no-change decision (`INTENT_ROUTING_NO_CHANGE_DECISION`). Improvement and no-change are mutually exclusive; both being present is a harness defect. The gate asserts `decision == INTENT_ROUTING_RECORDED_DECISION` and `AppConfig::default().search.routing.intent_mode == Apply` iff `should_apply`. No test asserts `Apply` directly.

5. **Default decision and documentation.** The implementation runs `make eval-retrieval-quality` twice and records both blocks. If `should_apply` is true, `RoutingSection::default()` flips to `apply`, `INTENT_ROUTING_RECORDED_DECISION` is set to `"applied"`, and the default assertions in `tests/config_validation.rs:1193` and `:1260` are renamed and updated. If it is false, no default or constant-flip edit is made: `INTENT_ROUTING_RECORDED_DECISION` stays `"observe_only"`, the regression or no-improvement evidence is recorded, and the plan closes with `intent_mode` still `off`. The no-change branch is recorded with the exact evidence that routing still satisfies M006's exit (for example capability coverage on shipped cases) and why the ranking metrics are unchanged.

6. **Regression coverage for the predicate and the lane.** `src/meta/routing.rs` unit tests add: `every_preference_entry_serves_keyword_queries`; `keyword_allowlist_has_no_unknown_or_duplicate_ids`; `rustsec_is_never_routed_even_if_listed` (via the `mock`-gated `select_intent_additions_with_order`, asserting `keyword_unsupported` and that `osv` is still added); and `keyword_exclusion_is_not_reported_as_a_capability_gap`. `src/meta/engines/rustsec.rs` adds a behavioural test that constructs `RustSecEngine { client: Client::new() }`, calls `search` with `EngineSearchRequest::simple("serde deserialization", 5, Duration::from_secs(1))`, and asserts `Err(EngineError::Unsupported { reason, .. })` whose reason contains `keyword search not supported`; the same test asserts `!serves_keyword_query("rustsec")`. Together these fail if `rustsec` is re-added to the table, if it is added to the allowlist, or if its engine is changed to accept keyword queries without updating either declaration.

   `tests/provider_routing.rs` adds an integration case with `rustsec` enabled and built, `intent_mode = Apply`, security intent: `rustsec` is reported `keyword_unsupported`, is absent from `selected_providers`, and the selected set is unchanged otherwise. Mode-Off identity, explicit-provider identity, profile identity, and determinism tests are extended rather than replaced.

## Storage, protocol, and compatibility effects

- No storage, migration, or persisted state. Routing is recomputed per call.
- The snapshot corpus schema is unchanged; `schema_version` and `corpus_revision` stay as they are, but the corpus fingerprint changes because cases were added. Every recorded metric block and fingerprint in the closure record is re-recorded from the post-change run.
- The routing decision's `intent_routing.considered[]` gains the additive disposition value `keyword_unsupported`. `provider_status.routing` (the disclosure object: `intent_mode`, `max_extra_providers`, `preferences`) is unchanged, and `routing.preferences` still mirrors the constant table. Existing disposition strings are unchanged.
- `provider_status`, the attempt ledger, `providers_queried`, and `providers_failed` keep their shapes. One routed provider fewer in the `rustsec` case removes a failure row; that is the intended correctness effect, not a schema change.
- Configuration: if the measured decision is `apply`, the default value of `[search.routing].intent_mode` changes from `off` to `apply`. This is a shipped-default change accepted by the roadmap §4 rule (no regression plus measured improvement or recorded no-change), and it is called out in `CHANGELOG.md` and the docs table. A config file that sets `intent_mode` explicitly is unaffected; a file without the table inherits the new default on parse and on `save()`. If the decision is `off`, there is no configuration change at all.
- Mode-Off behaviour is byte-identical to baseline in providers, warnings, ledger, and cache key; only `apply` can add a provider or a truthful `native_*_unavailable` warning.
- No MCP tool, argument, or result-shape removal; no new tool. No dependency edge is added or inverted (`core <- meta <- mcp` holds; `fetch` untouched). No new dependency.

## Ordered work packages

1. **Predicate.** Add `KEYWORD_QUERY_PROVIDERS`, `serves_keyword_query`, the `provider_supports` composition, and `RoutingDisposition::KeywordUnsupported`, plus the new value in the `IntentRoutingAttempt::disposition` doc list; add the unit tests for table membership, unknown/duplicate ids, the injected-`rustsec` route, and the distinction from `capability_missing`.
2. **Behavioural pin.** Add the `rustsec` engine test proving keyword rejection and the allowlist exclusion; prove the test fails when the exclusion is removed and when the reason string is changed to a generic error.
3. **Warning truthfulness.** Apply the predicate to `native_capability_warning` under `apply` only; add the test matrix (built `rustsec` alone now emits `native_advisory_search_unavailable` under `apply`, and remains suppressed under `off`, byte-identical to baseline).
4. **Fixtures.** Author the seven shipped-set cases with the generic-list and specialist rules; bump nothing in the schema; verify each query derives its annotated intent and each specialist provider has a list; re-derive the expected direction from the metrics rather than hand-writing metric values.
5. **Gate.** Implement the shipped-set subset, per-intent coverage from `intent_preferences()`, the re-pointed negative control with two-run reproducibility, the no-change branch, and the report fields; remove `rustsec` from the security order constants.
6. **Measure and decide.** Run `make eval-retrieval-quality` twice, record both blocks, and set `INTENT_ROUTING_RECORDED_DECISION` and the default from the derived decision. On a flip, update `RoutingSection::default`, `tests/config_validation.rs`, and the docs; on no flip, leave them and record the condition.
7. **Integration and compatibility.** Extend `tests/provider_routing.rs` and `tests/web_search_integration.rs` for `keyword_unsupported`, Mode-Off identity, explicit/provider/profile identity, one-row-per-routed-provider ledger ordering, and derived-intent coherence; audit any test that relies on `AppConfig::default()` meaning `off` and make the mode explicit.
8. **Docs and planning status.** Update `docs/config.md` (predicate, disposition, default), `docs/provider-setup.md`, `architecture/config.md`, `architecture/meta.md` §3 if the routing description is restated there, `docs/test-inventory.md`, `architecture/testing.md`, and `CHANGELOG.md`. On a flip, update `plans/closure/agentic-retrieval-integrity/006-status.md` to `closed` and the roadmap status; on no flip, restate M006's condition with the measured evidence and leave it `conditionally closed`. Write `plans/closure/agentic-retrieval-integrity/017-status.md` and keep the registry row in step.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test provider_routing
cargo test --locked --all-features --test retrieval_quality
cargo test --locked --all-features --test config_validation
cargo test --locked --all-features --test web_search_integration
cargo test --locked --all-features --test static_guards
make eval-retrieval-quality
cargo test --locked --all-features
```

Run `make eval-retrieval-quality` twice and record both full metric blocks; identical output is required. Because the closure moves planning documents, also run `python3 packaging/check-planning-consistency.py` and `make docs-check`. No new phase-named suite is created: the gate and corpus live in `retrieval_quality`, routing decisions in `provider_routing`, config defaults in `config_validation`, and pure predicate properties in `src/meta/routing.rs` unit tests.

## Acceptance and stop conditions

- A shipped-set case exists for every intent disclosed by the preference table, including the empty `web` and `docs` rows; the coverage test fails if one is missing or if its `default_providers` differs from `AppConfig::default().search.default_providers`.
- The negative control on the shipped security case is detected with at least a `0.25` `recall_at_5` drop and reproduces across two replays; a lane that cannot detect it is rejected as a harness defect and the default must not flip.
- Every preference entry is in `KEYWORD_QUERY_PROVIDERS`; `rustsec` is not; `RustSecEngine` provably returns `Unsupported` for a keyword query; a table edit that re-adds `rustsec` fails a test before it can dispatch.
- Under `apply`, a built keyword-rejecting provider cannot suppress a true `native_*_unavailable` warning; under `off`, providers, warnings, ledger, and cache key are byte-identical to baseline.
- The gate derives `should_apply` from the recorded corpus and asserts both `INTENT_ROUTING_RECORDED_DECISION` and the shipped default equal it. A hardcoded `apply`, an unmeasured flip, or a gate that disagrees with the recorded decision is a defect.
- If the shipped-set lane shows a regression or no improvement and no no-change decision is recorded, the default stays `off`; the milestone still closes with the measurement recorded, and M006's condition is restated with that evidence rather than waived.
- If the flip is supported, `apply` never removes, reorders, or deduplicates away a base provider, additions stay within `max_extra_providers`, the routed order is deterministic across runs, and explicit and profile routing are byte-identical.
- Stop and return for a decision if: a shipped-set case cannot be authored without changing production ranking; a preference entry needs an id outside `KNOWN_PROVIDER_IDS`; the predicate would require a `ProviderCapabilities` flag change; truthful warnings would require editing an existing warning string; explicit or profile routing cannot be left byte-identical; the lane cannot distinguish the deliberately worse ordering; or the work would need a new MCP tool, a new required argument, or a new dependency.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/017-status.md` must record: the implementation candidate SHA; the final `KEYWORD_QUERY_PROVIDERS` allowlist and the `rustsec` behavioural proof with the exact error reason; the seven shipped-set case table with each query's derived intent; both `make eval-retrieval-quality` metric blocks and the corpus fingerprint; the per-case baseline-vs-routed deltas for every shipped-set case; the negative-control recall drop and two-run reproducibility; the coverage verdict for all seven intents; the explicit measured decision `apply` or `off` with its evidence, or the recorded no-change decision naming the accepted evidence and why routing still satisfies the exit; the Mode-Off and explicit/profile byte-identity evidence; the `keyword_unsupported` disposition and warning-matrix evidence; the configuration default and docs alignment (or the recorded no-flip state); and residual risk. On a flip, the same closure commit updates `plans/closure/agentic-retrieval-integrity/006-status.md` to `closed`, the roadmap status, and the registry row together. On no flip, it updates M006's condition text with the measured regression and leaves `conditionally closed` in place.
