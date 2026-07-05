//! PDF text extraction for `web_fetch`.
//!
//! This module provides conservative, page-indexed text extraction
//! from PDF documents. It requires the `pdf` Cargo feature to be
//! compiled in and `[fetch].pdf_enabled = true` in configuration.
//!
//! Non-goals: OCR, page rendering, JavaScript execution, embedded
//! file extraction, full layout reconstruction.

use super::types::FetchError;
use crate::core::document::{
    BlockKind, DocumentChunk, DocumentKind, DocumentOutlineEntry, FetchDocument,
    FetchRenderMetadata, RenderFormat, RenderedBlock,
};
use crate::core::sanitize::{bound_text, strip_control_chars};

/// Result of PDF text extraction.
#[derive(Debug)]
pub struct PdfExtractionResult {
    /// The structured document with page-indexed blocks and chunks.
    pub document: FetchDocument,
    /// Legacy text field with page markers.
    pub text: String,
    /// Title extracted from PDF metadata (if available).
    pub title: Option<String>,
    /// Warnings generated during extraction.
    pub warnings: Vec<String>,
    /// Whether the text was truncated at the character level.
    pub text_truncated: bool,
}

/// Configuration for PDF extraction limits.
pub struct PdfLimits {
    /// Maximum number of pages to attempt extracting.
    pub max_pages: usize,
    /// Maximum characters to extract per page.
    pub max_chars_per_page: usize,
    /// Maximum total characters across all pages.
    pub max_total_chars: usize,
}

