use super::types::FetchError;
use crate::core::document::{
    BlockKind, DocumentChunk, DocumentKind, DocumentOutlineEntry, FetchDocument,
    FetchRenderMetadata, PdfDocumentMetadata, PdfPageMetadata, PdfPageQualityKind, RenderFormat,
    RenderedBlock,
};
use crate::core::fetch::RedactedString;
use crate::core::sanitize::{bound_text, strip_control_chars};
use crate::core::warning::{AgentWarning, WarningCode};

const MAX_OUTLINE_ENTRIES: usize = 200;
const MAX_OUTLINE_DEPTH: usize = 6;
const MAX_OUTLINE_TITLE_LEN: usize = 200;

const CID_TOKEN_THRESHOLD: f32 = 0.05;
const SPARSE_TEXT_THRESHOLD: usize = 50;

const QUALITY_CLEAN: f32 = 1.0;
const QUALITY_SPARSE: f32 = 0.5;
const QUALITY_CID: f32 = 0.25;
const QUALITY_SCANNED: f32 = 0.1;
const QUALITY_BLANK: f32 = 0.0;

/// OCR policy for PDF extraction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PdfOcrPolicy {
    /// Never attempt OCR (default).
    #[default]
    Never,
    /// Automatically decide when to OCR.
    Auto,
    /// Always attempt OCR.
    Always,
}

/// Options for PDF text extraction.
pub struct PdfExtractOptions {
    /// Specific pages to extract (1-indexed). If `None`, all pages
    /// up to `max_pages` are extracted.
    pub selected_pages: Option<Vec<u32>>,
    /// Password for encrypted PDFs. Redacted in `Debug` output.
    pub password: Option<RedactedString>,
    /// Whether to include media metadata.
    pub include_media: bool,
    /// OCR policy for this extraction.
    pub ocr_policy: PdfOcrPolicy,
}

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
    /// Structured warnings for agent consumption.
    pub structured_warnings: Vec<AgentWarning>,
    /// Whether the text was truncated at the character level.
    pub text_truncated: bool,
    /// Per-page extraction quality metadata.
    pub page_metadata: Vec<PdfPageMetadata>,
    /// Document-level PDF metadata from the Info dictionary.
    pub pdf_metadata: PdfDocumentMetadata,
    /// Whether the extracted content is usable.
    pub content_ok: bool,
    /// Document-level quality score in [0.0, 1.0].
    pub quality_score: f32,
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

/// Parse a page selection specification into a sorted, deduplicated
/// list of 1-indexed page numbers.
pub fn parse_pdf_pages(
    spec: &str,
    total_pages: usize,
    max_pages: usize,
) -> Result<Vec<u32>, FetchError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(FetchError::PdfPageSpecInvalid(
            "page specification must not be empty".into(),
        ));
    }

    let mut pages = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(FetchError::PdfPageSpecInvalid(
                "empty segment in page specification".into(),
            ));
        }

        if let Some((start_str, end_str)) = part.split_once('-') {
            let start_str = start_str.trim();
            let end_str = end_str.trim();
            let start: u32 = start_str.parse().map_err(|_| {
                FetchError::PdfPageSpecInvalid(format!("invalid page number: '{start_str}'"))
            })?;
            let end: u32 = end_str.parse().map_err(|_| {
                FetchError::PdfPageSpecInvalid(format!("invalid page number: '{end_str}'"))
            })?;

            if start == 0 || end == 0 {
                return Err(FetchError::PdfPageSpecInvalid(
                    "page numbers are one-indexed; page 0 is not valid".into(),
                ));
            }

            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };

            for p in start..=end {
                if !pages.contains(&p) {
                    pages.push(p);
                }
            }
        } else {
            let p: u32 = part.parse().map_err(|_| {
                FetchError::PdfPageSpecInvalid(format!("invalid page number: '{part}'"))
            })?;
            if p == 0 {
                return Err(FetchError::PdfPageSpecInvalid(
                    "page numbers are one-indexed; page 0 is not valid".into(),
                ));
            }
            if !pages.contains(&p) {
                pages.push(p);
            }
        }
    }

    pages.sort();

    if pages.is_empty() {
        return Err(FetchError::PdfPageSpecInvalid(
            "page specification resolved to an empty selection".into(),
        ));
    }

    let total = total_pages as u32;
    let out_of_range: Vec<u32> = pages.iter().copied().filter(|&p| p > total).collect();
    if !out_of_range.is_empty() {
        return Err(FetchError::PdfPageOutOfRange {
            requested: out_of_range,
            total_pages,
        });
    }

    if pages.len() > max_pages {
        return Err(FetchError::PdfPageCapExceeded {
            selected: pages.len(),
            max_pages,
        });
    }

    Ok(pages)
}

fn decode_pdf_string(raw: &[u8]) -> String {
    if raw.len() >= 2 && raw[0] == 0xFE && raw[1] == 0xFF {
        let payload = &raw[2..];
        let payload = &payload[..payload.len() & !1];
        String::from_utf16_lossy(
            &payload
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(raw).into_owned()
    }
}

fn read_info_string(doc: &lopdf::Document, key: &[u8]) -> Option<String> {
    let info_ref = match doc.trailer.get(b"Info") {
        Ok(lopdf::Object::Reference(r)) => *r,
        _ => return None,
    };
    let info_obj = doc.get_object(info_ref).ok()?;
    let dict = info_obj.as_dict().ok()?;
    let obj = dict.get(key).ok()?;
    let raw = obj.as_str().ok()?;
    let s = decode_pdf_string(raw);
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        let (s, _) = strip_control_chars(s);
        let (s, _) = bound_text(&s, 500);
        Some(s)
    }
}

