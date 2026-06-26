# web_fetch Phase 4: Optional PDF Text Extraction

## Objective

Add conservative PDF text extraction to `web_fetch` behind an optional Cargo feature and explicit fetch configuration. The implementation must preserve eggsearch's tool boundary: one explicit URL, bounded download, no crawling, no JavaScript, no OCR, no embedded file extraction, and all extracted content labeled `external_untrusted`.

The target user experience is that an agent can fetch a small/medium text-based PDF and receive page-indexed text, chunks, and warnings. Scanned/image-only PDFs should return a clear warning rather than attempting OCR.

## Dependency on prior phases

This phase assumes:

- Phase 1 document model exists.
- Phase 2/3 block/chunk/truncation machinery exists.
- `DocumentKind::Pdf` exists or can be added compatibly.

Do not add PDF support before the structured document response exists. Flattened PDF text without page locality is not sufficient for agent use.

## Non-goals

- Do not perform OCR.
- Do not render PDF pages to images.
- Do not execute PDF JavaScript.
- Do not extract embedded files.
- Do not process multimedia annotations.
- Do not attempt full layout reconstruction.
- Do not accept unbounded page counts or byte sizes.
- Do not enable PDF support by default if it materially increases dependency or attack surface.

## Feature and config design

Add a Cargo feature:

```toml
[features]
default = []
pdf = ["dep:<chosen-pdf-crate>"]
```

The exact dependency must be selected during implementation. Choose the smallest maintained Rust crate that can extract text without a native system dependency if possible. Avoid dependencies that require external binaries like `pdftotext` for the core implementation.

Add config fields under `[fetch]`:

```toml
pdf_enabled = false
pdf_max_pages = 25
pdf_max_chars_per_page = 12000
pdf_max_total_chars = 50000
```

Recommended Rust fields:

```rust
pub pdf_enabled: bool,
pub pdf_max_pages: usize,
pub pdf_max_chars_per_page: usize,
pub pdf_max_total_chars: usize,
```

Behavior matrix:

- Built without `pdf` feature: PDF responses are rejected as `unsupported_content_type` with a message explaining that PDF support is not compiled in.
- Built with `pdf` feature but `pdf_enabled = false`: reject PDFs with a config-disabled message.
- Built with `pdf` feature and `pdf_enabled = true`: attempt bounded text extraction.

## Content-Type and URL detection

Recognize PDFs by:

- `Content-Type: application/pdf`
- URL path ending in `.pdf`
- body magic `%PDF-` only after byte cap validation has started

Do not accept arbitrary binary files as PDFs unless at least one of these signals is present. If content-type is missing but URL extension is `.pdf`, allow parse attempt subject to byte cap.

## Extraction behavior

The extraction function should return a structured result, for example:

```rust
pub struct PdfExtractResult {
    pub page_count: Option<usize>,
    pub pages_extracted: usize,
    pub blocks: Vec<RenderedBlock>,
    pub text: String,
    pub warnings: Vec<String>,
    pub text_truncated: bool,
    pub blocks_truncated: bool,
}
```

Rendering rules:

- Each extracted page should become at least one `RenderedBlock` with `kind = PageBreak` or `RawText`/`Paragraph` and `page = Some(page_number)`.
- Chunks should include `page_start` and `page_end`.
- Legacy `text` should include explicit page markers such as `\n\n--- Page 3 ---\n\n` unless this would be too noisy. Page markers are useful to agents.
- If a page has no extractable text, emit a warning such as `page 7 had no extractable text` only when not too noisy. For many blank pages, aggregate warnings.
- If the PDF appears scanned/image-only, return a warning such as `PDF has little or no extractable text; OCR is not supported`.

## Limits

Limits must be layered:

- Existing `max_bytes` caps downloaded body bytes.
- `pdf_max_pages` caps pages attempted.
- `pdf_max_chars_per_page` caps each page.
- `pdf_max_total_chars` caps total extracted PDF text.
- User `max_chars` must still cap returned legacy `text`.

If limits are hit, set `text_truncated` and/or `blocks_truncated` and add a warning specifying which limit was hit.

## Error handling

Add explicit error paths or messages for:

- PDF feature not compiled.
- PDF disabled by config.
- PDF parse failure.
- Encrypted/password-protected PDF unsupported.
- No extractable text.
- PDF exceeds configured page/text limits.

Prefer returning a structured fetch error for complete failures and warnings for partial extraction. If the first N pages extract successfully and a later page fails, return partial content with warnings if the PDF library allows safe continuation.

## Security constraints

- Keep byte cap before parsing.
- Avoid memory-amplifying operations where possible.
- Do not write PDF bytes to disk.
- Do not shell out.
- Do not call external programs.
- Do not parse embedded attachments.
- Do not expose PDF metadata as trusted instructions.
- All extracted text must go through the same sanitation/trust-marker path as other fetched text.

## Tests

Add tests that do not require network access. Use small fixture PDFs in `tests/fixtures/` only if license/size are acceptable. If binary fixtures are undesirable, generate a minimal PDF fixture in test code or use a small checked-in text-based PDF.

Required tests with `--features pdf`:

- Text-based PDF extracts text.
- Page markers or page-indexed blocks are present.
- `page_start`/`page_end` are populated on chunks.
- `pdf_max_pages` is enforced.
- `pdf_max_total_chars` is enforced.
- `metadata_only` does not emit page text.
- Unsupported/encrypted/malformed PDF produces a clear error or warning.
- Scanned/no-text PDF fixture returns no-OCR warning if fixture is available.

Required tests without `pdf` feature:

- Default `cargo test` passes.
- Fetching PDF content type returns a clear unsupported/config message, not a panic.
- Cargo default build does not include the PDF dependency.

## Documentation updates

Update README:

- State PDF support is optional.
- Document feature flag and config fields.
- Clarify text extraction only; no OCR.
- Clarify limits and page-indexed output.

Update `Cargo.toml` feature comments if the project uses them. Update AGENTS docs to warn future agents not to add OCR or browser-style PDF rendering to eggsearch.

## Acceptance criteria

- Default build remains lightweight and passes all tests without PDF support.
- `--features pdf` build passes all tests.
- PDF extraction is page-indexed and bounded.
- Scanned/image-only PDFs do not trigger OCR attempts.
- No shell-outs or external binaries are required.
- Existing HTML/code/Markdown behavior remains intact.

Run both paths:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```
