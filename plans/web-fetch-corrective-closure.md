# web_fetch Corrective Closure Plan

## Objective

Close the loose ends left after implementing the web_fetch agent-rendering roadmap. This is a corrective pass only. Do not add new render formats, new crawling behavior, new browser behavior, summarization, OCR, or model-driven analysis. The goal is to harden the current implementation so its response contracts and bounds are internally consistent for codegg agents and smaller models.

## Scope

This plan targets these issues:

1. PDF `metadata_only` currently leaks extracted body text because the PDF branch returns before the shared metadata-only path.
2. `FetchDocument.link_truncated` is hardcoded false even when top-level `links_truncated` is true.
3. HTML outline entries can point to blocks that were removed by block-boundary truncation.
4. Code/diff renderers can emit a single line longer than `max_chars`.
5. Plain-text renderer drops an over-budget first paragraph instead of returning a bounded partial paragraph.
6. Accepted application content types are not fully mirrored in the classifier.
7. PDF document metadata loses fetch-level context such as redirects and content length.
8. HTML content-root selection lacks the planned sparse-root fallback.
9. Some comments/docs still describe Markdown/PDF as reserved even though they are implemented.
10. The closure needs targeted regression tests and default/all-features validation.

## Non-goals

Do not crawl extracted links. Do not recursively fetch assets. Do not execute JavaScript. Do not add browser rendering. Do not add OCR. Do not parse PDF attachments. Do not summarize fetched content. Do not add tree-sitter or language parsers. Do not change the minimal `web_fetch` call shape. Do not remove the legacy `text` field.

## Phase A: PDF metadata-only contract fix

### Problem

The PDF path is an early return inside `FetchClient::fetch`. It checks PDF feature/config gates, reads the body, extracts PDF text, sanitizes it, and returns `text: Some(...)` plus a populated PDF document. Because this happens before the shared `ExtractMode::MetadataOnly` branch, `extract_mode = metadata_only` leaks PDF body content.

### Required behavior

When `extract_mode = MetadataOnly` and the target is PDF:

- The tool may validate that the target is a PDF.
- The tool may return URL, final URL, status, content type, fetched flag, trust label, byte truncation flag, warnings, and fetch metadata.
- It must not return page text in `text`.
- It must not return extracted page text in `document.blocks` or `document.chunks`.
- It should not call expensive PDF text extraction.
- If a cheap title is not available without parsing, title may be `None`.
- It must still include the external-untrusted warning.

### Implementation steps

1. In `FetchClient::fetch`, after PDF detection and PDF feature/config gates, branch on `extract_mode == ExtractMode::MetadataOnly` before calling `super::pdf::extract_pdf_text`.
2. Construct a metadata-only `FetchDocument` with `kind = Pdf`, `render_format = AgentBlocksV1`, `text_format = plain`, zero `text_chars_returned`, `text_truncated = false`, `block_truncated = false`, `link_truncated = false`, no outline, no blocks, no chunks, and metadata populated from fetch context.
3. Return `text: None`, `links: Vec::new()`, `links_seen: None`, `links_truncated: false`.
4. Add a warning only if relevant, plus the standard external-untrusted warning.
5. Add an integration test using a local PDF fixture or generated PDF under `--features pdf` proving metadata-only does not leak text through `text`, `document.blocks`, or `document.chunks`.
6. Add a default-feature test proving PDF metadata-only still returns the correct `pdf_not_compiled_in` or disabled error when PDF support is unavailable, if the existing gating behavior makes this observable.

## Phase B: Link truncation metadata consistency

### Problem

Top-level `WebFetchResponse.links_truncated` can be true, but the nested `FetchDocument.link_truncated` is currently set to false during normal document construction.

### Required behavior

Top-level and document-level link truncation metadata must agree when a document is present.

### Implementation steps

