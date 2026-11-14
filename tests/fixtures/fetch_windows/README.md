# Fetch window corpus

Measurement fixture for `plans/archive/implementation/agentic-retrieval-integrity/020-offset-addressing-and-cursor-retirement.md`
§ Verification, subsection "First-slice precision, recorded rather than asserted".
It answers one question about the addressed `web_fetch` read: how often does the
first default window contain the sentence that actually answers the query?

The corpus lives in `corpus.json` (`schema_version` 1). It carries no copied
upstream text: every body is synthetic and written for this fixture.

## What the measurement is

For each case the test serves `body` from a loopback `httpmock` server under the
case's `content_type`, then calls `eggsearch::mcp::tools::run_web_fetch` with:

- `offset: Some(0)` and no `limit`, which is the default addressed window;
  the window extent is identical to an unaddressed default read, and the
  response additionally carries `text_from`, `text_to`, `text_total` and
  `doc_id`;
- `focus` set to the case's `query` when the case specifies one.

Per case it records:

| Field | Meaning |
|---|---|
| `contained` | `answer_start >= text_from && answer_end <= text_to`, i.e. the whole ground-truth span lies in `[text_from, text_to)` |
| `answer_start` | characters before the answer, in addressable-text coordinates |
| `text_from`, `text_to`, `text_total` | the returned window and the addressable total |

Aggregates over the corpus are first-window recall (`contained` cases / cases)
and the median chars-to-answer (median of `answer_start`; for an even count the
two middle values are averaged with integer division).

The test asserts only that the measurement is deterministic and complete: every
case yields `text_from == 0`, `text_to > 0`, `text_to <= text_total`, a
`doc_` + 16-hex `doc_id` whose span fits inside `text_total`, and two runs of the
whole measurement produce identical per-case records. It asserts no recall or
chars-to-answer threshold, because the milestone introduces none: a threshold
without a candidate ranking change would be a requirement the milestone cannot
discharge.

## Coordinate basis

The addressable text of a fetched document is the document's chunk texts joined
by a single `\n` (the construction `windowed_fetch` uses in
`src/mcp/tools/web_fetch.rs`), taken **before** `strip_control_chars` and
**before** `frame`. `answer_start` and `answer_end` are half-open
`[answer_start, answer_end)` offsets into exactly that string, counted in
Unicode scalar values (`char`), never bytes and never line units.

Two consequences are deliberate:

- Sanitization is a per-scalar map, so a window may return text shorter than
  `text_to - text_from` without moving an offset; the measurement never
  compares a returned length with a window length.
- `document.text_chars_returned` is window-scoped and is *not* this coordinate.
  `text_total` is the document-scoped number, and `answer_end <= text_total`
  is asserted for every case so a drifted fixture fails instead of silently
  measuring the wrong basis.

Each case stores `id`, `query` (`null` when the read is unfocused),
`content_type`, `body`, `answer_start` and `answer_end`.

## How the offsets were derived

Every body is generated from a paragraph model:

- `text/html` bodies contain one `<p>` per paragraph, each paragraph a single
  line of single-space-separated words (or of CJK text, which needs no spaces);
- `text/plain` bodies are paragraphs separated by one blank line.

Because `build_document_chunks` joins consecutive block texts with `\n` and the
addressed join uses the same separator at chunk boundaries, the addressable
text equals the paragraphs joined by `\n`. The offset of a paragraph is
therefore `sum(len(previous paragraphs)) + count(previous paragraphs)`.

That derivation was then checked against the implementation rather than trusted:
each body was fetched with `limit` at the server maximum and sanitize-output
off, `text_total` matched the constructed join length, and the returned text
sliced `[answer_start, answer_end)` reproduced the intended answer paragraph for
all 11 cases. The values in `corpus.json` are the verified ones.

Filler is deterministic ASCII in most cases, and multi-byte in three, so that
scalar-versus-byte confusion changes the outcome:

| Case | Scalars before answer | Bytes before answer |
|---|---|---|
| `html-multibyte-answer-inside-window` (CJK) | 4,200 | 12,586 |
| `html-multibyte-answer-far-past-window` (Cyrillic) | 12,350 | 22,771 |
| `text-multibyte-answer-inside-window` (Greek) | 760 | 1,419 |

`html-multibyte-answer-inside-window` is the discriminator: by scalars the answer
is inside the 12,000-scalar window, by bytes it is not.

## Corpus

