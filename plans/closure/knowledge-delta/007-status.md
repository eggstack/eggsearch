# M007 Real-World Discovery Quality Qualification — Closure Evidence

Status: closed (measurement-only)

Source plan: `plans/archive/implementation/knowledge-delta/007-live-discovery-quality-qualification.md`

Roadmap: `plans/subsystems/knowledge-delta-corrective-addendum.md` M007

Corrective to: M003, plan `plans/archive/implementation/knowledge-delta/003-source-driven-topic-discovery.md`,
closure `plans/closure/knowledge-delta/003-status.md`.

Decision authority: accepted ADR-0020 and ADR-0021, plus accepted ADR-0006 (keyless
deterministic retrieval). No ranking, planner, scoring, or discovery code changed, so no new
ADR and no behavior change is proposed.

## Scope and closure basis

M003 proved one synthetic omitted-name fixture and its negative controls, and explicitly did
not measure precision or recall. M007 builds the measurement that was missing and applies it
to the shipped behavior:

1. A frozen 88-case corpus over eight ecosystems, with withheld-name, negative, ambiguity,
   and conflict classes.
2. A network-free harness that runs three equal-budget variants over the same corpus and
   asserts structural invariants on every run.
3. A separate prospective live study of 15 contemporary topics with a pinned
   timestamp-capable provider.
4. No production change. The ablation did not justify one.

The plan permits a measurement-only closure with explicit maintainer approval and forbids
claiming improved recall from it. The maintainer's instruction to complete this milestone in
full is that approval. Nothing below claims that shipped recall improved; it reports what the
shipped behavior does on a frozen corpus and what one live provider returned on one day.

## Exact code candidate

Implementation candidate `e817dfde35372aa307f5580a144d7ededcb55574`. The corpus is 58,328
bytes with sha256 `4868f1fe09de2ba4c66b8d487a0f205f9335e14ff257da94c27b30ee9be02d7b`.
This closure commit carries this record, the plan status, the roadmap and addendum status,
and the registry rows together.

## Corpus composition

| Class | Cases | Expectation |
|---|---|---|
| `direct` | 14 | dated gold card is in the live pool and must be emitted |
| `withheld` | 27 | gold card is indexed only and shares no term with the topic; only a source-backed edge reaches it |
| `negative_control` | 14 | publication is on the cutoff day; must not be reported after it |
| `negative_past` | 13 | publication precedes the cutoff; must not be emitted |
| `ambiguous_month` | 14 | month-precision cutoff in the same month is indeterminate, not after |
| `conflict` | 6 | same canonical source declared twice with two dates; at most one delta |
| **Total** | **88** | 82 dated release events across Python, Node.js, PostgreSQL, Kubernetes, Go, Rust, Ruby, and Django |

The plan asked for at least 80 dated cases, 20 withheld-name cases, 20 negative controls, and
15 ambiguous or conflicting cases; the corpus delivers 88, 27, 27, and 20.

Publication dates come from the `endoflife.date` release catalog, recorded in the corpus
`_meta` with the exact feed URLs and labelled `secondary_catalog`. Card URLs are frozen
identifiers modelled on each product's official release host; the corpus `_meta` states that
they are not claimed to be fetchable and that no vendor text is copied. The provider profile
is pinned to `hn_algolia`, which advertises `result_timestamps`; the harness fails an
invalid run when `coverage.timestamp_capable_providers` is empty.

## Measured result

Command: `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_quality -- --nocapture`

| Metric | `direct_live` | `indexed_lexical` | `indexed_expanded` |
|---|---|---|---|
| Cases | 88 | 88 | 88 |
| Expectations met | 88 | 88 | 88 |
| Gold emitted | 20 | 20 | 47 |
| Withheld-name recall (27) | 0.000 | 0.000 | 1.000 |
| False temporal claims | 0 | 0 | 0 |
| Duplicate identities | 0 | 0 | 0 |
| Max emitted payload (chars) | 2,575 | 455 | 936 |
| Mean emitted deltas | 1.73 | 0.53 | 0.84 |
| Span coverage, expanded deltas | n/a | n/a | 1.000 |
| Mean wall time (ms) | 0.85 | 0.08 | 0.30 |

