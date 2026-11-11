# M005 — PDF Chunking, Focus, and Payload Economy

Status: proposed
Source: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M005
Primary class: capability (corrective)
Baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731`
References: `src/fetch/pdf.rs`; `src/fetch/client.rs`; `src/core/document.rs`; `src/core/focus.rs`; `src/core/fetch_policy.rs`; `architecture/fetch.md` § 5–7; `tests/extract_fetch_contract.rs`; `tests/fixtures/pdf/` (M002).
Dependencies: hard M002 for the corpus and focus annotations. Independent of M003 and M004: it operates on blocks and outline entries, so better lines and structure improve it without changing its contract.

## Objective

Make a PDF a first-class document in the shared model. Query focus must select the relevant part of a PDF instead of its prefix, and one caller-declared character budget must bound the whole response rather than one field of it.

## Non-goals

- No extraction, layout, or quality change (M003, M004).
- No removal of the `blocks` or `chunks` fields and no schema removal: HTML documents already carry both, and the fix is to bound them, not to delete them.
- No change to the focus scoring function, its caps, or its arguments.
- No new MCP argument; `focus_query` and its caps already exist.
- No change to the page-selection syntax or to `pdf_max_total_chars` as a parse ceiling.

## Current implementation evidence

- The PDF branch hand-builds a single chunk whose text is every block joined, spanning all extracted pages. HTML and text documents call the shared outline-aware builder instead.
- `select_focus_chunks` ranks the document's existing chunks; with one candidate it returns that chunk, and because the chunk exceeds `max_chars` the selection keeps its first `max_chars` characters. Measured on a 15-page paper: one chunk selected, 12,000 characters, and two disjoint queries (`label smoothing residual dropout` versus `Adam optimizer learning rate warmup steps`) produced **byte-identical** output. Terms present later in the document were absent from the focused text, and document-late terms were reported as absent.
- The legacy `text` field is bounded by the caller's `max_chars`; `blocks` are bounded by `pdf_max_total_chars`; `chunks` rejoin the blocks. One response can therefore carry the same characters three times, and the caller's declared budget bounds only one field.
- The PDF outline contains real entries plus synthetic `Page N` entries. The shared chunk builder derives heading paths from outline entries when no heading blocks exist, so passing the unfiltered outline would give every chunk a `Page N` heading path and change chunk IDs for the wrong reason.
- PDF documents never compute a `doc_id`, so chunk identity has no parent document to derive from.

## Invariants that cannot regress

- Chunk IDs stay content-derived FNV-1a through the shared `chunk_id` helper; no random or timestamp input.
- Determinism: identical bytes and arguments produce identical chunk count, boundaries, IDs, heading paths, and page ranges.
- The focus contract is unchanged: selection is lexical, deterministic, bounded by `max_chunks` and `max_chars`, ordered by original document order, and empty for an empty query or a zero cap.
- Existing response fields keep their documented meanings. `text_chars_returned` continues to describe the legacy `text` field only.
- Caps: page count, per-page characters, total characters, and the new effective document budget are all still enforced, and every truncation sets its flag.
- Sanitization and trust framing are unchanged; chunk text is untrusted content like any other.
- Cache: derived entries store the document, so the chunk-shape change **must** bump `renderer_version` with an isolation test.

## Expected production changes

1. **Document identity.** Compute `doc_id` for PDFs from the same inputs the HTML path uses (final URL, extracted title or PDF metadata title, document kind), so chunk IDs derive from a stable parent.
2. **Shared chunk construction.** Replace the hand-built chunk with `build_document_chunks`, passing a filtered outline that excludes synthetic page-entry titles and keeps real outline entries. Keep the response `outline` unchanged, including synthetic page entries, so navigation is not lost.
3. **One effective document budget.** Bound blocks, chunks, and legacy text by `min(pdf_max_total_chars, max_chars)`; keep `pdf_max_total_chars` as the parse ceiling. Set `text_truncated` and `block_truncated` from the actual outcome and never report a truncation that did not happen.
4. **Focus annotations in the corpus.** Extend the M002 manifest with `focus_query` and `focus_must_contain` so the lane asserts that focus returns relevant text, not a prefix.
5. **Behavioral focus tests.** In `extract_fetch_contract`, assert that two disjoint queries over a multi-page PDF return different selections, that each selection contains its own query terms, and that a query matching only document-late text yields that text.
6. **Cache bump and docs.** Bump `renderer_version`, add the isolation test, and document the chunk contract for PDFs in `architecture/fetch.md`.

## Storage, protocol, and compatibility effects

- PDF `document.chunks` changes from one entry to many. Consumers that assumed one chunk for PDFs must be corrected; this is a documented contract change, not an additive one, and the roadmap records it as a corrective.
- No field added or removed. Block and chunk content becomes bounded by the caller's budget, so some responses become smaller.
- `renderer_version` moves; existing derived entries invalidate and re-extract.
- Heading paths become real outline paths instead of empty or synthetic ones, which changes chunk IDs for the same document. That is intended and is why the bump is mandatory.

## Ordered work packages

1. Add `doc_id` derivation for PDFs and a test asserting stability for the same URL, title, and kind across runs.
2. Add the outline filter and switch to `build_document_chunks`; test that synthetic page entries do not appear in heading paths while real entries do.
3. Apply the effective document budget across blocks and chunks; add truncation-flag tests at the boundary (exactly at the cap, one character over).
4. Extend the corpus manifest with focus annotations and wire them into `tests/pdf_fidelity.rs`.
5. Add the disjoint-query and document-late-term assertions to `extract_fetch_contract`.
6. Bump `renderer_version` and extend cache isolation.
7. Update `architecture/fetch.md` § 5–7, `docs/features.md`, and the test inventory counts.

## Verification

- `cargo test --locked --features pdf --test extract_fetch_contract`
- `cargo test --locked --features pdf --test pdf_fidelity -- --nocapture`
- `cargo test --locked --features pdf --test web_fetch_integration`
- `cargo test --locked --all-features --lib core::focus:: --lib core::document::`
- `cargo test --locked --all-features --test static_guards --test schema_identity_registry`
- `cargo test --locked --all-features --test evidence_contract --test batch_fetch_retrieval`
- `make check`; `make docs-check`

## Static guards and documentation updates

- `architecture/fetch.md` owns the chunk contract and the budget rule; `docs/features.md` states that a PDF response is chunked like any other document and bounded by the caller's `max_chars`.
- `docs/test-inventory.md` counts update with the added assertions; `architecture/testing.md` contract text for `extract_fetch_contract` gains the disjoint-query case.
- `tests/schema_identity_registry.rs` must be checked for pinned chunk expectations before landing.
- No production comments; the contract lives in tests and documentation.

## Acceptance and stop conditions

- Two disjoint focus queries on a multi-page PDF return different selections, each containing its own query terms; a query matching only document-late text returns that text.
- The corpus focus annotations pass on every case that declares them.
- A response with `max_chars` below `pdf_max_total_chars` contains no more than that many characters of block and chunk text, and reports truncation flags that match the outcome.
- Chunk count is greater than one for any captured multi-page case, and chunk IDs are stable across two runs.
- `renderer_version` isolation is proven; no cached pre-change document is served.
- Stop if the shared builder cannot express PDF page ranges without changing its public behaviour for HTML; report the limitation and propose a builder-level change rather than forking the document model.
- Stop if the focus assertions can only pass by tuning the scoring function; that would be a separate, measured milestone.
- Stop if bounding blocks by `max_chars` breaks the maximum-quality case in the corpus; report the conflict between the two budgets instead of silently re-raising the cap.

## Closure evidence

`plans/closure/pdf-ingestion-fidelity/005-status.md` records the candidate SHA, the chunk count and heading paths per corpus case, the disjoint-query focus outputs before and after, the payload sizes before and after for the same URL and `max_chars`, the truncation-flag matrix at the boundary, the `renderer_version` bump, and the corrective statement explaining why the original single-chunk construction and its verification missed this.