Default window: `[fetch].max_chars_default` = 12,000 scalars. Document budget:
`[fetch].max_chars_cap` = 50,000 scalars. Every body is under 25 KB.

| `id` | Content type | Query | Answer span | Body bytes |
|---|---|---|---|---|
| `html-answer-inside-first-window` | text/html | archive retention window | 68–157 | 611 |
| `text-answer-inside-first-window` | text/plain | retry budget attempts | 182–255 | 518 |
| `html-answer-far-past-window` | text/html | nightly reindex budget pages | 13,050–13,132 | 14,285 |
| `html-answer-straddles-window-edge` | text/html | certificate rotation old key | 11,940–12,023 | 12,976 |
| `html-answer-starts-at-window-edge` | text/html | cold storage tier seals objects | 12,000–12,082 | 13,028 |
| `html-answer-ends-at-window-edge` | text/html | failover promotion quorum | 11,919–12,000 | 12,949 |
| `html-multibyte-answer-inside-window` | text/html | 索引 压缩 磁盘预算 | 4,200–4,230 | 14,641 |
| `html-multibyte-answer-far-past-window` | text/html | данные сжатия повтор | 12,350–12,416 | 24,444 |
| `text-multibyte-answer-inside-window` | text/plain | υπηρεσία ταξινόμησης | 760–840 | 2,173 |
| `text-no-focus-answer-inside-first-window` | text/plain | none (unfocused read) | 240–319 | 622 |
| `text-body-exceeds-window-cap` | text/plain | compaction write amplification | 900–980 | 13,584 |

The three edge cases pin the half-open semantics at the right boundary: an
answer ending exactly at `text_to` is contained
(`html-answer-ends-at-window-edge`), an answer starting exactly at `text_to` is
not (`html-answer-starts-at-window-edge`), and an answer straddling it is not
(`html-answer-straddles-window-edge`).

## Addressable basis for non-HTML documents

Every representation is addressed on the same basis: blocks are rendered from
the document-bounded text, so `text_total` is the extracted document rather than
the caller window. `text-body-exceeds-window-cap` (13,584-character
`text/plain` body) reports `text_total` = 13,581 against a 12,000-scalar window,
and `html-answer-far-past-window` reports 14,033. Before M021 the non-HTML path
rendered from the window-bounded text, so that case reported 11,997 and no
plain-text case could place an answer past the first window; the corpus now
treats both representations identically and the row below is the recorded
consequence.

## Recorded run

`cargo test --locked --all-features --test extract_fetch_contract
fetch_window_first_slice_precision_is_recorded -- --nocapture` on 2026-11-12:

```
id                                             contained  answer_start  text_total   text_to
html-answer-inside-first-window                      yes            68         478       478
text-answer-inside-first-window                      yes           182         516       516
html-answer-far-past-window                           no         13050       14033     12000
html-answer-straddles-window-edge                     no         11940       12724     12000
html-answer-starts-at-window-edge                     no         12000       12783     12000
html-answer-ends-at-window-edge                      yes         11919       12701     12000
html-multibyte-answer-inside-window                  yes          4200        4831      4831
html-multibyte-answer-far-past-window                 no         12350       13117     12000
text-multibyte-answer-inside-window                  yes           760        1161      1161
text-no-focus-answer-inside-first-window             yes           240         620       620
text-body-exceeds-window-cap                         yes           900       13581     12000
first-window recall: 7/11 = 63.6%
median chars-to-answer: 4200
```

The numbers are a property of this corpus under the shipped selection code, not
an estimate of production web quality: the corpus is synthetic, the cases are
not independently adjudicated, four cases probe window-edge arithmetic directly,
and three probe multi-byte skew.

## Live two-window record

`cargo test --locked --all-features --test web_fetch_integration -- --ignored offset_windows_live
--nocapture` reads two disjoint windows of one real document and records them here:

```
url=https://www.rfc-editor.org/rfc/rfc9110.txt
doc_id=doc_bdacf44c950f8e6d
text_total=49852
first=[0,4000)   first_chars=4000
second=[5000,9000) second_chars=4000
```

The two windows share `doc_id` and `text_total`, are disjoint, and their union
stays inside `text_total`. This is the record that discharged M020's named
condition: RFC 9110 is a plain-text document far larger than the default window,
and after M021 `text_total` is 49,852 — the document bounded by the 50,000-scalar
extraction budget — where the same measurement previously reported 11,981, the
caller-window bound. The 148 scalars beyond the extraction budget are reported by
`document.document_truncated`, not hidden.
