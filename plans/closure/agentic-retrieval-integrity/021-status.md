# M021 Non-HTML Document-Scoped Addressable Text — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/021-non-html-document-scoped-addressable-text.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

ADR: `plans/adrs/ADR-0011-fetch-document-addressing.md`

Planning baseline: `3259166`

Implementation candidate and closure commit: the commit that carries this record, on top of the M020
closure commit `189ee07` and its three record commits.

## Result

The block and chunk set of every non-HTML document is now built from the document-bounded text
instead of the caller's window. The production change is one branch condition in
`src/fetch/client.rs::derive_from_raw`: the non-HTML document arm matches `raw_text` (which is
already `bound_text(strip_control_chars(decoded), document_chars)`) instead of `pre_framing_text`
(which is `bound_text(full_text, window_chars)`). `text_chars` still counts `pre_framing_text`,
because it describes the legacy `text` field. No signature, response key, error code, renderer, or
cache class changed.

Measured on a 20,031-byte `text/plain` body with 299 paragraphs at default budgets
(`max_chars_default` 12,000, `max_chars_cap` 50,000):

| Observation | Before | After |
|---|---|---|
| addressed `text_total` (independent of `limit`) | 11,821 | 19,733 |
| addressed read at `offset = 0`, default window: `next_offset` | absent | 12,000 |
| addressed `document.text_truncated` on that read | false | true |
| addressed read at `offset = 13,000`, `limit = 200` | empty window (`from == to == 11,821`) | `[13000, 13200)`, 200 scalars (282 on the wire with the 82-scalar trust envelope) |
| addressed read at `offset = 0`, `limit = 50,000` | — | `[0, 19733)`, no `next_offset`, `text_truncated = false` |
| unaddressed `text` | 12,082 scalars, raw decoded prefix | 11,697 scalars, chunk-join projection |
| unaddressed `document.blocks` | 180 | 299 |
| unaddressed payload | 52,295 bytes | 67,656 bytes |

And on the live measurement the M020 record used, `https://www.rfc-editor.org/rfc/rfc9110.txt`:

```
before: text_total=11981  first=[0,4000) first_chars=4000  second=[5000,9000) second_chars=4000
after:  text_total=49852  first=[0,4000) first_chars=4000  second=[5000,9000) second_chars=4000
```

The RFC is several hundred kilobytes, so 49,852 is the 50,000-scalar extraction budget, and the
remainder is reported by `document.document_truncated` rather than hidden. First-window recall and
median chars-to-answer in `tests/fixtures/fetch_windows/` are unchanged (7/11 = 63.6%, 4,200),
because containment is a property of the window rather than of the total; the one plain-text corpus
row moves from `text_total` 11,997 to 13,581 with `text_to` 12,000.

## Correction to M020's record

Research at this baseline found that M020's named condition was half wrong, and that the wrong half
had been committed as fact. Both halves are now resolved:

1. **PDF was never window-scoped.** `src/fetch/pdf.rs` bounds blocks by `PdfLimits`
   (`max_chars_per_page` 12,000, `max_total_chars` 50,000) and builds its single chunk from all
   blocks, so `extract_pdf_text`'s `max_chars` argument bounds only `PdfResult.text`, its counts,
   and its flags. A six-page fixture served locally and fetched at `limit = 50,000` with the
   12,000-scalar default window reports `text_total = 12,599`, `next_offset` absent because the whole
   document fits the limit, and one chunk of 12,599 scalars. M020's residual-risk item 1 named the
   `client.rs:677` call as the cause; that call does not have that effect. The record now says so.
2. **The non-HTML half was real and worse than recorded.** At `3259166` a 20,063-byte `text/plain`
   body fetched with `offset = 0` reported `text_total = 12,000`, no `next_offset`, and
   `document.text_truncated = false`, while the unaddressed read of the same document reported
   `document.text_truncated = true` and `offset = 13,000` clamped to an empty window. That is two
   reads of one document contradicting each other and the addressed read claiming completeness, not
   merely a missing capability.

## Discharge

