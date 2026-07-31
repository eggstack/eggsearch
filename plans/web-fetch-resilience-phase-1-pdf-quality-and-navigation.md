# Phase 1 — PDF Quality, Navigation, and Request Contract

**Repository:** `eggstack/eggsearch`  
**Roadmap:** `plans/web-fetch-pdf-and-browser-resilience-roadmap.md`  
**Planning baseline:** `699e25fd3dff5980629514dee4746fba581f7905`  
**Status:** Implementation handoff  
**Scope:** Improve the existing lightweight `lopdf` PDF path without adding OCR, PDFium, Chrome, or persistent caching

---

## 1. Objective

Make the existing optional PDF extraction path meaningfully more useful and honest for coding and research agents before introducing any heavyweight dependency.

The current implementation already:

- detects PDF by content type, URL suffix, and `%PDF-` magic;
- enforces fetch byte limits before extraction;
- rejects disabled or non-compiled PDF support clearly;
- extracts bounded text per page through `lopdf`;
- returns page-indexed blocks and legacy page markers;
- reports encrypted and no-text failures;
- preserves the existing `FetchDocument` and sanitation pipeline.

This phase adds:

1. request-level PDF page selection;
2. richer metadata and document outline extraction where `lopdf` supports it cleanly;
3. per-page extraction quality classification;
4. scanned/image-only and CID/font-corruption detection;
5. honest document-level `quality_score` and `content_ok` metadata;
6. a stable result contract for Phase 2 OCR without implementing OCR now.

This phase must not add a rendering engine, native library, OCR model, browser, cache database, or new MCP tool.

---

## 2. Fixed Decisions

### 2.1 Preserve the current fast path

`lopdf` remains the only PDF dependency in this phase. Do not replace it with another crate merely to obtain more layout information.

The extraction path must remain available under the existing `pdf` feature and remain disabled by default unless the current project configuration says otherwise.

### 2.2 Extend `web_fetch`; do not add `pdf_fetch`

Add optional PDF-specific request fields to `WebFetchArgs` or a small nested options struct. Recommended public fields:

```rust
pub pages: Option<String>,
pub pdf_password: Option<String>,
pub include_media: Option<bool>,
pub pdf_ocr: Option<PdfOcrPolicy>,
```

For this phase:

- `pages` is implemented;
- `pdf_password` may be implemented if `lopdf` can authenticate using the supplied password without widening logging risk;
- `include_media` returns only bounded metadata available without rendering;
- `pdf_ocr` is parsed and validated, but values other than `never` or unavailable `auto` must return a clear capability warning until Phase 2.

An acceptable alternative is:

```rust
pub pdf: Option<PdfFetchOptions>,
```

Use whichever shape produces the clearest MCP schema. Do not create duplicate top-level and nested aliases.

### 2.3 Page specifications are strict and deterministic

Accepted page syntax:

```text
1
1,3,5
1-5
1,3,7-10
```

Rules:

- pages are one-indexed in the public API;
- whitespace around comma-separated items may be accepted;
- reversed ranges may either normalize or reject, but behavior must be documented and tested;
- malformed tokens must produce a validation error rather than being silently ignored;
- duplicate pages are deduplicated;
- output page order is ascending document order;
- the selected set is clamped only by configured limits, not silently by invalid page numbers;
- out-of-range requested pages produce a clear error or structured warning;
- selected-page count must still respect `pdf_max_pages`.

Prefer strict rejection over permissive parsing that surprises the caller.

### 2.4 Quality is advisory but honest

Do not claim semantic correctness. The quality model should answer a narrower question:

> Does the extracted Unicode text appear usable, or is the page likely blank, scanned, corrupted, or incomplete?

Recommended per-page classification:

```rust
pub enum PdfPageQualityKind {
    CleanText,
    SparseText,
    CidCorrupt,
    ScannedOrImageOnly,
    Blank,
    ExtractionFailed,
}
```

Recommended metadata:

```rust
pub struct PdfPageMetadata {
    pub page: usize,
    pub quality_kind: PdfPageQualityKind,
    pub quality_score: f32,
    pub extracted_chars: usize,
    pub cid_token_count: usize,
    pub image_count: Option<usize>,
    pub warnings: Vec<String>,
}
```

The exact field names may follow existing repository naming conventions.

### 2.5 Do not infer more structure than the source supports

This phase may extract:

- document title;
- author;
- subject;
- keywords;
- creator/producer;
- creation/modification dates;
- bookmark/outline entries when readily available;
- page labels when readily available;
- page count;
- simple image/object counts when cheaply available.

