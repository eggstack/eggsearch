# M004 — Brief Delta Production and Honest Diagnostics

Status: ready
Date: 2026-11-20
Subsystem: knowledge delta
Milestone class: capability (corrective to M001)
Baseline: `16b4c2888206caeb6654ffebaa039862fc143712` on `feat/auto-20261117-044c4885`
Decision authority: existing contracts — accepted ADR-0020 (cutoff-aware knowledge brief),
accepted ADR-0021 (optional local knowledge index), accepted ADR-0006 (keyless deterministic
retrieval), and the local workspace boundary in `architecture/local-workspace.md`. No new ADR.

References: `plans/000-long-term-specification.md` §4 and §7; `plans/001-terminology-and-domain-model.md` §2;
`plans/002-long-term-roadmap.md` §9; `plans/003-planning-process.md` §7;
`plans/subsystems/knowledge-delta-roadmap.md` M001;
original plan `plans/archive/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md`;
original closure `plans/closure/knowledge-delta/001-status.md`.

## Objective

Make `knowledge_brief` produce a delta whenever a dated source exists, and make every empty
delta set name its reason. Today the shipped default deployment can never emit a delta, and a
caller cannot tell that apart from "there was nothing new".

Concretely: a brief must account for every selected source card by date availability and
cutoff relation, must name the reason an empty delta set is empty, and must not return a bare
`deltas: []` while `no_temporal_claim` is false and no warning explains the absence.

## Evidence

Measured on 2026-11-20 against baseline `16b4c2888206caeb6654ffebaa039862fc143712`, driving the
MCP server over stdio with no provider API keys configured (the shipped keyless default).

1. The shipped default web providers are `duckduckgo`, `brave`, and `yahoo`. All three are
   `html_scrape` engines advertising only the `basic` capability; none advertises
   `result_timestamps`.
2. Every HTML scrape engine hardcodes `published_at: None`:
   `src/meta/engines/duckduckgo.rs:104`, `brave.rs:98`, `yahoo.rs:103`,
   `startpage.rs:261`, plus `mojeek.rs` and `searxng.rs:106`.
3. `package_deltas` (`src/meta/knowledge_delta/execute.rs:448`) returns an empty vector when no
   cutoff resolved (line 454) and skips every card whose `metadata.published_at` is absent
   (line 459). A card without a publication date can therefore never become a delta.
4. Live brief against the default configuration (`topic: "rust ownership"`): `deltas: []`,
   `coverage.fetch_candidates: 0`, `coverage.fetch_invoked: false`,
   `coverage.no_temporal_claim: true`, `total_payload_chars: 0`. The only warnings named
   provider failures. Nothing stated that no source carried a date.
5. Live brief with `cutoff_override` (`2020-01-01`, day precision): the cutoff resolved
   (`model_resolution.status: resolved`, `origin: caller_override`, `no_temporal_claim: false`,
   `date_range_applied: true`) and the result was still `deltas: []` with
   `fetch_candidates: 0`. A resolved cutoff over undated sources is indistinguishable from a
   quiet success.
6. Live brief with `hn_algolia` enabled (keyless, advertises `result_timestamps`): **7 deltas**,
   `kdelta_` identities, `date_basis: publication`, `precision: day`,
   `temporal_relationship: after_cutoff`, `fetch_attempted: 3`, `fetch_completed: 2`. The
   brief logic is correct; only the default provider set cannot feed it.
7. Retaining that brief wrote **2 of 7** candidate events to the index. This is correct:
   `execute.rs:215` sets `indexable` only from a successful cacheable fetch, and one of the
   three fetches failed. No defect here.
8. The excerpt stored for a Hacker News delta begins with site chrome
   (`| Hacker Newsnew|past|comments|ask|show|jobs|submitlogin |`) before the article text.
   `BriefToolFetcher` in `src/mcp/tools/knowledge_brief.rs` takes `document.chunks` 0..2, and
   for that page the first chunks are navigation.
9. Pointing `[knowledge_delta.index].path` at an explicit `.db` inside a pre-existing
   group-readable directory made `open_index` fail with
   `knowledge index path must be private to the current user`, `index_status: "offline"`, and
   a warning naming the rule but not the directory that had to be fixed.

## Why the M001 verification missed this

