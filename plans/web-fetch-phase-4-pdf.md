# web_fetch Phase 4: Optional PDF Text Extraction

## Objective

Add conservative PDF text extraction to `web_fetch` behind an optional Cargo feature and explicit fetch configuration. Preserve eggsearch's tool boundary: one explicit URL, bounded download, no crawling, no JavaScript, no OCR, no embedded file extraction, and all extracted content labeled `external_untrusted`.

The target user experience is that an agent can fetch a small or medium text-based PDF and receive page-indexed text, chunks, and warnings. Scanned or image-only PDFs should return a clear warning rather than attempting OCR.

## Dependency on prior phases

Do not add PDF support before the structured document response exists. This phase assumes the document model, block/chunk machinery, and truncation metadata from prior phases. Flattened PDF text without page locality is not sufficient for agent use.

## Non-goals

Do not perform OCR. Do not render PDF pages to images. Do not execute PDF JavaScript. Do not extract embedded files. Do not process multimedia annotations. Do not attempt full layout reconstruction. Do not accept unbounded page counts or byte sizes. Do not enable PDF support by default if it materially increases dependency weight or attack surface.

## Feature and config design

Add a Cargo feature named `pdf`. The default feature set should remain lightweight. Select the smallest maintained Rust PDF text-extraction crate that can extract text without native system dependencies if possible. Avoid requiring external binaries such as `pdftotext` for the core implementation.

Add config fields under `[fetch]`: `pdf_enabled`, `pdf_max_pages`, `pdf_max_chars_per_page`, and `pdf_max_total_chars`. Reasonable defaults are disabled, 25 pages, 12000 chars per page, and 50000 total chars.

Behavior must be explicit:

- Built without the `pdf` feature: PDF responses are rejected with a clear unsupported message explaining that PDF support is not compiled in.
- Built with `pdf` but `pdf_enabled = false`: PDFs are rejected with a config-disabled message.
- Built with `pdf` and `pdf_enabled = true`: bounded text extraction is attempted.

## Detection

Recognize PDFs by `Content-Type: application/pdf`, URL path ending in `.pdf`, or body magic `%PDF-` after normal byte-cap validation has started. Do not accept arbitrary binary files as PDFs unless at least one strong signal is present.

## Extraction behavior

The PDF extraction path should return page count when available, pages extracted, rendered blocks, legacy text, warnings, and truncation flags.

Each extracted page should produce page-indexed blocks. Chunks must include page start and page end. Legacy `text` should include explicit page markers, such as `--- Page 3 ---`, unless doing so would exceed bounds. Page markers are valuable for agent citation and orientation.

If a page has no extractable text, emit a warning. If many pages are blank, aggregate warnings rather than producing noisy per-page spam. If the PDF appears scanned or image-only, return a warning such as `PDF has little or no extractable text; OCR is not supported`.

## Limits

Layer limits carefully:

- Existing `max_bytes` caps downloaded body bytes.
- `pdf_max_pages` caps pages attempted.
- `pdf_max_chars_per_page` caps each page.
- `pdf_max_total_chars` caps total extracted PDF text.
- User `max_chars` still caps returned legacy `text`.

When limits are hit, set `text_truncated` and/or `blocks_truncated` and add a warning that names the limit.

## Error handling

Add clear error paths for PDF feature not compiled, PDF disabled by config, parse failure, encrypted/password-protected PDF unsupported, no extractable text, and configured limits exceeded.

Prefer partial content with warnings when some pages extract successfully and a later page fails, provided the chosen PDF library allows safe continuation. Use structured fetch errors for complete failure.

## Security constraints

Keep byte caps before parsing. Avoid memory-amplifying operations. Do not write PDF bytes to disk. Do not shell out. Do not call external programs. Do not parse embedded attachments. Do not treat PDF metadata or text as trusted instructions. All extracted text must go through the same sanitation and trust-marker path as other fetched text.

## Tests

Tests must not require network access. Use small local fixture PDFs only if license and size are acceptable. If binary fixtures are undesirable, generate a minimal text-based PDF fixture in test code.

With the `pdf` feature enabled, test text extraction, page-indexed blocks, page-indexed chunks, page cap enforcement, total char cap enforcement, metadata-only body suppression, malformed/encrypted PDF handling when feasible, and scanned/no-text warning if a fixture exists.

Without the `pdf` feature, test that default `cargo test` passes, PDF content type returns a clear unsupported/config message, and the default build does not include the PDF dependency.

## Documentation updates

Update README to state that PDF support is optional, text-only, page-indexed, bounded, and does not include OCR. Document the feature flag and config fields. Update contributor guidance to warn future agents not to add OCR or browser-style PDF rendering to eggsearch.

## Acceptance criteria

Default build remains lightweight and passes all tests without PDF support. All-features build passes with PDF support. PDF extraction is page-indexed and bounded. Scanned/image-only PDFs do not trigger OCR attempts. No shell-outs or external binaries are required. Existing HTML/code/Markdown behavior remains intact.

Run both default and all-feature test paths: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.
