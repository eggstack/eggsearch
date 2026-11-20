# Knowledge-delta discovery evaluation

The evaluation is a frozen, network-free corpus that measures how `knowledge_brief` ranks and dates evidence it can already see. It is a qualification harness, not a production feature: nothing in the shipped binary changes because of it.

## What it measures and what it does not

Measured: conditional ranking and date accounting. The gold release record is already in the candidate pool or in the seeded local index, so the harness answers "given the right source, does the brief select it, date it correctly, drop duplicates, and stay inside its budget".

Not measured: live discovery. A source that no selected provider returns cannot appear in a frozen pool, so a miss of that kind is invisible here. That failure class is measured only by the separate prospective live study, which is operational evidence and never a CI gate.

## Corpus

`tests/fixtures/knowledge_delta_quality/corpus.json` (frozen 2026-11-20, sha256 `4868f1fe09de2ba4c66b8d487a0f205f9335e14ff257da94c27b30ee9be02d7b`) holds 82 dated release events across eight ecosystems and 88 cases:

| Class | Cases | Expectation |
|---|---|---|
| `direct` | 14 | the dated gold card is in the live pool and must be emitted |
| `withheld` | 27 | the gold card is indexed only and shares no term with the topic; only a source-backed edge reaches it |
| `negative_control` | 14 | publication is on the cutoff day, so it must not be reported after the cutoff |
| `negative_past` | 13 | publication precedes the cutoff and must not be emitted |
| `ambiguous_month` | 14 | a month-precision cutoff in the same month is indeterminate, not after |
| `conflict` | 6 | the same canonical source is declared twice with two dates; at most one delta |

Publication dates come from the endoflife.date release catalog, a clearly labelled secondary source; the exact feed URLs are recorded in the corpus `_meta`. Card URLs are frozen identifiers modelled on each product's official release host. They keep identities stable and spread the pool across hosts; they are not claimed to be fetchable pages, and no vendor text is copied.

Every case pins a selected provider set that advertises `result_timestamps` (`hn_algolia`) and a caller cutoff. A run whose `coverage.timestamp_capable_providers` is empty is invalid, not a zero score, and the harness asserts that.

## Variants

| Variant | Request | Question |
|---|---|---|
| `direct_live` | `refresh: live`, no index | does the live selection and date filter work on its own |
| `indexed_lexical` | `refresh: index` | does lexical recall over retained evidence work |
| `indexed_expanded` | `refresh: index`, `related_topics: true` | does one source-backed edge recover a source the topic never names |

## Measured result

`RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_quality -- --nocapture`

| Metric | direct_live | indexed_lexical | indexed_expanded |
|---|---|---|---|
| Cases | 88 | 88 | 88 |
| Expectation met | 88 | 88 | 88 |
| Gold emitted | 20 | 20 | 47 |
| Withheld recall (27 cases) | 0.000 | 0.000 | 1.000 |
| False temporal claims | 0 | 0 | 0 |
| Duplicate ids | 0 | 0 | 0 |
| Max emitted payload (chars) | 2,575 | 455 | 936 |
| Mean emitted deltas | 1.73 | 0.53 | 0.84 |
| Span coverage, expanded deltas only | n/a | n/a | 1.000 |
| Mean wall time (ms) | 0.85 | 0.08 | 0.30 |

The expansion step is the only route to a withheld source: without it the withheld set scores 0/27, with it 27/27. That is an ablation of behaviour already shipped in M003, not a new recall claim, and no ranking, planner, or scoring code changed for this milestone. Span coverage is reported for expanded deltas only because a live or lexical delta has no indexed edge to cite until a source is fetched.

## Determinism and budget assertions

The harness asserts, for every case and variant: no duplicate ids, no false temporal claim on a negative or ambiguous case, at most one delta per conflicting source, `deltas <= 8`, emitted payload `<= 6000` characters, `kdelta_` identities, non-empty source ids, HTTPS urls, `external_untrusted` trust, and `index_status: open` for index variants. A separate test replays six cases twice and requires identical deltas and coverage.

## Limits

- The pool is synthetic in membership and size; it cannot show provider recall or live crawl breadth.
- Dates are secondary-catalog facts, not re-verified per release page.
- The single permitted provider profile means the corpus cannot compare providers.
- The corpus is English-only and release-centric.