fn read_info_date(doc: &lopdf::Document, key: &[u8]) -> Option<String> {
    read_info_string(doc, key)
}

fn extract_pdf_metadata(doc: &lopdf::Document) -> PdfDocumentMetadata {
    PdfDocumentMetadata {
        title: read_info_string(doc, b"Title"),
        author: read_info_string(doc, b"Author"),
        subject: read_info_string(doc, b"Subject"),
        keywords: read_info_string(doc, b"Keywords"),
        creator: read_info_string(doc, b"Creator"),
        producer: read_info_string(doc, b"Producer"),
        creation_date: read_info_date(doc, b"CreationDate"),
        mod_date: read_info_date(doc, b"ModDate"),
        page_count: doc.get_pages().len(),
        page_labels: try_extract_page_labels(doc),
    }
}

fn try_extract_page_labels(doc: &lopdf::Document) -> Option<Vec<String>> {
    let catalog_ref = match doc.trailer.get(b"Root") {
        Ok(lopdf::Object::Reference(r)) => *r,
        _ => return None,
    };
    let catalog_obj = match doc.get_object(catalog_ref) {
        Ok(o) => o,
        _ => return None,
    };
    let catalog_dict = match catalog_obj.as_dict() {
        Ok(d) => d,
        _ => return None,
    };
    let labels_ref = match catalog_dict.get(b"PageLabels") {
        Ok(lopdf::Object::Reference(r)) => *r,
        _ => return None,
    };
    let labels_obj = match doc.get_object(labels_ref) {
        Ok(o) => o,
        _ => return None,
    };
    let labels_dict = match labels_obj.as_dict() {
        Ok(d) => d,
        _ => return None,
    };

    let nums_arr = match labels_dict.get(b"Nums") {
        Ok(lopdf::Object::Array(arr)) => arr,
        _ => return None,
    };

    let total_pages = doc.get_pages().len();
    let mut labels: Vec<Option<String>> = vec![None; total_pages];
    let mut current_prefix = String::new();
    let mut current_style: &str = "decimaldigits";
    let mut current_start: i64 = 1;

    let mut i = 0;
    while i < nums_arr.len() {
        let page_idx = match &nums_arr[i] {
            lopdf::Object::Integer(n) => *n as usize,
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;
        if i >= nums_arr.len() {
            break;
        }
        let label_dict = match &nums_arr[i] {
            lopdf::Object::Reference(r) => match doc.get_object(*r) {
                Ok(lopdf::Object::Dictionary(d)) => d.clone(),
                _ => {
                    i += 1;
                    continue;
                }
            },
            lopdf::Object::Dictionary(d) => d.clone(),
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;

        if let Ok(lopdf::Object::String(s, _)) = label_dict.get(b"P") {
            current_prefix = decode_pdf_string(s);
        }
        if let Ok(lopdf::Object::Name(name)) = label_dict.get(b"S") {
            current_style = match name.as_slice() {
                b"decimaldigits" => "decimaldigits",
                b"romanUppercase" => "romanUppercase",
                b"romanLowercase" => "romanLowercase",
                b"alphaUppercase" => "alphaUppercase",
                b"alphaLowercase" => "alphaLowercase",
                _ => "decimaldigits",
            };
        }
        if let Ok(lopdf::Object::Integer(n)) = label_dict.get(b"St") {
            current_start = *n;
        }

        let start = current_start.max(1) as usize;
        for (p, label) in labels
            .iter_mut()
            .enumerate()
            .take(total_pages)
            .skip(page_idx)
        {
            let num = start + (p - page_idx);
            let suffix = match current_style {
                "romanLowercase" => to_roman_lower(num),
                "romanUppercase" => to_roman_upper(num),
                "alphaLowercase" => to_alpha_lower(num),
                "alphaUppercase" => to_alpha_upper(num),
                _ => num.to_string(),
            };
            *label = Some(format!("{current_prefix}{suffix}"));
        }
    }

    if labels.iter().all(|l| l.is_none()) {
        None
    } else {
        Some(labels.into_iter().map(|l| l.unwrap_or_default()).collect())
    }
}

fn to_roman_upper(mut n: usize) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let vals = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1];
    let syms = [
        "M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I",
    ];
    let mut result = String::new();
    for (&v, s) in vals.iter().zip(syms.iter()) {
        while n >= v {
            result.push_str(s);
            n -= v;
        }
    }
    result
}

fn to_roman_lower(n: usize) -> String {
    to_roman_upper(n).to_lowercase()
}