M020's condition is discharged at this candidate. `plans/closure/agentic-retrieval-integrity/020-status.md`
now reads `Status: closed`, carries a "Named condition — discharged" section with both measurements,
corrects deviation 5 and residual risk 1, and re-records its live two-window numbers;
`plans/implementation/agentic-retrieval-integrity/020-offset-addressing-and-cursor-retirement.md`
reads `closed`. Two header corrections are already in history rather than repeated here: M019's
plan header read `ready` while the registry read `closed` until the M020 closure commit corrected
it, and M020's header moved from `blocked` to `conditionally closed` at its own closure. Every
`closed` row in `plans/registry.md` for this workstream now matches its plan header and its closure
record.

## What changed

- `src/fetch/client.rs`: the non-HTML document arm renders from `raw_text`; the comment above
  `text_chars` now states why that count stays window-scoped.
- `tests/web_fetch_integration.rs`: six tests that asserted block text is bounded by `max_chars` are
  retargeted through `assert_window_scoped_text`, which asserts the corrected three-part contract
  (`text_chars_returned` inside the window, at least one block wider than the window, the window cut
  flagged). `web_fetch_code_block_text_never_exceeds_max_chars` is renamed
  `web_fetch_code_block_text_covers_the_document_budget`, and the section comment is corrected.
- `tests/mcp_tools.rs`: `document_budget::plain_text_total_does_not_vary_with_the_caller_window`
  (three caller windows, one `text_total`, and it exceeds the default window) and
  `document_budget::plain_text_windows_reach_the_tail_and_compose` (2,000-scalar windows tile the
  single wide read exactly; the unaddressed and addressed reads agree on `text_truncated`; the
  unaddressed read carries no `text_total` or `next_offset`).
- `tests/pdf_fidelity.rs`: `pdf_chunks_are_document_scoped_while_legacy_text_follows_max_chars`
  pins the corrected PDF fact — for a committed `clean_text` fixture at `max_chars = 100`, the
  legacy text stays within 100 scalars while the chunk set exceeds it. It lives in the fidelity
  suite rather than in `src/fetch/pdf.rs` because that module sits at a 70,000-byte ratchet ceiling
  (69,505 bytes after this change) and the guard forbids growing it.
- `tests/fixtures/fetch_windows/README.md`: the non-HTML basis section, the recorded corpus row, and
  the live two-window record.
- Records: this plan, this record, M020's corrections, the roadmap, and the registry, in one commit.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --no-fail-fast
cargo test --locked --all-features --test web_fetch_integration -- --ignored offset_windows_live --nocapture
cargo test --locked --features mock -- --list
python3 packaging/check-planning-consistency.py
./packaging/check-repo-hygiene.sh
```

Results:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo check --locked --no-default-features`: clean.
- `cargo test --locked --all-features --no-fail-fast`: 6197 passed, 0 failed, 28 ignored (85 suites, exit 0)
- `cargo test --locked --features mock -- --list`: 5841 tests, 4 ignored
- `cargo test --locked --all-features --test web_fetch_integration`: 87 passed, 1 ignored; the
  ignored `offset_windows_live` was run explicitly against the network and passed with the numbers
  above.
- `python3 packaging/check-planning-consistency.py`: `planning-consistency: ok`.
- `./packaging/check-repo-hygiene.sh`: `repo-hygiene: ok`.
- Hosted CI: run `43331485842` on `027f483`. The Ubuntu `ci` job (`make ci`, which is `make check`) passed and so did the Windows `x86_64-pc-windows-msvc` all-features check; the macOS job's canonical-verification step passed, and its filesystem-portability steps were still running when this note was written. That run also settles the excerpt-economy hazard below in the favourable direction for this commit: a fresh seed did not draw the counterexample.

`make check` cannot complete on this workstation because `packaging/check-contract.sh` stops at
`mavis-trash: unsupported OS: Plan9` after passing the workflow-pin, planning-consistency, eggpack
qualifier, and egress qualification sub-checks; the hosted `ci` job supplies that leg.

## Findings not fixed — PDF ingestion (recorded, out of scope here)

These are real and measured, but they belong to the PDF ingestion subsystem; none affects
`text_total`, offsets, or the non-HTML work in this milestone. PDF ingestion M005 is `ready` and must
reconcile item 5 before it lands.

