# M004 — Document Formats Beyond PDF

Status: closed
Date: 2026-11-24
Subsystem: web acquisition expansion
Milestone class: capability
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: ADR-0026 Decision 2 (accepted); `architecture/fetch.md` (representation selection); ADR-0007 (format artifact availability) and ADR-0008 (extraction fidelity)

References: `plans/003-planning-process.md` §2.4; `docs/features.md` § PDF
Extraction; `src/fetch/pdf.rs`; `src/fetch/detect.rs`;
`src/fetch/render/csv.rs`; `src/core/document.rs`.

## Objective

Read the container documents agents actually receive — CSV, EPUB, DOCX, XLSX,
PPTX — with the same bounded, explicit-outcome discipline PDF already has. No
OCR, no layout geometry, no model.

## Decisions taken (ADR-0026 Decision 2)

| Concern | Decision |
|---|---|
| Formats | CSV/TSV, EPUB, DOCX, XLSX, PPTX |
| Archive | `zip` major 9, exact minor pinned at implementation, `default-features = false` with `deflate-flate2`; MSRV must stay at or below 1.89 |
| Spreadsheet | `calamine 0.38` with `dates`; it depends on zip 9, so the graph keeps one archive major |
| Existing crates | `quick-xml` for OOXML, `scraper` for EPUB XHTML, `flate2` with the Rust backend |
| CSV/TSV | No dependency; the existing bounded table renderer is extended with part blocks |
| Feature | `documents = ["dep:zip", "dep:calamine"]`, non-default for the first release |
| Detection | Content type, then extension, then container introspection — `mimetype` for EPUB, `[Content_Types].xml` for OOXML; `PK` alone is insufficient |
| Outcomes | content, `document_empty`, `document_malformed`, `document_unsupported`, `document_encrypted` |
| Type shape | A separate `DocumentFormat` value; `DocumentKind` is not extended |

**Recorded fallback:** if no zip 9 minor with MSRV at or below 1.89 is
available when work package 1 starts, XLSX is deferred and `zip 2.6` is used for
EPUB, DOCX, and PPTX. A second archive major is not accepted as a workaround.
The selected pins are recorded here before implementation, as this plan
requires.

## Current implementation evidence

- PDF is text-only via `lopdf`: 16 MiB body, 64 MiB decompressed, 25 pages,
  12k chars per page, no OCR, no rendering (`docs/features.md` § PDF
  Extraction).
- `src/fetch/detect.rs` classifies by content type, URL extension, and byte
  heuristics into `DocumentKind`.
- `DocumentKind::Csv` exists (`src/core/document.rs:39`) and
  `src/fetch/render/csv.rs` already renders a bounded table preview capped at
  100 rows with a column count. M004 adds part addressing, not CSV parsing.
- `DocumentKind` has no EPUB, DOCX, XLSX, or PPTX variants, and it is matched
  exhaustively across the renderer; hence the separate `DocumentFormat`.
- No archive, Office, or EPUB crate is a current dependency. `zip 9.8.1` is
  MIT with MSRV 1.89; `calamine 0.38` is MIT with MSRV 1.88; both are
  available in the local registry.

## Contract

A supported document URL returns a document object instead of the ordinary
text body:

```text
document: {
  format: "csv" | "epub" | "docx" | "xlsx" | "pptx",
  parts: [{ index, kind, title?, text, chars, truncated }],
  parts_total: integer,
  text: string,
  truncated: boolean
}
```

- `parts` mirrors PDF page addressing: one-indexed, and `parts_total` reports the
  count before selection.
- **Contract correction taken during implementation (recorded, not silent):** the
  planned `offset`/`limit` selection over parts assumed such a selector already
  existed on the web-fetch request shape. It does not; only PDF page selection
  exists (`pdf.pages`). Adding a second request-level selector would change the
  request surface for every web item and re-pin the advertised schemas, so v1
  returns all parts and reports `parts_total`. Part selection is deferred to a
  follow-up milestone; the closure record names it as the one deferred item.
- CSV/TSV returns one part per fixed 100-row block; the block size is a
  constant so output is deterministic. `title` is `rows 1-100`.
- EPUB returns one part per spine document, in `container.xml` → OPF spine
  order; `title` is the section heading or spine item label.
- DOCX returns one part per heading-delimited section in body order; `title`
  is the heading text.
- PPTX returns one part per slide in `presentation.xml` order; `title` is the
  slide title placeholder when present.