The ablation is unambiguous on this corpus: one source-backed index edge is the only route to
a withheld release, and without it the withheld set scores 0 of 27. The gain is a property of
the expansion path M003 already shipped, not of a change made here.

The plan pre-registered a 10 percentage-point recall@5 gain, zero false post-cutoff
classifications, full source traceability, and a 3-point false-positive tolerance for any
*changed* ranking or expansion implementation. No change was proposed, so those thresholds are
not asserted as a pass; the three invariants that can be checked are: zero false temporal
claims, zero duplicates, and 100% span coverage on expanded deltas.

Failure taxonomy applied to the corpus runs: no observed instance of (b) no dated field,
(c) wrong date basis, (d) source not fetched, (e) fetched but selected out, (f) generic
expansion noise, or (g) duplicate or stale index hit. Class (a), no source discovered, is not
observable in a frozen pool and is measured only by the live study below. Class (h), provider
unavailable, is pinned out by the frozen profile.

## Prospective live study

Operational evidence, not a CI gate. MCP `stdio`, provider `hn_algolia` only, caller cutoff
`2026-01-01` day precision, 15 contemporary topics across software releases, dependency
compatibility, public infrastructure, public health, science, and two no-material-change
controls. Run on 2026-11-20 against the candidate binary.

| Outcome | Topics |
|---|---|
| Dated deltas emitted | 3 — `postgresql-19` (4 deltas), `rust-1-100` (1), `go-1-27` (1) |
| Zero cards returned by the provider | 12 — all reported `no_dated_sources` with `selected_cards: 0`, `dated: 0`, `undated: 0` |

All six emitted deltas were `after_cutoff`, day precision, and on-topic on adjudication
(6/6): Phoronix, the PostgreSQL news archive, a Snowflake engineering post about the
PostgreSQL 19 release timing, the Rust 1.100.0 release tag, and the Go 1.27 release notes.
`coverage.timestamp_capable_providers` was `["hn_algolia"]` on every row, so every run is
valid under the pinned-profile rule.

The dominant live finding is coverage, not dating: twelve topics produced no candidate cards
at all. That is failure class (a), no source discovered, and it is a provider-breadth limit of
the single keyless timestamp-capable provider, not a date-accounting defect. The shipped
default providers cannot date anything, which is why the profile was pinned.

Two excerpt-quality limits were observed and are recorded rather than fixed: one
`postgresql.org` delta carried site navigation chrome ("Quick Links About Governance…"), and
one Hacker News delta carried the visible
`<<<EXTERNAL_UNTRUSTED field=snippet …>>>` framing. Both are existing selection behavior;
neither is a temporal or trust defect.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | Corpus of at least 80 dated cases across the required classes | 88 cases, 82 events, eight ecosystems; `corpus_meets_the_declared_composition`. |
| 2 | Every case records cutoff, expected classification, topic, gold anchor, and class | Corpus `cases` entries carry `cutoff`, `expect`, `topic`, `goldEventId`, `class`, and a note; the harness rejects an unknown gold event or an invalid precision. |
| 3 | A selected provider set advertises `result_timestamps`; an empty set is invalid | Pinned `hn_algolia`; `assert_run_invariants` fails a live run with an empty `timestamp_capable_providers`, and the report counts zero capability gaps. |
| 4 | Equal-budget variants over the same corpus | All three variants use `max_items: 8`, `max_chars: 6000`, the same case, the same cutoff, and the same frozen date. |
| 5 | Zero false post-cutoff classifications and no duplicate identities | Negative and ambiguous classes met 41/41 in every variant; `duplicate_ids: 0` in every variant. |
| 6 | Traceable source identity and span for expanded claims | Every delta carries a `kdelta_` id and non-empty `source_ids`; expanded deltas carry `edge_source_ids` and a span, 100% in the expanded variant. |
| 7 | Deterministic replay | `repeated_runs_are_identical` replays six cases twice on two variants and requires identical deltas and coverage. |
| 8 | Budgets never exceed configured caps | Per-run assertions: at most 8 deltas and at most 6,000 emitted characters; the observed maxima are 2,575 and 936. |
| 9 | A no-improvement outcome is reported, not overstated | No source change; the closure claims measurement, not improvement. |
| 10 | Exact-candidate broad gate passes | `make check` exit 0 and `make docs-check` exit 0 on `e817dfd`. |

