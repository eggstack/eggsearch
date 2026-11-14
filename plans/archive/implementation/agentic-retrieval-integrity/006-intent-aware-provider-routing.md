# M006 — Intent-Aware Provider Routing

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M006
Primary class: capability
Baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a` (the worktree also carries the uncommitted retrieval-defaults change from the prior turn; re-read `git status` before starting)
References: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` §3, §4, §6, §7, §9, §10 decisions 2, 3, 5; `plans/003-planning-process.md`; `plans/closure/agentic-retrieval-integrity/004-status.md`; `plans/closure/agentic-retrieval-integrity/005-status.md`; `src/meta/intent.rs`; `src/meta/provider_diagnostics.rs` (`resolve_provider_routing`, `ProviderRoutingDecision`); `src/meta/adapter/web.rs` (capability warnings); `src/meta/adapter/normalization.rs` (`aggregate_rrf_with`); `src/meta/dispatch/`; `src/core/config.rs` (`SearchSection`, `resolve_providers`, `resolve_profile_providers`); `src/core/provider.rs` (`KNOWN_PROVIDER_IDS`, `ProviderCapabilities`); `src/core/warning.rs`; `src/core/query.rs` (`SearchIntent`); `tests/retrieval_quality.rs`; `tests/fixtures/retrieval_quality/candidate_snapshots.json`; `architecture/meta.md` §3; `architecture/engines.md` § Native Enforcement Matrix; `architecture/config.md`; `docs/config.md`; `docs/provider-setup.md`.
Dependencies: hard M004 (closed, candidate `c9912fc0e4824c4ec6b1231be2057fb4cd0ba8ab`) and hard M005 (closed, same candidate). The snapshot lane is the acceptance gate; the derived intent is the routing input when the caller omits one.

## Objective

Make provider selection depend on the effective retrieval intent using one declared, code-resident preference table, so a query that asks for a security advisory, code, issue, release, or news result reaches a provider that natively supports it instead of only generic text providers — and make the `native_*_unavailable` capability warnings truthful while doing so.

The change is additive selection: it never removes, disables, reorders, or deprioritises a provider the caller or operator already selected, and it changes no ranking, fusion, dedup, or truncation behaviour.

## Non-goals

- No inference, embedding, vector index, model classifier, query expansion, synonym generation, or multi-query fan-out. Routing consumes the existing `SearchIntent` and the M005 classifier verbatim.
- No new provider, no provider removal, no change to `KNOWN_PROVIDER_IDS`, no change to default provider enablement or `default_providers`.
- No ranking change. RRF constants, dedup, diversity, truncation, and the M005 intent reranking weights are untouched; routing only decides which providers are asked.
- No change to explicit-provider routing and no change to profile routing. `repo_search` keeps its profile resolution; `research_search` and `security_search` keep their current default-path provider sets because they pass no routing intent.
- No new MCP tool, no new `web_search` argument, no removal or rename. `provider_status` gains one additive object.
- No new cache dimension, no new HTTP client, no `reqwest`, no new network call, no live network in CI.
- No provider added on a quality hunch. An intent with no capability flag that justifies a provider (`docs`, `web`) gets an empty preference list; `docs` is deferred to a milestone with snapshot evidence rather than guessed.
- No automatic `local_workspace` routing: workspace search is caller-scoped and must not be injected into a default web fan-out.

## Invariants that cannot regress

