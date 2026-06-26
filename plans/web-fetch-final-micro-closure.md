# web_fetch Final Micro-Closure Plan

## Objective

Close the last two correctness gaps found after the corrective pass. This is a very small closure pass, not another feature phase. The only implementation work should be link-truncation metadata parity, HTML outline pruning after block truncation, and focused regression tests.

## Current status

The corrective pass resolved the major issues: PDF metadata-only body suppression, PDF metadata propagation, classifier parity for application JavaScript/TypeScript/shell types, hard bounds for long code/diff/plain-text lines, sparse HTML content-root fallback, and stale Markdown/PDF documentation.

Two gaps remain:

1. `FetchDocument.link_truncated` is still hardcoded to `false` in the normal non-PDF document path even when top-level `WebFetchResponse.links_truncated` is true.
2. `render_blocks` truncates `blocks` after budget exhaustion but returns the original `outline` without removing entries whose `block_index` points beyond the truncated block list.

## Non-goals

Do not add new document kinds. Do not change the MCP request shape. Do not alter PDF extraction. Do not alter search behavior. Do not crawl links. Do not execute JavaScript. Do not summarize content. Do not add OCR or browser rendering. Do not refactor the whole renderer unless a smaller direct fix is impossible.

## Item 1: Document link-truncation parity

### Problem

The top-level response correctly exposes `links_truncated`, but the nested `FetchDocument` currently sets `link_truncated: false` during normal document construction. Agents reading only `document` can therefore receive stale metadata.

### Required behavior

When a non-PDF fetch returns a structured document, document-level `link_truncated` must mirror top-level `links_truncated`.

Expected invariants:

- If `WebFetchResponse.links_truncated == true`, then `response.document.link_truncated == true` when `document` is present.
- If `WebFetchResponse.links_truncated == false`, then `response.document.link_truncated == false` unless a future renderer has a separate document-link cap. No such separate cap exists today.
- PDF documents can keep `link_truncated = false` because PDF link extraction is not implemented.
- Metadata-only responses without a document do not need document-level link metadata.

### Implementation steps

1. In `src/fetch/client.rs`, find the non-PDF `FetchDocument` construction path.
2. Replace `link_truncated: false` with `link_truncated: links_truncated`.
3. Do not rename the field during this pass. The current serialized field is `link_truncated`; changing to `links_truncated` would be a response-shape change and should be avoided unless handled separately.
4. Verify no other path constructs a normal HTML/text/code document with stale `link_truncated` metadata.

### Tests

Add a focused MCP/integration test using a local `httpmock` HTML page with more links than the extractor cap.

The test should:

- enable localhost/private-network fetch for the mock server;
- call `web_fetch` with `include_links = Some(true)`;
- assert top-level `links_truncated == true`;
- assert `links_seen > links.len()`;
- assert `document.link_truncated == true`;
- assert the document still exists and has expected `kind = html`.

Also add a small non-truncated control assertion if low cost:

- for a page with only a few links, assert top-level `links_truncated == false` and `document.link_truncated == false`.

## Item 2: HTML outline pruning after block truncation

### Problem

The HTML renderer populates `outline` while walking headings. It then truncates `blocks` according to the character budget. If a heading block is removed by truncation, its outline entry can still point to the removed block index.

### Required behavior

Every outline entry with `block_index = Some(i)` must satisfy `i < blocks.len()` in the returned `RenderedBlocks` and final `FetchDocument`.

Preferred behavior:

- After block truncation, remove outline entries whose `block_index` points outside the retained block list.
- Preserve outline entries with valid indexes.
- Preserve title-derived fallback outline entries created later in `FetchClient` only when they intentionally use `None` or a valid block index.

Do not clear all outline entries blindly. A truncated document should still retain the outline for headings whose blocks remain.

### Implementation steps

1. In `src/fetch/render/blocks.rs`, after `blocks.truncate(last_valid)`, prune `outline`.
2. Use logic equivalent to: retain entries where `entry.block_index.map(|i| i < blocks.len()).unwrap_or(true)`.
3. If the renderer should never emit `None` block indexes, prefer retaining only `Some(i) if i < blocks.len()`. Check existing code first. The current heading walker uses `Some(block_index)`; title fallback happens later in `FetchClient`.
4. Keep the pruning close to truncation so future renderer changes cannot forget it.
5. Consider adding a tiny helper such as `prune_outline_to_blocks(&mut outline, blocks.len())` if that improves readability.

### Tests

Add unit-level tests in `src/fetch/render/blocks.rs` or MCP-level tests in `tests/integration.rs`. Unit tests are preferred for exact block/outline behavior.

Test A: valid retained outline.

- HTML contains `h1`, paragraph, `h2`, paragraph.
- `max_chars` is large enough to keep all blocks.
- Assert outline has both headings and all `block_index` values are in range.

Test B: truncated outline pruning.

- HTML contains a retained first heading plus enough text/headings after it to force block truncation.
- Use a low `max_chars` that keeps the first heading but removes later heading blocks.
- Assert every outline `block_index` is less than `blocks.len()`.
- Assert the later removed heading title is not present in outline.
- Assert `block_truncated` or `text_truncated` is true.

Test C: MCP response invariant.

- Fetch an HTML page through `web_fetch` with low `max_chars`.
- Inspect `document.outline` and `document.blocks`.
- Assert all outline indexes are in bounds.

The unit tests should catch renderer behavior directly; the MCP test catches serialization and final response shape.

## Validation commands

Run default feature checks:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run all-feature checks:

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

If CI status is not visible on GitHub, record local command results in the commit message or follow-up note.

## Acceptance criteria

The final micro-closure pass is complete only when:

- `FetchDocument.link_truncated` mirrors top-level `links_truncated` for normal HTML/text/code/Markdown fetches.
- PDF documents remain unchanged unless explicitly tested; they may keep `link_truncated = false`.
- HTML renderer output never contains an outline `block_index` outside the returned block list.
- Tests cover both fixed invariants.
- No new crawler, browser, JavaScript, OCR, summarization, or model-analysis behavior is introduced.
- Default and all-feature test/clippy paths pass.

## Files expected to change

Likely implementation files:

- `src/fetch/client.rs`
- `src/fetch/render/blocks.rs`

Likely test files:

- `src/fetch/render/blocks.rs` tests, and/or
- `tests/integration.rs`

Docs should not need broad updates. A CHANGELOG entry is optional; if added, keep it to one concise bullet under the current unreleased section.