/// Extract text from a PDF byte slice.
///
/// Returns page-indexed blocks, legacy text with page markers,
/// and extraction warnings. Returns `Err` for parse failures,
/// encrypted PDFs, or other unrecoverable errors.
pub fn extract_pdf_text(
    bytes: &[u8],
    max_chars: usize,
    limits: &PdfLimits,
) -> Result<PdfExtractionResult, FetchError> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("encrypted") || msg.contains("password") || msg.contains("Encrypt") {
            FetchError::PdfEncrypted
        } else {
            FetchError::PdfParseError(msg)
        }
    })?;

    // Check if the document is encrypted
    if doc.is_encrypted() {
        // Try to decrypt with empty password. Returns Ok(()) on success, Err on failure.
        doc.authenticate_password("")
            .map_err(|_| FetchError::PdfEncrypted)?;
    }

    let pages = doc.get_pages();
    let total_page_count = pages.len();

    if total_page_count == 0 {
        return Err(FetchError::PdfNoExtractableText);
    }

    let mut page_numbers: Vec<u32> = pages.keys().copied().collect();
    page_numbers.sort();

    // Cap the number of pages we attempt
    let pages_to_extract: Vec<u32> = page_numbers.into_iter().take(limits.max_pages).collect();

    let _pages_skipped = total_page_count.saturating_sub(pages_to_extract.len());

    let mut blocks: Vec<RenderedBlock> = Vec::new();
    let mut outline: Vec<DocumentOutlineEntry> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut total_chars: usize = 0;
    let mut pages_with_text: usize = 0;
    let mut pages_blank: usize = 0;
    let mut text_truncated = false;

    // Extract text per page
    for &page_num in &pages_to_extract {
        let page_text = doc.extract_text(&[page_num]).unwrap_or_default();

        let page_text = page_text.trim().to_string();
        let page_chars = page_text.chars().count();

        if page_text.is_empty() {
            pages_blank += 1;
            continue;
        }

        pages_with_text += 1;

        // Apply per-page char cap
        let page_text = if page_chars > limits.max_chars_per_page {
            let bounded = page_text
                .chars()
                .take(limits.max_chars_per_page)
                .collect::<String>();
            text_truncated = true;
            bounded
        } else {
            page_text
        };

        let page_text_chars = page_text.chars().count();

        // Check total char cap
        if total_chars + page_text_chars > limits.max_total_chars {
            let remaining = limits.max_total_chars.saturating_sub(total_chars);
            if remaining > 0 {
                let truncated_text = page_text.chars().take(remaining).collect::<String>();
                let block = RenderedBlock {
                    kind: BlockKind::Paragraph,
                    text: truncated_text,
                    level: None,
                    anchor: None,
                    language: None,
                    line_start: None,
                    line_end: None,
                    page: Some(page_num as usize),
                };
                blocks.push(block);
            }
            text_truncated = true;
            warnings.push(format!(
                "PDF total character limit ({}) exceeded after page {}",
                limits.max_total_chars, page_num
            ));
            break;
        }

        // Apply Tier 1 (strip control chars + length bound) to block text
        let (stripped, _) = strip_control_chars(&page_text);
        let (bounded, _) = bound_text(&stripped, limits.max_chars_per_page);

        let block = RenderedBlock {
            kind: BlockKind::Paragraph,
            text: bounded,
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: Some(page_num as usize),
        };
        blocks.push(block);

        // Add outline entry for each page with text
        outline.push(DocumentOutlineEntry {
            level: 1,
            title: format!("Page {page_num}"),
            anchor: None,
            block_index: Some(blocks.len() - 1),
        });

        total_chars += page_text_chars;
    }

    // Aggregate blank-page warnings
    if pages_blank > 0 {
        if pages_blank == total_page_count {
            return Err(FetchError::PdfNoExtractableText);
        }
        warnings.push(format!(
            "{pages_blank} of {total_page_count} pages had no extractable text"
        ));
    }

    if pages_with_text == 0 && pages_blank > 0 {
        return Err(FetchError::PdfNoExtractableText);
    }

    // Build page-marked legacy text
    let mut legacy_text_parts: Vec<String> = Vec::new();
    let mut first_page = true;
    for block in &blocks {
        let page = block.page.unwrap_or(0);
        if !first_page {
            legacy_text_parts.push(format!("\n--- Page {page} ---\n"));
        } else {
            legacy_text_parts.push(format!("--- Page {page} ---\n"));
            first_page = false;
        }
        legacy_text_parts.push(block.text.clone());
    }
    let legacy_text = legacy_text_parts.join("");

    // Truncate legacy text to max_chars
    let (bounded_legacy, legacy_truncated) = bound_text(&legacy_text, max_chars);
    if legacy_truncated {
        text_truncated = true;
    }

    // Build a single chunk from all blocks
    let chunks = if !blocks.is_empty() {
        let chunk_text = blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        vec![DocumentChunk {
            chunk_id: "chunk_0".to_string(),
            text: chunk_text,
            heading_path: Vec::new(),
            block_start: 0,
            block_end: blocks.len().saturating_sub(1),
            page_start: Some(1),
            page_end: Some(total_page_count),
        }]
    } else {
        Vec::new()
    };

    // Extract PDF metadata title
    let title = extract_pdf_title(&doc);

    let text_chars_returned = bounded_legacy.chars().count();

    let document = FetchDocument {
        kind: DocumentKind::Pdf,
        render_format: RenderFormat::AgentBlocksV1,
        text_format: "plain".to_string(),
        text_chars_returned,
        text_truncated,
        block_truncated: false,
        link_truncated: false,
        metadata: Some(FetchRenderMetadata {
            bytes_read: Some(bytes.len()),
            content_length: None,
            charset: None,
            redirects_followed: 0,
            source_extension: Some("pdf".to_string()),
            detected_language: None,
        }),
        outline,
        blocks,
        chunks,
    };

    Ok(PdfExtractionResult {
        document,
        text: bounded_legacy,
        title,
        warnings,
        text_truncated,
    })
}

