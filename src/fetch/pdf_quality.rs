//! PDF page-quality classification.
//!
//! Image presence is always determinable here, independent of whether the
//! caller asked for media metadata, so an image-only page classifies as
//! scanned rather than blank at default settings.

use crate::core::document::PdfPageQualityKind;

const CID_TOKEN_THRESHOLD: f32 = 0.05;
const SPARSE_TEXT_THRESHOLD: usize = 50;

const QUALITY_CLEAN: f32 = 1.0;
const QUALITY_SPARSE: f32 = 0.5;
const QUALITY_CID: f32 = 0.25;
const QUALITY_SCANNED: f32 = 0.1;
const QUALITY_BLANK: f32 = 0.0;

pub(crate) fn classify_page_quality(text: &str, has_images: bool) -> (PdfPageQualityKind, f32) {
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

pub(crate) fn count_cid_tokens(text: &str) -> usize {
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

pub(crate) fn count_page_images(doc: &lopdf::Document, page_num: u32) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