- Explicit `providers` produce byte-identical selected providers, warnings, and attempt ledger. Intent routing does not run when `requested_providers` is non-empty.
- Profile routing is unchanged: when a profile is supplied, the selected set, `degraded`, `partial`, and skip reasons are byte-identical.
- Mode-Off performs zero remote dispatch. The `live_allowed` denial returns before provider resolution, so routing can never widen a denied request.
- RRF tie ordering is unchanged. `aggregate_rrf_with` sorts provider lists by provider id and ranks rows by `(score, title, url)`, so appending providers cannot reorder an existing pair; the guarantee is asserted on the snapshot corpus, not assumed.
- Stable FNV-1a IDs, dedup, truncation, sanitization tiers, trust framing, and deadline behaviour are unchanged. No new ID domain.
- The attempt ledger keeps exactly one row per routed provider, ordered by routing rank, using existing outcome strings only. `SkippedCapabilityUnavailable` and `NotApplicable` dispatch semantics are untouched.
- A real capability gap is still reported. Routing may remove a spurious warning, never a true one.
- The routed set is bounded: `base + max_extra_providers`, with `max_extra_providers <= 4`, so a default `web_search` adds at most four dispatch jobs under the existing concurrency and global deadline.
- Routing is pure and deterministic: fixed table order, dedup, no clock, no randomness, no hash-map iteration in output, no user-controlled text in warning messages.
- The search cache needs no new dimension: `SearchCacheKey::from_request` already hashes the routed provider ids and the caller intent, and a partial or deadline-interrupted fan-out is still never cached.

## Expected production changes

1. **Configuration.** New `[search.routing]` table on `SearchSection`, serde-defaulted so every existing config parses unchanged:
   | Key | Type | Default | Validation |
   |---|---|---|---|
   | `intent_mode` | `IntentRoutingMode` (`"off"`, `"apply"`) | `off` at landing; `apply` only after the gate in work package 7 passes | enum; an unknown value is a parse error naming the two variants |
   | `max_extra_providers` | `usize` | `2` | `0..=4`, error names the bound; `0` reports but never adds |

   The three default sites must agree and are each asserted: `RoutingSection::default()` plus its serde `default_*` functions in `src/core/config.rs`; `resolve_provider_routing` reading `config.search.routing` with no second copy of the value; and the additive `provider_status.routing.intent_mode` disclosure. Docs rows in `docs/config.md`, `architecture/config.md`, and `docs/provider-setup.md` carry the same values; `docs/config.md` snippets are executed by `tests/docs_config_snippets.rs`, so a drift fails a test.

2. **Routing table module.** New `src/meta/routing.rs` (`pub(crate)`, target under 250 lines; ratchet not needed):

   ```rust
   pub(crate) struct IntentProviderPreference {
       pub(crate) intent: SearchIntent,
       pub(crate) providers: &'static [&'static str],
       pub(crate) capability: IntentCapability,
   }
   pub(crate) const INTENT_PROVIDER_PREFERENCES: [IntentProviderPreference; 7];
   ```

   The table is total over `SearchIntent`, so a new variant cannot be added without a decision here. Fixed order within each list; only ids from `KNOWN_PROVIDER_IDS`.

   | Intent | Preference order (fixed) | Capability predicate that justifies every entry |
   |---|---|---|
   | `web` | — (empty) | none |
   | `docs` | — (empty) | no capability flag expresses documentation; deferred |
   | `code` | `github_code`, `gitlab_code`, `gitea_code`, `sourcegraph` | `supports_code_search \|\| supports_repo_filter` |
   | `issues` | `github_issues`, `gitlab_issues`, `gitea_issues`, `firecrawl_developer` | `supports_issue_search` |
   | `releases` | `github_releases`, `gitlab_releases`, `gitea_releases` | `supports_release_search` |
   | `security` | `osv`, `github_advisory`, `rustsec`, `nvd` | `supports_security_search` |
   | `news` | `tavily`, `brave_api` | `supports_news` |

   Ordering rationale, not a quality claim: forge-native code and issue providers follow the order already used by the built-in `coding` profile; `osv` leads `security` because the built-in `security` profile leads with it; `sourcegraph` and `firecrawl_developer` are appended after the forge providers because no built-in profile orders them; `tavily` precedes `brave_api` because it declares `supports_domain_filters`, so a news query with domain filters keeps native enforcement. Every entry satisfies the same predicate its warning uses, enforced by a unit test over the constant table, so routing cannot suppress a warning by adding a provider that lacks the capability.

   Selection is one pure function:

   ```rust
   pub(crate) fn select_intent_additions(
       intent: SearchIntent,
       base: &[String],
       enabled: &[String],
       cooling: &[String],
       max_extra: usize,
   ) -> Vec<RoutingAttempt>;
   ```

   with a stable disposition per considered provider (`added`, `already_selected`, `not_enabled`, `not_built`, `cooling_down`, `capability_missing`, `cap_exceeded`). `base` is never reordered or dropped; the result is a deduplicated suffix in table order, truncated at `max_extra`.