## Verification

Using pinned Rust 1.89 on candidate `e817dfd`:

- `make check`: exit 0 — 6,549 passed, 0 failed, 28 ignored on `--all-features`.
- `make docs-check`: exit 0.
- `tests/knowledge_delta_quality.rs`: 3 passed (composition, qualification, replay).
- `tests/knowledge_delta_discovery.rs`: 10 passed.
- `tests/knowledge_delta_brief.rs`: 17 passed.
- `tests/knowledge_delta_index.rs`: 15 passed.
- `tests/property_knowledge_delta.rs`: 9 passed.
- `tests/static_guards.rs`: 67 passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.

## Known limitations and residual risk

1. The corpus measures conditional ranking and date accounting. The gold source is already in
   the pool or the index, so the corpus cannot show provider recall, crawl breadth, or live
   discovery. Only the live study touches that class, and it samples one provider on one day.
2. Publication dates are secondary-catalog facts. Individual release pages were not re-read
   per case, and a catalog error would propagate into the corpus.
3. The corpus is English-only and release-centric. Scientific and public-health classes are
   represented in the live study only.
4. The live study's 3-of-15 topic coverage is thin enough that it must not be read as a recall
   estimate. It shows that dating works when cards arrive and that card arrival is the
   bottleneck.
5. Span coverage is 100% only for expanded deltas. A live or lexical delta has no indexed edge
   to cite until a source is fetched, and these runs use no evidence fetcher.
6. Excerpt selection can still surface site chrome or untrusted-field framing; both were
   observed and neither is fixed here.

## Compatibility and migration

No production behavior, tool, argument, schema, configuration, or storage change. The changes
are a test suite, a frozen fixture, and documentation.

## Hosted qualification

The candidate was pushed to `origin/feat/auto-20261117-044c4885`. On `bc52970` (this closure
commit, which differs from the candidate only in planning documents), CI run
`44812224641` needed three attempts:

1. The `ci` and `windows portability` jobs succeeded. The
   `macOS canonical verification and local filesystem portability` job failed on
   `tests/local_workspace_integration.rs::local_repo_search_finds_source_files`
   (`tests/local_workspace_integration.rs:793`), a temporary-git-repository fixture that this
   milestone does not touch.
2. `gh run rerun --failed` failed at the same assertion.
3. A third attempt passed all three jobs, including macOS canonical verification.

The compiled test binary in both failing attempts hashes `3ace87ef457959f8`, the same binary
that passed this test in the `a9fc806` macOS job (`44797522795`). A byte-identical test binary
cannot be changed by this milestone's source, so the two failures are environment-dependent
rather than a regression introduced here. Locally the exact test passes 8 of 8 repeats and the
full `make check` passes on the same tree. `Egress feature qualification` and
`Release drift guard` succeeded on the candidate in the same push.

The hosted pipeline is green on the final attempt; the two earlier macOS failures are recorded
rather than omitted. The determinism gap they exposed is registered as M043 in `plans/archive/implementation/agentic-retrieval-integrity/043-local-repo-search-empty-result-determinism.md`.

## Recommendation

Accept M007 as a measurement-only closure on `e817dfde35372aa307f5580a144d7ededcb55574`.
The corpus and the ablation now exist as regression assets; the honest finding is that the
shipped expansion path is what recovers withheld evidence, and that keyless timestamp-capable
provider breadth, not date accounting, limits live discovery.

This acceptance includes a green hosted CI run on the third attempt; the two earlier macOS failures on an unrelated environment-sensitive test are recorded above rather than omitted.
