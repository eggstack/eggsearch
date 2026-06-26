# web_fetch Phase 1: Document Model and Compatibility Boundary

## Objective

Introduce a structured document model for `web_fetch` while preserving the current public response contract. This phase should not attempt a full HTML renderer rewrite yet. It should create the types, compatibility behavior, metadata plumbing, and test scaffolding that later phases can build on.

The result should let existing agents keep reading `text`, while codegg and newer agents can inspect a structured `document` object with `kind`, `render_format`, `outline`, `blocks`, `chunks`, and explicit truncation metadata.

## Current baseline

The existing fetch stack is organized under `src/fetch/` with `client`, `extract`, `limits`, and `types` modules. `FetchClient::fetch` validates a URL, follows bounded validated redirects manually, checks content type, streams bytes up to the configured byte cap, extracts HTML or text, sanitizes title/description/text, and returns `WebFetchResponse`.

The current `WebFetchResponse` fields are:

- `url`
- `final_url`
- `title`
- `description`
- `content_type`
- `status`
- `fetched`
- `truncated` byte-level flag
- `trust`
- `text`
- `links`
- `warnings`
- `trust_markers`

Do not break these fields.

## Non-goals

- Do not implement PDF extraction in this phase.
- Do not implement full Markdown mode in this phase.
- Do not change network validation behavior.
- Do not change the no-JS/no-crawling behavior.
- Do not remove or rename `text`.
- Do not summarize fetched content.

## New type design

Add document-oriented types under `src/core/fetch.rs` or a new submodule such as `src/core/document.rs` if that keeps `fetch.rs` readable. Prefer small serializable structs with `serde` and `schemars` derives so MCP schema generation remains useful.

Recommended enums:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Html,
    PlainText,
    Markdown,
    Code,
    Json,
    Toml,
    Yaml,
    Diff,
    Patch,
    Pdf,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenderFormat {
    LegacyText,
    AgentBlocksV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Heading,
    Paragraph,
    ListItem,
    Code,
    Table,
    BlockQuote,
    Definition,
    HorizontalRule,
    PageBreak,
    RawText,
}
```

Recommended structs:

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct FetchDocument {
    pub kind: DocumentKind,
    pub render_format: RenderFormat,
    pub text_format: Option<String>,
    pub text_chars_returned: usize,
    pub text_truncated: bool,
    pub blocks_truncated: bool,
    pub links_truncated: bool,
    pub metadata: FetchRenderMetadata,
    pub outline: Vec<DocumentOutlineEntry>,
    pub blocks: Vec<RenderedBlock>,
    pub chunks: Vec<DocumentChunk>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct FetchRenderMetadata {
    pub bytes_read: usize,
    pub content_length: Option<usize>,
    pub charset: Option<String>,
    pub redirects_followed: usize,
    pub source_extension: Option<String>,
    pub detected_language: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DocumentOutlineEntry {
    pub level: u8,
    pub title: String,
    pub anchor: Option<String>,
    pub block_index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenderedBlock {
    pub kind: BlockKind,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DocumentChunk {
    pub id: String,
    pub text: String,
    pub heading_path: Vec<String>,
    pub block_start: usize,
    pub block_end: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_end: Option<usize>,
}
```

Exact names can vary, but keep these concepts. The response must expose a top-level optional `document: Option<FetchDocument>` field on `WebFetchResponse`.

## Implementation steps

1. Add the new types with documentation comments. The crate currently warns on missing docs, so every public item needs a doc comment.

2. Add `document: Option<FetchDocument>` to `WebFetchResponse` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.

3. Update MCP response serialization in `src/mcp/tools.rs` so the JSON payload includes `document` when present. Do not remove any existing fields.

4. In `FetchClient::fetch`, record `content_length`, `bytes_read`, and `redirects_followed`. The existing loop already has redirect count and the body vector length, so this should be low risk.

5. Add a minimal compatibility document for existing HTML/plaintext extraction:

   - For HTML: `DocumentKind::Html`, `RenderFormat::AgentBlocksV1`, one `RawText` or `Paragraph` block containing the same legacy extracted text, no outline yet unless title is enough to populate one safely.
   - For `text/plain`: `DocumentKind::PlainText`, one `RawText` block containing the same legacy text.
   - For `metadata_only`: omit `document.blocks` and body chunks, or return `document` with metadata only. Do not return body text.

6. Add a simple chunk builder that groups blocks into bounded chunks. In Phase 1 this can create one chunk from the one block. Do not over-engineer semantic chunking yet.

7. Ensure all text stored in `document.blocks` and `document.chunks` is sanitized using the same trust policy as legacy `text`. Avoid a second independent sanitation implementation. Prefer helper functions so title/description/text/document block sanitation stays consistent.

8. Add `text_truncated` separately from the existing byte-level `truncated`. This should reflect character-level bounding. If current extraction uses `.take(max_chars)`, set `text_truncated` when the source text had more than `max_chars` chars. If this cannot be known without changing extractor returns, add an internal return struct from extractors that includes `text_truncated`.

## Tests

Add unit tests and MCP-level tests covering:

- Existing `web_fetch` JSON still includes all legacy fields.
- Successful HTML fetch includes `document.kind = "html"` and `document.render_format = "agent_blocks_v1"`.
- Successful `text/plain` fetch includes `document.kind = "plain_text"`.
- `metadata_only` does not leak body text through `text`, `document.blocks`, or `document.chunks`.
- `text_truncated` is true when extracted text exceeds `max_chars`.
- Byte-level `truncated` remains distinct from `text_truncated`.
- Sanitization framing/trust markers still apply when `sanitize_output = true`.
- Default feature build remains lightweight and passes existing tests.

## Acceptance criteria

- Existing README examples remain valid.
- Current tests pass without weakening assertions.
- New response fields are additive only.
- `web_fetch` still fetches exactly one explicit URL.
- No new network behavior is introduced.
- No PDF, OCR, browser, JavaScript, or crawling functionality is introduced.

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test
```
