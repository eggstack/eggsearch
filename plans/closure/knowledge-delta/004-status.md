# M004 Brief Delta Production and Honest Diagnostics — Closure Evidence

Status: closed

Source plan: `plans/implementation/knowledge-delta/004-brief-delta-production-and-honest-diagnostics.md`

Roadmap: `plans/subsystems/knowledge-delta-roadmap.md` M004

Corrective to: M001, plan `plans/archive/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md`,
closure `plans/closure/knowledge-delta/001-status.md`.

Decision authority: accepted ADR-0020 (cutoff-aware knowledge brief), accepted ADR-0021
(optional local knowledge index), accepted ADR-0006 (keyless deterministic retrieval), and the
local workspace boundary in `architecture/local-workspace.md`. No new ADR.

## Scope

M004 corrects one defect: the shipped default deployment could never emit a delta and could
not say why. The correction is diagnostic, not a capability expansion:

1. Coverage accounts for the date availability of every selected card: `selected_cards`,
   `dated`, `undated`, `at_or_before_cutoff`, and `not_after_cutoff`. Which cards become
   deltas is unchanged.
2. Every empty result carries an `empty_state_reason` from a closed vocabulary:
   `no_cutoff_resolved`, `no_dated_sources`, `all_sources_at_or_before_cutoff`,
   `no_matching_index_hits`, or `deltas_emitted`. Every non-emitting reason is also returned
   as a `knowledge_brief_<reason>: ...` warning. A brief never returns a bare empty delta
   list.
3. Coverage lists the selected providers and the subset advertising `result_timestamps`, so
   an operator sees that an empty result follows from the provider set rather than the topic.
4. A retained excerpt prefers a heading-marked chunk over site chrome and records any
   residual chrome.
5. An index permission rejection names the rejected directory, its mode, and the required
   `0700`. The rejection stays fail-closed.

The paper-first contract is in `architecture/research.md`, `architecture/evidence-workflow.md`,
`docs/agent-workflows.md`, and `docs/config.md`.

The concurrently drafted knowledge-delta proposals that also used the number M004 were
renumbered to M005, M006, and M007 at the operator's direction, so this milestone keeps M004
and the closure record path `plans/closure/knowledge-delta/004-status.md`. The drafts remain
`proposed` and are not implemented by this milestone.

## Exact code candidate

Implementation candidate `380a55cd7ea7c0b37dc6eff53851926e96e894d0` (parent
`688948b40394563315bbaea6ef9680cc94af3676`). Every command and probe below ran against
that tree. The candidate was rebased onto the concurrently drafted planning commit
`688948b` so that this milestone and the concurrently drafted knowledge-delta proposals share
one linear branch; the rebased candidate carries the same source, test, and documentation tree
as the tree that was qualified, and `688948b` touches planning documents only. This closure commit carries this
record, the plan status, the roadmap status, and the registry rows together.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | A brief against the shipped default provider set names why no delta was produced and does not return a bare empty delta list | Live before: `deltas: []`, no `empty_state_reason`, no date accounting, warnings naming only provider failures. Live after: `empty_state_reason: no_dated_sources`, `selected_cards: 8`, `dated: 0`, `undated: 8`, `selected_providers: ["brave","duckduckgo","yahoo"]`, `timestamp_capable_providers: []`, warning `knowledge_brief_no_dated_sources: selected live cards contain no parseable provider-declared publication timestamp`. Test `tests/knowledge_delta_brief.rs::undated_default_provider_reports_date_reason_and_capability`. |
| 2 | A brief with a resolved cutoff and at least one dated source emits deltas whose `kdelta_` identities are unchanged from M001 | Live before and after on the `hn_algolia` set produced the same identity for the same source: `kdelta_0821997cd89d3e7d` and `kdelta_1c7f204c06ee13c9`, `date_basis: publication`, `temporal_relationship: after_cutoff`. Test `::after_cutoff_publication_is_emitted_and_labeled` asserts the emitted id equals `knowledge_delta_id(url, "publication", "day", "2024-06-02")` and `empty_state_reason: deltas_emitted`. |
| 3 | The date-accounting counts agree with the emitted deltas and with the selected card count | Test `::date_accounting_matches_selected_cards_and_emitted_deltas`. Property `tests/property_knowledge_delta.rs::date_accounting_relations_partition_dated_sources` asserts `dated == not_after_cutoff + after_cutoff` and `at_or_before_cutoff <= not_after_cutoff` over generated date and cutoff precisions. |
| 4 | The empty-state reason is one of the enumerated values and never contradicts `no_temporal_claim` | `EmptyStateReason` in `src/meta/knowledge_delta/plan.rs` is a five-variant enum; `empty_state_reason()` returns `NoCutoffResolved` exactly when no cutoff resolved, which is exactly when `no_temporal_claim` is true. Tests `::same_day_and_unknown_model_make_no_temporal_claim` and `::undated_default_provider_reports_date_reason_and_capability` assert both sides. |
| 5 | A retained excerpt does not begin with site chrome for the captured fixture | `extract_brief_excerpt()` in `src/mcp/tools/knowledge_brief.rs` orders chunks by heading metadata, then block order, then position, and records `selection_basis` and `chrome_may_remain` per span. Test `brief_fetcher_prefers_heading_chunks_and_records_residual_chrome` uses the captured Hacker News chrome text. |
| 6 | The index path rejection warning names the rejected directory | `IndexError::InsecureDirectoryPermissions { directory, mode }` message names the directory, its current mode, and the required `0700`. Test `tests/knowledge_delta_index.rs::explicitly_configured_database_rejects_a_shared_parent` asserts the parent path, `0755`, and `0700`. |
| 7 | A live brief still does not open the index, and `retain` still requires the index to be enabled | The existing live-call test in `tests/knowledge_delta_brief.rs` is retained and passes; `coverage.index_status` remains `not_opened` for a live call with no retain. |
| 8 | No new tool, argument, schema property, configuration key, provider, or dependency; no date is inferred from prose | `make check` static guards pass. The comparison rule, cutoff precedence, partition model, and `kdelta_` identity are untouched; the only date source is a provider-declared publication timestamp compared with a resolved cutoff. |
| 9 | The broad gate passes with 0 failures | `make check` exit 0 on the candidate; `make docs-check` exit 0. |