Do not implement font-size heading detection, column reconstruction, table recognition, figure extraction, or page rendering in this phase.

### 2.6 Secrets must remain out of telemetry

If `pdf_password` is implemented:

- never include it in `Debug` output;
- never include it in logs or tracing spans;
- never include it in stable IDs;
- never include it in warning text;
- never include it in cache keys planned for Phase 3;
- avoid retaining it beyond the extraction call;
- do not persist decrypted content by default in later phases.

Use a redacted wrapper or manual `Debug` implementation if deriving `Debug` would expose it.

---

## 3. Required Code Changes

### 3.1 Introduce PDF option and metadata types

Likely locations:

```text
src/core/fetch.rs
src/core/document.rs
src/fetch/pdf.rs
src/mcp/tools.rs
```

Add only the fields needed by this roadmap. Avoid creating a generic document-processing framework.

Recommended internal options:

```rust
pub struct PdfExtractOptions {
    pub selected_pages: Option<Vec<usize>>,
    pub password: Option<SecretString>,
    pub include_media: bool,
    pub ocr_policy: PdfOcrPolicy,
}
```

Recommended OCR policy:

```rust
pub enum PdfOcrPolicy {
    Never,
    Auto,
    Always,
}
```

`Auto` and `Always` must not silently behave as `Never` when OCR is unavailable. Return a structured capability warning or a validation error according to the repository's existing optional-feature conventions.

### 3.2 Add a strict page parser

Create a small pure helper such as:

```rust
fn parse_pdf_pages(spec: &str, total_pages: usize, max_pages: usize)
    -> Result<Vec<u32>, FetchError>
```

Keep parsing independent from `lopdf` so it can be unit-tested without constructing PDFs.

Required error distinctions:

```text
malformed syntax
page zero
page beyond document
selection exceeds configured page cap
empty normalized selection
```

Reuse existing `FetchError` patterns where practical. Add narrowly scoped PDF variants only when existing variants cannot express the condition clearly.

### 3.3 Extract document information metadata

Expand the current title-only helper into a metadata reader.

Normalize text conservatively:

- decode PDF strings correctly where possible;
- strip control characters through the existing sanitation layer;
- bound each metadata field;
- normalize PDF date strings only when parsing is unambiguous;
- preserve raw-ish strings rather than fabricating dates.

Map metadata into the existing document response structure. If `FetchRenderMetadata` is too transport-specific, add a small PDF-specific metadata object under the document rather than overloading unrelated fields.

### 3.4 Extract bookmarks/outlines if feasible with `lopdf`

Inspect the catalog outline tree and emit bounded `DocumentOutlineEntry` values.

Rules:

- impose a hard maximum outline entry count;
- impose maximum nesting depth;
- bound title length;
- ignore malformed individual entries rather than failing the entire PDF;
- map destinations to page numbers when resolvable;
- include unresolved entries only if they still provide useful navigation;
- never recurse without an explicit depth bound.

If robust bookmark extraction proves disproportionately complex in `lopdf`, implement only metadata and page-quality work in this phase and leave a clear warning in the completion report. Do not build a bespoke PDF object graph framework.

### 3.5 Compute per-page quality

Use a small set of transparent signals:

```text
extracted character count
printable character ratio
Unicode replacement character count
control-character ratio before sanitation
(cid:NN) token count and ratio
image/object presence where cheaply visible
extraction error state
```

Recommended behavior:

- clean text receives a high score;
- large CID-token ratios strongly reduce quality;
- image-bearing pages with almost no text become `ScannedOrImageOnly`;
- pages with neither text nor image evidence become `Blank`;
- failed extraction becomes `ExtractionFailed`;
- document quality is a character-weighted or page-weighted aggregate documented in code comments;
- `content_ok` is false when all selected pages are unusable and may remain true when a minority of pages are degraded, provided warnings enumerate them.

Avoid tuning dozens of constants. Keep thresholds in one section with comments and focused fixture coverage.

### 3.6 Preserve page-local blocks and chunks

The current PDF path creates one paragraph block per page and a single aggregate chunk. Improve this without redesigning the chunker:

- retain page numbers on every block;
- include selected-page boundaries in chunks;
- do not claim `page_start = 1` when extraction begins later;
- do not claim `page_end = total_pages` when a subset is selected;
- consider one chunk per bounded page group if the existing document chunk builder can support it cleanly;
- retain legacy `--- Page N ---` markers.

The result must be useful for an agent requesting a later page range.

### 3.7 Add structured warnings

