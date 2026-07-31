# Phase 2 — Optional PDF Layout and OCR

**Repository:** `eggstack/eggsearch`  
**Roadmap:** `plans/web-fetch-pdf-and-browser-resilience-roadmap.md`  
**Predecessor:** `plans/web-fetch-resilience-phase-1-pdf-quality-and-navigation.md`  
**Status:** Implementation handoff  
**Scope:** Optional page rendering, basic layout reconstruction, and page-local OCR for degraded PDF pages

---

## 1. Objective

Add an optional richer PDF backend that can recover text from scanned pages and broken font mappings without changing the lightweight default build.

Phase 1 establishes page selection, page quality classification, metadata, warnings, and a backend-independent result contract. This phase adds:

1. an optional page renderer/layout backend;
2. an optional OCR backend;
3. `pdf_ocr = auto|always|never` behavior;
4. per-page extraction provenance;
5. bounded heading/paragraph reconstruction where the renderer exposes glyph coordinates;
6. bounded media metadata useful to an agent deciding whether a page needs visual inspection.

The central rule is:

> Under `auto`, render and OCR only pages that Phase 1 classifies as scanned, image-only, or severely corrupted.

Do not OCR every page merely because OCR support is compiled.

---

## 2. Dependency and Feature Boundary

### 2.1 Keep heavyweight dependencies optional

Recommended feature direction:

```toml
[features]
pdf = ["dep:lopdf"]
pdf-layout = ["pdf", "dep:pdfium-render"]
pdf-ocr = ["pdf-layout", "dep:ocrs", "dep:rten"]
```

The exact OCR dependency set should follow the selected crate's current requirements. Do not add ONNX Runtime, Python, Tesseract system packages, or a second async runtime unless the chosen Rust-native path proves impossible.

Before committing dependencies, verify:

- license compatibility;
- supported Rust version;
- supported Linux/macOS targets relevant to Eggsearch;
- whether a native PDFium library must be discovered or bundled;
- model file size and loading behavior;
- whether dependencies compile with the repository's existing `rust-version`.

If `pdfium-render` or `ocrs` creates disproportionate build/distribution complexity, stop and document the blocker rather than silently embedding a downloader or system package manager.

### 2.2 No automatic runtime downloads

Eggsearch must not automatically download:

- PDFium;
- OCR model files;
- browser binaries;
- language packs.

Support explicit operator configuration and conventional local discovery. Missing components must degrade to the Phase 1 text path with an explicit capability warning.

### 2.3 Separate compile capability from runtime availability

Capability reporting must distinguish:

```text
not_compiled
compiled_but_runtime_missing
available
```

For example, `pdf-layout` may compile while the PDFium shared library is absent. Do not report layout/OCR as usable until initialization succeeds.

---

## 3. Internal Architecture

### 3.1 Introduce narrow backend traits

Recommended shape:

```rust
pub trait PdfLayoutBackend: Send + Sync {
    fn availability(&self) -> CapabilityAvailability;

    fn inspect_page(
        &self,
        bytes: &[u8],
        page: usize,
        limits: &PdfRenderLimits,
    ) -> Result<PdfRenderedPage, FetchError>;
}

pub trait PdfOcrBackend: Send + Sync {
    fn availability(&self) -> CapabilityAvailability;

    fn recognize(
        &self,
        page: &PdfRenderedPage,
        limits: &PdfOcrLimits,
    ) -> Result<PdfOcrPage, FetchError>;
}
```

The traits may be synchronous because PDF rendering/OCR crates are usually blocking. Invoke them through one bounded `spawn_blocking` boundary rather than adding async wrappers throughout the PDF module.

Do not create a general plugin system. Two internal traits are sufficient.

### 3.2 Use one bounded blocking controller

Rendering and OCR must share explicit limits:

```rust
pub struct PdfRenderLimits {
    pub max_pages: usize,
    pub max_width_px: u32,
    pub max_height_px: u32,
    pub max_pixels_per_page: u64,
    pub max_total_pixels: u64,
    pub render_timeout_ms: u64,
}

pub struct PdfOcrLimits {
    pub max_pages: usize,
    pub max_total_pixels: u64,
    pub ocr_timeout_ms: u64,
}
```

Use a small global semaphore, preferably one or two concurrent blocking PDF jobs. Do not add a worker pool framework.

Cancellation behavior must be honest: a timed-out blocking task may not be immediately cancellable. Bound the number of in-flight tasks so timeout cannot create unbounded orphan work.

### 3.3 Preserve per-page provenance

Each page should report:

```rust
pub enum PdfPageExtractionMethod {
    Text,
    Layout,
    Ocr,
    TextAndOcr,
    Failed,
}
```

Recommended page fields:

