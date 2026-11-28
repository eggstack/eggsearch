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

fn stored_archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn xlsx_with_sheets(sheets: &[(String, String)]) -> Vec<u8> {
    let mut entries: Vec<(String, String)> = vec![
        (
            "[Content_Types].xml".to_string(),
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#.to_string(),
        ),
        (
            "_rels/.rels".to_string(),
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_string(),
        ),
    ];
    let sheet_entries: Vec<(String, String)> = sheets
        .iter()
        .enumerate()
        .map(|(index, (_, xml))| (format!("xl/worksheets/sheet{}.xml", index + 1), xml.clone()))
        .collect();
    let workbook_sheets: String = sheets
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            format!(
                r#"<sheet name="{name}" sheetId="{}" r:id="rId{}"/>"#,
                index + 1,
                index + 2
            )
        })
        .collect();
    entries.push((
        "xl/workbook.xml".to_string(),
        format!(
            r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{workbook_sheets}</sheets></workbook>"#
        ),
    ));
    let relationships: String = sheets
        .iter()
        .enumerate()
        .map(|(index, _)| {
            format!(
                r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{}.xml"/>"#,
                index + 2,
                index + 1
            )
        })
        .collect();
    entries.push((
        "xl/_rels/workbook.xml.rels".to_string(),
        format!(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
        ),
    ));
    entries.extend(sheet_entries);
    let refs: Vec<(&str, &str)> = entries
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    stored_archive(&refs)
}

fn sheet_xml(body: &str) -> String {
    format!(
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">{body}</worksheet>"#
    )
}

fn pptx_with_slide_attributes(attributes: &str) -> Vec<u8> {
    let presentation = format!(
        r#"<p:presentation xmlns:p="urn:p" xmlns:r="urn:r"><p:sldIdLst><p:sldId {attributes}/></p:sldIdLst></p:presentation>"#
    );
    archive(&[
        ("ppt/presentation.xml", presentation.as_str()),
        (
            "ppt/_rels/presentation.xml.rels",
            r#"<Relationships><Relationship Id="rId2" Target="slides/slide2.xml"/></Relationships>"#,
        ),
        (
            "ppt/slides/slide2.xml",
            r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a"><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Slide Two</a:t></a:r></a:p></p:txBody></p:sp></p:sld>"#,
        ),
    ])
}

fn xlsx_error(body: &[u8]) -> FetchError {
    extract_document(
        body,
        DocumentFormat::Xlsx,
        "https://example.test/book.xlsx",
        None,
        false,
        MAX_DOCUMENT_CHARS,
        MAX_DOCUMENT_CHARS,
    )
    .unwrap_err()
}

fn assert_bound(error: FetchError, expected: &str) {
    match error {
        FetchError::DocumentMalformed(message) => {
            assert!(message.contains(expected), "unexpected message: {message}");
        }
        other => panic!("expected a malformed document, got {other:?}"),
    }
}

#[test]
fn xml_references_resolve_into_the_surrounding_text_run() {
    let body = archive(&[(
        "word/document.xml",
        r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:r><w:t>Q&amp;A &lt;tag&gt; caf&#233; &#xE9; <![CDATA[cdata]]> &quot;x&quot; &apos;y&apos;</w:t></w:r></w:p></w:body></w:document>"#,
    )]);
    let result = extract(&body, DocumentFormat::Docx);
    assert_eq!(result.parts.len(), 1);
    assert_eq!(
        result.parts[0].text,
        "Q&A <tag> caf\u{e9} \u{e9} cdata \"x\" 'y'"
    );

    let epub = archive(&[
        ("mimetype", "application/epub+zip"),
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>"#,
        ),
        (
            "OPS/book.opf",
            r#"<package><manifest><item id="one" href="one.xhtml"/></manifest><spine><itemref idref="one"/></spine></package>"#,
        ),
        (
            "OPS/one.xhtml",
            r#"<html><body><p>a&nbsp;b</p></body></html>"#,
        ),
    ]);
    let text = extract(&epub, DocumentFormat::Epub);
    assert_eq!(text.parts[0].text, "a&nbsp;b");
}

