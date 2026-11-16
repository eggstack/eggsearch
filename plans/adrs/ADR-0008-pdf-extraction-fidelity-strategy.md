# ADR-0008: PDF Extraction Fidelity Strategy

Status: accepted
Date: 2026-11-11
Decision owners: project maintainers
Affected subsystems: fetch and extraction; retrieval quality measurement; optional PDF capability; documentation.
Related: `plans/000-long-term-specification.md` §5, 7, 8; `plans/001-terminology-and-domain-model.md`; `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`; `plans/adrs/ADR-0006-keyless-deterministic-retrieval-and-optional-extraction.md`; `plans/adrs/ADR-0007-pdf-artifact-availability.md`; `plans/subsystems/pdf-ingestion-fidelity-roadmap.md`.

## Context and forces

Eggsearch extracts PDF text with `lopdf`'s `extract_text`. That function walks the page content stream in stream order and emits text on `Tj`/`TJ`, newlines on `'`, `"`, `T*`, and `ET`, and spaces on `TJ` array boundaries and on kerning offsets below −100. It never reads `Td`, `TD`, or `Tm`, so it has no position model.

Measured at baseline `9d4d16fc32500196a1b88554b898cd8eddc84731` against two ordinary public documents, extracting the first 25 pages within the existing 50,000-character budget:

| Document | Eggsearch output | `pdftotext -layout` reference |
|---|---|---|
| arXiv 1706.03762, two-column, 15 pp | 141 non-empty lines, avg 274 chars, **max 4,301** | 642 lines, avg 91, max 196 |
| RFC 9110, single-column with a table, 311 pp | 928 lines, avg 52, **max 3,173** | 33 lines on page 1, max 103 |

Both documents reported `quality_score: 1.00` with every page classified `CleanText`.

Three defect shapes follow directly:

- **Reading order and line structure collapse.** Page 1 of the arXiv paper is one line: permission notice, title, eight authors, then the abstract. A reader has no paragraph or section boundaries, and any consumer that reasons about lines sees noise.
- **Tables lose label-to-value adjacency.** The RFC header table extracts as all labels (`Stream: RFC: STD: … Authors:`) followed by all values (`9110`, `97`, `2818`, …), because the two columns are separate text objects in stream order. `pdftotext -layout` keeps `RFC:             9110` on one line.
- **Ligature runs acquire spurious spaces.** Of 175 ligature codepoints in the RFC's first 25 pages, 153 arrived space-wrapped: `de ﬁ nes`, `Identi ﬁ er`. The words `defines`, `definition`, and `identifier` are absent from the extracted text entirely. A three-line normalization (collapse spaces around U+FB00–FB06, then expand to ASCII) recovers all three and removes 249 spurious tokens from 7,917 (−3.1%).

The quality classifier cannot see any of this: it measures emptiness, control-character ratio, and `(cid:NN)` density, not line shape or column structure.

Forces: a single-binary, keyless, deterministic product with a strict dependency and licence policy; `cargo deny` and MSRV gates applied to `--all-features`; seven published targets including 32-bit ARM; no native library shipping; no inference in any retrieval path; and a repository convention that an extraction contract is proven by captured fixtures and negative controls, not by absence of complaints.

## Considered alternatives

