# M021 — Non-HTML Document-Scoped Addressable Text

Status: closed — evidence in `plans/closure/agentic-retrieval-integrity/021-status.md`

Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M021

Primary class: invariant (corrective to M020's named condition)

Baseline: `3259166`

References: ADR-0011; `plans/closure/agentic-retrieval-integrity/020-status.md`; `plans/implementation/agentic-retrieval-integrity/020-offset-addressing-and-cursor-retirement.md`; `plans/closure/agentic-retrieval-integrity/019-status.md`; `src/fetch/client.rs`; `src/fetch/pdf.rs`; `src/mcp/tools/web_fetch.rs`; `src/fetch/projection.rs`; `docs/config.md` § Offset Windows, Progress, and Tasks.

Dependencies: hard M020, which is `conditionally closed`; its single named condition is what this milestone discharges. M020's condition as recorded is that `text_total` is document-scoped only for HTML and browser-rendered documents. Measurement at this baseline splits that condition in two: the PDF half is already document-scoped and was recorded wrongly, and the non-HTML half is a real defect whose reporting layer silently claims completeness. This plan corrects the record and fixes the real half.

## Objective

Make the addressable text of every non-HTML document the extracted document rather than the caller's window, so that `text_total`, `next_offset` and offset windows mean the same thing for `text/plain`, CSV, code, notebooks and markdown source as they already do for HTML, and so that an addressed read never reports a document as complete while text remains unreachable.

## Non-goals

- No PDF behaviour change. Measurement (`plans/closure/.../021-status.md` § Findings) shows PDFs already build one chunk from all blocks with blocks bounded by `PdfLimits` (`max_total_chars` 50,000), independent of `max_chars`; `text_total` for a six-page fixture is 12,599 against a 12,000 window. The PDF defects found while checking that claim — `document_truncated` conflated with the legacy-text window cut, a hardcoded `chunk_0` that is not content-derived, `max_pages` silently dropping pages, and every addressed window shipping the one whole chunk — are recorded, not fixed here. They belong to the PDF ingestion subsystem and must be reconciled with PDF ingestion M005, whose plan re-pins the caller window (`min(pdf_max_total_chars, max_chars)`) and would regress this behaviour.
- No `text` field redesign. The legacy field stays window-scoped and keeps its existing projection rules; this milestone changes only what the block and chunk set is built from.
- No cache-class change. The derived-key class stays `caller_chars: Some(window)` for non-HTML, because cache-hit re-projection is gated to HTML (`src/mcp/tools/common.rs`) and the stored `text` is served verbatim for other kinds. Sharing non-HTML entries across caller windows is a separate change with no addressability benefit.
- No new MCP tool, argument, response key, or error code. No change to `chunk_id` derivation, RRF ordering, sanitization tiers, trust framing, redirect policy, or bounded I/O.
- No renderer redesign. `render_plaintext` groups paragraphs on blank lines, so the chunk join collapses them; that is pre-existing renderer behaviour that becomes visible on unaddressed reads of over-window documents. Changing it is a renderer milestone.

## Current implementation evidence (baseline `3259166`)

- `src/fetch/client.rs` computes `full_text` at the document budget in the non-HTML extraction arm and discards it except to bound `text`: `let (full_text, document_truncated) = bound_text(&decoded, document_chars); let (text, text_truncated) = bound_text(&full_text, window_chars);`.
- The same function sets `pre_framing_text = text.clone()` (window-bounded) and the non-HTML renderers consume `pre_framing_text`, each with `document_chars` as their budget — a correct bound over an already-truncated input. `raw_text` in the same function is already `bound_text(strip_control_chars(decoded), document_chars)`.
- Consequence, measured on a 20,063-byte `text/plain` body: an addressed read reports `text_total = 12,000`, no `next_offset`, `document.text_truncated = false`, `document.document_truncated = false`, while `offset = 13,000` clamps to an empty window. The unaddressed read of the same document reports `document.text_truncated = true`. Two reads of one document therefore contradict each other, and the addressed read claims completeness while 8,063 scalars are unreachable.
- M019 recorded the reason non-HTML blocks are not re-projected from `document.blocks` (their `text` is the raw decoded prefix, their blocks are renderer output) and used it to keep the cache class caller-scoped. That reason concerns `text` and the cache key, not the block source, and still holds after this change.

## Invariants that cannot regress

- Ten MCP tools; no argument, key, or code added or removed.
- `text_from <= text_to <= text_total`, `text_total` caller-independent per document, and `next_offset` present exactly when addressable text remains.
- `document.text_chars_returned` and `document.text_truncated` stay window-scoped; `document.document_truncated` stays the extraction-budget fact; the addressed read and the unaddressed read of one document agree.
- The legacy `text` field never exceeds the caller window, and `chunk_id` derivation is unchanged.
- Unaddressed reads still carry no addressing key.
- Ordinary files stay under 1,600 lines and 80 KB; no comments; no new dependency.

## Expected production changes

1. **`src/fetch/client.rs`** — the non-HTML document arm renders from the document-bounded text (`raw_text`) instead of the window-bounded `pre_framing_text`. `text_chars` keeps counting `pre_framing_text`, because it describes the legacy field. This is the whole production change: one branch condition and its comment.
2. **`tests/web_fetch_integration.rs`** — the six tests that asserted block text is bounded by `max_chars` are retargeted to the corrected contract through one helper: `document.text_chars_returned` stays inside the caller window, at least one block exceeds it (the block set is document-scoped), and the window cut is flagged. `web_fetch_code_block_text_never_exceeds_max_chars` is renamed `web_fetch_code_block_text_covers_the_document_budget`; the section comment is corrected.
3. **`tests/mcp_tools.rs`** — two regression tests in the `document_budget` module: `plain_text_total_does_not_vary_with_the_caller_window` (three caller windows, one `text_total`) and `plain_text_windows_reach_the_tail_and_compose` (2,000-scalar windows tile the single wide read exactly, and the unaddressed and addressed reads agree on `text_truncated`).
4. **`src/fetch/pdf.rs`** — one unit test pinning the corrected PDF fact: chunks are document-scoped while `text` is window-bounded.
5. **`tests/fixtures/fetch_windows/README.md`** — the non-HTML basis section, the recorded row for `text-body-exceeds-window-cap` (`text_total` 11,997 → 13,581), and the live two-window record (`text_total` 11,981 → 49,852).
6. **Records** — `plans/closure/agentic-retrieval-integrity/020-status.md` corrects its PDF claim, records the condition as discharged, and flips to `closed`; the roadmap, the registry, and this plan's header move in the same commit.

## Migration and compatibility effects

Non-HTML documents larger than the caller window change in three visible ways, all measured on a 20,031-byte `text/plain` body (299 paragraphs) at default budgets:

| Observation | Before | After |
|---|---|---|
| addressed `text_total` | 11,821 | 19,733 |
| addressed `next_offset` at `offset = 0` | absent | 12,000 |
| reach at `offset = 13,000` | empty window | 200-scalar window |
| unaddressed `text` | 12,082 scalars, raw decoded prefix | 11,697 scalars, chunk-join projection |
| unaddressed `document.blocks` | 180 | 299 |
| unaddressed payload | 52,295 bytes | 67,656 bytes |

The payload growth is the document-budget block set that HTML has carried since M019. The `text` change on unaddressed reads is whitespace: the chunk join is paragraph-normal, so blank lines between plain-text paragraphs collapse, matching what an addressed read of the same document already returned. Documents at or under the window are unchanged, and no addressing key appears on an unaddressed read.

## Ordered work packages

1. Change the non-HTML renderer input to the document-bounded text; rebuild and measure the before/after table above.
2. Retarget the six window-bound tests through one helper and correct the section comment and name.
3. Add the non-HTML caller-independence and tail-composition regression tests.
4. Add the PDF unit test that pins the corrected fact.
5. Re-record the corpus row and the live two-window record; correct the fixture README's basis section.
6. Correct M020's record, discharge its condition, and flip it to `closed`; add this plan, its closure record, the roadmap rows, and the registry rows in one commit.
7. Run the full gate and a hosted CI run; record both.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --no-fail-fast
cargo test --locked --all-features --test web_fetch_integration -- --ignored offset_windows_live --nocapture
python3 packaging/check-planning-consistency.py
./packaging/check-repo-hygiene.sh
```

Hosted CI (`make ci`) is the canonical gate; the local `packaging-check` leg aborts on this workstation at `mavis-trash: unsupported OS: Plan9`, so the hosted run supplies that leg.

## Acceptance and stop conditions

- A `text/plain` document larger than the caller window reports `text_total` equal to its extracted size, offers `next_offset`, and is reachable at offsets past the first window.
- `text_total` is identical for the same document under different caller windows.
- The unaddressed and addressed reads of one document agree on `text_truncated` and `document_truncated`.
- Deferred reads compose gap-free and equal the single wide read.
- No addressing key appears on an unaddressed read; `text_chars_returned` stays inside the caller window.
- The legacy `text` field never exceeds the caller window.
- Stop and report rather than proceeding if the fix requires changing `chunk_id` derivation, the legacy `text` projection, the cache-key class, or any response key; if PDF behaviour changes; or if the six retargeted tests cannot express the corrected contract without weakening another invariant.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/021-status.md` must record the implementation candidate commit, the corrections to M020's record with their measurements, the before/after table, the addressed and unaddressed transcripts for the 20 KB fixture and for the live RFC, the corpus re-record, the gate results including hosted CI, the discharge statement that moves M020 to `closed`, and the unresolved PDF findings with severity.