1. **`document_truncated` is conflated with the legacy-text cut (medium).** `pdf.rs` sets
   `document_truncated = text_truncated`, and `text_truncated` is set when the *legacy* text is cut at
   `max_chars`. A fully extracted six-page fixture reports `document_truncated = true`; an addressed
   read then reports `text_truncated = false` beside it.
2. **`chunk_id` is the literal `chunk_0` (medium-high).** Every PDF document in every process shares
   it, and it is not content-derived, which contradicts the repository rule that stable ids are
   FNV-1a derivations of `(doc_id, chunk_index, heading_path)`.
3. **`max_pages` drops pages silently (medium).** Pages beyond `pdf_max_pages` are cut by
   `take(limits.max_pages)` with no warning and no flag, so a 40-page PDF reports success with 25
   pages' content and no truncation marker.
4. **An addressed PDF window ships the whole chunk (low-medium).** `windowed_chunks` keeps any
   overlapping chunk whole, and PDF has one chunk covering the document, so a 200-scalar window
   carries up to 50,000 scalars of `document.chunks`.
5. **PDF ingestion M005 would regress the corrected behaviour.** That plan bounds blocks, chunks, and
   legacy text by `min(pdf_max_total_chars, max_chars)`, which would re-pin PDF to the caller window
   and make `text_total` window-scoped again. It should be reconciled with this record before it is
   implemented.

## Unresolved findings — outside this milestone

1. **`tests/property_excerpt_economy.rs::suppression_is_monotone_in_budget` has a real counterexample
   (medium; search side, pre-existing).** The property asserts that a larger excerpt budget never
   admits fewer passages. A generated payload with five near-identical snippets produced
   `admitted_count = 8` at budget 384 and `7` at 435. The projection itself is deterministic — twelve
   consecutive projections of one payload returned identical counters and identical serialized
   bytes — so the counterexample is a genuine property failure, most plausibly because admission is
   diversity-aware rather than a greedy prefix, so a longer admitted passage consumes budget that a
   shorter one used. It was not produced by any M020 run and it reproduces with M021's change
   stashed, so it is not caused here. It needs a corrective plan that decides whether the guarantee
   belongs in the implementation or whether the property should state only what the design provides.
   Until then the assertion can fail on a fresh seed, which is a latent CI hazard: a hosted run that
   draws the counterexample will fail this suite.

## Residual risk

1. **Unaddressed non-HTML payloads grow** (+29% measured: 52,295 to 67,656 bytes for a 20 KB body)
   because `document.blocks` now covers the document budget. HTML has carried this since M019; the
   `brief`/`compact` projections clip strings, `diagnostic` does not.
2. **Unaddressed non-HTML `text` changes whitespace** for paragraph-style plain text: the chunk join
   is paragraph-normal, so blank lines between paragraphs collapse. This matches what an addressed
   read already returned; changing the renderer is a separate concern.
3. **Chunk ids shift** for non-HTML documents whose text now exceeds the previous window — the same
   class of churn M019 recorded for HTML. Chunks wholly inside the first window keep their ids.
4. **Non-HTML cache entries remain per caller window** (`caller_chars: Some(window)`), because
   cache-hit re-projection is gated to HTML and the stored `text` is served verbatim for other kinds.
   Sharing them needs `text` re-derivation from `raw_text`, not this change.

## Deviations from the plan

1. **The plan and its closure land in one commit.** This is a one-branch-condition change with its
   regression tests; a handoff-then-implement split would add a commit without adding evidence. The
   plan's recorded measurement table is the one taken before the change, so the record is not
   retro-fitted.
2. **PDF was excluded after being measured, not assumed.** The plan's non-goals say why: the claim
   that PDF needed the same fix did not survive measurement, and its real defects are a different
   milestone's scope.
3. **`conditionally closed` was the wrong label for M020's condition.** `plans/closure/README.md`
   reserves it for named external or operational evidence that cannot be obtained; M020's condition
   was unimplemented scope, and it had no registered discharge vehicle, unlike M006 (M017). M021 is
   that vehicle, and this discharge restores the shape the process asks for rather than relabelling
   history.