M001's acceptance was exercised through the `mock` engine harness, whose engine fixture can set
`published_at` (`src/meta/engines/mock.rs:56`). Dated cards were therefore always present in
the suite, and the delta-production path was never exercised against the shipped default provider
set. No M001 acceptance criterion required a delta under the default configuration, and the
closure record lists the mock and property suites as the evidence. The gap is in the acceptance
surface, not in the implementation of ADR-0020's comparison rule, which is unchanged and remains
correct.

## Scope

In scope:

- per-card date accounting for a brief (dated, undated, before-cutoff, at-cutoff);
- explicit, distinct empty-state reasons for an empty delta set;
- reporting which selected providers advertise a result-timestamp capability, so an operator can
  see why no dated source existed;
- excerpt selection that prefers an article-bearing chunk over site chrome;
- an actionable index-path permission warning.

Out of scope:

- replacing an HTML scraper with an API, changing any shipped provider default, or adding a
  provider (ADR-0006; upstream markup drift is a maintenance cost of the design);
- parsing a date out of a snippet, an HTML page, or any text that is not a declared provider
  timestamp — ADR-0020 forbids inferring a date, and this milestone does not relax that;
- a feed importer, a crawler, a refresh scheduler, an embedding index, or any LLM call;
- a new MCP tool, argument, schema property, configuration key, or Cargo feature;
- changing the stable `kdelta_` identity rule, the cutoff precedence, or the partition model;
- reopening M002 storage format, M003 discovery semantics, or any other closed scope.

## Invariants

- The ADR-0020 comparison rule, its precision and basis handling, and the deterministic
  `kdelta_` identity are unchanged. A delta is still only emitted from a declared publication
  date compared with a resolved cutoff.
- A live call still does not read or write the index. `index_status: not_opened` still holds for
  a live brief with no `retain`.
- Only selected public evidence that `should_cache_response` would accept is stored, and the
  partition model (`anonymous` or `prof_`) is unchanged.
- Untrusted framing, trust markers, and the external-untrusted trust level are unchanged.
- No date is inferred from prose. An unknown or missing date stays unknown.
- Bounded fan-out, byte, character, and wall-clock budgets are unchanged.
- No new tool, argument, schema property, provider, dependency, or shipped default.

## Expected production changes

- `src/meta/knowledge_delta/execute.rs`: date accounting over the selected cards; bounded
  coverage fields and empty-state reasons; provider capability reporting for the selected
  providers.
- `src/meta/knowledge_delta/plan.rs`: typed empty-state reason vocabulary, if the reason is
  derived at planning time.
- `src/mcp/tools/knowledge_brief.rs`: excerpt chunk selection that prefers an article-bearing
  chunk; forwarding the new bounded coverage fields.
- `src/meta/knowledge_delta/index.rs`: the permission-failure message names the rejected
  directory. The fail-closed rule itself does not change.
- `tests/knowledge_delta_brief.rs`, `tests/knowledge_delta_index.rs`,
  `tests/property_knowledge_delta.rs`: regression coverage.
- `architecture/research.md`, `architecture/evidence-workflow.md`, `docs/tool-matrix.md`,
  `docs/agent-workflows.md`, `docs/config.md`, `docs/test-inventory.md`, and `CHANGELOG.md`.

## Compatibility and migration

- No tool, argument, schema property, or configuration change.
- Response shape change: `coverage` gains bounded fields describing date accounting and the
  empty-state reason. Consumers that read `coverage` read additional keys; no existing key is
  removed or renamed.
- Behaviour change: an empty delta set now carries a reason. A consumer that treated
  `deltas: []` as "no developments" is corrected; the tool already documents that a no-match
  result is not a verified absence.
- No index format change, no storage migration, no dependency change.

## Dependencies

- Hard: M001 (closed) — this is a corrective pass to M001 and reuses its cutoff resolution,
  comparison rule, and delta packaging unchanged.
- Interface: ADR-0020 (accepted) and ADR-0021 (accepted) for the comparison rule and the index
  contract; `architecture/local-workspace.md` for the local boundary.
- Soft: none. M002 and M003 remain closed and are not reopened.

## Work packages

1. **Paper first.** Update `architecture/research.md` to state the dated-provider requirement, the
   new empty-state reasons, and the date-accounting fields. Update `docs/tool-matrix.md` and
   `docs/agent-workflows.md` to match. This lands before code.
2. **Date accounting.** Over the selected cards, count `dated`, `undated`,
   `at_or_before_cutoff`, and `not_after_cutoff`. Emit the counts as bounded `coverage`
   fields. Do not change which cards become deltas.
