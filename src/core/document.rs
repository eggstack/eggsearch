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
    /// Jupyter notebook (.ipynb).
    Notebook,
    /// CSV or TSV spreadsheet.
    Csv,
    /// XML document (including RSS/Atom feeds).
    Xml,
    /// reStructuredText document.
    Rst,
    /// AsciiDoc document.
    AsciiDoc,
    /// Unrecognized content type.
    #[default]
    Unknown,
}

impl DocumentKind {
    /// Returns the stable snake_case label for this document kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::PlainText => "plain_text",
            Self::Markdown => "markdown",
            Self::Code => "code",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Diff => "diff",
            Self::Patch => "patch",
            Self::Pdf => "pdf",
            Self::Notebook => "notebook",
            Self::Csv => "csv",
            Self::Xml => "xml",
            Self::Rst => "rst",
            Self::AsciiDoc => "asciidoc",
            Self::Unknown => "unknown",
        }
    }
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
    /// Page number for this outline entry (for multi-page documents
    /// like PDFs where page navigation is meaningful).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
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

const DEFAULT_CHUNK_CHAR_LIMIT: usize = 4096;
const DEFAULT_CHUNK_BLOCK_LIMIT: usize = 8;

/// Build bounded, deterministic chunks from rendered document blocks.
pub fn build_document_chunks(
    doc_id: &str,
    outline: &[DocumentOutlineEntry],
    blocks: &[RenderedBlock],
    max_chars: usize,
) -> Vec<DocumentChunk> {
    if blocks.is_empty() {
        return Vec::new();
    }

    let chunk_char_limit = max_chars.clamp(1, DEFAULT_CHUNK_CHAR_LIMIT);
    let chunk_block_limit = DEFAULT_CHUNK_BLOCK_LIMIT.max(1);
    let has_heading_blocks = blocks.iter().any(|block| block.kind == BlockKind::Heading);
    let mut heading_stack: Vec<(usize, String)> = if has_heading_blocks {
        Vec::new()
    } else {
        outline
            .iter()
            .map(|entry| (entry.level, entry.title.clone()))
            .collect()
    };

    let mut chunks = Vec::new();
    let mut chunk_start = 0usize;
    let mut chunk_chars = 0usize;
    let mut chunk_blocks = 0usize;
    let mut chunk_heading_path = current_heading_path(&heading_stack);
    let mut chunk_page_start: Option<usize> = None;
    let mut chunk_page_end: Option<usize> = None;

    let mut push_chunk = |block_start: usize,
                          block_end: usize,
                          heading_path: &[String],
                          page_start: Option<usize>,
                          page_end: Option<usize>| {
        if block_start > block_end {
            return;
        }

        let text = blocks[block_start..=block_end]
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let heading_path_joined = heading_path.join("/");
        let chunk_index = chunks.len();
        chunks.push(DocumentChunk {
            chunk_id: crate::core::identity::chunk_id(doc_id, chunk_index, &heading_path_joined),
            text,
            heading_path: heading_path.to_vec(),
            block_start,
            block_end,
            page_start,
            page_end,
        });
    };

    for (block_index, block) in blocks.iter().enumerate() {
        let block_chars = block.text.chars().count();
        let next_chars = if chunk_blocks == 0 {
            block_chars
        } else {
            chunk_chars + 1 + block_chars
        };
        let would_overflow = chunk_blocks > 0
            && (next_chars > chunk_char_limit || chunk_blocks >= chunk_block_limit);

        if would_overflow {
            push_chunk(
                chunk_start,
                block_index - 1,
                &chunk_heading_path,
                chunk_page_start,
                chunk_page_end,
            );
            chunk_start = block_index;
            chunk_chars = 0;
            chunk_blocks = 0;
            chunk_heading_path = current_heading_path(&heading_stack);
            chunk_page_start = None;
            chunk_page_end = None;
        }

        if block.kind == BlockKind::Heading {
            if chunk_blocks > 0 {
                push_chunk(
                    chunk_start,
                    block_index - 1,
                    &chunk_heading_path,
                    chunk_page_start,
                    chunk_page_end,
                );
                chunk_start = block_index;
                chunk_chars = 0;
                chunk_blocks = 0;
                chunk_page_start = None;
                chunk_page_end = None;
            }
            update_heading_stack(
                &mut heading_stack,
                block.level.unwrap_or(1),
                block.text.clone(),
            );
            chunk_heading_path = current_heading_path(&heading_stack);
        }

        if chunk_blocks > 0 {
            chunk_chars += 1;
        }
        chunk_chars += block_chars;
        chunk_blocks += 1;
        if let Some(page) = block.page {
            chunk_page_start = Some(chunk_page_start.map_or(page, |current| current.min(page)));
            chunk_page_end = Some(chunk_page_end.map_or(page, |current| current.max(page)));
        }

        if block_index + 1 == blocks.len() {
            push_chunk(
                chunk_start,
                block_index,
                &chunk_heading_path,
                chunk_page_start,
                chunk_page_end,
            );
        }
    }

    chunks
}