```text
extraction_method
quality_before
quality_after
ocr_used
rendered_pixels
image_count
warnings
```

When OCR replaces corrupted text, keep a warning that the result is OCR-derived and may not preserve formulae or precise layout.

Do not expose raw raster bytes through the ordinary MCP response.

---

## 4. Layout Backend Work

### 4.1 Initialize PDFium explicitly

Implement one initialization path that:

- uses an operator-configured library path when supplied;
- otherwise attempts documented conventional system locations;
- caches successful initialization;
- caches an actionable unavailable result;
- never downloads a library;
- does not repeatedly probe the filesystem per page.

Recommended configuration:

```toml
[fetch.pdf]
layout_enabled = false
pdfium_library_path = "..." # optional
```

If the existing config structure favors flat fields, use that style rather than forcing a nested migration.

### 4.2 Render only selected pages

Rendering must honor:

- request page selection;
- configured PDF page cap;
- maximum page dimensions;
- maximum pixels per page;
- maximum total pixels;
- request deadline;
- OCR policy.

If a page exceeds limits, preserve Phase 1 text and emit a bounded warning. Do not downscale without reporting it.

### 4.3 Implement conservative layout reconstruction

The first layout implementation should focus on:

- reading order within ordinary single-column pages;
- line grouping from glyph coordinates;
- paragraph separation using vertical gaps;
- simple heading candidates based on relative font size and line length;
- list markers;
- page-local blocks;
- table-like aligned text preservation without claiming a semantic table.

Explicitly do not attempt:

- robust multi-column academic paper reconstruction;
- mathematical equation reconstruction;
- semantic table cell merging;
- chart interpretation;
- figure caption association;
- footnote/reference resolution.

If layout confidence is low, prefer OCR/plain text with a warning over inventing structure.

### 4.4 Build heading-derived outline only when useful

When the PDF has no bookmarks, a bounded synthetic outline may be built from high-confidence heading candidates.

Rules:

- maximum entry count;
- maximum title length;
- page number required;
- deduplicate repeated running headers;
- reject sentence-like or paragraph-length headings;
- preserve `outline_source = bookmarks|inferred|none` in metadata;
- do not merge inferred and bookmark outlines unless rules are explicit.

---

## 5. OCR Backend Work

### 5.1 Implement policy exactly

`never`:

- never render solely for OCR;
- preserve Phase 1 output and warnings.

`auto`:

- OCR pages classified as `CidCorrupt`, `ScannedOrImageOnly`, or `ExtractionFailed` when image rendering succeeds;
- do not OCR clean pages;
- optionally OCR `SparseText` pages only when they contain significant image coverage and the rule is documented.

`always`:

- OCR every selected page, still subject to limits;
- preserve text extraction alongside OCR when useful for comparison;
- clearly identify the OCR source.

### 5.2 Load OCR models once

Model initialization should be lazy and shared across requests.

Configuration should specify explicit local model paths or a documented data directory. Do not embed large model files in the crate or repository.

Recommended behavior:

- initialize on first OCR request;
- cache success or failure;
- return actionable missing-model diagnostics;
- do not repeatedly reload models per page;
- cap concurrent OCR work.

### 5.3 Choose replacement text conservatively

Under `auto`, compare pre- and post-OCR quality using simple signals:

- readable character ratio;
- CID token removal;
- extracted character count;
- OCR confidence if available;
- excessive repeated/noise tokens.

Rules:

- replace CID-corrupt text when OCR is materially cleaner;
- augment sparse text when OCR adds meaningful content;
- preserve original text if OCR output is empty or clearly worse;
- retain page provenance and warnings;
- do not combine text line-by-line using complex fuzzy matching in this phase.

A simple page-level choice is preferable to an elaborate merge algorithm.

### 5.4 OCR output sanitation

OCR output is external untrusted content and must pass the same control-character stripping, output bounds, prompt-injection scanning, and framing used for other fetched text.

Do not treat OCR output as more trusted because it was derived locally.

---

## 6. Media Metadata

When `include_media = true`, expose bounded page metadata such as:

```text
page
embedded_image_count
largest_image_width
largest_image_height
page_render_available
```

Do not extract all images into response payloads. Do not return base64 images. Do not add a binary artifact store.

If vector graphics cannot be counted reliably, state that the count covers raster images only.

---

## 7. Error and Degradation Behavior

The preferred degradation sequence is:

```text
layout/OCR available and succeeds
    -> enriched result
layout unavailable
    -> Phase 1 text result + capability warning
render limit exceeded
    -> Phase 1 text result + page warning
OCR fails for one page
    -> preserve original page + warning
all selected pages remain unusable
    -> content_ok=false with actionable result
```

