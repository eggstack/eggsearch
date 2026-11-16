# M003 — Text-Stream Fidelity

Status: ready
Source: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M003
Primary class: capability
Baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731`
References: `architecture/fetch.md` § 7; `src/fetch/pdf.rs`; `src/fetch/detect.rs`; `src/fetch/cache.rs`; `src/core/document.rs`; `tests/pdf_fidelity.rs` (M002); `plans/adrs/ADR-0008-pdf-extraction-fidelity-strategy.md`.
Dependencies: hard M002 (corpus and fidelity lane); ADR-0008 accepted, which satisfies the strategy interface this plan implements. Handoff awaits M002 closure.

## Objective

Recover the text stream. A PDF page must produce lines in reading order with word boundaries and intact words, rather than one content-stream-ordered string with ligature damage. This milestone delivers lines and words: position-aware run extraction, line assembly, gap-derived spaces, ligature normalization, hyphenation join, and running-head/foot suppression. Column ordering and table association are M004.

## Non-goals

- No column detection, no table row association, no heading inference, no block-kind classification beyond paragraph lines. Those are M004.
- No chunking or focus change (M005), and no payload or budget change (M005 owns payload alignment).
- No OCR, no rasterization, no image handling.
- No new dependency. `lopdf` stays the only parser.
- No change to page selection, caps, or error taxonomy.

## Current implementation evidence

- `lopdf`'s `extract_text` reads only `Tj`, `TJ`, `'`, `"`, `T*`, `ET`, and `Tf`. It never reads `Td`, `TD`, or `Tm`, so it has no coordinates. Spaces come from `TJ` array boundaries and from numeric offsets below −100.
- Measured at baseline on ordinary public documents of the shapes M002 will capture as cases: a 15-page two-column paper yields 141 non-empty lines with a maximum of 4,301 characters; a 311-page standards document yields 928 lines with a maximum of 3,173 characters, and 153 of its 175 ligature codepoints are space-wrapped so that `defines`, `definition`, and `identifier` never appear.
- Both documents report `quality_score: 1.00` and every page `CleanText`, because classification measures emptiness, control ratio, and CID density only.
- `src/fetch/pdf.rs` is at 2,089 lines against a 2,100-line / 70,000-byte ratchet ceiling, with the recorded next slice being page-label and outline extraction into their own modules. New layout code cannot be added in place.

## Invariants that cannot regress

- Determinism: identical bytes produce identical text, block order, page metadata, and warnings across runs and platforms. Tolerance-based grouping must use quantized keys and stable tie-breaks, never hash or insertion-order artifacts of a floating-point comparison.
- Every existing cap stays: pages, per-page characters, total characters, outline entries and depth, page-label expansion, byte cap, timeout.
- Sanitization runs after extraction exactly as today: `strip_control_chars`, `bound_text`, `sanitize_field`, trust markers, `untrusted` warning.
- Page metadata shape is unchanged; `extracted_chars`, `cid_token_count`, and quality kinds keep their meanings, though values may change because the text changed.
- A scanned or image-only page still classifies as such and never as clean text.
- Extraction output is cached. This milestone changes output and **must** bump the derived cache `renderer_version` from 2, with a test that a pre-bump entry is not served.
- No page may produce an unbounded intermediate buffer: line assembly state is capped per page and flushed deterministically.

## Expected production changes

1. **Module split first.** Decompose `src/fetch/pdf.rs` along the recorded next slice before adding code: page labels, outline extraction, and metadata into their own modules under a `src/fetch/pdf/` tree, leaving the extractor as the module root. Update the ratchet entry in `tests/static_guards.rs` to the new measured maximum and lower the ceiling so the split cannot silently regrow.
2. **Run extraction.** Interpret page content operations into text runs carrying text, x, y, effective font size, and stream index. Handle `BT`/`ET`, `Tf`, `Tm`, `Td`, `TD`, `T*`, `TL`, `Tj`, `TJ`, `'`, `"`. Derive spaces from `TJ` numeric gaps proportional to font size rather than the fixed `−100` heuristic. Apply the page's `/Rotate` value so a rotated page is not read sideways.
3. **Line assembly.** Group runs into lines by quantized y with a tolerance derived from font size, order by x within a line, emit one line per group, and terminate on large vertical gaps. Preserve the existing per-page character cap by truncating at line boundaries where possible.
4. **Text normalization.** Collapse spaces adjacent to ligature codepoints U+FB00–U+FB06 and expand them to ASCII equivalents; join words split by an end-of-line hyphen when the following line begins with a lowercase letter; collapse repeated whitespace; never alter non-space characters outside these rules.
5. **Running head and foot suppression.** Detect text bands repeated at the same vertical position with near-identical text across pages, drop them from body blocks, and report the count as a warning. Do not drop unique first-page content such as a title or abstract that shares the band position.
6. **Block emission.** Emit one paragraph block per assembled paragraph group with its page number. Keep block kinds, anchors, and outline shape unchanged from today's contract.
7. **Cache and metadata.** Bump `renderer_version`, add the isolation test, and keep `quality_score` computation unchanged in this milestone.
8. **Measurement.** Record corpus results before and after against ADR-0008 targets 1, 2, and 5 (line shape, word recovery, bounds and recorded time envelope). Target 3 and 4 remain open for M004 and must be reported as open, not as passing.