3. **Decision integration.** `resolve_provider_routing` gains one parameter, `intent: Option<SearchIntent>`; settings come from the `config` it already receives.

   Decision precedence, evaluated once, first match wins:
   1. non-empty `requested_providers` → existing explicit resolution, verbatim;
   2. a supplied `profile` → existing profile resolution, verbatim;
   3. default path → `[search].default_providers` filtered to enabled and built, then, only when `intent_mode = apply`, the intent additions appended;
   4. the existing cooldown filter and its "keep the unfiltered list when everything is cooling" fallback run last, unchanged.

   Call sites pass `intent: None` for `repo_search`, `research_search`, and `security_search`, so their routed sets stay byte-identical even though `research_search` and `security_search` also use the default branch: those workflows are already intent-specific and must not gain a second selection pass. `web_search` passes the effective intent. Additions therefore require both `intent_mode = apply` and an explicit intent argument; `apply` with `intent: None` is identical to baseline. `ProviderRoutingDecision.reason` keeps its current wording in every mode — the additive effect is observable through `providers_queried`, the attempt ledger, the capability warning, and `provider_status.routing`, so no new response field or reason string is introduced.

4. **Effective intent.** `run_web_search` computes `routing_intent = caller_intent.unwrap_or_else(|| derive_intent(query, &RepoQueryHints::parse(query)).intent)` and passes it to routing. `web_search_with_intent` keeps deriving independently for reranking; both are pure functions of the same bounded query, and a coherence test asserts the routed intent equals the reported `derived_intent` for every corpus query, so the two calls cannot silently diverge.

5. **Truthful capability warnings.** `src/meta/adapter/web.rs` keeps the four message strings byte-identical and splits emission on the mode:
   | Situation | `off` (today) | `apply` |
   |---|---|---|
   | a routed provider has the native capability | no warning | no warning |
   | no routed provider has it, but an enabled and built provider does | existing `native_*_unavailable` warning | new `intent_routing_capability_excluded` warning naming the enabled providers |
   | no enabled and built provider has it | existing `native_*_unavailable` warning | existing `native_*_unavailable` warning |

   The new prefix is additive and uses the existing warning taxonomy: `WarningCode::IntentRoutingExcluded`, a `KNOWN_PREFIXES` entry, and the message template `intent_routing_capability_excluded: intent={intent} requested; enabled native {capability} provider(s) {ids} were not routed, so results are from generic/contextual search`, with `{ids}` rendered in table order. `max_extra_providers = 0` under `apply` adds nothing and reports exactly this warning.

6. **Disclosure.** `provider_status` gains one additive top-level object, `routing`, derived from the constant table (no duplicate literal): `intent_mode`, `max_extra_providers`, and `preferences` as intent → ordered provider ids, including the empty lists. No existing field changes.

7. **Evidence surface for the lane.** `src/meta/adapter/selection_test_harness.rs` gains a `mock`-gated `route_default(intent, base, enabled, cooling, max_extra)` view over `select_intent_additions` so the snapshot lane replays the production selection function, not a copy. Nothing is exposed in a default build.

## Storage, protocol, and compatibility effects