## Live before and after

Probes used the candidate debug binary and the baseline `16b4c288` debug binary, MCP
`stdio`, and a configuration that pins the provider set. Topic `rust ownership`; the cutoff
case passes `cutoff_override` `2020-01-01`, day precision.

| Probe | Before (`16b4c288`) | After (`380a55c`) |
|---|---|---|
| Shipped default set (`duckduckgo`, `brave`, `yahoo`), no cutoff | `deltas: []`; no `empty_state_reason`; no date accounting; warnings named only provider failures | `deltas: []`; `empty_state_reason: no_cutoff_resolved`; 8 selected cards, 0 dated, 8 undated; `timestamp_capable_providers: []`; warning `knowledge_brief_no_cutoff_resolved: ...` |
| Shipped default set, resolved cutoff | `deltas: []`; no reason; warnings named only provider failures | `deltas: []`; `empty_state_reason: no_dated_sources`; 8 selected cards, 0 dated, 8 undated; warning `knowledge_brief_no_dated_sources: ...` |
| Single undated HTML provider (`duckduckgo`), resolved cutoff | `deltas: []`; no reason; the search phase failed outright when the provider blocked | `deltas: []`; `empty_state_reason: no_dated_sources`; 19 selected cards, 0 dated, 19 undated |
| Timestamp-capable provider (`hn_algolia`), resolved cutoff | 8 deltas, no coverage accounting | 7 deltas; `empty_state_reason: deltas_emitted`; 14 selected cards, 14 dated; `timestamp_capable_providers: ["hn_algolia"]`; shared identities `kdelta_0821997cd89d3e7d` and `kdelta_1c7f204c06ee13c9` unchanged |

The delta-count difference in the last row is live-source variation, not an identity change:
the identities present in both runs are identical, and the emitted count tracks how many
post-cutoff items the upstream index held at that moment.

The selected-card counts in the table are what that run returned, not a fixed expectation: a
re-run of the same probes against the integrated tip returned 20 selected cards for the shipped
default set without a cutoff and 22 with the cutoff, with `dated: 0` and `undated` equal to
`selected_cards` in every case. The empty-state reason, the accounting partition, and the
provider-capability report were identical across runs; only the number of live cards moved with
what the upstream providers served.

## Verification

Using the pinned Rust 1.89 toolchain on the candidate `380a55c`:

- `make check`: exit 0 — 90 suites, 6,520 passed, 0 failed, 28 ignored.
- `make docs-check`: exit 0.
- Focused: `knowledge_delta_brief` 12, `knowledge_delta_discovery` 10, `knowledge_delta_index` 13,
  `property_knowledge_delta` 9, `property_identity` 18, `mcp_tools` 29, `lib` 3,947.
