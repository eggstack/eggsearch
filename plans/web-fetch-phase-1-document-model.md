# web_fetch Phase 1: Document Model and Compatibility Boundary

## Objective

Introduce a structured document model for `web_fetch` while preserving the current public response contract. This phase should not rewrite the HTML renderer yet. It should create additive types, compatibility behavior, metadata plumbing, and test scaffolding for the later renderer phases.

The desired result is that existing agents can keep reading the legacy `text` field, while codegg and newer agents can inspect a structured `document` object with document kind, render format, outline, blocks, chunks, and explicit truncation metadata.

## Current baseline

The current fetch stack lives under `src/fetch/` with `client`, `extract`, `limits`, and `types` modules. `FetchClient::fetch` validates a URL, follows bounded validated redirects manually, checks content type, streams bytes up to the configured byte cap, extracts HTML or text, sanitizes title/description/text, and returns `WebFetchResponse`.

The current response fields must remain: `url`, `final_url`, `title`, `description`, `content_type`, `status`, `fetched`, byte-level `truncated`, `trust`, `text`, `links`, `warnings`, and `trust_markers`.

## Non-goals

Do not implement PDF extraction. Do not implement full Markdown mode. Do not change network validation. Do not change the no-JS/no-crawling behavior. Do not remove or rename `text`. Do not summarize fetched content.

## Type additions

Add document-oriented types in `src/core/fetch.rs` or a focused new module such as `src/core/document.rs`. All public types need documentation comments because the crate warns on missing docs.

Required concepts:

- `DocumentKind`: at minimum `html`, `plain_text`, `markdown`, `code`, `json`, `toml`, `yaml`, `diff`, `patch`, `pdf`, and `unknown`.
- `RenderFormat`: at minimum `legacy_text` and `agent_blocks_v1`.
- `BlockKind`: at minimum `heading`, `paragraph`, `list_item`, `code`, `table`, `block_quote`, `definition`, `horizontal_rule`, `page_break`, and `raw_text`.
- `FetchDocument`: contains kind, render format, text format, text chars returned, text truncation, block truncation, link truncation, metadata, outline, blocks, and chunks.
- `FetchRenderMetadata`: contains bytes read, content length, charset, redirects followed, source extension, and detected language where known.
- `DocumentOutlineEntry`: contains heading level, title, optional anchor, and block index.
- `RenderedBlock`: contains block kind, text, optional level, optional anchor, optional language, optional line start/end, and optional page.
- `DocumentChunk`: contains chunk id, text, heading path, block start/end, and optional page start/end.

Exact Rust names may be adjusted, but the response must expose the same concepts.

## Response compatibility

Add `document: Option<FetchDocument>` to `WebFetchResponse` with serde defaulting and skip-when-none behavior. Update MCP response serialization in `src/mcp/tools.rs` so the JSON payload includes `document` when present. Do not remove any existing response field.

The minimal call remains valid: `{"url":"https://example.com"}`.

For `extract_mode = metadata_only`, do not leak body text through `text`, `document.blocks`, or `document.chunks`. It is acceptable to return a metadata-only `document` object if it contains no body text.

## Initial document construction

In this phase, build a minimal compatibility document from the current extraction output.

For HTML, set kind to `html`, render format to `agent_blocks_v1`, and create a simple paragraph/raw-text block from the same extracted text used for the legacy `text` field. Populate title/description as today. Outline can be empty unless a reliable title-derived entry is useful.

For `text/plain`, set kind to `plain_text`, render format to `agent_blocks_v1`, and create one raw-text block from the legacy text.

Build chunks from the blocks. In Phase 1, a single chunk is acceptable. Later phases can improve semantic chunking.

## Metadata plumbing

Record `content_length`, `bytes_read`, and `redirects_followed`. The existing client already has the content length header, final body length, and redirect count available. Add `charset` if it can be parsed cheaply from the content type.

Add `text_truncated` separately from the current byte-level `truncated`. The current `truncated` field means the streamed body hit the byte cap. `text_truncated` must mean the extracted/rendered text exceeded the character budget. If the current extractor cannot report this, change internal extractor returns to include both returned text and whether character truncation occurred.

## Sanitation

All new document text fields must pass through the same sanitation and trust-marker policy as legacy fields. Avoid creating a parallel sanitizer. Prefer helper functions used by title, description, legacy text, blocks, and chunks.

Avoid double-framing that makes block text unreadable. A reasonable Phase 1 approach is to keep framing on the top-level legacy `text` as today, while block/chunk text is control-character-stripped, bounded, and scanned. If a different approach is chosen, add tests proving output remains readable.

## Tests

Add tests for these cases:

- Existing `web_fetch` JSON still includes all legacy fields.
- HTML fetch includes `document.kind = html` and `document.render_format = agent_blocks_v1`.
- Plain-text fetch includes `document.kind = plain_text`.
- Metadata-only mode does not leak body text through any legacy or new field.
- Character truncation sets `text_truncated`.
- Byte-level truncation remains distinct from character truncation.
- Sanitization framing and trust markers still apply when `sanitize_output = true`.
- Default feature build remains lightweight.

## Acceptance criteria

Existing callers remain compatible. New fields are additive only. `web_fetch` still fetches exactly one explicit URL. No new network behavior is introduced. No PDF, OCR, browser, JavaScript, crawling, or summarization functionality is introduced.

Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and `cargo test` before closing the phase.