1. In non-PDF document construction, set `FetchDocument.link_truncated = links_truncated`.
2. If PDF documents never include links, keep `link_truncated = false` in the PDF document.
3. Add an HTML fixture with more links than the extractor cap.
4. Test that top-level `links_truncated` is true and `document.link_truncated` is true.
5. Test that `links_seen` exceeds `links.len()` when truncation occurs.

## Phase C: Outline/index integrity after block truncation

### Problem

The HTML renderer builds outline entries while walking headings, then truncates the block list by character budget. Outline entries for truncated-away heading blocks can retain stale `block_index` values.

### Required behavior

Every outline entry with a `block_index` must point to an existing block in `document.blocks`. If a block is removed by truncation, its outline entry must be removed or have `block_index` cleared only if that remains useful and tested. Prefer removal.

### Implementation steps

1. After `blocks.truncate(last_valid)` in `render_blocks`, filter `outline` to retain only entries whose `block_index` is `Some(i)` and `i < blocks.len()`.
2. If any existing fallback outline logic uses `None`, preserve those only when intentionally generated after truncation.
3. Add a test with multiple headings where a low `max_chars` truncates after the first heading.
4. Assert all outline block indexes are in bounds.
5. Assert no outline title references a heading that was removed from blocks.

## Phase D: Hard output bounds for code, diff, and long-line text

### Problem

The code and diff renderers set `end_line = start_line + 1` when a single line exceeds the remaining budget, but then push the entire line. This can violate `max_chars` for minified JSON, JavaScript bundles, large generated files, and long log lines.

The plain-text renderer breaks when a paragraph exceeds the remaining budget, which can return no blocks for a useful long paragraph.

### Required behavior

No rendered block or chunk should exceed the configured budget because of a single oversized line or paragraph. The renderer should return a bounded partial block and mark truncation.

### Implementation steps

1. In `render_code`, when `end_line == start_line` due to an oversized line, take only `char_budget` chars from that line. If `char_budget` is zero, stop without pushing an empty block. Mark `block_truncated = true` and `text_truncated = true`.
2. Preserve line range as the original line number for the partial block: `line_start = line_end = start_line + 1`.
3. Apply the same logic in `render_diff`.
4. In `render_plaintext`, when `para_chars > char_budget` and `char_budget > 0`, push a paragraph block containing the first `char_budget` chars of the paragraph, preserve line_start/line_end as the paragraph range, and mark truncation. If `char_budget == 0`, stop.
5. Consider adding a small helper for bounded single-line/paragraph truncation to avoid divergent behavior.
6. Add tests for:
   - one-line minified JSON longer than `max_chars`;
   - one-line JavaScript longer than `max_chars`;
   - one-line diff hunk longer than `max_chars`;
   - one long plain-text paragraph longer than `max_chars`.
7. Assert block text length is <= `max_chars`, chunk text is bounded, and truncation flags are set.

## Phase E: Content-type classifier parity

### Problem

The client accepts several application content types as text-like, including `application/javascript`, `application/typescript`, and `application/x-sh`, but the classifier does not classify all of them as code because `detect_from_content_type` lacks those entries.

### Required behavior

Every content type accepted as text-like by the client should either classify deterministically or intentionally fall through to URL/body heuristics with tests. For obvious code media types, classify directly as `DocumentKind::Code` with language.

### Implementation steps

1. Update `detect_from_content_type` to classify at least:
   - `application/javascript` as code;
   - `application/typescript` as code;
   - `application/x-sh` as code;
   - common aliases such as `application/x-javascript` if desired.
2. Confirm `language_from_content_type` returns matching language values for these types.
3. Add tests for these content types proving `kind = Code`, `line_preserving = true`, and expected language.
4. Audit accepted text-like content types in `FetchClient` and add classifier coverage or tests for each.

## Phase F: PDF fetch metadata propagation

### Problem

PDF document metadata currently uses `redirects_followed = 0` and `content_length = None` inside the PDF extraction module, even though the client knows the actual redirect count, body length, and content-length header.

### Required behavior