- `cargo fmt --all -- --check` and `git diff --check`: clean.
- Suite totals moved from 6,506 to 6,520 on `--all-features` and from 6,479 to 6,493 on
  `--features mock`; the committed `docs/test-inventory.md` counts were stale by 61 before
  this work and are refreshed here.

### Transient failures recorded, not omitted

Two full-gate attempts failed on load-sensitive tests that this milestone does not touch:

1. `challenge_replay::cache::tests::an_expired_entry_is_dropped_on_read` asserts that an entry
   with a 10 ms TTL is still alive on its first read. `src/challenge_replay/cache.rs` is
   unmodified by this milestone, and the failure mode is the entry having already expired.
2. `knowledge_delta_index::two_processes_serialize_writes_and_read_the_same_database` spawns a
   child writer against the same SQLite file. The index sets a 500 ms busy timeout, and under
   full-gate load the child can exit non-zero. The test body is unmodified by this milestone;
   only its assertion was strengthened to include the child stderr, so a recurrence is
   self-diagnosing.

Both suites passed 30/30 standalone runs on the candidate, and the baseline `16b4c288` full
`--all-features` run passed with 0 failures under the same load. Neither failure was
reproduced on the candidate. The final gate run is green.

## Known limitations and residual risk

1. A delta still requires a selected provider that advertises `result_timestamps`. The shipped
   keyless defaults (`duckduckgo`, `brave`, `yahoo`) do not, so a default-only brief reports
   `no_dated_sources` rather than emitting a delta. This is the ADR-0006 design: replacing a
   scraper with an API or adding a provider was explicitly out of scope.
2. Upstream provider availability is an environment fact this milestone does not change. On
   2026-11-20 the probes observed DuckDuckGo 403 or a challenge wall, Brave 429, Yahoo 500,
   and a Mojeek challenge wall; only `hn_algolia` was reliably reachable. The empty-state
   reason is computed from whatever the search phase actually returned.
3. The excerpt preference uses heading metadata first. A page whose article text carries no
   heading metadata still contributes chrome, which is recorded per span as
   `chrome_may_remain` rather than hidden.
4. `at_or_before_cutoff` and `not_after_cutoff` are both zero when no cutoff resolves; they
   describe a comparison that did not happen.
5. The response shape gains additive `coverage` keys. No existing key is removed or renamed,
   and the advertised input schema is unchanged.

## Compatibility and migration

No tool, argument, schema, or configuration change. Behaviour change: an empty delta set now
carries a reason, and a consumer that treated `deltas: []` as "no developments" is corrected;
the tool already documents that a no-match result is not a verified absence. No index format
change, no storage migration, no dependency change.

## Hosted qualification

The candidate is pushed to `origin/feat/auto-20261117-044c4885` for the hosted pipeline.

The first hosted attempt on `10986ed` (run
[44788814570](https://github.com/eggstack/eggsearch/actions/runs/44788814570)) reported an
overall failure because the macOS canonical verification and local filesystem portability job
was cancelled; the cancellation reason was not exposed in the run result. The routine CI
verification and Windows portability jobs in that run succeeded. The separate egress
qualification, release drift guard, and Python native qualification runs also succeeded
([egress](https://github.com/eggstack/eggsearch/actions/runs/44788814555),
[release drift](https://github.com/eggstack/eggsearch/actions/runs/44788814607),
[Python native](https://github.com/eggstack/eggsearch/actions/runs/44788814692)).

A re-run on `a9fc806` — the same source and test tree, with this record and the inventory
counts as the only differences — is fully green:
[CI](https://github.com/eggstack/eggsearch/actions/runs/44797522795) passed all three jobs
(`ci`, macOS canonical verification and local filesystem portability, and Windows portability),
as did [egress qualification](https://github.com/eggstack/eggsearch/actions/runs/44797522796),
[release drift guard](https://github.com/eggstack/eggsearch/actions/runs/44797522762), and
[Python native qualification](https://github.com/eggstack/eggsearch/actions/runs/44797522799).
The cancellation on the earlier attempt therefore does not stand as a macOS signal on this
tree.

## Recommendation

Accept M004 on `380a55cd7ea7c0b37dc6eff53851926e96e894d0`. Every acceptance criterion has
deterministic test evidence, the live probe shows the bare empty delta list replaced by a
named reason, the dated-source identity is unchanged, and the residual bounds are explicit.