3. **Empty-state reason.** Derive one reason from the accounting and the cutoff state:
   `no_cutoff_resolved`, `no_dated_sources`, `all_sources_at_or_before_cutoff`,
   `no_matching_index_hits`, or `deltas_emitted`. Emit it in `coverage` and, for every
   non-emitting reason, as a warning through the existing warning-prefix table. A brief must never
   return a bare empty delta list.
4. **Provider capability reporting.** Report which of the selected providers advertise a
   result-timestamp capability, so an operator sees that the empty result follows from the
   provider set rather than from the topic. Report capability only; do not change routing.
5. **Excerpt selection.** Prefer a chunk that carries article text over a chunk that is site
   chrome. Use the existing bounded excerpt budget and the existing chunk metadata. If the
   preference cannot be made without heuristic content scoring, use the existing heading and
   block metadata first and record any residual chrome rather than adding a dependency.
6. **Index path message.** When the permission rule rejects a path, name the directory whose
   mode caused the rejection and the mode it needs. The rejection itself stays fail-closed.
7. **Tests.** Extend `tests/knowledge_delta_brief.rs` and `tests/property_knowledge_delta.rs` with:
   - a brief whose selected providers carry no result timestamps reports
     `no_dated_sources` (or `no_cutoff_resolved` when no cutoff resolved) and emits no bare
     empty list;
   - a brief with a resolved cutoff and dated sources emits deltas with unchanged `kdelta_`
     identities;
   - the date-accounting counts are internally consistent with the emitted deltas;
   - the empty-state reason is one of the enumerated values and never contradicts
     `no_temporal_claim`;
   - a retained excerpt does not begin with site chrome for the captured fixture;
   - the index path rejection warning names the rejected directory;
   - a live brief still reports `index_status: not_opened` and creates no index file.
8. **Documentation.** `architecture/research.md`, `architecture/evidence-workflow.md`,
   `docs/tool-matrix.md`, `docs/agent-workflows.md`, `docs/config.md`,
   `docs/test-inventory.md`, `CHANGELOG.md`.

## Verification

```bash
cargo test --locked --features mock --test knowledge_delta_brief --test knowledge_delta_discovery
cargo test --locked --all-features --test knowledge_delta_index
cargo test --locked --all-features --test property_knowledge_delta --test property_identity
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test static_guards
make check
make docs-check
```

A live acceptance probe against the shipped default provider set is also required, because the
defect is only observable outside the mock harness. The probe must record the default
configuration, the selected providers, the emitted delta count, the new empty-state reason, and
the before/after response. It is evidence, not a threshold.

## Acceptance criteria

- A brief against the shipped default provider set names why no delta was produced and does not
  return a bare empty delta list.
- A brief with a resolved cutoff and at least one dated source emits deltas whose `kdelta_`
  identities are unchanged from M001.
- The date-accounting counts agree with the emitted deltas and with the selected card count.
- The empty-state reason is one of the enumerated values and never contradicts
  `no_temporal_claim`.
- A retained excerpt does not begin with site chrome for the captured fixture.
- The index path rejection warning names the rejected directory.
- A live brief still does not open the index, and `retain` still requires the index to be enabled.
- No new tool, argument, schema property, configuration key, provider, or dependency; no date
  inferred from prose.
- The broad gate passes with 0 failures.

## Stop conditions

Stop and record rather than working around any of these:

- producing a delta under the default provider set would require reading a date out of a snippet,
  an HTML page, or any undeclared text — that is forbidden by ADR-0020 and this milestone does
  not relax it;
- reporting the dated-provider requirement would need a new public argument — keep it inside
  `coverage` and record the constraint;
- the excerpt preference would change a stable `kdelta_` identity — identity semantics are
  fixed, and a stable-id change is a separate decision;
- making the default set produce deltas would require replacing a scraper with an API or adding a
  provider — out of scope and explicitly forbidden by ADR-0006;
- the new coverage fields would change the advertised schema of `knowledge_brief` — if the
  contract change is material, stop and record it for an ADR rather than shipping it silently.

## Required closure evidence

`plans/closure/knowledge-delta/004-status.md` with the exact candidate SHA, the original M001
closure record reference, a requirement-to-evidence matrix, the live default-configuration probe
before and after, the date-accounting and empty-state fixtures, the suite and broad-gate
results, and residual risk — including that a dated source still requires a provider that
advertises a result-timestamp capability, and that upstream provider availability is an
environment fact this milestone does not change.