Do not fail the whole PDF because one page cannot render or OCR unless the request explicitly requires `pdf_ocr = always` and no useful output can be produced.

Suggested structured warning codes:

```text
PdfLayoutUnavailable
PdfRenderLimitExceeded
PdfRenderFailed
PdfOcrUnavailable
PdfOcrFailed
PdfOcrOutputRejected
PdfOcrApplied
PdfOutlineInferred
```

---

## 8. Configuration and Capability Reporting

Recommended configuration surface:

```toml
[fetch]
pdf_enabled = true

[fetch.pdf]
layout_enabled = false
ocr_enabled = false
pdfium_library_path = ""
ocr_detection_model = ""
ocr_recognition_model = ""
max_render_pages = 8
max_page_pixels = 16000000
max_total_pixels = 64000000
render_timeout_ms = 15000
ocr_timeout_ms = 30000
```

Adjust placement to fit the existing config model. Keep defaults conservative and disabled.

`provider_status` should report:

```text
compiled feature state
runtime dependency state
configured state
usable state
missing component reason
```

Do not run a real PDF render probe during every status call.

---

## 9. Non-Goals

Do not implement:

- automatic dependency/model downloads;
- Tesseract subprocess integration;
- Python OCR;
- remote OCR APIs;
- document-wide fuzzy text/OCR merging;
- high-fidelity table reconstruction;
- formula recognition;
- image captioning;
- page screenshots in MCP output;
- browser rendering;
- cache persistence;
- OCR benchmarks in CI;
- platform CI matrices for PDFium;
- new release automation.

---

## 10. Focused Verification

### 10.1 Fixtures

Keep the fixture set small:

```text
clean text PDF
mixed text + scanned-page PDF
image-only scanned PDF
CID-corrupt or missing-ToUnicode PDF
PDF with headings and no bookmarks
PDF with one oversized page for limit behavior
```

Use one compact real fixture for CID corruption if generating it is not practical. Record license/source information for checked-in fixtures.

### 10.2 Required focused tests

- feature-disabled behavior remains the Phase 1 path;
- runtime-missing PDFium reports unavailable without panic;
- page selection controls rendering and OCR;
- `auto` does not OCR clean pages;
- `auto` OCRs degraded pages;
- `always` respects page/pixel limits;
- one-page OCR failure does not discard other pages;
- replacement selection rejects empty/worse OCR output;
- per-page provenance is accurate;
- inferred outline is bounded and deduplicated;
- media metadata is bounded;
- sanitation applies to OCR text.

### 10.3 Commands

Use targeted development commands:

```bash
cargo check --locked --features pdf-layout
cargo check --locked --features pdf-ocr
cargo test --locked --features pdf-ocr --test pdf_extraction
```

Then run:

```bash
make check
```

If the optional runtime library or models are unavailable in ordinary CI, compile the feature and run pure tests with a fake backend. Keep real PDFium/OCR smoke checks manual and local.

Do not add a CI service container, model download, or matrix.

---

## 11. Documentation Updates

Document:

- optional feature and runtime requirements;
- explicit installation/configuration steps without automatic downloads;
- `never|auto|always` semantics;
- page and pixel limits;
- OCR provenance and limitations;
- missing runtime degradation;
- supported platforms based on actual verified behavior;
- the absence of semantic table/formula guarantees.

Update only active fetch/PDF/config documentation and capability tables.

---

## 12. Acceptance Criteria

- [ ] Default and `pdf`-only builds do not include layout/OCR dependencies.
- [ ] `pdf-layout` and `pdf-ocr` are optional and separately report runtime availability.
- [ ] No dependency or model is automatically downloaded.
- [ ] `pdf_ocr = never|auto|always` behaves exactly as documented.
- [ ] `auto` renders/OCRs only degraded selected pages.
- [ ] Rendering and OCR enforce page, pixel, timeout, and concurrency limits.
- [ ] A failed page does not discard usable pages.
- [ ] Every page reports its extraction method and quality provenance.
- [ ] OCR replacement is used only when materially more useful than original text.
- [ ] OCR output passes the existing untrusted-content sanitation path.
- [ ] Media metadata is bounded and does not include binary image payloads.
- [ ] Missing PDFium/models degrade to Phase 1 output with actionable warnings.
- [ ] No complex table, formula, or figure interpretation is introduced.
- [ ] No new CI matrix, runtime download, or benchmark framework is added.
- [ ] `make check` passes.

---

## 13. Handoff Notes

The primary success criterion is reliable recovery of selected degraded pages, not perfect PDF reconstruction. Implement the smallest renderer/OCR adapter that respects limits and provenance. If a dependency requires packaging machinery inconsistent with Eggsearch's lightweight release model, stop at a documented optional-runtime boundary rather than expanding scope.