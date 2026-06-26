//! Structured document model for `web_fetch` responses.
//!
//! This module introduces a structured representation of fetched
//! content alongside the legacy `text` field. Existing agents can
//! keep reading `text`, while newer agents can inspect the
//! `document` object for document kind, render format, outline,
//! blocks, chunks, and truncation metadata.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The kind of document that was fetched.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    /// HTML document.
    Html,
    /// Plain text document.
    PlainText,
    /// Markdown document (rendered from Markdown source files via pulldown-cmark).
    Markdown,
    /// Source code file.
    Code,
    /// JSON document.
    Json,
    /// TOML document.
    Toml,
    /// YAML document.
    Yaml,
    /// Diff or patch file.
    Diff,
    /// Patch file.
    Patch,
    /// PDF document (optional feature-gated extraction via the `pdf` Cargo feature).
    Pdf,
    /// Unrecognized content type.
    #[default]
    Unknown,
}

/// The render format used to represent the document content.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenderFormat {
    /// Legacy flat text string (the `text` field).
    #[default]
    LegacyText,
    /// Structured block-based rendering for agent consumption.
    AgentBlocksV1,
}

/// The kind of a rendered block within a document.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    /// A heading block (`h1`-`h6`).
    Heading,
    /// A paragraph block.
    Paragraph,
    /// A list item block.
    ListItem,
    /// A code block (inline or fenced).
    Code,
    /// A table block.
    Table,
    /// A block quote.
    BlockQuote,
    /// A definition list entry.
    Definition,
    /// A horizontal rule / thematic break.
    HorizontalRule,
    /// A page break marker.
    PageBreak,
    /// Raw unstructured text (fallback).
    #[default]
    RawText,
}

/// Render-level metadata about how the document was fetched and extracted.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct FetchRenderMetadata {
    /// Number of bytes read from the response body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_read: Option<usize>,
    /// Content-Length header value, if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_length: Option<usize>,
    /// Character set parsed from the Content-Type header, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    /// Number of redirects followed to reach the final URL.
    pub redirects_followed: usize,
    /// File extension inferred from the URL path, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_extension: Option<String>,
    /// Detected programming language or content language, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_language: Option<String>,
}

/// An entry in the document outline (table of contents).
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DocumentOutlineEntry {
    /// Heading level (1-6 for HTML headings).
    pub level: usize,
    /// Heading text.
    pub title: String,
    /// Optional HTML anchor id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    /// Index of the corresponding block in `FetchDocument.blocks`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_index: Option<usize>,
}

/// A rendered content block within a document.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenderedBlock {
    /// The kind of this block.
    pub kind: BlockKind,
    /// The text content of this block.
    pub text: String,
    /// Heading level (1-6) for heading blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<usize>,
    /// Optional anchor id for heading blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    /// Programming language for code blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// 1-based line number where this block starts in the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<usize>,
    /// 1-based line number where this block ends in the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<usize>,
    /// Page number (for multi-page documents like PDFs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
}

/// A semantic chunk of the document, grouping one or more blocks.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DocumentChunk {
    /// Unique identifier for this chunk within the document.
    pub chunk_id: String,
    /// The text content of this chunk (concatenation of block texts).
    pub text: String,
    /// Heading path from the document root to this chunk
    /// (e.g. `["Introduction", "Getting Started"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heading_path: Vec<String>,
    /// Index of the first block in `FetchDocument.blocks`.
    pub block_start: usize,
    /// Index of the last block (inclusive) in `FetchDocument.blocks`.
    pub block_end: usize,
    /// Page number where this chunk starts (for multi-page docs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_start: Option<usize>,
    /// Page number where this chunk ends (inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_end: Option<usize>,
}

/// A structured document representation of fetched content.
///
/// This is the primary new type introduced in Phase 1 of the
/// document model. It sits alongside the legacy `text` field in
/// `WebFetchResponse` and provides a machine-readable structure
/// for agents that want to inspect content more selectively.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FetchDocument {
    /// The kind of document that was fetched.
    pub kind: DocumentKind,
    /// The render format used for blocks and chunks.
    pub render_format: RenderFormat,
    /// The format of the legacy `text` field.
    pub text_format: String,
    /// Number of characters returned in the legacy `text` field.
    pub text_chars_returned: usize,
    /// Whether the text content was truncated at the character level.
    pub text_truncated: bool,
    /// Whether the block list was truncated.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub block_truncated: bool,
    /// Whether the link list was truncated.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub link_truncated: bool,
    /// Render-level metadata about the fetch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<FetchRenderMetadata>,
    /// Document outline (table of contents).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outline: Vec<DocumentOutlineEntry>,
    /// Rendered content blocks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<RenderedBlock>,
    /// Semantic chunks of the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunks: Vec<DocumentChunk>,
}