## Storage, protocol, and compatibility effects

- Extracted text and block contents change for every PDF: this is the point of the milestone. Consumers see the same fields with better content.
- `renderer_version` moves; derived entries invalidate cleanly and re-extract on next use.
- No response field is added or removed. Page metadata values change only where the underlying text changed.
- No config key, tool argument, or wire string changes.

## Ordered work packages

1. Split the module and update the ratchet; prove `make check` green before any behaviour change.
2. Implement run extraction with the interpreter and a unit test per operator family, including rotation and a `TJ` gap case.
3. Implement line assembly with quantization and tie-break tests, including a deliberately adversarial y-jitter case.
4. Implement normalization with unit tests for ligature collapse, ligature expansion, hyphen join, and a no-op case for prose containing ordinary hyphens.
5. Implement running-head suppression with a multi-page synthetic fixture and a negative case where a unique first-page band must survive.
6. Bump `renderer_version` and add the cache-isolation test.
7. Run the M002 corpus, record before/after, and write the measurement into the closure record and `docs/retrieval-quality-extraction-matrix.md`.

## Verification

- `cargo test --locked --features pdf --test pdf_fidelity -- --nocapture`
- `cargo test --locked --all-features --lib fetch::pdf::`
- `cargo test --locked --features pdf --test web_fetch_integration --test extract_fetch_contract`
- `cargo test --locked --all-features --test static_guards`
- `cargo test --locked --all-features --test fetch_safety --test adversarial_corpus`
- `cargo test --locked --all-features --test cache_behavior` or the owning fetch-cache suite, per `architecture/testing.md`
- `make check`; `make docs-check`

## Static guards and documentation updates

- `tests/static_guards.rs` ratchet entry updated in the same change as the split; the maintenance table in `architecture/maintenance.md` records the new ceiling and the next slice.
- `architecture/fetch.md` § 7 describes position-aware extraction, normalization, and running-head handling instead of content-stream order.
- `docs/features.md` stops listing "layout does not map to linear text flow" as an unqualified limitation and states measured behaviour.
- `docs/test-inventory.md` and `architecture/testing.md` counts update if the suite set changes.
- Do not add explanatory comments to production code; behaviour belongs in tests and documentation.

## Acceptance and stop conditions

- ADR-0008 target 1 is met on every corpus case: at least 95% of body lines are at most 200 characters and no body line exceeds 400.
- ADR-0008 target 2 is met: every `required_words` entry is present and no ligature-damaged token remains.
- ADR-0008 target 5 is met: caps unchanged, and the recorded extraction time for a 25-page document stays inside the measured envelope from M002.
- Running heads are removed on multi-page cases and unique first-page content survives.
- `renderer_version` isolation is proven by test.
- Reachability and bound checks still pass on the adversarial corpus and fuzz target.
- Stop if line assembly cannot be made deterministic under quantized grouping; report the non-determinism rather than shipping an order-dependent result.
- Stop if the module split exceeds the ratchet in either dimension; split further instead of raising the ceiling.
- Stop if meeting target 1 requires column detection; that is M004 and must not be pulled forward.

## Closure evidence

`plans/closure/pdf-ingestion-fidelity/003-status.md` records the candidate SHA, the module split with new ratchet ceilings, the per-case before/after line-shape and word-recovery table, the running-head counts, the `renderer_version` bump with its isolation test, caps and time-envelope evidence, and an explicit statement of which ADR-0008 targets remain open for M004.