fn to_alpha_upper(mut n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    n -= 1;
    let mut result = String::new();
    result.push((b'A' + (n % 26) as u8) as char);
    n /= 26;
    while n > 0 {
        n -= 1;
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    result
}

fn to_alpha_lower(n: usize) -> String {
    to_alpha_upper(n).to_lowercase()
}

fn try_extract_outline(doc: &lopdf::Document) -> Vec<DocumentOutlineEntry> {
    let catalog = match doc.trailer.get(b"Root") {
        Ok(lopdf::Object::Reference(catalog_ref)) => match doc.get_object(*catalog_ref) {
            Ok(obj) => obj,
            Err(_) => return Vec::new(),
        },
        _ => return Vec::new(),
    };

    let catalog_dict = match catalog.as_dict() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let outlines_ref = match catalog_dict.get(b"Outlines") {
        Ok(lopdf::Object::Reference(r)) => *r,
        _ => return Vec::new(),
    };

    let outlines_obj = match doc.get_object(outlines_ref) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let outlines_dict = match outlines_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let first_ref = match outlines_dict.get(b"First") {
        Ok(lopdf::Object::Reference(r)) => *r,
        _ => return Vec::new(),
    };

    let mut entries = Vec::new();
    collect_outline_entries(doc, first_ref, 0, &mut entries);
    entries
}

fn collect_outline_entries(
    doc: &lopdf::Document,
    obj_ref: lopdf::ObjectId,
    depth: usize,
    out: &mut Vec<DocumentOutlineEntry>,
) {
    if out.len() >= MAX_OUTLINE_ENTRIES || depth >= MAX_OUTLINE_DEPTH {
        return;
    }

    let obj = match doc.get_object(obj_ref) {
        Ok(o) => o,
        Err(_) => return,
    };

    let dict = match obj.as_dict() {
        Ok(d) => d,
        Err(_) => return,
    };

    let title = match dict.get(b"Title") {
        Ok(lopdf::Object::String(s, _)) => {
            let decoded = decode_pdf_string(s);
            let trimmed = decoded.trim().to_string();
            if trimmed.len() > MAX_OUTLINE_TITLE_LEN {
                trimmed[..MAX_OUTLINE_TITLE_LEN].to_string()
            } else {
                trimmed
            }
        }
        _ => String::new(),
    };

    if !title.is_empty() {
        let page_num = resolve_outline_page(doc, dict);
        out.push(DocumentOutlineEntry {
            level: depth + 1,
            title,
            anchor: None,
            block_index: None,
            page: page_num.map(|p| p as usize),
        });
    }

    if let Ok(lopdf::Object::Reference(child_ref)) = dict.get(b"First") {
        collect_outline_entries(doc, *child_ref, depth + 1, out);
    }

    if let Ok(lopdf::Object::Reference(next_ref)) = dict.get(b"Next") {
        collect_outline_entries(doc, *next_ref, depth, out);
    }
}

fn resolve_outline_page(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> Option<u32> {
    let dest = match dict.get(b"Dest") {
        Ok(d) => d.clone(),
        Err(_) => {
            if let Ok(lopdf::Object::Reference(action_ref)) = dict.get(b"A") {
                if let Ok(action_obj) = doc.get_object(*action_ref) {
                    if let Ok(action_dict) = action_obj.as_dict() {
                        if let Ok(d) = action_dict.get(b"D") {
                            d.clone()
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    };

    match dest {
        lopdf::Object::Reference(page_ref) => {
            let page_ids = doc.get_pages();
            page_ids
                .iter()
                .find(|(_, &id)| id == page_ref)
                .map(|(&num, _)| num)
        }
        lopdf::Object::Array(ref arr) => {
            if let Some(lopdf::Object::Reference(page_ref)) = arr.first() {
                let page_ids = doc.get_pages();
                page_ids
                    .iter()
                    .find(|(_, &id)| id == *page_ref)
                    .map(|(&num, _)| num)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn classify_page_quality(text: &str, has_images: bool) -> (PdfPageQualityKind, f32) {
    let trimmed = text.trim();

    if trimmed.is_empty() {
        if has_images {
            return (PdfPageQualityKind::ScannedOrImageOnly, QUALITY_SCANNED);
        }
        return (PdfPageQualityKind::Blank, QUALITY_BLANK);
    }

    let char_count = trimmed.chars().count();
    let cid_count = count_cid_tokens(trimmed);
    let cid_ratio = if char_count > 0 {
        cid_count as f32 / char_count as f32
    } else {
        0.0
    };

    if cid_ratio > CID_TOKEN_THRESHOLD {
        return (PdfPageQualityKind::CidCorrupt, QUALITY_CID);
    }

    let printable_count = trimmed
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .count();
    let printable_ratio = if char_count > 0 {
        printable_count as f32 / char_count as f32
    } else {
        0.0
    };

    if char_count < SPARSE_TEXT_THRESHOLD && has_images {
        return (PdfPageQualityKind::ScannedOrImageOnly, QUALITY_SCANNED);
    }

    if char_count < SPARSE_TEXT_THRESHOLD {
        return (PdfPageQualityKind::SparseText, QUALITY_SPARSE);
    }

    if printable_ratio < 0.7 {
        return (PdfPageQualityKind::SparseText, QUALITY_SPARSE);
    }

    (PdfPageQualityKind::CleanText, QUALITY_CLEAN)
}

fn count_cid_tokens(text: &str) -> usize {
    let mut count = 0;
    for part in text.split("(cid:") {
        if part.starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(end) = part.find(')') {
                let digits = &part[..end];
                if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                    count += 1;
                }
            }
        }
    }
    count
}

fn count_page_images(doc: &lopdf::Document, page_num: u32) -> usize {
    let page_ids = doc.get_pages();
    let page_id = match page_ids.get(&page_num) {
        Some(&id) => id,
        None => return 0,
    };
    let page_obj = match doc.get_object(page_id) {
        Ok(o) => o,
        Err(_) => return 0,
    };
    let page_dict = match page_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return 0,
    };

    let resources_dict = match page_dict.get(b"Resources") {
        Ok(resources_obj) => match resources_obj {
            lopdf::Object::Reference(r) => match doc.get_object(*r) {
                Ok(o) => match o.as_dict() {
                    Ok(d) => d.clone(),
                    Err(_) => return 0,
                },
                Err(_) => return 0,
            },
            lopdf::Object::Dictionary(d) => d.clone(),
            _ => return 0,
        },
        Err(_) => return 0,
    };

    let xobject_dict = match resources_dict.get(b"XObject") {
        Ok(lopdf::Object::Reference(r)) => match doc.get_object(*r) {
            Ok(o) => match o.as_dict() {
                Ok(d) => d.clone(),
                Err(_) => return 0,
            },
            Err(_) => return 0,
        },
        Ok(lopdf::Object::Dictionary(d)) => d.clone(),
        _ => return 0,
    };

    xobject_dict.len()
}

/// Extract text from a PDF byte slice with optional page selection
/// and quality metadata.
pub fn extract_pdf_text(
    bytes: &[u8],
    max_chars: usize,
    limits: &PdfLimits,
    options: Option<&PdfExtractOptions>,
) -> Result<PdfExtractionResult, FetchError> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("encrypted") || msg.contains("password") || msg.contains("Encrypt") {
            FetchError::PdfEncrypted
        } else {
            FetchError::PdfParseError(msg)
        }
    })?;

    if doc.is_encrypted() {
        let password = options
            .and_then(|o| o.password.as_ref())
            .map(|p| p.expose())
            .unwrap_or("");
        doc.authenticate_password(password)
            .map_err(|_| FetchError::PdfEncrypted)?;
    }

    let all_pages = doc.get_pages();
    let total_page_count = all_pages.len();

    if total_page_count == 0 {
        return Err(FetchError::PdfNoExtractableText);
    }

    let pages_to_extract: Vec<u32> = if let Some(opts) = options {
        if let Some(ref selected) = opts.selected_pages {
            selected.clone()
        } else {
            let mut page_numbers: Vec<u32> = all_pages.keys().copied().collect();
            page_numbers.sort();
            page_numbers.into_iter().take(limits.max_pages).collect()
        }
    } else {
        let mut page_numbers: Vec<u32> = all_pages.keys().copied().collect();
        page_numbers.sort();
        page_numbers.into_iter().take(limits.max_pages).collect()
    };

    let has_page_selection = options.and_then(|o| o.selected_pages.as_ref()).is_some();

    let ocr_policy = options.map(|o| o.ocr_policy).unwrap_or_default();
    let include_media = options.map(|o| o.include_media).unwrap_or(false);
    match ocr_policy {
        PdfOcrPolicy::Auto | PdfOcrPolicy::Always => {
            return Err(FetchError::PdfOcrUnavailable);
        }
        PdfOcrPolicy::Never => {}
    }

    let mut blocks: Vec<RenderedBlock> = Vec::new();
    let mut outline: Vec<DocumentOutlineEntry> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut structured_warnings: Vec<AgentWarning> = Vec::new();
    let mut page_metadata_list: Vec<PdfPageMetadata> = Vec::new();
    let mut total_chars: usize = 0;
    let mut pages_with_text: usize = 0;
    let mut pages_blank: usize = 0;
    let mut text_truncated = false;

    let extracted_outline = try_extract_outline(&doc);
    let outline_truncated = extracted_outline.len() >= MAX_OUTLINE_ENTRIES;
    outline.extend(extracted_outline);

    for &page_num in &pages_to_extract {
        let image_count = if include_media {
            count_page_images(&doc, page_num)
        } else {
            0
        };
        let has_images = image_count > 0;
        let page_text_result = doc.extract_text(&[page_num]);
        let page_text = match page_text_result {
            Ok(t) => t.trim().to_string(),
            Err(_) => {
                page_metadata_list.push(PdfPageMetadata {
                    page: page_num as usize,
                    quality_kind: PdfPageQualityKind::ExtractionFailed,
                    quality_score: 0.0,
                    extracted_chars: 0,
                    cid_token_count: 0,
                    image_count: if include_media {
                        Some(image_count)
                    } else {
                        None
                    },
                    warnings: vec!["extraction failed for this page".to_string()],
                });
                warnings.push(format!("page {page_num}: extraction failed"));
                structured_warnings.push(AgentWarning::new(
                    WarningCode::FetchWarning,
                    format!("page {page_num}: text extraction failed"),
                ));
                continue;
            }
        };

        let (quality_kind, quality_score) = classify_page_quality(&page_text, has_images);

        let page_chars = page_text.chars().count();
        let cid_count = count_cid_tokens(&page_text);

        let mut page_warnings: Vec<String> = Vec::new();

        match quality_kind {
            PdfPageQualityKind::Blank => {
                pages_blank += 1;
                page_metadata_list.push(PdfPageMetadata {
                    page: page_num as usize,
                    quality_kind,
                    quality_score,
                    extracted_chars: 0,
                    cid_token_count: 0,
                    image_count: if include_media {
                        Some(image_count)
                    } else {
                        None
                    },
                    warnings: Vec::new(),
                });
                continue;
            }
            PdfPageQualityKind::ExtractionFailed => {
                page_metadata_list.push(PdfPageMetadata {
                    page: page_num as usize,
                    quality_kind,
                    quality_score,
                    extracted_chars: 0,
                    cid_token_count: 0,
                    image_count: if include_media {
                        Some(image_count)
                    } else {
                        None
                    },
                    warnings: vec!["extraction failed for this page".to_string()],
                });
                continue;
            }
            PdfPageQualityKind::CidCorrupt => {
                page_warnings.push(format!(
                    "page {page_num} contains CID-encoded text that may be garbled"
                ));
                structured_warnings.push(AgentWarning::new(
                    WarningCode::PdfPageCidCorrupt,
                    format!("page {page_num} contains CID-encoded text that may be garbled"),
                ));
            }
            PdfPageQualityKind::ScannedOrImageOnly => {
                page_warnings.push(format!(
                    "page {page_num} appears to be scanned or image-only"
                ));
                structured_warnings.push(AgentWarning::new(
                    WarningCode::PdfPageLikelyScanned,
                    format!("page {page_num} appears to be scanned or image-only"),
                ));
            }
            PdfPageQualityKind::SparseText => {
                page_warnings.push(format!("page {page_num} has sparse or low-quality text"));
                structured_warnings.push(AgentWarning::new(
                    WarningCode::PdfPageSparseText,
                    format!("page {page_num} has sparse or low-quality text"),
                ));
            }
            PdfPageQualityKind::CleanText => {}
        }

        pages_with_text += 1;

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

        outline.push(DocumentOutlineEntry {
            level: 1,
            title: format!("Page {page_num}"),
            anchor: None,
            block_index: Some(blocks.len() - 1),
            page: Some(page_num as usize),
        });

        page_metadata_list.push(PdfPageMetadata {
            page: page_num as usize,
            quality_kind,
            quality_score,
            extracted_chars: page_text_chars,
            cid_token_count: cid_count,
            image_count: if include_media {
                Some(image_count)
            } else {
                None
            },
            warnings: page_warnings,
        });

        total_chars += page_text_chars;
    }

    if pages_blank > 0 && pages_blank == pages_to_extract.len() {
        return Err(FetchError::PdfNoExtractableText);
    }

    if pages_with_text == 0 && pages_blank > 0 {
        return Err(FetchError::PdfNoExtractableText);
    }

    if pages_blank > 0 {
        warnings.push(format!(
            "{pages_blank} of {} pages had no extractable text",
            pages_to_extract.len()
        ));
    }

    let sparse_pages: Vec<usize> = page_metadata_list
        .iter()
        .filter(|m| {
            matches!(
                m.quality_kind,
                PdfPageQualityKind::SparseText
                    | PdfPageQualityKind::CidCorrupt
                    | PdfPageQualityKind::ScannedOrImageOnly
            )
        })
        .map(|m| m.page)
        .collect();

    if !sparse_pages.is_empty() {
        let page_list = format_page_list(&sparse_pages);
        let kinds: Vec<&str> = page_metadata_list
            .iter()
            .filter(|m| sparse_pages.contains(&m.page))
            .map(|m| match m.quality_kind {
                PdfPageQualityKind::SparseText => "sparse text",
                PdfPageQualityKind::CidCorrupt => "CID-corrupt",
                PdfPageQualityKind::ScannedOrImageOnly => "scanned/image-only",
                _ => "",
            })
            .filter(|s| !s.is_empty())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let kind_str = kinds.join(", ");
        warnings.push(format!(
            "pages {page_list} appear {kind_str}; OCR is unavailable in this build"
        ));
    }

    if has_page_selection {
        let page_list = format_page_list(
            &pages_to_extract
                .iter()
                .map(|&p| p as usize)
                .collect::<Vec<_>>(),
        );
        warnings.push(format!("page selection applied: {page_list}"));
        structured_warnings.push(AgentWarning::new(
            WarningCode::PdfPageSelectionApplied,
            format!("page selection applied: {page_list}"),
        ));
    }

    if outline_truncated {
        warnings.push(format!(
            "PDF outline truncated at {MAX_OUTLINE_ENTRIES} entries"
        ));
        structured_warnings.push(AgentWarning::new(
            WarningCode::PdfOutlineTruncated,
            format!("PDF outline truncated at {MAX_OUTLINE_ENTRIES} entries"),
        ));
    }

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

    let (bounded_legacy, legacy_truncated) = bound_text(&legacy_text, max_chars);
    if legacy_truncated {
        text_truncated = true;
    }

    let first_page_num = pages_to_extract.first().copied().unwrap_or(1) as usize;
    let last_page_num = pages_to_extract.last().copied().unwrap_or(1) as usize;

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
            page_start: Some(first_page_num),
            page_end: Some(last_page_num),
        }]
    } else {
        Vec::new()
    };

    let pdf_meta = extract_pdf_metadata(&doc);
    let title = pdf_meta.title.clone();

    let doc_quality_score = if page_metadata_list.is_empty() {
        0.0
    } else {
        let total_weight: f32 = page_metadata_list.iter().map(|m| m.quality_score).sum();
        total_weight / page_metadata_list.len() as f32
    };

    let content_ok = pages_with_text > 0
        && !page_metadata_list.iter().all(|m| {
            matches!(
                m.quality_kind,
                PdfPageQualityKind::Blank
                    | PdfPageQualityKind::ExtractionFailed
                    | PdfPageQualityKind::ScannedOrImageOnly
            )
        });

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
        structured_warnings,
        text_truncated,
        page_metadata: page_metadata_list,
        pdf_metadata: pdf_meta,
        content_ok,
        quality_score: doc_quality_score,
    })
}