- No storage, no migration, no persisted state. Routing is recomputed per call.
- `[search.routing]` is additive configuration with serde defaults. A TOML file without the table keeps today's behaviour exactly; `AppConfig::save()` now serialises the table, which is additive on rewrite.
- No MCP tool, argument, or result-shape removal. `provider_status` gains `routing`; `web_search` gains no field forced by routing (the existing `derived_intent` and `warnings` carry the observable effect).
- One additive warning prefix. The four `native_*_unavailable` strings, `freshness_unenforced`, `domain_filters_local`, and every other existing wire string are unchanged.
- Search-cache behaviour is unchanged: the key already carries the routed provider ids and caller intent, and warnings are computed after fan-out so a cached fan-out cannot serve a stale warning.
- Batch, fetch, repo, research, security, and evidence-bundle paths are unaffected; no dependency edge is added or inverted (`core <- meta <- mcp` holds; `fetch` untouched).

## Ordered work packages

1. **Config and defaults.** Add `IntentRoutingMode`, `RoutingSection`, `SearchSection.routing`, `default_intent_routing_mode()`, `default_max_extra_providers()`, and validation naming the `0..=4` bound; add the docs rows. Prove existing configs parse and validate unchanged, and that all three default sites agree.
2. **Routing module.** Implement `src/meta/routing.rs`: total preference table, capability binding, `select_intent_additions`, dispositions. Unit tests: one entry per intent, every id known, every entry satisfies its own predicate, every empty list is deliberate. Property tests: base never reordered or removed, result is a deduplicated suffix, length never exceeds `max_extra`, repeated calls identical.
3. **Decision integration.** Add the `intent` parameter, wire the four call sites, apply additions only on the default path, and leave `reason` untouched. Differential test: `off` vs baseline produces identical `selected_providers`, `reason`, `degraded`, `partial`, `skipped_providers` for the full config matrix used by `tests/provider_routing.rs`; `apply` with `intent: None` produces the same identity.
4. **Truthful warnings.** Implement the emission matrix, add `WarningCode::IntentRoutingExcluded` and its prefix, update the warning taxonomy docs and the CodeGG warning table. Test the three-row matrix per actionable intent, plus assert the four legacy strings are byte-identical.
5. **Disclosure.** Add `provider_status.routing` from the constant table; test that the disclosed preferences equal `INTENT_PROVIDER_PREFERENCES` and that `intent_mode` equals `SearchSection::default().routing.intent_mode`.
6. **Lane fixtures and view.** Extend `tests/fixtures/retrieval_quality/candidate_snapshots.json` with an optional per-case `routing` object (`enabled_providers`, `default_providers`, `intent_mode`, `max_extra_providers`); add the mock-gated `route_default` view; make replay fail closed when a routed provider has no snapshot list. Fixture rule for every routing case: the judged authoritative document sits at rank `1` or `2` inside the capability provider's list and is absent from every generic list, so its RRF contribution (`1/(60+rank)`) beats the generic tail deterministically. Add four tier-1 cases:
   - `security-osv-routing-001` (security): lists for `duckduckgo`, `startpage`, `osv`; the advisory document is rank 2 in `osv` and absent from both generic lists.
   - `code-github-routing-001` (code): lists for `duckduckgo`, `yahoo`, `github_code`; the authoritative source file is rank 2 in `github_code`.
   - `issues-firecrawl-routing-001` (issues): lists for `duckduckgo`, `startpage`, `firecrawl_developer`; the authoritative issue thread is rank 2 in `firecrawl_developer`.
   - `docs-no-routing-001` (docs): lists for `duckduckgo`, `startpage`, no preference list, expecting a recorded no-change case.
   The report gains a `routing` block (`intent_mode`, per-case added providers, capability coverage, whether an exclusion warning would fire) beside the unchanged ranking metrics.
