use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::*;

fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        let options = if *name == "mimetype" {
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
        } else {
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
        };
        writer.start_file(*name, options).unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn extract(body: &[u8], format: DocumentFormat) -> DocumentExtraction {
    extract_document(
        body,
        format,
        "https://example.test/file",
        None,
        false,
        MAX_DOCUMENT_CHARS,
        MAX_DOCUMENT_CHARS,
    )
    .unwrap()
}

#[test]
fn csv_emits_deterministic_one_indexed_row_blocks() {
    let body = (1..=101)
        .map(|row| format!("{row},value{row}"))
        .collect::<Vec<_>>()
        .join("\n");
    let first = extract_document(
        body.as_bytes(),
        DocumentFormat::Csv,
        "https://example.test/data.csv",
        Some("text/csv"),
        false,
        MAX_DOCUMENT_CHARS,
        MAX_DOCUMENT_CHARS,
    )
    .unwrap();
    let second = extract_document(
        body.as_bytes(),
        DocumentFormat::Csv,
        "https://example.test/data.csv",
        Some("text/csv"),
        false,
        MAX_DOCUMENT_CHARS,
        MAX_DOCUMENT_CHARS,
    )
    .unwrap();
    assert_eq!(first.parts.len(), 2);
    assert_eq!(first.parts_total, 2);
    assert_eq!(first.parts[0].index, 1);
    assert_eq!(first.parts[0].title.as_deref(), Some("rows 1-100"));
    assert_eq!(first.parts[1].title.as_deref(), Some("rows 101-101"));
    assert_eq!(first.parts[0].text, second.parts[0].text);
    assert_eq!(first.text, second.text);
    assert!(first.parts[0].text.starts_with("1\tvalue1"));
    let tsv = extract_document(
        b"a\tb\nc\td",
        DocumentFormat::Csv,
        "https://example.test/data.tsv",
        None,
        false,
        MAX_DOCUMENT_CHARS,
        MAX_DOCUMENT_CHARS,
    )
    .unwrap();
    assert!(tsv.parts[0].text.starts_with("a\tb"));
}

#[test]
fn epub_uses_spine_order_and_heading_titles() {
    let body = archive(&[
        ("mimetype", "application/epub+zip"),
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>"#,
        ),
        (
            "OPS/book.opf",
            r#"<package><manifest><item id="one" href="one.xhtml"/><item id="two" href="two.xhtml"/></manifest><spine><itemref idref="two"/><itemref idref="one"/></spine></package>"#,
        ),
        (
            "OPS/one.xhtml",
            r#"<html><body><h1>First chapter</h1><p>One text</p></body></html>"#,
        ),
        (
            "OPS/two.xhtml",
            r#"<html><body><h1>Second chapter</h1><p>Two text</p></body></html>"#,
        ),
    ]);
    let result = extract(&body, DocumentFormat::Epub);
    assert_eq!(result.parts.len(), 2);
    assert_eq!(result.parts[0].title.as_deref(), Some("Second chapter"));
    assert!(result.parts[0].text.contains("Two text"));
    assert_eq!(result.parts[1].title.as_deref(), Some("First chapter"));
    assert!(result.parts[1].text.contains("One text"));
    assert_eq!(detect_container_format(&body), Some(DocumentFormat::Epub));
}

#[test]
fn docx_splits_body_at_heading_paragraphs() {
    let body = archive(&[(
        "word/document.xml",
        r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Overview</w:t></w:r></w:p><w:p><w:r><w:t>Intro text</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Details</w:t></w:r></w:p><w:p><w:r><w:t>More text</w:t></w:r></w:p></w:body></w:document>"#,
    )]);
    let result = extract(&body, DocumentFormat::Docx);
    assert_eq!(result.parts.len(), 2);
    assert_eq!(result.parts[0].title.as_deref(), Some("Overview"));
    assert!(result.parts[0].text.contains("Intro text"));
    assert_eq!(result.parts[1].title.as_deref(), Some("Details"));
    assert!(result.parts[1].text.contains("More text"));
}

#[test]
fn xlsx_preserves_workbook_sheet_order() {
    let body = archive(&[
        (
            "[Content_Types].xml",
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
        ),
        (
            "xl/workbook.xml",
            r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Second in order" sheetId="1" r:id="rId2"/><sheet name="First in order" sheetId="2" r:id="rId1"/></sheets></workbook>"#,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#,
        ),
        (
            "xl/worksheets/sheet1.xml",
            r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:A1"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>First data</t></is></c></row></sheetData></worksheet>"#,
        ),
        (
            "xl/worksheets/sheet2.xml",
            r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:A1"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Second data</t></is></c></row></sheetData></worksheet>"#,
        ),
    ]);
    let result = extract(&body, DocumentFormat::Xlsx);
    assert_eq!(result.parts.len(), 2);
    assert_eq!(result.parts[0].title.as_deref(), Some("Second in order"));
    assert!(result.parts[0].text.contains("Second data"));
    assert_eq!(result.parts[1].title.as_deref(), Some("First in order"));
    assert!(result.parts[1].text.contains("First data"));
}