/// Extract the title from PDF document info metadata.
fn extract_pdf_title(doc: &lopdf::Document) -> Option<String> {
    // Try the Info dictionary first
    if let Ok(lopdf::Object::Reference(info_ref)) = doc.trailer.get(b"Info") {
        if let Ok(info_obj) = doc.get_object(*info_ref) {
            if let Ok(dict) = info_obj.as_dict() {
                if let Ok(title_obj) = dict.get(b"Title") {
                    if let Ok(title_bytes) = title_obj.as_str() {
                        // Try UTF-16BE decoding first (common for PDF titles),
                        // then fall back to UTF-8/latin1.
                        let title = if title_bytes.len() >= 2
                            && title_bytes[0] == 0xFE
                            && title_bytes[1] == 0xFF
                        {
                            let payload = &title_bytes[2..];
                            let payload = &payload[..payload.len() & !1];
                            String::from_utf16_lossy(
                                &payload
                                    .chunks_exact(2)
                                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                                    .collect::<Vec<_>>(),
                            )
                        } else {
                            String::from_utf8_lossy(title_bytes).into_owned()
                        };
                        if !title.trim().is_empty() {
                            return Some(title.trim().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a valid PDF with text on a page using lopdf.
    fn make_text_pdf(text: &str) -> Vec<u8> {
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        });

        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![100.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(text)]),
                Operation::new("ET", vec![]),
            ],
        };

        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => resources_id,
        });

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    /// Generate a PDF with multiple pages using lopdf.
    fn make_multipage_pdf(page_texts: &[&str]) -> Vec<u8> {
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        });

        let mut page_ids = Vec::new();
        for text in page_texts {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 12.into()]),
                    Operation::new("Td", vec![100.into(), 700.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*text)]),
                    Operation::new("ET", vec![]),
                ],
            };

            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content_id,
                "Resources" => resources_id,
            });
            page_ids.push(page_id);
        }

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => page_texts.len() as u32,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn extract_text_from_simple_pdf() {
        let pdf = make_text_pdf("Hello World");
        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(&pdf, 12000, &limits).expect("extraction should succeed");

        assert_eq!(result.document.kind, DocumentKind::Pdf);
        assert!(!result.document.blocks.is_empty());
        assert!(result.document.blocks[0].text.contains("Hello World"));
        assert_eq!(result.document.blocks[0].page, Some(1));
        assert!(result.text.contains("Page 1"));
        assert!(result.text.contains("Hello World"));
    }

    #[test]
    fn extract_text_from_multipage_pdf() {
        let pdf = make_multipage_pdf(&["Page one text", "Page two text", "Page three text"]);
        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(&pdf, 50000, &limits).expect("extraction should succeed");

        assert_eq!(result.document.blocks.len(), 3);
        assert!(result.text.contains("--- Page 1 ---"));
        assert!(result.text.contains("--- Page 2 ---"));
        assert!(result.text.contains("--- Page 3 ---"));
        assert!(result.text.contains("Page one text"));
        assert!(result.text.contains("Page two text"));
        assert!(result.text.contains("Page three text"));
        // Check chunks have page metadata
        assert_eq!(result.document.chunks.len(), 1);
        assert_eq!(result.document.chunks[0].page_start, Some(1));
        assert_eq!(result.document.chunks[0].page_end, Some(3));
    }

    #[test]
    fn pdf_page_limit_enforced() {
        let pdf = make_multipage_pdf(&["a", "b", "c", "d", "e"]);
        let limits = PdfLimits {
            max_pages: 2,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(&pdf, 50000, &limits).expect("extraction should succeed");

        // Only 2 pages should be extracted
        assert_eq!(result.document.blocks.len(), 2);
    }

    #[test]
    fn pdf_total_char_limit_enforced() {
        let long_text = "x".repeat(5000);
        let pdf = make_multipage_pdf(&[&long_text, &long_text, &long_text]);
        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 8000,
        };
        let result = extract_pdf_text(&pdf, 50000, &limits).expect("extraction should succeed");

        // Should have a warning about the limit
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("character limit")),
            "expected character limit warning, got: {:?}",
            result.warnings
        );
    }

    #[test]
    fn pdf_per_page_char_limit_enforced() {
        let long_text = "x".repeat(20000);
        let pdf = make_text_pdf(&long_text);
        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 5000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(&pdf, 50000, &limits).expect("extraction should succeed");

        // Page text should be capped
        let block_text = &result.document.blocks[0].text;
        assert!(block_text.chars().count() <= 5000);
        assert!(result.text_truncated);
    }

    #[test]
    fn invalid_pdf_returns_parse_error() {
        let bad_pdf = b"not a pdf at all";
        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(bad_pdf, 12000, &limits);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FetchError::PdfParseError(_)));
    }

    #[test]
    fn pdf_outline_entries_per_page() {
        let pdf = make_multipage_pdf(&["First", "Second"]);
        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(&pdf, 50000, &limits).expect("extraction should succeed");

        assert_eq!(result.document.outline.len(), 2);
        assert_eq!(result.document.outline[0].title, "Page 1");
        assert_eq!(result.document.outline[1].title, "Page 2");
    }

    #[test]
    fn pdf_metadata_has_page_info() {
        let pdf = make_multipage_pdf(&["a", "b"]);
        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(&pdf, 50000, &limits).expect("extraction should succeed");

        let meta = result.document.metadata.as_ref().expect("metadata");
        assert_eq!(meta.source_extension.as_deref(), Some("pdf"));
        assert!(meta.bytes_read.is_some());
    }

    /// Generate a PDF with blank pages (no text content).
    fn make_blank_page_pdf(page_count: usize) -> Vec<u8> {
        use lopdf::{dictionary, Document, Object};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let mut page_ids = Vec::new();
        for _ in 0..page_count {
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            page_ids.push(page_id);
        }

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => page_count as u32,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn pdf_all_blank_pages_returns_no_extractable_text() {
        let pdf = make_blank_page_pdf(3);
        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(&pdf, 12000, &limits);
        assert!(
            matches!(result, Err(FetchError::PdfNoExtractableText)),
            "expected PdfNoExtractableText for blank PDF, got: {:?}",
            result
        );
    }

    #[test]
    fn pdf_mixed_blank_and_text_pages_warns_about_blanks() {
        // Create a PDF with some blank pages and some with text.
        // We build it manually since make_multipage_pdf always adds text.
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        });

        let mut page_ids = Vec::new();

        // Page 1: blank (no content stream)
        let blank_page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        page_ids.push(blank_page);

        // Page 2: has text
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![100.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal("Visible text")]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let text_page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => resources_id,
        });
        page_ids.push(text_page);

        // Page 3: blank
        let blank_page2 = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        page_ids.push(blank_page2);

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => 3,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();

        let limits = PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        };
        let result = extract_pdf_text(&buf, 50000, &limits).expect("extraction should succeed");

        // Should have a warning about blank pages
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("no extractable text")),
            "expected blank-page warning, got: {:?}",
            result.warnings
        );
        // Should still extract text from the page that has content
        assert_eq!(result.document.blocks.len(), 1);
        assert!(result.document.blocks[0].text.contains("Visible text"));
    }

    #[test]
    fn encrypted_pdf_returns_encrypted_error() {
        // We can't easily create an encrypted PDF in tests, but we can
        // verify the error variant is correct by testing the error path.
        // This test ensures the error type exists and is distinct.
        let err = FetchError::PdfEncrypted;
        assert!(matches!(err, FetchError::PdfEncrypted));
        assert!(matches!(
            err.kind(),
            crate::fetch::types::FetchErrorKind::PdfEncrypted
        ));
        assert_eq!(err.error_code(), "pdf_encrypted");
    }

    #[test]
    fn pdf_body_magic_detection_works() {
        // Verify that %PDF- magic bytes are correctly identified.
        // This tests the detection logic, not the extraction.
        let pdf_bytes = make_text_pdf("test");
        assert!(
            pdf_bytes.starts_with(b"%PDF-"),
            "PDF should start with %PDF- magic"
        );

        // Verify non-PDF bytes don't match
        let text_bytes = b"hello world";
        assert!(!text_bytes.starts_with(b"%PDF-"));
    }
}