7. **Gate and rollout decision.** Add the seeded negative control `security-nvd-first`: a capability-valid but wrong ordering (`nvd` before `osv` for `security`) must be detected as a per-case `recall_at_5` drop of at least `0.25`, reproduced twice; if it is not detected, the lane is rejected as a harness defect and the default must not flip. Then run `make eval-retrieval-quality` twice and require, for every case with a `routing` block: no decrease in `recall_at_5`, `mrr_at_5`, `ndcg_at_5`, `authoritative_survival`, `distinct_domains_topk`; no increase in `duplicate_slots`; and byte-identical metrics for the eleven pre-existing cases. With non-regression established, the default flips to `apply` in the same milestone only when the recorded run shows either a measured improvement (a ranking metric on a routing case or the routing block's capability coverage) or an explicitly recorded no-change decision that names the accepted evidence and why routing still satisfies the exit. The gate test `intent_routing_rollout_gate_matches_the_recorded_decision` recomputes the comparison from the corpus and asserts the default equals the derived decision, exactly as the M005 switch test does. If any case regresses, the default stays `off`, the closure record states the regression, and the milestone closes the capability as not-enabled.
8. **End-to-end and ledger.** Add `web_search_integration` tests with mock engines (apply adds the enabled capability provider; `off` is identical; derived-intent coherence; Mode-Off denial with routing enabled performs no dispatch) and `retrieval_attempt_ledger` tests (one row per routed provider, routing-order stability, no new outcome strings). Update `docs/test-inventory.md`, `architecture/meta.md` §3 (derived intent feeding routing), `architecture/engines.md` § Native Enforcement Matrix (preference entries carry the predicate they rely on), `docs/provider-setup.md`, `docs/codegg-integration.md` (warning table row), `CHANGELOG.md`, and the closure record inputs.

## Verification

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test provider_routing
cargo test --locked --all-features --test web_search_integration
cargo test --locked --all-features --test retrieval_attempt_ledger
cargo test --locked --all-features --test config_validation
cargo test --locked --all-features --test property_retrieval
cargo test --locked --all-features --test schema_identity_registry --test mcp_tools
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features --test docs_config_snippets
make eval-retrieval-quality
make docs-check
python3 packaging/check-planning-consistency.py
cargo test --locked --all-features
```

Run `make eval-retrieval-quality` twice and record both metric blocks; identical output is required. The gate test lives in the existing `retrieval_quality` suite; routing decision tests extend the existing `provider_routing` suite; the pure-function properties extend `property_retrieval`. No new phase-named suite is created.

## Acceptance and stop conditions

- `intent_mode = off` is behaviourally identical to baseline: same selected providers, same reason, same warnings, same ledger, same metrics on all eleven pre-existing snapshot cases.
- `apply` with an enabled capability provider for the effective intent routes it, suppresses the false `native_*_unavailable` warning, and reports `intent_routing_capability_excluded` instead when a capable provider remains unrouted.
- `apply` never removes, reorders, or deduplicates away a base provider; additions never exceed `max_extra_providers`; the routed set stays deterministic across runs.
- Explicit `providers`, every profile, Mode-Off, and every non-`web_search` tool are unchanged, proven by test rather than by inspection.
- The negative control is detected, and the gate shows no regression plus either a measured improvement or an explicitly recorded no-change decision. A default flip with a regression, without recorded evidence, or with a gate test that disagrees with the recorded decision is a defect, not a preference.
- Stop and return for a decision if: the snapshot lane cannot show non-regression (the default then stays `off` and the milestone closes with the regression recorded, never with an unmeasured flip); a preference entry needs an id outside `KNOWN_PROVIDER_IDS`; a capability flag would have to change to justify an entry; profile or explicit routing cannot be left byte-identical; truthful warnings would require editing an existing wire string; the lane cannot distinguish the distractor mapping; or the work would need a new required MCP argument, a new tool, or a ranking change.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/006-status.md` must record: the implementation candidate SHA; the final `INTENT_PROVIDER_PREFERENCES` table with the capability predicate per intent; both `make eval-retrieval-quality` metric blocks; the per-case routed-vs-baseline deltas for every case with a `routing` block; the eleven-case byte-identical evidence; the distractor negative-control drop; the warning matrix (routed capability / enabled-but-unrouted / true gap) per intent with exact message strings; the `off`-mode differential evidence; ledger ordering and outcome evidence; and the explicit decision `apply` or `off` with its evidence, or the recorded no-change decision and residual risk.