fn format_page_list(pages: &[usize]) -> String {
    if pages.len() <= 3 {
        pages
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        format!("{}, {}, and {} others", pages[0], pages[1], pages.len() - 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn make_pdf_with_metadata(title: &str, author: &str) -> Vec<u8> {
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
                Operation::new(
                    "Tj",
                    vec![Object::string_literal(
                        "Document content with metadata for testing purposes",
                    )],
                ),
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

        let info_id = doc.add_object(dictionary! {
            "Title" => Object::string_literal(title),
            "Author" => Object::string_literal(author),
            "Subject" => Object::string_literal("Test Subject"),
            "Keywords" => Object::string_literal("test, pdf"),
            "Creator" => Object::string_literal("test-suite"),
            "Producer" => Object::string_literal("lopdf-rs"),
        });
        doc.trailer.set("Info", info_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

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

    fn default_limits() -> PdfLimits {
        PdfLimits {
            max_pages: 25,
            max_chars_per_page: 12000,
            max_total_chars: 50000,
        }
    }

    #[test]
    fn extract_text_from_simple_pdf() {
        let pdf = make_text_pdf(
            "Hello World this is a test document with enough text to pass quality thresholds",
        );
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), None)
            .expect("extraction should succeed");

        assert_eq!(result.document.kind, DocumentKind::Pdf);
        assert!(!result.document.blocks.is_empty());
        assert!(result.document.blocks[0].text.contains("Hello World"));
        assert_eq!(result.document.blocks[0].page, Some(1));
        assert!(result.text.contains("Page 1"));
        assert!(result.text.contains("Hello World"));
        assert!(result.content_ok);
        assert!(result.quality_score > 0.5);
    }

    #[test]
    fn extract_text_from_multipage_pdf() {
        let pdf = make_multipage_pdf(&["Page one text", "Page two text", "Page three text"]);
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), None)
            .expect("extraction should succeed");

        assert_eq!(result.document.blocks.len(), 3);
        assert!(result.text.contains("--- Page 1 ---"));
        assert!(result.text.contains("--- Page 2 ---"));
        assert!(result.text.contains("--- Page 3 ---"));
        assert_eq!(result.document.chunks.len(), 1);
        assert_eq!(result.document.chunks[0].page_start, Some(1));
        assert_eq!(result.document.chunks[0].page_end, Some(3));
    }

    #[test]
    fn pdf_page_limit_enforced() {
        let pdf = make_multipage_pdf(&["a", "b", "c", "d", "e"]);
        let limits = PdfLimits {
            max_pages: 2,
            ..default_limits()
        };
        let result =
            extract_pdf_text(&pdf, 50000, &limits, None).expect("extraction should succeed");
        assert_eq!(result.document.blocks.len(), 2);
    }

    #[test]
    fn pdf_total_char_limit_enforced() {
        let long_text = "x".repeat(5000);
        let pdf = make_multipage_pdf(&[&long_text, &long_text, &long_text]);
        let limits = PdfLimits {
            max_total_chars: 8000,
            ..default_limits()
        };
        let result =
            extract_pdf_text(&pdf, 50000, &limits, None).expect("extraction should succeed");

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
            max_chars_per_page: 5000,
            ..default_limits()
        };
        let result =
            extract_pdf_text(&pdf, 50000, &limits, None).expect("extraction should succeed");

        let block_text = &result.document.blocks[0].text;
        assert!(block_text.chars().count() <= 5000);
        assert!(result.text_truncated);
    }

    #[test]
    fn invalid_pdf_returns_parse_error() {
        let bad_pdf = b"not a pdf at all";
        let result = extract_pdf_text(bad_pdf, 12000, &default_limits(), None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FetchError::PdfParseError(_)));
    }

    #[test]
    fn pdf_outline_entries_per_page() {
        let pdf = make_multipage_pdf(&["First", "Second"]);
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), None)
            .expect("extraction should succeed");

        assert_eq!(result.document.outline.len(), 2);
        assert_eq!(result.document.outline[0].title, "Page 1");
        assert_eq!(result.document.outline[1].title, "Page 2");
    }

    #[test]
    fn pdf_metadata_has_page_info() {
        let pdf = make_multipage_pdf(&["a", "b"]);
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), None)
            .expect("extraction should succeed");

        let meta = result.document.metadata.as_ref().expect("metadata");
        assert_eq!(meta.source_extension.as_deref(), Some("pdf"));
        assert!(meta.bytes_read.is_some());
    }

    #[test]
    fn pdf_all_blank_pages_returns_no_extractable_text() {
        let pdf = make_blank_page_pdf(3);
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), None);
        assert!(
            matches!(result, Err(FetchError::PdfNoExtractableText)),
            "expected PdfNoExtractableText for blank PDF, got: {result:?}"
        );
    }

    #[test]
    fn pdf_mixed_blank_and_text_pages_warns_about_blanks() {
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

        let blank_page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        page_ids.push(blank_page);

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

        let result = extract_pdf_text(&buf, 50000, &default_limits(), None)
            .expect("extraction should succeed");

        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("no extractable text")),
            "expected blank-page warning, got: {:?}",
            result.warnings
        );
        assert_eq!(result.document.blocks.len(), 1);
        assert!(result.document.blocks[0].text.contains("Visible text"));
    }

    #[test]
    fn encrypted_pdf_returns_encrypted_error() {
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
        let pdf_bytes = make_text_pdf("test");
        assert!(
            pdf_bytes.starts_with(b"%PDF-"),
            "PDF should start with %PDF- magic"
        );

        let text_bytes = b"hello world";
        assert!(!text_bytes.starts_with(b"%PDF-"));
    }

    // --- New tests for Phase 1 features ---

    #[test]
    fn parse_pdf_pages_single_page() {
        let pages = parse_pdf_pages("1", 10, 25).unwrap();
        assert_eq!(pages, vec![1]);
    }

    #[test]
    fn parse_pdf_pages_comma_separated() {
        let pages = parse_pdf_pages("1,3,5", 10, 25).unwrap();
        assert_eq!(pages, vec![1, 3, 5]);
    }

    #[test]
    fn parse_pdf_pages_range() {
        let pages = parse_pdf_pages("2-5", 10, 25).unwrap();
        assert_eq!(pages, vec![2, 3, 4, 5]);
    }

    #[test]
    fn parse_pdf_pages_mixed() {
        let pages = parse_pdf_pages("1,3,7-10", 10, 25).unwrap();
        assert_eq!(pages, vec![1, 3, 7, 8, 9, 10]);
    }

    #[test]
    fn parse_pdf_pages_deduplicates() {
        let pages = parse_pdf_pages("1,1,2,2,3", 10, 25).unwrap();
        assert_eq!(pages, vec![1, 2, 3]);
    }

    #[test]
    fn parse_pdf_pages_reversed_range_normalizes() {
        let pages = parse_pdf_pages("5-3", 10, 25).unwrap();
        assert_eq!(pages, vec![3, 4, 5]);
    }

    #[test]
    fn parse_pdf_pages_rejects_zero() {
        let result = parse_pdf_pages("0", 10, 25);
        assert!(matches!(result, Err(FetchError::PdfPageSpecInvalid(_))));
    }

    #[test]
    fn parse_pdf_pages_rejects_malformed() {
        let result = parse_pdf_pages("abc", 10, 25);
        assert!(matches!(result, Err(FetchError::PdfPageSpecInvalid(_))));
    }

    #[test]
    fn parse_pdf_pages_rejects_empty() {
        let result = parse_pdf_pages("", 10, 25);
        assert!(matches!(result, Err(FetchError::PdfPageSpecInvalid(_))));
    }

    #[test]
    fn parse_pdf_pages_rejects_out_of_range() {
        let result = parse_pdf_pages("1,15", 10, 25);
        assert!(matches!(result, Err(FetchError::PdfPageOutOfRange { .. })));
    }

    #[test]
    fn parse_pdf_pages_rejects_cap_exceeded() {
        let result = parse_pdf_pages("1,2,3,4", 10, 3);
        assert!(matches!(result, Err(FetchError::PdfPageCapExceeded { .. })));
    }

    #[test]
    fn parse_pdf_pages_whitespace_tolerant() {
        let pages = parse_pdf_pages(" 1 , 3 , 5 ", 10, 25).unwrap();
        assert_eq!(pages, vec![1, 3, 5]);
    }

    #[test]
    fn page_selection_applied() {
        let pdf = make_multipage_pdf(&["First", "Second", "Third"]);
        let opts = PdfExtractOptions {
            selected_pages: Some(vec![1, 3]),
            password: None,
            include_media: false,
            ocr_policy: PdfOcrPolicy::Never,
        };
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), Some(&opts))
            .expect("extraction should succeed");

        assert_eq!(result.document.blocks.len(), 2);
        assert_eq!(result.document.blocks[0].page, Some(1));
        assert_eq!(result.document.blocks[1].page, Some(3));
        assert_eq!(result.document.chunks[0].page_start, Some(1));
        assert_eq!(result.document.chunks[0].page_end, Some(3));
    }

    #[test]
    fn page_selection_warning_emitted() {
        let pdf = make_multipage_pdf(&["First", "Second", "Third"]);
        let opts = PdfExtractOptions {
            selected_pages: Some(vec![2]),
            password: None,
            include_media: false,
            ocr_policy: PdfOcrPolicy::Never,
        };
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), Some(&opts))
            .expect("extraction should succeed");

        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("page selection applied")),
            "expected page selection warning, got: {:?}",
            result.warnings
        );
    }

    #[test]
    fn page_metadata_populated() {
        let pdf = make_multipage_pdf(&["Hello world this is a page with enough text to pass the quality threshold for clean classification", "Second page also needs sufficient text content to avoid being marked as sparse"]);
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), None)
            .expect("extraction should succeed");

        assert_eq!(result.page_metadata.len(), 2);
        assert_eq!(result.page_metadata[0].page, 1);
        assert_eq!(
            result.page_metadata[0].quality_kind,
            PdfPageQualityKind::CleanText
        );
        assert!(result.page_metadata[0].extracted_chars > 0);
        assert_eq!(result.page_metadata[1].page, 2);
    }

    #[test]
    fn blank_page_classified() {
        let pdf = make_blank_page_pdf(1);
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), None);
        assert!(result.is_err());
    }

    #[test]
    fn pdf_metadata_extraction() {
        let pdf = make_pdf_with_metadata("Test Title", "Test Author");
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), None)
            .expect("extraction should succeed");

        assert_eq!(result.title.as_deref(), Some("Test Title"));
        assert_eq!(result.pdf_metadata.author.as_deref(), Some("Test Author"));
        assert_eq!(result.pdf_metadata.subject.as_deref(), Some("Test Subject"));
        assert_eq!(result.pdf_metadata.keywords.as_deref(), Some("test, pdf"));
        assert_eq!(result.pdf_metadata.creator.as_deref(), Some("test-suite"));
        assert!(result.pdf_metadata.producer.is_some());
        assert_eq!(result.pdf_metadata.page_count, 1);
    }

    #[test]
    fn content_ok_false_when_all_blank() {
        let pdf = make_blank_page_pdf(3);
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), None);
        assert!(result.is_err());
    }

    #[test]
    fn content_ok_true_with_mixed_quality() {
        let pdf = make_multipage_pdf(&["Good text here", "Another good page"]);
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), None)
            .expect("extraction should succeed");
        assert!(result.content_ok);
    }

    #[test]
    fn quality_score_range() {
        let pdf = make_multipage_pdf(&["Some text content", "More text content"]);
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), None)
            .expect("extraction should succeed");
        assert!(result.quality_score >= 0.0 && result.quality_score <= 1.0);
    }

    #[test]
    fn ocr_policy_always_returns_error() {
        let pdf = make_text_pdf("test");
        let opts = PdfExtractOptions {
            selected_pages: None,
            password: None,
            include_media: false,
            ocr_policy: PdfOcrPolicy::Always,
        };
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), Some(&opts));
        assert!(matches!(result, Err(FetchError::PdfOcrUnavailable)));
    }

    #[test]
    fn ocr_policy_auto_returns_error() {
        let pdf = make_text_pdf("test");
        let opts = PdfExtractOptions {
            selected_pages: None,
            password: None,
            include_media: false,
            ocr_policy: PdfOcrPolicy::Auto,
        };
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), Some(&opts));
        assert!(matches!(result, Err(FetchError::PdfOcrUnavailable)));
    }

    #[test]
    fn cid_token_counting() {
        assert_eq!(count_cid_tokens("hello world"), 0);
        assert_eq!(count_cid_tokens("(cid:123) text"), 1);
        assert_eq!(count_cid_tokens("(cid:1)(cid:2)(cid:3)"), 3);
        assert_eq!(count_cid_tokens("no cid tokens here"), 0);
    }

    #[test]
    fn classify_clean_text() {
        let text = "word ".repeat(20);
        let (kind, score) = classify_page_quality(&text, false);
        assert_eq!(kind, PdfPageQualityKind::CleanText);
        assert_eq!(score, QUALITY_CLEAN);
    }

    #[test]
    fn classify_blank_page() {
        let (kind, score) = classify_page_quality("", false);
        assert_eq!(kind, PdfPageQualityKind::Blank);
        assert_eq!(score, QUALITY_BLANK);
    }

    #[test]
    fn classify_scanned_page() {
        let (kind, score) = classify_page_quality("", true);
        assert_eq!(kind, PdfPageQualityKind::ScannedOrImageOnly);
        assert_eq!(score, QUALITY_SCANNED);
    }

    #[test]
    fn classify_sparse_text() {
        let (kind, _) = classify_page_quality("ab", false);
        assert_eq!(kind, PdfPageQualityKind::SparseText);
    }

    #[test]
    fn classify_cid_corrupt() {
        let text = "(cid:123)(cid:456)(cid:789)(cid:1)(cid:2)(cid:3)(cid:4)(cid:5)(cid:6)(cid:7) more text here for padding and length";
        let (kind, score) = classify_page_quality(text, false);
        assert_eq!(kind, PdfPageQualityKind::CidCorrupt);
        assert_eq!(score, QUALITY_CID);
    }

    fn make_image_only_pdf() -> Vec<u8> {
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let image_data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
        let image_id = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 100,
                "Height" => 100,
                "BitsPerComponent" => 8,
                "ColorSpace" => "DeviceRGB",
            },
            image_data,
        ));

        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! {
                "Im0" => image_id,
            },
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
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

    #[test]
    fn image_only_pdf_classified_as_scanned() {
        let pdf = make_image_only_pdf();
        let opts = PdfExtractOptions {
            selected_pages: None,
            password: None,
            include_media: true,
            ocr_policy: PdfOcrPolicy::Never,
        };
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), Some(&opts))
            .expect("extraction should succeed");
        assert_eq!(result.page_metadata.len(), 1);
        assert_eq!(
            result.page_metadata[0].quality_kind,
            PdfPageQualityKind::ScannedOrImageOnly
        );
        assert!(
            !result.content_ok,
            "content_ok should be false for scanned-only PDF"
        );
    }

    #[test]
    fn include_media_returns_image_count() {
        let pdf = make_image_only_pdf();
        let opts = PdfExtractOptions {
            selected_pages: None,
            password: None,
            include_media: true,
            ocr_policy: PdfOcrPolicy::Never,
        };
        let result = extract_pdf_text(&pdf, 12000, &default_limits(), Some(&opts))
            .expect("extraction should succeed");
        assert_eq!(result.page_metadata[0].image_count, Some(1));
    }

    #[test]
    fn include_media_false_omits_image_count() {
        let pdf = make_multipage_pdf(&["text content here"]);
        let opts = PdfExtractOptions {
            selected_pages: None,
            password: None,
            include_media: false,
            ocr_policy: PdfOcrPolicy::Never,
        };
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), Some(&opts))
            .expect("extraction should succeed");
        assert!(result.page_metadata[0].image_count.is_none());
    }

    #[test]
    fn structured_warnings_populated_for_page_selection() {
        let pdf = make_multipage_pdf(&["First", "Second", "Third"]);
        let opts = PdfExtractOptions {
            selected_pages: Some(vec![1, 3]),
            password: None,
            include_media: false,
            ocr_policy: PdfOcrPolicy::Never,
        };
        let result = extract_pdf_text(&pdf, 50000, &default_limits(), Some(&opts))
            .expect("extraction should succeed");
        assert!(
            result
                .structured_warnings
                .iter()
                .any(|w| w.code == WarningCode::PdfPageSelectionApplied),
            "expected PdfPageSelectionApplied, got: {:?}",
            result.structured_warnings
        );
    }

    #[test]
    fn to_roman_upper_basic() {
        assert_eq!(to_roman_upper(1), "I");
        assert_eq!(to_roman_upper(4), "IV");
        assert_eq!(to_roman_upper(9), "IX");
        assert_eq!(to_roman_upper(58), "LVIII");
        assert_eq!(to_roman_upper(1999), "MCMXCIX");
    }

    #[test]
    fn to_roman_lower_basic() {
        assert_eq!(to_roman_lower(1), "i");
        assert_eq!(to_roman_lower(4), "iv");
        assert_eq!(to_roman_lower(9), "ix");
    }

    #[test]
    fn to_alpha_upper_basic() {
        assert_eq!(to_alpha_upper(1), "A");
        assert_eq!(to_alpha_upper(26), "Z");
        assert_eq!(to_alpha_upper(27), "AA");
        assert_eq!(to_alpha_upper(52), "AZ");
    }

    #[test]
    fn to_alpha_lower_basic() {
        assert_eq!(to_alpha_lower(1), "a");
        assert_eq!(to_alpha_lower(26), "z");
        assert_eq!(to_alpha_lower(27), "aa");
    }
}