1. **Keep `extract_text` unchanged and document the limits.** Cheapest, but the measured output is not usable for two-column documents or tables, and the quality score asserts that it is. Rejected.
2. **Repair the text string with regular expressions.** Recovers ligature damage but cannot recover reading order, line boundaries, or table adjacency, because the information was discarded before the string existed. Rejected as a primary strategy; retained as a normalization step.
3. **Build a layout model in-house on `lopdf`'s public content-stream API.** Text-showing operators, `Td`/`TD`/`Tm` positions, and font sizes are all reachable through `Content`/`Operation`. Group runs into lines by y-band, order by x, detect columns, associate table cells by row, drop repeated page-edge bands, join hyphenation, and normalize ligatures. No new dependency; the layout logic becomes ours to test and ratchet. **Selected.**
4. **Adopt a position-aware extractor such as `pdf-extract`.** Pure Rust and more complete than `extract_text`, but it adds a second parser beside `lopdf`, duplicates font and CMap work already present, and its licence and advisory posture would need a policy pass. Retained as a fallback if alternative 3 misses its fidelity targets.
5. **Bind PDFium or MuPDF.** Best fidelity, but ships a native library per target, breaks the single-binary contract, adds cross-compilation burden for `armv7`, and brings licence obligations this project has not accepted. Rejected.
6. **Add OCR.** Requires rasterization, which requires one of the native renderers rejected above, or an image-only partial route that works for JPEG scans and fails on JPEG 2000 and JBIG2. Rejected for this decision; a scanned document stays an explicit outcome rather than a silent empty result.

## Decision

If accepted, this ADR selects **alternative 3**:

- `lopdf` remains the only PDF parser, with `default-features = false` unchanged.
- A layout model is built in-house over content-stream operators: line assembly from `Td`/`TD`/`Tm`, column detection, row-wise table association, repeated running-head and running-foot suppression, hyphenation join, and ligature normalization.
- Extraction stays **text-only, deterministic, and bounded**. No image extraction, no rendering, no OCR, no inference.
- A scanned or image-only page is an explicit classified outcome, not an empty-success and not an error that hides the reason.
- Every extraction-output change bumps the derived cache `renderer_version` (currently 2) so cached documents cannot be served under a new extractor.

Fidelity targets, measured on the captured corpus established by roadmap M002, before this strategy may be called successful:

1. **Line shape:** at least 95% of body lines are at most 200 characters, and no body line exceeds 400 characters, on every captured document.
2. **Word recovery:** every word in the per-document required-word annotations is present; zero ligature-damaged tokens remain.
3. **Table adjacency:** at least 90% of annotated table rows place label and value on the same line.
4. **Reading order:** the annotated opening passage of a two-column document appears in author order, as an exact ordered prefix.
5. **Bounds:** all existing page, per-page character, total character, and byte caps are unchanged, and extraction of a 25-page document stays inside the recorded baseline time envelope.

If targets 1-4 are still missed after roadmap M003 and M004, re-evaluate alternative 4 before further in-house investment, and record the comparison rather than extending the deadline silently.

## Consequences and tradeoffs

Advantages: no new dependency; the extraction contract becomes testable and measurable; tables, columns, and reading order stop being silent losses; the quality signal can start telling the truth; the fixture lane proves fidelity the way the scraper-fidelity contract proves provider behaviour.

Costs: meaningful new code inside `src/fetch/pdf.rs` and likely a new extraction submodule, which runs into the file-size ratchets and requires decomposition rather than a growing single file; per-page CPU rises because positions must be processed, so the recorded baseline envelope must be re-measured; output changes are user-visible, so `renderer_version` and the documentation both move.

Risks: layout heuristics are hard to keep deterministic and cheap; a hostile PDF can present pathological text geometry; column detection can mis-order a document that mixes spanning headings with columns; and the captured corpus can become a set of documents the heuristic happens to fit. The negative-control requirement in roadmap M002 exists for exactly that last risk.

## Compatibility, security, and migration

Additive response detail (richer blocks, honest quality kinds) with no MCP tool or argument change. Existing wire strings are preserved; new outcome classification uses new additive values and does not rename `pdf_no_extractable_text`. Cached derived documents invalidate through the renderer-version bump. Sanitization tiers, trust framing, SSRF and redirect policy, and the text-only data-handling posture are unchanged: extracted text remains untrusted external content, and no image bytes leave the process.

## Verification and status

This ADR is `accepted` (maintainer decision, 2026-11-11). Acceptance settles the extraction strategy and unblocks roadmap M003 and M004; it does not certify fidelity. The M002 measurement lane must exist before M003 begins, and the five fidelity targets remain milestone acceptance gates owned by M003 and M004, not claims this record satisfies.
