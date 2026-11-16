# M004 — Structural Reconstruction and Honest Quality

Status: ready
Source: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M004
Primary class: capability
Baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731`
References: `architecture/fetch.md` § 7; `src/fetch/pdf.rs`; `src/core/document.rs`; `src/core/warning.rs`; `docs/features.md`; `tests/pdf_fidelity.rs` (M002); `plans/adrs/ADR-0008-pdf-extraction-fidelity-strategy.md`.
Dependencies: hard M003 (line and word model); ADR-0008 accepted, which satisfies the strategy interface this plan implements.

## Objective

Turn lines into a document. A two-column page must read in author order, a table row must keep label and value adjacent, a heading must be distinguishable from body text, and the quality signal must stop reporting full marks for output the extractor knows it mangled. This milestone delivers ADR-0008 targets 3 and 4, and closes the "quality score cannot see layout collapse" defect.

## Non-goals

- No chunking, focus, or payload change (M005).
- No OCR, rasterization, image handling, or scanned-page reconstruction.
- No table-to-structured-data extraction beyond row adjacency: no cells array, no merged-cell resolution, no CSV projection.
- No change to page selection, caps, budgets, or error taxonomy.
- No new MCP tool, argument, or renamed field.

## Current implementation evidence

- Column order is content-stream order. The measured two-column case produces a single line containing the attribution notice, title, eight authors, and the abstract.
- Table rows are separated: a standards document's header table extracts as all labels (`Stream: RFC: STD: … Authors:`) followed by all values (`9110`, `97`, `2818`, …), because the columns are distinct text objects in stream order.
- Outline entries carry a page number but no `block_index`, so heading and body text are indistinguishable in `blocks`; every emitted block is a paragraph.
- `classify_page_quality` scores by emptiness, control-character ratio, CID density, and image presence only. Measured: both deliberately broken documents scored `quality_score: 1.00` with every page `CleanText`.
- `PdfPageQualityKind` currently has six kinds and no way to express "text was recovered but its structure is unreliable".

## Invariants that cannot regress

- Determinism: column assignment, row grouping, and heading levels must be total orders with written tie-breaks. No hash iteration, no unstable sort, no float equality.
- Existing bounds hold; new structure state (runs per line, lines per page, rows per page, columns per page) gets an explicit cap with a saturation test.
- Sanitization, trust markers, and the `untrusted` warning are unchanged; headings are untrusted text like any other block.
- Page metadata shape is preserved. New quality kinds are additive values, not renames.
- `content_ok` keeps its meaning: a document whose pages are all blank, failed, or image-only is not usable, and a layout-degraded document is still usable text.
- Extraction output is cached; this milestone changes output and **must** bump `renderer_version` again with an isolation test.
- Outline entry count, depth, and title bounds are unchanged.

## Expected production changes

1. **Column detection.** Per page, build an x-histogram of run start positions, find gutters wider than a threshold derived from body font size, and partition lines into columns. Order columns left to right and lines top to bottom within a column. A page with no credible gutter keeps single-column order. Record the decision per page for diagnostics.
2. **Row-wise table association.** Group runs into rows by y band. When a row contains cells separated by a gap wider than the column threshold, emit a single line preserving label-to-value adjacency with a stable separator, and classify the block as `BlockKind::Table`. Cap rows per page and cells per row.
3. **Heading inference.** Rank font sizes on the page against the body size, map distinct larger sizes to heading levels with a stable ordering, and emit `BlockKind::Heading` with a level. Where an outline entry names a page, attach `block_index` for the first heading block on that page so outline-to-block linkage becomes real.
4. **Quality classification.** Add a page kind for recovered-but-structurally-unreliable text (proposed name `LayoutCollapsed`) and let structure confidence participate in `quality_score`: collapsed line shape, undetected columns on a wide page, and unassociable table rows are all negative evidence. Emit a structured warning with a new documented warning code when a page is downgraded this way.
5. **Document-level signalling.** Document quality becomes a weighted aggregate that cannot be high when any page is layout-collapsed; `pdf_content_ok` stays about usability, not quality.
6. **Cache and docs.** Bump `renderer_version`; document the new page kind and warning code where the taxonomy lives.

## Storage, protocol, and compatibility effects

- Additive enum value and warning code. Existing values keep their wire strings; the schema gains an enumerable member, which is additive for JSON consumers.
- Blocks gain `Heading` and `Table` kinds for PDFs; consumers that switch on kind already handle these kinds for HTML.
- Outline entries may gain `block_index` where today they carry only a page.
- `quality_score` values change downward for degraded documents. This is the intended, documented behaviour change and must be called out in `docs/features.md`.
- `renderer_version` moves again; no migration beyond cache invalidation.

## Ordered work packages

1. Implement column detection with unit tests on a synthetic two-column page, a single-column page, and a spanning-heading page that must not be split.
2. Implement row grouping and table-row association with a synthetic label/value table and a negative case where a wide-but-not-tabular paragraph must stay prose.
3. Implement heading inference and outline `block_index` attachment; test level ordering and a page with no headings.
4. Add the layout-collapsed page kind, the warning code, structure confidence in `quality_score`, and the document aggregate rule; test that a deliberately flattened document can no longer score full marks.
5. Bump `renderer_version` and extend the isolation test.
6. Run the M002 corpus and record targets 3 and 4 together with the full target table from ADR-0008.
7. Update taxonomy documentation and the extraction matrix report.

## Verification

- `cargo test --locked --features pdf --test pdf_fidelity -- --nocapture`
- `cargo test --locked --all-features --lib fetch::pdf::`
- `cargo test --locked --features pdf --test web_fetch_integration --test extract_fetch_contract`
- `cargo test --locked --all-features --test schema_identity_registry --test mcp_projection`
- `cargo test --locked --all-features --test static_guards --test adversarial_corpus`
- `cargo test --locked --all-features --test docs_tool_names --test docs_config_snippets`
- `make check`; `make docs-check`

## Static guards and documentation updates

- `docs/features.md` documents the new quality kind, the warning code, and the changed meaning of the score.
- `architecture/fetch.md` § 7 owns the structural model, the confidence inputs, and the additive-taxonomy note.
- `architecture/maintenance.md` ratchet table updates if the layout module grows near a ceiling.
- `tests/schema_identity_registry.rs` and `tests/mcp_projection.rs` must be checked for pinned enumerations before landing; if either pins quality kinds, the additive value must be reflected deliberately rather than by loosening the test.
- No production comments; behaviour lives in tests and documentation.

## Acceptance and stop conditions

- ADR-0008 target 3 is met on the corpus: at least 90% of annotated table rows place label and value on the same line.
- ADR-0008 target 4 is met: the annotated two-column opening passage appears in author order as an exact ordered prefix.
- A spanning heading on a two-column page is not split across columns.
- A flattened, structure-destroyed document can no longer report a full quality score, and the downgrade carries a warning an agent can act on.
- Outline entries that name a page resolve to a block index where a heading exists on that page.
- `renderer_version` isolation is proven; caps and saturation tests pass.
- Stop if column detection requires guessing on pages with mixed spanning and columnar layout beyond the documented cap; record the limitation rather than adding heuristics without a fixture.
- Stop if meeting target 3 requires cell-level table reconstruction; that is explicitly deferred.
- Stop if the quality change would mark a well-extracted single-column page as collapsed; false downgrades are worse than a missing signal.

## Closure evidence

`plans/closure/pdf-ingestion-fidelity/004-status.md` records the candidate SHA, the full ADR-0008 target table with measured values, column and table decisions per corpus case, the quality-score before/after on the deliberately flattened case, the new kind and warning code with their documentation diff, the `renderer_version` bump, and any page shapes the heuristic declines to classify.