fn update_heading_stack(stack: &mut Vec<(usize, String)>, level: usize, title: String) {
    let level = level.max(1);
    while let Some((current_level, _)) = stack.last() {
        if *current_level < level {
            break;
        }
        stack.pop();
    }
    stack.push((level, title));
}

fn current_heading_path(stack: &[(usize, String)]) -> Vec<String> {
    stack.iter().map(|(_, title)| title.clone()).collect()
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

/// Quality classification for an extracted PDF page.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PdfPageQualityKind {
    /// Page has readable, mostly clean Unicode text.
    CleanText,
    /// Page has some text but it appears sparse or low-quality.
    SparseText,
    /// Page text contains significant `(cid:NN)` tokens indicating
    /// CID-font corruption; extracted text may be garbled.
    CidCorrupt,
    /// Page appears to be scanned or image-only with little or no
    /// extractable text. OCR is not available in this build.
    ScannedOrImageOnly,
    /// Page has no extractable text and no image evidence.
    Blank,
    /// Text extraction failed for this page.
    ExtractionFailed,
}

/// Per-page extraction quality metadata for a PDF page.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PdfPageMetadata {
    /// 1-based page number within the document.
    pub page: usize,
    /// Quality classification for this page.
    pub quality_kind: PdfPageQualityKind,
    /// Advisory quality score in [0.0, 1.0]. Higher is better.
    pub quality_score: f32,
    /// Number of characters extracted from this page.
    pub extracted_chars: usize,
    /// Count of `(cid:NN)` tokens found in the extracted text.
    pub cid_token_count: usize,
    /// Number of images detected on this page, if cheaply available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_count: Option<usize>,
    /// Page-specific warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Document-level PDF metadata extracted from the Info dictionary.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PdfDocumentMetadata {
    /// Document title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Document author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Document subject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Document keywords.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
    /// Creator application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// Producer application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
    /// Creation date string (raw PDF date format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<String>,
    /// Modification date string (raw PDF date format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_date: Option<String>,
    /// Total page count in the document.
    pub page_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_document_chunks_splits_on_headings() {
        let outline = vec![DocumentOutlineEntry {
            level: 1,
            title: "Doc Title".to_string(),
            anchor: None,
            block_index: Some(0),
            page: None,
        }];
        let blocks = vec![
            RenderedBlock {
                kind: BlockKind::Heading,
                text: "Intro".to_string(),
                level: Some(1),
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: Some(1),
            },
            RenderedBlock {
                kind: BlockKind::Paragraph,
                text: "Alpha".to_string(),
                level: None,
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: Some(1),
            },
            RenderedBlock {
                kind: BlockKind::Heading,
                text: "Next".to_string(),
                level: Some(2),
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: Some(2),
            },
            RenderedBlock {
                kind: BlockKind::Paragraph,
                text: "Beta".to_string(),
                level: None,
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: Some(2),
            },
        ];

        let chunks = build_document_chunks("doc_test", &outline, &blocks, 4096);
        assert_eq!(chunks.len(), 2);
        assert_eq!(
            chunks[0].chunk_id,
            crate::core::identity::chunk_id("doc_test", 0, "Intro")
        );
        assert_eq!(chunks[0].block_start, 0);
        assert_eq!(chunks[0].block_end, 1);
        assert_eq!(chunks[0].page_start, Some(1));
        assert_eq!(chunks[0].page_end, Some(1));
        assert_eq!(
            chunks[1].chunk_id,
            crate::core::identity::chunk_id("doc_test", 1, "Intro/Next")
        );
        assert_eq!(chunks[1].block_start, 2);
        assert_eq!(chunks[1].block_end, 3);
        assert_eq!(chunks[1].page_start, Some(2));
        assert_eq!(chunks[1].page_end, Some(2));
    }

    #[test]
    fn build_document_chunks_uses_outline_when_no_heading_blocks_exist() {
        let outline = vec![DocumentOutlineEntry {
            level: 1,
            title: "Page Title".to_string(),
            anchor: None,
            block_index: Some(0),
            page: None,
        }];
        let blocks = vec![RenderedBlock {
            kind: BlockKind::Paragraph,
            text: "Alpha".to_string(),
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        }];

        let chunks = build_document_chunks("doc_outline", &outline, &blocks, 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading_path, vec!["Page Title".to_string()]);
    }
}