PDF `FetchDocument.metadata` should report the same fetch context quality as HTML/text documents.

### Implementation options

Preferred: keep `pdf.rs` focused on PDF extraction and let `FetchClient` patch metadata before returning.

Alternative: pass a small `PdfFetchMetadata` or `FetchRenderMetadata` input into `extract_pdf_text`.

### Implementation steps

1. After `extract_pdf_text` returns, update `pdf_result.document.metadata` with:
   - `bytes_read = Some(body.len())`;
   - `content_length = content_length_header`;
   - `redirects_followed = redirect_count`;
   - `source_extension = Some("pdf")` where appropriate.
2. Preserve PDF-specific page/block data.
3. Add a redirecting PDF test if feasible using `httpmock`, or at least a unit/integration test proving content length and bytes read are populated.
4. Ensure metadata-only PDF path from Phase A uses the same metadata helper.

## Phase G: Sparse content-root fallback for HTML

### Problem

The renderer selects the first `main`, `article`, `[role=main]`, or `body` element. If `main` exists but is empty or nearly empty, useful body content can be missed.

### Required behavior

Use `main`/`article` preferentially, but fallback to `body` when the chosen root produces too little useful content.

### Implementation steps

1. Add a helper to render a candidate root and compute useful text length/block count.
2. Try roots in priority order, but if the rendered result has no blocks or total useful text below a conservative threshold, try the next root.
3. Avoid double warnings or duplicate state while probing. The implementation can first select by estimated text length if simpler.
4. Keep this deterministic; do not add model/readability heuristics.
5. Add tests:
   - non-empty `main` is preferred over noisy body;
   - empty `main` falls back to body;
   - tiny `main` with useful body falls back;
   - normal body-only page still works.

## Phase H: Documentation and comments cleanup

### Problem

Some comments still describe Markdown and PDF as reserved even though both are now implemented.

### Required behavior

Docs and code comments should reflect current behavior without overstating capabilities.

### Implementation steps

1. Update `DocumentKind::Markdown` docs to remove reserved/future wording.
2. Update `DocumentKind::Pdf` docs to say optional feature-gated PDF document.
3. Audit README, AGENTS.md, CHANGELOG, and MCP tool descriptions for any stale statement that Markdown is rejected or PDF is future-only.
4. Make sure docs still state:
   - no crawling;
   - no JavaScript execution;
   - no OCR;
   - PDF extraction is optional and disabled unless feature/config allow it;
   - fetched content is external untrusted data.

## Phase I: Regression test closure

Add or verify tests for all corrected behavior. The minimum closure suite must include:

- PDF metadata-only does not leak body content through `text`, `document.blocks`, or `document.chunks` when built with `pdf`.
- Default build behavior for PDFs remains clear and non-panicking.
- Document link truncation mirrors top-level link truncation.
- HTML outline indexes are valid after block truncation.
- Long single-line code is bounded.
- Long single-line diff is bounded.
- Long plain-text paragraph returns a bounded partial block instead of no content.
- Accepted application code content types classify as code.
- PDF document metadata includes bytes read and redirect/content-length context when available.
- Empty/sparse `main` falls back to body.
- Markdown/PDF docs/comments are no longer stale.

## Required validation commands

Run the default feature path:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run all features:

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

If the project has CI but no status context is visible, still run these locally before marking the pass complete.

## Acceptance criteria

This corrective pass is complete only when:

- `web_fetch` still fetches exactly one explicit HTTP(S) URL.
- No code path crawls or automatically follows extracted links.
- No JavaScript/browser/OCR/summarization functionality is introduced.
- `metadata_only` suppresses body content for every supported document kind, including PDF.
- All block/chunk text respects configured bounds, including single-line edge cases.
- Outline entries never point outside the block list.
- Top-level and document-level truncation metadata agree.
- PDF metadata reports real fetch context where available.
- Documentation matches implemented behavior.
- Default and all-feature test/clippy paths pass.