#[test]
fn xml_accepts_plain_doctypes_and_still_refuses_dtd_payloads() {
    let tokens = xml_tokens(br#"<!DOCTYPE html><html><body><p>text</p></body></html>"#)
        .expect("a doctype without an internal subset or external identifier is accepted");
    assert!(tokens.iter().any(|token| token.name == "p"));
    assert!(xml_tokens(br#"<!DOCTYPE a [<!ENTITY x "x">]><a>&x;</a>"#).is_err());
    assert!(xml_tokens(br#"<!DOCTYPE a SYSTEM "a.dtd"><a/>"#).is_err());
    assert!(xml_tokens(br#"<!DOCTYPE a system "a.dtd"><a/>"#).is_err());
    assert!(xml_tokens(br#"<!DOCTYPE a PUBLIC "public" "system"><a/>"#).is_err());
    assert!(xml_tokens(br#"<!DOCTYPE a public "p" "s"><a/>"#).is_err());

    let epub = archive(&[
        ("mimetype", "application/epub+zip"),
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>"#,
        ),
        (
            "OPS/book.opf",
            r#"<package><manifest><item id="one" href="one.xhtml"/></manifest><spine><itemref idref="one"/></spine></package>"#,
        ),
        (
            "OPS/one.xhtml",
            r#"<!DOCTYPE html><html><body><p>Doctype body</p></body></html>"#,
        ),
    ]);
    assert_eq!(
        extract(&epub, DocumentFormat::Epub).parts[0].text,
        "Doctype body"
    );
}

#[test]
fn pptx_relationship_ids_are_independent_of_attribute_order() {
    let declared_first = extract(
        &pptx_with_slide_attributes(r#"id="2" r:id="rId2""#),
        DocumentFormat::Pptx,
    );
    let qualified_first = extract(
        &pptx_with_slide_attributes(r#"r:id="rId2" id="2""#),
        DocumentFormat::Pptx,
    );
    assert_eq!(declared_first.parts[0].text, "Slide Two");
    assert_eq!(qualified_first.parts[0].text, "Slide Two");
    assert_eq!(
        declared_first.parts[0].title.as_deref(),
        qualified_first.parts[0].title.as_deref()
    );
    assert!(!declared_first.parts[0].truncated);
    assert!(!qualified_first.parts[0].truncated);
}

#[test]
fn xlsx_declared_dimension_does_not_bound_observed_cells() {
    let body = xlsx_with_sheets(&[(
        "Only".to_string(),
        sheet_xml(
            r#"<dimension ref="A1:XFD1048576"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Only data</t></is></c></row></sheetData>"#,
        ),
    )]);
    let result = extract(&body, DocumentFormat::Xlsx);
    assert_eq!(result.parts.len(), 1);
    assert_eq!(result.parts[0].text.trim(), "Only data");
}

#[test]
fn xlsx_many_rows_reach_the_cell_bound_not_the_token_bound() {
    let mut data = String::from("<sheetData>");
    for row in 1..=45_000usize {
        data.push_str(&format!(
            r#"<row r="{row}"><c r="A{row}"><v>1</v></c></row>"#
        ));
    }
    data.push_str("</sheetData>");
    let body = xlsx_with_sheets(&[("Rows".to_string(), sheet_xml(&data))]);
    let result = extract(&body, DocumentFormat::Xlsx);
    assert_eq!(result.parts.len(), 1);
    assert!(result.parts[0].text.starts_with('1'));
    assert!(result.parts[0].chars > 40_000);
}

#[test]
fn xlsx_cell_count_and_bounding_area_are_bounded_on_observed_cells() {
    let mut over_cap = String::from("<sheetData><row>");
    for row in 1..=200_001usize {
        over_cap.push_str(&format!(r#"<c r="A{row}"/>"#));
    }
    over_cap.push_str("</row></sheetData>");
    assert_bound(
        xlsx_error(&xlsx_with_sheets(&[(
            "Over".to_string(),
            sheet_xml(&over_cap),
        )])),
        "spreadsheet cell bound exceeded",
    );

    let sparse =
        sheet_xml(r#"<sheetData><row r="1"><c r="A1"/><c r="XFD1048576"/></row></sheetData>"#);
    assert_bound(
        xlsx_error(&xlsx_with_sheets(&[("Sparse".to_string(), sparse)])),
        "spreadsheet cell bound exceeded",
    );
}

#[test]
fn csv_cell_bound_refuses_during_the_scan() {
    let body = ",".repeat(300_000);
    let error = extract_document(
        body.as_bytes(),
        DocumentFormat::Csv,
        "https://example.test/data.csv",
        Some("text/csv"),
        false,
        MAX_DOCUMENT_CHARS,
        MAX_DOCUMENT_CHARS,
    )
    .unwrap_err();
    assert_bound(error, "CSV cell bound exceeded");
}

#[test]
fn truncation_is_attributed_to_the_part_that_was_cut() {
    let heading = "h".repeat(5_000);
    let first = format!(r#"<html><body><h1>{heading}</h1><p>First text</p></body></html>"#);
    let body = archive(&[
        ("mimetype", "application/epub+zip"),
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>"#,
        ),
        (
            "OPS/book.opf",
            r#"<package><manifest><item id="one" href="one.xhtml"/><item id="two" href="two.xhtml"/><item id="three" href="three.xhtml"/></manifest><spine><itemref idref="one"/><itemref idref="two"/><itemref idref="three"/></spine></package>"#,
        ),
        ("OPS/one.xhtml", first.as_str()),
        (
            "OPS/two.xhtml",
            r#"<!DOCTYPE html><html><body><p>Middle text</p></body></html>"#,
        ),
        (
            "OPS/three.xhtml",
            r#"<html><body><p>Last text</p></body></html>"#,
        ),
    ]);
    let result = extract(&body, DocumentFormat::Epub);
    assert_eq!(result.parts.len(), 3);
    assert!(result.parts[0].truncated);
    assert!(!result.parts[1].truncated);
    assert!(!result.parts[2].truncated);
    assert!(result.document_truncated);
    assert!(result.parts[1].text.contains("Middle text"));
}
