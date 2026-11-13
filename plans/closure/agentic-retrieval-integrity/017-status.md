# M017 Routing Shipped-Set Evidence and Keyword Predicate — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/017-routing-shipped-set-evidence-and-keyword-predicate.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `dace41c3a75307ea35b745d695a11630ec6495c0`

Implementation candidate and closure commit: the commit that carries this record.

## Result

M006's named condition is discharged and `[search.routing].intent_mode` now ships **`apply`**.

- `src/meta/routing.rs` adds the positive keyword-query allowlist `KEYWORD_QUERY_PROVIDERS`
  (16 ids) and `serves_keyword_query`; `provider_supports` composes it with the descriptor
  capability predicate, so selection, `capable_built_providers`, the `apply` native-coverage check,
  and the exclusion warning share one definition.
- `RoutingDisposition::KeywordUnsupported` (`keyword_unsupported`) is evaluated after
  `cooling_down` and before `capability_missing`.
- Under `apply`, `native_capability_warning` decides "does a native provider exist" from the routed
  engines through the keyword-aware predicate; the `off` branch is unchanged.
- `RustSecEngine::search` rejects a keyword query with
  `keyword search not supported; use RUSTSEC-YYYY-NNNN ID lookup`; the behavioural pin asserts both
  the rejection and `!serves_keyword_query("rustsec")`. Mutations were proven: allowlisting
  `rustsec`, re-adding it to the security table row, or changing the reason string each fails a
  named test.
- Seven shipped-set tier-1 cases were added (one per disclosed intent, including the empty `web`
  and `docs` rows), each with `default_providers = ["duckduckgo", "brave", "yahoo"]`.
- The rollout gate measures shipped cases only, derives `applied` from non-regression + a detected
  negative control + complete coverage + measured improvement, and asserts the shipped default
  agrees with the recorded decision.

## Measurement — two identical runs

`CARGO_TARGET_DIR=/tmp/m017-target make eval-retrieval-quality` twice: exit 0 both times,
`16 passed / 2 ignored`; every `retrieval-quality-*` line byte-identical between runs. Corpus
fingerprint `fnv1a64:158b5adb6486aa8d`.

Aggregate candidate-snapshot report (identical both runs):

```json
{"authoritative_survival":0.9285714285714286,"cases":22,
 "denominators":{"authoritative_documents":28,"judged_results":109,"passages":0,
  "precision_at_5":110,"relevant_documents":30,"top_slots":109},
 "distinct_domains_topk":4.545454545454546,"diversity_cases":22,
 "duplicate_slot_rate":0.0,"duplicate_slots":0,"evidence_coverage":0.9318181818181818,
 "mrr_at_5":0.6015151515151516,"ndcg_at_5":0.6803021638040998,
 "passage_hit_at_1200_chars":0.0,
 "precision_at_5":0.25454545454545463,"ranking_cases":22,"recall_at_5":0.9318181818181818,
 "source_diversity_at_5":0.9181818181818183,"status_accuracy":1.0}
```

Routing rollout report (identical both runs):

```json
{"corpus_fingerprint":"fnv1a64:158b5adb6486aa8d","decision":"applied","intent_mode":"apply",
 "pre_existing_cases_changed":[],"worsened_metrics":[],
 "negative_control":{"case":"security-osv-shipped-001","distractor":"security-nvd-first",
  "recall_drop":1.0,"detected":true},
 "no_change_decision":null,"shipped_case_count":7,
 "shipped_default_providers":["duckduckgo","brave","yahoo"],
 "improved_metrics":[25 entries across the five specialist cases]}
```

The 25 improvements are the five specialist cases' `recall_at_5`, `mrr_at_5`, `ndcg_at_5`,
`authoritative_survival`, and `distinct_domains_topk`. Per case: `recall@5` 0→1, `MRR@5` 0→0.5,
`nDCG@5` 0→0.6309, survival 0→1, distinct domains 4→5; the two control cases are unchanged
(1/1/1/1/5). No case regressed and no pre-existing case changed.

Coverage `complete: true`, `missing_intents: []`: `web-no-routing-shipped-001`,
`docs-no-routing-shipped-001`, `code-github-shipped-001`, `issues-github-shipped-001`,
`releases-github-shipped-001`, `security-osv-shipped-001`, `news-tavily-shipped-001`.

## Verification

```bash
cargo fmt --all -- --check                    # clean
cargo clippy --locked --all-targets --all-features -- -D warnings   # clean
cargo check --locked --no-default-features    # clean
cargo test --locked --all-features --test retrieval_quality         # 16 passed / 2 ignored
cargo test --locked --all-features --test config_validation         # 91 passed
cargo test --locked --all-features --test provider_routing          # 97 passed, including the added keyword case
cargo test --locked --all-features --test static_guards             # 62 passed
python3 packaging/check-planning-consistency.py                     # ok
./packaging/check-repo-hygiene.sh                                   # ok
```

`make check`'s packaging leg cannot run on this workstation (`mavis-trash: unsupported OS:
Plan9`); the individual legs above were run instead and hosted CI covers the canonical gate.

## Default change and compatibility

`src/core/config/routing.rs` now defaults `IntentRoutingMode` to `Apply`; the serde default and
`RoutingSection::default()` agree. A config file without the table inherits `apply`; an explicit
`intent_mode` is unaffected. Mode `off` remains byte-identical to baseline in providers, warnings,
the attempt ledger, and the cache key.

## Deviations recorded

- The `apply` presence check uses the routed `engines` rather than `built_provider_ids`; using the
  full built set would suppress M006's truthful exclusion warning when a capable provider is
  withheld by `max_extra_providers`.
- The warning matrix lives in `src/meta/adapter/web.rs` unit tests (the integration file was not
  available to the implementation worker); the production `native_capability_warning` is exercised.
- The plan's literal expectation that `resolve_provider_routing` reports `rustsec` as
  `keyword_unsupported` is unreachable while `rustsec` is absent from the preference table; the
  durable pin is the mock-gated `route_with_order` case plus the engine behavioural test.
- The two control cases carry a grade-3 judged document so the corpus structure invariant holds and
  their routed set stays byte-identical to baseline.
- The security specialist distractor is a mirror host to keep both the routing and derived-intent
  lanes host-neutral.


Hosted CI: run `43472592990` on `0019f48` passed all three jobs — Ubuntu `ci` (12m11s),
Windows all-features check, and macOS canonical verification plus filesystem portability.

## Residual risk

- A future engine that rejects keyword queries could be added to the allowlist without evidence;
  the `rustsec` behavioural pin and the allowlist membership test make that a two-file review, not
  a one-line table edit.
- The shipped default is now measured on structured-synthetic candidate snapshots, not live
  providers; live quality remains subject to upstream behaviour.
- `keyword_unsupported` currently has no production producer outside an injected routing table
  because no preference row lists a rejecting provider; the disposition is covered by the
  mock-gated harness and the engine test.