Prefer existing warning infrastructure. Add codes only when useful to callers, such as:

```text
PdfPageSparseText
PdfPageCidCorrupt
PdfPageLikelyScanned
PdfOutlineTruncated
PdfPageSelectionApplied
PdfOcrUnavailable
```

Warnings should identify page numbers and recommended action without overwhelming the response. Aggregate repeated warnings where possible:

```text
pages 4, 7, and 9 appear image-only; OCR is unavailable in this build
```

### 3.8 Update capability reporting

`provider_status` or the existing capability response should distinguish:

```text
pdf_text: available/unavailable
pdf_layout: unavailable
pdf_ocr: unavailable
browser_rendering: unavailable
```

Do not imply later-phase capabilities are active merely because request fields exist.

---

## 4. Non-Goals

Do not implement:

- PDFium;
- OCR;
- image rendering;
- screenshots;
- font-size heading inference;
- semantic tables;
- formula extraction;
- image extraction;
- browser rendering;
- response caching;
- retry or backoff redesign;
- new CI workflows;
- live internet PDF tests;
- a PDF benchmark suite;
- a second PDF-specific MCP tool.

---

## 5. Focused Verification

### 5.1 Deterministic fixtures

Add or reuse a small fixture set:

```text
one-page text PDF
multi-page text PDF
PDF with metadata
PDF with selected pages
blank page PDF
image-only or no-text PDF fixture
CID-like extracted text fixture if a compact real PDF is available
password-protected PDF only if password support is implemented
```

Do not add a large corpus. Generated `lopdf` fixtures are acceptable for simple structure. A small checked-in binary fixture is acceptable for behavior that cannot be generated reliably.

### 5.2 Focused tests

Required test categories:

- strict page parser;
- page-cap enforcement;
- selected-page block and chunk boundaries;
- metadata extraction and bounds;
- quality classification helper;
- scanned/blank distinction when image evidence exists;
- CID warning aggregation;
- unavailable OCR policy behavior;
- password redaction if password support is added;
- legacy response compatibility.

Avoid tests that only assert enum serialization when the MCP schema corpus already covers serialization.

### 5.3 Commands

Run the existing routine gate:

```bash
make check
```

During development, use a targeted PDF test command such as:

```bash
cargo test --locked --features pdf --test pdf_extraction
```

If PDF tests currently live elsewhere, use the narrowest existing target rather than creating a duplicate test binary solely for command aesthetics.

Do not add external PDF downloads or OCR dependencies to CI.

---

## 6. Documentation Updates

Update the minimum active documents necessary:

```text
README.md
AGENTS.md only if contributor guidance changes
docs/config.md
docs/architecture/fetch.md or the closest existing fetch architecture document
docs/tool-matrix.md if capabilities are enumerated there
```

Document:

- page range syntax;
- quality metadata semantics;
- the difference between blank, scanned, and corrupted text;
- the absence of OCR in this phase;
- password handling if supported;
- default limits;
- the fact that PDF content remains external untrusted input.

Do not rewrite unrelated search-provider documentation.

---

## 7. Acceptance Criteria

- [ ] Existing `web_fetch` requests remain valid without new fields.
- [ ] `pages` accepts documented one-indexed syntax and rejects malformed input.
- [ ] Selected pages respect `pdf_max_pages` and output only requested pages.
- [ ] PDF blocks and chunks report accurate selected page boundaries.
- [ ] PDF metadata includes more than title when present and remains bounded.
- [ ] Outline/bookmark extraction is bounded and best-effort, or is explicitly deferred with no partial unsafe implementation.
- [ ] Each extracted page has a quality classification and score.
- [ ] CID-like corruption and likely scanned pages produce honest warnings.
- [ ] Document-level `content_ok` does not report clean content when all selected pages are unusable.
- [ ] OCR request behavior is explicit when OCR capability is unavailable.
- [ ] Passwords, if supported, are never logged, serialized, or included in stable identifiers.
- [ ] Default builds remain unchanged when the `pdf` feature is absent.
- [ ] No heavyweight dependency is added.
- [ ] No new CI workflow or duplicated full-suite verification command is added.
- [ ] `make check` passes.

---

## 8. Handoff Notes

Implement the smallest complete version of each section. The most important outputs are strict page selection, accurate page boundaries, and honest quality signals. Bookmark extraction and media counts are secondary and may remain best-effort if `lopdf` makes them disproportionately complex.

Do not begin Phase 2 OCR work inside this phase. Leave a clean internal `PdfPageMetadata`/quality contract that Phase 2 can consume.