#[test]
fn pptx_uses_presentation_order_and_slide_titles() {
    let body = archive(&[
        (
            "ppt/presentation.xml",
            r#"<p:presentation xmlns:p="urn:p" xmlns:r="urn:r"><p:sldIdLst><p:sldId id="2" r:id="rId2"/><p:sldId id="1" r:id="rId1"/></p:sldIdLst></p:presentation>"#,
        ),
        (
            "ppt/_rels/presentation.xml.rels",
            r#"<Relationships><Relationship Id="rId1" Target="slides/slide1.xml"/><Relationship Id="rId2" Target="slides/slide2.xml"/></Relationships>"#,
        ),
        (
            "ppt/slides/slide1.xml",
            r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a"><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Slide One</a:t></a:r></a:p></p:txBody></p:sp></p:sld>"#,
        ),
        (
            "ppt/slides/slide2.xml",
            r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a"><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Slide Two</a:t></a:r></a:p></p:txBody></p:sp></p:sld>"#,
        ),
    ]);
    let result = extract(&body, DocumentFormat::Pptx);
    assert_eq!(result.parts.len(), 2);
    assert_eq!(result.parts[0].title.as_deref(), Some("Slide Two"));
    assert!(result.parts[0].text.contains("Slide Two"));
    assert_eq!(result.parts[1].title.as_deref(), Some("Slide One"));
    assert!(result.parts[1].text.contains("Slide One"));
}

#[test]
fn malformed_empty_encrypted_and_high_ratio_inputs_are_distinguished() {
    assert!(matches!(
        extract_document(
            b"",
            DocumentFormat::Csv,
            "https://example.test/a.csv",
            None,
            false,
            100,
            100,
        ),
        Err(FetchError::DocumentEmpty)
    ));
    let empty_archive = archive(&[]);
    assert!(matches!(
        extract_document(
            &empty_archive,
            DocumentFormat::Epub,
            "https://example.test/a.epub",
            None,
            false,
            100,
            100,
        ),
        Err(FetchError::DocumentEmpty)
    ));
    assert!(matches!(
        extract_document(
            b"PK\x03\x04broken",
            DocumentFormat::Docx,
            "https://example.test/a.docx",
            None,
            false,
            100,
            100,
        ),
        Err(FetchError::DocumentMalformed(_))
    ));
    assert!(matches!(
        extract_document(
            CFB_MAGIC,
            DocumentFormat::Docx,
            "https://example.test/a.docx",
            None,
            false,
            100,
            100,
        ),
        Err(FetchError::DocumentEncrypted)
    ));
    let bomb = "x".repeat(100_000);
    let body = archive(&[("large.txt", bomb.as_str())]);
    assert!(matches!(
        extract_document(
            &body,
            DocumentFormat::Docx,
            "https://example.test/a.docx",
            None,
            false,
            100,
            100,
        ),
        Err(FetchError::DocumentMalformed(_))
    ));
}

#[test]
fn xml_rejects_dtds_and_part_text_budget_marks_truncation() {
    assert!(matches!(
        xml_tokens(br#"<!DOCTYPE a [<!ENTITY x "x">]><a>&x;</a>"#),
        Err(FetchError::DocumentMalformed(_))
    ));
    assert!(matches!(
        xml_tokens(b"<root>"),
        Err(FetchError::DocumentMalformed(_))
    ));
    let result = extract_document(
        b"a,b\nc,d",
        DocumentFormat::Csv,
        "https://example.test/a.csv",
        None,
        false,
        2,
        2,
    )
    .unwrap();
    assert_eq!(result.parts[0].chars, 2);
    assert!(result.document_truncated);
    assert!(!result.text_truncated);
    let window = extract_document(
        b"a,b\nc,d",
        DocumentFormat::Csv,
        "https://example.test/a.csv",
        None,
        false,
        MAX_DOCUMENT_CHARS,
        2,
    )
    .unwrap();
    assert_eq!(window.parts[0].chars, 7);
    assert!(!window.document_truncated);
    assert!(window.text_truncated);
}