- XLSX returns one part per sheet in workbook order; `title` is the sheet name;
  dates render as ISO 8601 and numbers without locale.
- `extract_mode = "text"` and `"markdown"` are supported; `metadata_only`
  returns document metadata and no parts.
- `text` is the bounded concatenation of the selected parts, matching the
  document budget semantics.

## Format matrix

| Input | Support |
|---|---|
| `text/csv`, `text/tab-separated-values`, `.csv`, `.tsv` | supported |
| `application/epub+zip`, `.epub` | supported |
| OOXML wordprocessing, `.docx` | supported |
| OOXML spreadsheet, `.xlsx` | supported |
| OOXML presentation, `.pptx` | supported |
| ODF, `.odt`/`.ods`/`.odp` | `document_unsupported` |
| Legacy binary `.doc`/`.xls`/`.ppt`, RTF | `document_unsupported` |
| Password-protected OOXML (CFB `EncryptedPackage`) | `document_encrypted` |
| Images, scanned PDFs | unsupported; no OCR |

## Bounds and safety

- 16 MiB input, 64 MiB decompressed, 100:1 inflation ratio, 4,096 archive
  entries, 4,096 parts, 200,000 characters before the caller budget, XML depth
  64.
- ZIP central-directory sizes are attacker-controlled. Count actual inflated
  bytes through a bounded reader; run the pre-scan before `calamine` inflates
  the archive. A bound breach is a typed error, never a partial success.
- Read entries in memory only; never extract to disk. Reject absolute or
  parent-traversal entry names defensively even though nothing is written.
- No DTDs, external entities, XInclude, or external media resolution.
- Macro-bearing OOXML (for example `vbaProject.bin`) is read for text only and
  never executed.

## Determinism

DOCX body order, PPTX `presentation.xml` slide order (not filename order),
workbook sheet order, EPUB spine order, fixed table serialisation (tab in text,
pipe tables in markdown), ISO 8601 dates, and locale-independent numbers.
Ordering never depends on ZIP directory order.

## Ordered work packages

1. **Dependency pins and feature.** `documents` feature, optional `zip` and
   `calamine`, MSRV check on the pinned versions, and a size measurement
   recorded before the feature is proposed for the default set.
2. **Detection, dispatch, and outcomes.** `DocumentFormat` on detected content,
   container introspection, and a shared `DocumentOutcome` with the four
   failure kinds plus `document_encrypted`.
3. **CSV parts and EPUB.** Part blocks over the existing CSV renderer;
   `container.xml` → OPF spine → XHTML text for EPUB.
4. **DOCX.** Paragraph and heading extraction from `word/document.xml`.
5. **XLSX and PPTX.** `calamine` sheets with dates and numbers; slide-order
   extraction from `presentation.xml` and rels.
6. **Feature documentation and packaging.** `docs/features.md`,
   `architecture/fetch.md`, `docs/tool-matrix.md` if the document shape
   changes, test inventories, and a packaging check with the feature off and on.

## Verification

Focused: a new `tests/documents.rs` gated on the `documents` feature, with a
committed fixture corpus per format: valid, empty container, malformed archive,
malformed XML, encrypted container, and a high-ratio decompression bomb. Assert
part ordering, addressing, caps, the four outcomes, and byte-identical repeated
runs. Feature-off tests assert that CSV keeps its current behavior and that
`.docx`/`.epub` inputs return the existing unsupported outcome. No test
requires the network. Broad: `make check`; `make docs-check`;
`make packaging-check`.

## Acceptance and stop conditions

- Each supported format yields deterministic parts and text from fixtures.
- Malformed, empty, encrypted, and unsupported inputs are distinguishable.
- A bomb is refused by the size or ratio guard without exhausting memory.
- Feature off leaves the default fetch behavior and PDF path unchanged.
- **Stop:** a format needs OCR, a model, an unbounded parse, or a dependency
  that breaks the 1.89 MSRV. Take the recorded fallback rather than forcing the
  dependency.

## Non-goals

- OCR, image understanding, or scanned-PDF recovery.
- Layout blocks, bounding boxes, forms, comments, or tracked changes.
- Macros, embedded objects, or external relationships.
- Writing or converting documents.
- ODF and legacy binary Office formats.
- Local arbitrary-path parsing beyond existing authorized workspace roots.

## Closure evidence

Dependency and MSRV evidence, size measurement, format matrix results, fixture
results for malformed/empty/encrypted/bomb inputs, feature-off regression,
docs diff, packaging output, and limitations.
