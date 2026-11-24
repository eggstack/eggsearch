# M004 — Document Formats Beyond PDF

Status: proposed (format and dependency decision recorded before work package 1)
Date: 2026-11-24
Subsystem: web acquisition expansion
Milestone class: capability
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: `architecture/fetch.md` (§ PDF extraction and representation selection); ADR-0007/ADR-0008 precedent for feature-gated availability

References: `plans/003-planning-process.md` §2.4; `docs/features.md` § PDF Extraction; `src/fetch/pdf.rs`; `src/fetch/detect.rs`.

## Objective

Read the documents agents actually receive - CSV, EPUB, DOCX, XLSX, PPTX - with
the same bounded, explicit-outcome discipline PDF already has. No OCR, no
layout geometry, no model.

## Current implementation evidence

- PDF is text-only via `lopdf`: 16 MiB body, 64 MiB decompressed, 25 pages,
  12k chars per page, no OCR, no rendering, per-page quality classification
  (blank, scanned, CID-corrupt, sparse).
- Detection uses content type, extension, and magic bytes; a body that
  contradicts its extension is classified from the body.
- Feature gating precedent: `pdf` shipped non-default, then default, with
  published-artifact caveats recorded in `docs/features.md`.
- No archive, Office, EPUB, or spreadsheet crate is a current dependency.

## Format and dependency decision (work package 0)

Record in this plan before implementation:

- Supported in M004: CSV/TSV (no dependency), EPUB (`zip` + `quick-xml`),
  DOCX (`zip` + `quick-xml`), XLSX (`calamine` or equivalent), PPTX
  (`zip` + `quick-xml`).
- Explicitly unsupported: ODF, legacy `.doc`/`.xls`/`.ppt`, RTF, images, and
  OCR of scanned pages.
- The `documents` feature stays non-default for the first release that carries
  it; the default-set decision belongs to a later packaging change with size
  measurements.

## Contract

Detection extends the existing representation classifier. A supported document
URL returns a `document` object:

```text
document: {
  format: "csv" | "epub" | "docx" | "xlsx" | "pptx",
  parts: [{ index, kind, title?, text, chars, truncated }],
  parts_total: integer,
  text: string,
  truncated: boolean
}
```

- `parts` mirrors PDF page addressing: one-indexed, selectable with the existing
  `offset`/`limit` semantics applied to parts, not bytes.
- CSV returns one part per bounded row block; a table row block is a fixed
  number of rows so output is deterministic.
- Office and EPUB return one part per heading-delimited section, in document
  order; sheet names and slide numbers are `title` values.
- Text and markdown extraction are supported; `metadata_only` returns document
  metadata only.

## Failure vocabulary

Four outcomes, matching the scraper-fidelity rule:

- content: a document was extracted;
- `document_empty`: the container parsed and contained no text;
- `document_malformed`: structural break (bad zip, bad XML, truncated stream);
- `document_unsupported`: recognised but outside the supported set;
  `document_encrypted` for password-protected containers.

A parser must never return an empty success for a malformed document.

## Bounds

16 MiB input, 64 MiB decompressed, 4,096 parts, 200k characters before the
ordinary `max_chars` budget, XML depth 64, 100:1 decompression-ratio guard, and
no external entity resolution. A bound breach is a typed error, not a partial
success.

## Determinism

Table serialisation is fixed (pipe tables in markdown, tab-separated in text),
numbers and dates are rendered without locale, sheet and slide order follow the
container, and heading detection uses a fixed rule set. No OCR confidence, no
probabilistic ordering.

## Ordered work packages

1. Format and dependency decision recorded here with size and MSRV evidence.
2. Detection and dispatch with a shared `DocumentOutcome`.
3. CSV and EPUB.
4. DOCX.
5. XLSX and PPTX.
6. Feature flag, packaging notes, and documentation.

## Verification

Per-format fixture corpus including a deliberately malformed file, an empty
container, an encrypted container, and a decompression bomb; assertions on
ordering, part addressing, and caps. A conformance test per format enforcing the
four outcomes above. `make check`; `make docs-check`; packaging check unchanged
except for the new optional feature.

## Acceptance and stop conditions

- Each supported format yields deterministic parts and text from fixtures.
- Malformed, empty, encrypted, and unsupported inputs are distinguishable.
- A bomb is refused by the ratio or size guard without exhausting memory.
- **Stop:** a format requires OCR, a model, an unbounded parse, or a dependency
  that breaks the 1.89 MSRV or the packaging guards.

## Non-goals

- OCR, image understanding, or scanned-PDF recovery.
- Layout blocks, bounding boxes, forms, macros, or embedded objects.
- Writing or converting documents.
- Local arbitrary-path file parsing beyond the existing authorized workspace
  roots.

## Closure evidence

Format matrix, fixture results, bound-breach evidence, feature and packaging
evidence, docs diff, and limitations.
