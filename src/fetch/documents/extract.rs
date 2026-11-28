use std::collections::HashMap;
use std::io::{Cursor, Read};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use quick_xml::events::Event;
use zip::ZipArchive;

use crate::core::document::{DocumentFormat, DocumentPart, DocumentPartKind};
use crate::fetch::types::FetchError;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_DECOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_INFLATION_RATIO: u64 = 100;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const MAX_PARTS: usize = 4096;
const MAX_DOCUMENT_CHARS: usize = 200_000;
const MAX_XML_TOKENS: usize = 262_144;
const MAX_CELL_COUNT: usize = 200_000;
const ZIP_MAGIC: &[u8] = b"PK\x03\x04";
const CFB_MAGIC: &[u8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1";

/// A deterministic extraction result for a supported container document.
#[derive(Clone, Debug)]
pub struct DocumentExtraction {
    /// Detected document format.
    pub format: DocumentFormat,
    /// Document title when a format-specific title is available.
    pub title: Option<String>,
    /// Addressable document parts.
    pub parts: Vec<DocumentPart>,
    /// Total number of parts before caller character budgets.
    pub parts_total: usize,
    /// Concatenated text clipped to the output window.
    pub text: String,
    /// Whether the output-window text was clipped.
    pub text_truncated: bool,
    /// Whether extraction or the document character budget clipped content.
    pub document_truncated: bool,
    /// Format label for the extracted text.
    pub text_format: String,
}

#[derive(Clone, Debug)]
struct RawPart {
    kind: DocumentPartKind,
    title: Option<String>,
    text: String,
    truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    Start,
    Empty,
    End,
    Text,
    Truncated,
}

#[derive(Clone, Debug)]
struct XmlToken {
    kind: TokenKind,
    name: String,
    attributes: HashMap<String, String>,
    text: String,
    truncated: bool,
}

/// Detects a supported format from ZIP container markers.
pub fn detect_container_format(body: &[u8]) -> Option<DocumentFormat> {
    if !body.starts_with(ZIP_MAGIC) || body.len() > MAX_INPUT_BYTES {
        return None;
    }
    if read_preview(body, "mimetype", 1024)
        .is_some_and(|value| value.trim() == "application/epub+zip")
    {
        return Some(DocumentFormat::Epub);
    }
    let content_types = read_preview(body, "[Content_Types].xml", 64 * 1024)?;
    format_from_content_types(&content_types)
}

/// Returns an unsupported document label for known formats outside M004.
pub fn unsupported_document_format(
    content_type: Option<&str>,
    url: &str,
    body: &[u8],
) -> Option<String> {
    let mime = content_type
        .and_then(|value| value.split(';').next())
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if matches!(mime.as_str(), "text/html" | "application/xhtml+xml") {
        return None;
    }
    if mime.contains("opendocument") {
        return Some(mime);
    }
    if matches!(
        mime.as_str(),
        "application/msword"
            | "application/vnd.ms-excel"
            | "application/vnd.ms-powerpoint"
            | "application/rtf"
            | "text/rtf"
    ) {
        return Some(mime);
    }
    let extension = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(
        extension.as_str(),
        "odt" | "ods" | "odp" | "doc" | "xls" | "ppt" | "rtf" | "docm" | "xlsm" | "pptm"
    ) {
        return Some(extension);
    }
    if body.starts_with(CFB_MAGIC) {
        return Some("compound_office_binary".to_string());
    }
    let mimetype = read_preview(body, "mimetype", 1024).unwrap_or_default();
    if mimetype
        .trim()
        .starts_with("application/vnd.oasis.opendocument.")
    {
        return Some(mimetype.trim().to_string());
    }
    let content_types = read_preview(body, "[Content_Types].xml", 64 * 1024).unwrap_or_default();
    let lower = content_types.to_ascii_lowercase();
    if lower.contains("opendocument") {
        return Some("opendocument".to_string());
    }
    None
}

/// Extracts a supported document into bounded, one-indexed parts.
pub fn extract_document(
    body: &[u8],
    format: DocumentFormat,
    url: &str,
    content_type: Option<&str>,
    markdown: bool,
    document_chars: usize,
    window_chars: usize,
) -> Result<DocumentExtraction, FetchError> {
    if body.len() > MAX_INPUT_BYTES {
        return Err(FetchError::DocumentMalformed(
            "input size bound exceeded".to_string(),
        ));
    }
    if body.is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    if format != DocumentFormat::Csv && body.starts_with(CFB_MAGIC) {
        return Err(FetchError::DocumentEncrypted);
    }
    let raw_parts = match format {
        DocumentFormat::Csv => extract_csv(body, url, content_type, markdown)?,
        DocumentFormat::Epub => extract_epub(body)?,
        DocumentFormat::Docx => extract_docx(body, markdown)?,
        DocumentFormat::Xlsx => extract_xlsx(body, markdown)?,
        DocumentFormat::Pptx => extract_pptx(body)?,
    };
    build_result(format, raw_parts, markdown, document_chars, window_chars)
}

fn build_result(
    format: DocumentFormat,
    raw_parts: Vec<RawPart>,
    markdown: bool,
    document_chars: usize,
    window_chars: usize,
) -> Result<DocumentExtraction, FetchError> {
    if raw_parts.is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    if raw_parts.len() > MAX_PARTS {
        return Err(FetchError::DocumentMalformed(
            "part count bound exceeded".to_string(),
        ));
    }
    let parts_total = raw_parts.len();
    let mut remaining = document_chars.min(MAX_DOCUMENT_CHARS);
    let mut document_truncated = false;
    let mut parts = Vec::with_capacity(raw_parts.len());
    for (offset, raw) in raw_parts.into_iter().enumerate() {
        let char_count = raw.text.chars().count();
        let text: String = raw.text.chars().take(remaining).collect();
        let chars = text.chars().count();
        let truncated = raw.truncated || chars < char_count;
        document_truncated |= truncated;
        remaining = remaining.saturating_sub(chars);
        parts.push(DocumentPart {
            index: offset + 1,
            kind: raw.kind,
            title: raw.title,
            text,
            chars,
            truncated,
        });
    }
    let joined = parts
        .iter()
        .map(|part| part.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let text_limit = window_chars.max(1);
    let text: String = joined.chars().take(text_limit).collect();
    let text_truncated = text.chars().count() < joined.chars().count();
    let title = parts.iter().find_map(|part| part.title.clone());
    Ok(DocumentExtraction {
        format,
        title,
        parts,
        parts_total,
        text,
        text_truncated,
        document_truncated,
        text_format: if markdown { "markdown" } else { "plain" }.to_string(),
    })
}

fn extract_csv(
    body: &[u8],
    url: &str,
    content_type: Option<&str>,
    markdown: bool,
) -> Result<Vec<RawPart>, FetchError> {
    let input = String::from_utf8_lossy(body);
    if input.trim().is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    let is_tsv = url.to_ascii_lowercase().ends_with(".tsv")
        || content_type.is_some_and(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("text/tab-separated-values")
        });
    let delimiter = if is_tsv
        || (!input.lines().next().unwrap_or_default().contains(',')
            && input.lines().next().unwrap_or_default().contains('\t'))
    {
        '\t'
    } else {
        ','
    };
    let mut rows_in_part = Vec::with_capacity(100);
    let mut parts = Vec::new();
    let mut rows_total = 0usize;
    let mut truncated_at_row = None;
    let mut emit_row = |row: Vec<String>, truncated_row: Option<usize>| -> Result<(), FetchError> {
        rows_total += 1;
        if rows_total > MAX_PARTS * 100 {
            return Err(FetchError::DocumentMalformed(
                "part count bound exceeded".to_string(),
            ));
        }
        rows_in_part.push(row);
        if rows_in_part.len() == 100 {
            push_csv_part(
                &mut parts,
                &mut rows_in_part,
                rows_total,
                truncated_row,
                markdown,
            )?;
        }
        Ok(())
    };
    for_each_delimited_row(&input, delimiter, &mut emit_row, &mut truncated_at_row)?;
    if !rows_in_part.is_empty() {
        push_csv_part(
            &mut parts,
            &mut rows_in_part,
            rows_total,
            truncated_at_row,
            markdown,
        )?;
    }
    if parts.is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    Ok(parts)
}

fn for_each_delimited_row<F>(
    input: &str,
    delimiter: char,
    callback: &mut F,
    truncated_at_row: &mut Option<usize>,
) -> Result<(), FetchError>
where
    F: FnMut(Vec<String>, Option<usize>) -> Result<(), FetchError>,
{
    let mut chars = input.chars().peekable();
    let mut cell = String::new();
    let mut row = Vec::new();
    let mut quoted = false;
    let mut stored_chars = 0usize;
    let mut rows_seen = 0usize;
    let mut cells = 0usize;
    while let Some(character) = chars.next() {
        if character == '"' {
            if quoted && chars.peek() == Some(&'"') {
                chars.next();
                store_csv_char(
                    '"',
                    &mut cell,
                    &mut stored_chars,
                    truncated_at_row,
                    rows_seen + 1,
                );
            } else {
                quoted = !quoted;
            }
        } else if character == delimiter && !quoted {
            row.push(std::mem::take(&mut cell));
            count_csv_cell(&mut cells)?;
        } else if character == '\n' && !quoted {
            row.push(std::mem::take(&mut cell));
            count_csv_cell(&mut cells)?;
            rows_seen += 1;
            callback(std::mem::take(&mut row), *truncated_at_row)?;
        } else if character == '\r' && !quoted && chars.peek() == Some(&'\n') {
        } else {
            store_csv_char(
                character,
                &mut cell,
                &mut stored_chars,
                truncated_at_row,
                rows_seen + 1,
            );
        }
    }
    if quoted {
        return Err(FetchError::DocumentMalformed(
            "unterminated CSV quoted field".to_string(),
        ));
    }
    if !cell.is_empty() || !row.is_empty() || !input.ends_with('\n') {
        row.push(cell);
        count_csv_cell(&mut cells)?;
        callback(row, *truncated_at_row)?;
    }
    Ok(())
}

fn count_csv_cell(cells: &mut usize) -> Result<(), FetchError> {
    *cells += 1;
    if *cells > MAX_CELL_COUNT {
        return Err(FetchError::DocumentMalformed(
            "CSV cell bound exceeded".to_string(),
        ));
    }
    Ok(())
}

fn store_csv_char(
    character: char,
    cell: &mut String,
    stored: &mut usize,
    truncated_at_row: &mut Option<usize>,
    row: usize,
) {
    if *stored < MAX_DOCUMENT_CHARS {
        cell.push(character);
        *stored += 1;
    } else {
        truncated_at_row.get_or_insert(row);
    }
}

fn push_csv_part(
    parts: &mut Vec<RawPart>,
    rows: &mut Vec<Vec<String>>,
    rows_total: usize,
    truncated_at_row: Option<usize>,
    markdown: bool,
) -> Result<(), FetchError> {
    if parts.len() >= MAX_PARTS {
        return Err(FetchError::DocumentMalformed(
            "part count bound exceeded".to_string(),
        ));
    }
    let start = rows_total - rows.len() + 1;
    let title = format!("rows {start}-{rows_total}");
    let mut text = String::new();
    for (index, row) in rows.iter().enumerate() {
        if markdown {
            text.push('|');
            for cell in row {
                text.push(' ');
                text.push_str(&cell.replace('|', "\\|").replace(['\r', '\n'], " "));
                text.push_str(" |");
            }
            text.push('\n');
            if index == 0 {
                text.push('|');
                for _ in row {
                    text.push_str(" --- |");
                }
                text.push('\n');
            }
        } else {
            text.push_str(&row.join("\t"));
            text.push('\n');
        }
    }
    text.truncate(text.trim_end_matches('\n').len());
    let truncated = truncated_at_row.is_some_and(|row| rows_total >= row);
    parts.push(RawPart {
        kind: DocumentPartKind::TableBlock,
        title: Some(title),
        text,
        truncated,
    });
    rows.clear();
    Ok(())
}

fn extract_epub(body: &[u8]) -> Result<Vec<RawPart>, FetchError> {
    preflight_zip(body)?;
    let container = xml_tokens(&read_zip_entry(body, "META-INF/container.xml")?)?;
    let opf_path = container
        .iter()
        .find(|token| token.name == "rootfile")
        .and_then(|token| token.attributes.get("full-path"))
        .ok_or_else(|| FetchError::DocumentMalformed("EPUB package path missing".to_string()))?;
    let opf = xml_tokens(&read_zip_entry(body, opf_path)?)?;
    let mut manifest = HashMap::new();
    let mut spine = Vec::new();
    for token in &opf {
        if token.name == "item" && matches!(token.kind, TokenKind::Start | TokenKind::Empty) {
            if let (Some(id), Some(href)) =
                (token.attributes.get("id"), token.attributes.get("href"))
            {
                manifest.insert(id.clone(), href.clone());
            }
        }
        if token.name == "itemref" && matches!(token.kind, TokenKind::Start | TokenKind::Empty) {
            if let Some(id) = token.attributes.get("idref") {
                spine.push(id.clone());
            }
        }
    }
    if spine.is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    if spine.len() > MAX_PARTS {
        return Err(FetchError::DocumentMalformed(
            "part count bound exceeded".to_string(),
        ));
    }
    let base = opf_path.rsplit_once('/').map_or("", |(base, _)| base);
    let mut parts = Vec::with_capacity(spine.len());
    for id in spine {
        let href = manifest
            .get(&id)
            .ok_or_else(|| FetchError::DocumentMalformed("EPUB spine item missing".to_string()))?;
        let target = href.split('#').next().unwrap_or(href);
        let path = resolve_archive_path(base, target)?;
        let tokens = xml_tokens(&read_zip_entry(body, &path)?)?;
        let (title, text, truncated) = extract_xhtml_text(&tokens);
        parts.push(RawPart {
            kind: DocumentPartKind::SpineItem,
            title: title.or(Some(id)),
            text,
            truncated,
        });
    }
    if parts.iter().all(|part| part.text.trim().is_empty()) {
        return Err(FetchError::DocumentEmpty);
    }
    Ok(parts)
}

fn extract_xhtml_text(tokens: &[XmlToken]) -> (Option<String>, String, bool) {
    let mut stack: Vec<&str> = Vec::new();
    let mut text = String::new();
    let mut text_chars = 0usize;
    let mut heading = String::new();
    let mut heading_chars = 0usize;
    let mut heading_active = false;
    let mut title = None;
    let mut truncated = tokens.iter().any(|token| token.truncated);
    for token in tokens {
        match token.kind {
            TokenKind::Start => {
                stack.push(&token.name);
                if matches!(token.name.as_str(), "h1" | "h2") {
                    heading_active = true;
                    heading.clear();
                    heading_chars = 0;
                }
            }
            TokenKind::Text => {
                let in_body = stack.contains(&"body");
                let hidden = stack
                    .iter()
                    .any(|name| matches!(*name, "script" | "style" | "head"));
                if in_body && !hidden {
                    append_spaced(
                        &mut text,
                        &mut text_chars,
                        &token.text,
                        MAX_DOCUMENT_CHARS,
                        &mut truncated,
                    );
                }
                if heading_active {
                    append_spaced(
                        &mut heading,
                        &mut heading_chars,
                        &token.text,
                        4096,
                        &mut truncated,
                    );
                }
            }
            TokenKind::End => {
                if matches!(token.name.as_str(), "h1" | "h2") && heading_active {
                    if title.is_none() && !heading.trim().is_empty() {
                        title = Some(heading.trim().to_string());
                    }
                    heading_active = false;
                    append_limited(
                        &mut text,
                        &mut text_chars,
                        "\n",
                        MAX_DOCUMENT_CHARS,
                        &mut truncated,
                    );
                } else if matches!(token.name.as_str(), "p" | "li") {
                    append_limited(
                        &mut text,
                        &mut text_chars,
                        "\n",
                        MAX_DOCUMENT_CHARS,
                        &mut truncated,
                    );
                }
                stack.pop();
            }
            TokenKind::Empty | TokenKind::Truncated => {}
        }
    }
    (title, text.trim().to_string(), truncated)
}

fn extract_docx(body: &[u8], markdown: bool) -> Result<Vec<RawPart>, FetchError> {
    preflight_zip(body)?;
    let tokens = xml_tokens(&read_zip_entry(body, "word/document.xml")?)?;
    let mut parts = Vec::new();
    let mut paragraph = String::new();
    let mut paragraph_chars = 0usize;
    let mut style = String::new();
    let mut in_paragraph = false;
    let mut in_text = false;
    let mut section_truncated = false;
    let mut section_title: Option<String> = None;
    let mut section_text = String::new();
    let mut section_chars = 0usize;
    for token in &tokens {
        section_truncated |= token.truncated;
        match token.kind {
            TokenKind::Start if token.name == "p" => {
                in_paragraph = true;
                paragraph.clear();
                paragraph_chars = 0;
                style.clear();
            }
            TokenKind::Start | TokenKind::Empty if token.name == "pstyle" && in_paragraph => {
                if let Some(value) = attribute(&token.attributes, "w:val", "val") {
                    style = value.clone();
                }
            }
            TokenKind::Start if token.name == "t" && in_paragraph => in_text = true,
            TokenKind::Text if in_text && in_paragraph => {
                append_limited(
                    &mut paragraph,
                    &mut paragraph_chars,
                    &token.text,
                    MAX_DOCUMENT_CHARS,
                    &mut section_truncated,
                );
            }
            TokenKind::Empty if in_paragraph && token.name == "tab" => {
                append_limited(
                    &mut paragraph,
                    &mut paragraph_chars,
                    "\t",
                    MAX_DOCUMENT_CHARS,
                    &mut section_truncated,
                );
            }
            TokenKind::Empty if in_paragraph && token.name == "br" => {
                append_limited(
                    &mut paragraph,
                    &mut paragraph_chars,
                    "\n",
                    MAX_DOCUMENT_CHARS,
                    &mut section_truncated,
                );
            }
            TokenKind::End if token.name == "t" => in_text = false,
            TokenKind::End if token.name == "p" && in_paragraph => {
                let value = paragraph.trim();
                if !value.is_empty() {
                    if let Some(level) = heading_level(&style) {
                        if !section_text.trim().is_empty() || section_title.is_some() {
                            parts.push(RawPart {
                                kind: DocumentPartKind::Section,
                                title: section_title.take(),
                                text: std::mem::take(&mut section_text),
                                truncated: section_truncated,
                            });
                            section_truncated = false;
                            section_chars = 0;
                        }
                        section_title = Some(value.to_string());
                        let heading = if markdown {
                            format!("{} {}", "#".repeat(level), value)
                        } else {
                            value.to_string()
                        };
                        append_limited(
                            &mut section_text,
                            &mut section_chars,
                            &heading,
                            MAX_DOCUMENT_CHARS,
                            &mut section_truncated,
                        );
                    } else {
                        if !section_text.is_empty() {
                            append_limited(
                                &mut section_text,
                                &mut section_chars,
                                "\n",
                                MAX_DOCUMENT_CHARS,
                                &mut section_truncated,
                            );
                        }
                        append_limited(
                            &mut section_text,
                            &mut section_chars,
                            value,
                            MAX_DOCUMENT_CHARS,
                            &mut section_truncated,
                        );
                    }
                }
                in_paragraph = false;
            }
            _ => {}
        }
    }
    if !section_text.trim().is_empty() || section_title.is_some() {
        parts.push(RawPart {
            kind: DocumentPartKind::Section,
            title: section_title,
            text: section_text,
            truncated: section_truncated,
        });
    }
    if parts.is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    if parts.len() > MAX_PARTS {
        return Err(FetchError::DocumentMalformed(
            "part count bound exceeded".to_string(),
        ));
    }
    Ok(parts)
}

fn heading_level(style: &str) -> Option<usize> {
    let lower = style.to_ascii_lowercase();
    if lower == "title" || lower == "subtitle" {
        return Some(1);
    }
    let suffix = lower.strip_prefix("heading")?;
    let level = suffix.parse::<usize>().ok()?;
    (1..=6).contains(&level).then_some(level)
}

fn extract_xlsx(body: &[u8], markdown: bool) -> Result<Vec<RawPart>, FetchError> {
    preflight_zip(body)?;
    check_xlsx_sheet_sizes(body)?;
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(body.to_vec()))
        .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    if sheet_names.len() > MAX_PARTS {
        return Err(FetchError::DocumentMalformed(
            "part count bound exceeded".to_string(),
        ));
    }
    let mut parts = Vec::with_capacity(sheet_names.len());
    let mut extracted_chars = 0usize;
    for sheet_name in sheet_names {
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
        let mut text = String::new();
        let mut text_chars = 0usize;
        let mut sheet_truncated = false;
        for row in range.rows() {
            if text_chars > 0 {
                append_limited(
                    &mut text,
                    &mut text_chars,
                    "\n",
                    MAX_DOCUMENT_CHARS.saturating_sub(extracted_chars),
                    &mut sheet_truncated,
                );
            }
            for (index, cell) in row.iter().enumerate() {
                if index > 0 {
                    let separator = if markdown { " | " } else { "\t" };
                    append_limited(
                        &mut text,
                        &mut text_chars,
                        separator,
                        MAX_DOCUMENT_CHARS.saturating_sub(extracted_chars),
                        &mut sheet_truncated,
                    );
                }
                let value = format_cell(cell);
                append_limited(
                    &mut text,
                    &mut text_chars,
                    &value,
                    MAX_DOCUMENT_CHARS.saturating_sub(extracted_chars),
                    &mut sheet_truncated,
                );
            }
        }
        extracted_chars = extracted_chars.saturating_add(text_chars);
        parts.push(RawPart {
            kind: DocumentPartKind::Sheet,
            title: Some(sheet_name),
            text,
            truncated: sheet_truncated,
        });
    }
    if parts.iter().all(|part| part.text.trim().is_empty()) {
        return Err(FetchError::DocumentEmpty);
    }
    Ok(parts)
}

fn format_cell(cell: &Data) -> String {
    let value = match cell {
        Data::DateTimeIso(value) | Data::DurationIso(value) => value.clone(),
        Data::DateTime(value) => value
            .as_datetime()
            .map(|date| date.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
            .unwrap_or_else(|| value.to_string()),
        _ => cell.to_string(),
    };
    value.replace(['\r', '\n'], " ")
}

fn check_xlsx_sheet_sizes(body: &[u8]) -> Result<(), FetchError> {
    let mut archive = ZipArchive::new(Cursor::new(body)).map_err(map_zip_error)?;
    let mut sheet_paths = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(map_zip_error)?;
        let name = entry.name().map_err(map_zip_error)?.into_owned();
        if name.starts_with("xl/worksheets/") && name.ends_with(".xml") {
            sheet_paths.push(name);
        }
    }
    let mut cells = 0usize;
    for path in sheet_paths {
        let sheet = read_zip_entry(body, &path)?;
        let xml = std::str::from_utf8(&sheet)
            .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
        let mut reader = quick_xml::Reader::from_str(xml);
        reader.config_mut().trim_text(false);
        let mut min_column = usize::MAX;
        let mut max_column = 0usize;
        let mut min_row = usize::MAX;
        let mut max_row = 0usize;
        loop {
            let event = reader
                .read_event()
                .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
            match event {
                Event::Start(element) | Event::Empty(element) => {
                    if element.local_name().as_ref() != b"c" {
                        continue;
                    }
                    cells += 1;
                    if cells > MAX_CELL_COUNT {
                        return Err(cell_bound_exceeded());
                    }
                    for attribute in element.attributes().with_checks(false) {
                        let attribute = attribute
                            .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
                        if attribute.key.local_name().as_ref() != b"r" {
                            continue;
                        }
                        let reference = attribute
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
                        if reference.chars().count() > 4096 {
                            return Err(FetchError::DocumentMalformed(
                                "XML attribute bound exceeded".to_string(),
                            ));
                        }
                        if let Some((column, row)) = cell_position(&reference) {
                            min_column = min_column.min(column);
                            max_column = max_column.max(column);
                            min_row = min_row.min(row);
                            max_row = max_row.max(row);
                            let columns = max_column.saturating_sub(min_column).saturating_add(1);
                            let rows = max_row.saturating_sub(min_row).saturating_add(1);
                            if columns.saturating_mul(rows) > MAX_CELL_COUNT {
                                return Err(cell_bound_exceeded());
                            }
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
    Ok(())
}

fn cell_bound_exceeded() -> FetchError {
    FetchError::DocumentMalformed("spreadsheet cell bound exceeded".to_string())
}

fn cell_position(reference: &str) -> Option<(usize, usize)> {
    let mut column = 0usize;
    let mut row = 0usize;
    let mut in_row = false;
    for character in reference.chars() {
        match character {
            '$' => {}
            'A'..='Z' | 'a'..='z' if !in_row => {
                column = column
                    .checked_mul(26)?
                    .checked_add(character.to_ascii_uppercase() as usize - 'A' as usize + 1)?;
            }
            '0'..='9' => {
                in_row = true;
                row = row
                    .checked_mul(10)?
                    .checked_add(character as usize - '0' as usize)?;
            }
            _ => return None,
        }
    }
    (column > 0 && row > 0).then_some((column, row))
}

fn extract_pptx(body: &[u8]) -> Result<Vec<RawPart>, FetchError> {
    preflight_zip(body)?;
    let presentation = xml_tokens(&read_zip_entry(body, "ppt/presentation.xml")?)?;
    let relationships = xml_tokens(&read_zip_entry(body, "ppt/_rels/presentation.xml.rels")?)?;
    let mut ordered_ids = Vec::new();
    for token in &presentation {
        if token.name == "sldid" && matches!(token.kind, TokenKind::Start | TokenKind::Empty) {
            if let Some(id) = token.attributes.get("r:id") {
                ordered_ids.push(id.clone());
            }
        }
    }
    if ordered_ids.is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    if ordered_ids.len() > MAX_PARTS {
        return Err(FetchError::DocumentMalformed(
            "part count bound exceeded".to_string(),
        ));
    }
    let mut targets = HashMap::new();
    for token in &relationships {
        if token.name == "relationship" && matches!(token.kind, TokenKind::Start | TokenKind::Empty)
        {
            if token
                .attributes
                .get("targetmode")
                .is_some_and(|value| value.eq_ignore_ascii_case("external"))
            {
                continue;
            }
            if let (Some(id), Some(target)) =
                (token.attributes.get("id"), token.attributes.get("target"))
            {
                targets.insert(id.clone(), target.clone());
            }
        }
    }
    let mut parts = Vec::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        let target = targets.get(&id).ok_or_else(|| {
            FetchError::DocumentMalformed("PPTX slide relationship missing".to_string())
        })?;
        let path = resolve_archive_path("ppt", target)?;
        let tokens = xml_tokens(&read_zip_entry(body, &path)?)?;
        let (title, text, truncated) = extract_slide_text(&tokens);
        parts.push(RawPart {
            kind: DocumentPartKind::Slide,
            title,
            text,
            truncated,
        });
    }
    if parts.iter().all(|part| part.text.trim().is_empty()) {
        return Err(FetchError::DocumentEmpty);
    }
    Ok(parts)
}

fn extract_slide_text(tokens: &[XmlToken]) -> (Option<String>, String, bool) {
    let mut in_shape = false;
    let mut shape_is_title = false;
    let mut shape_text = String::new();
    let mut shape_chars = 0usize;
    let mut in_paragraph = false;
    let mut in_text = false;
    let mut paragraph = String::new();
    let mut paragraph_chars = 0usize;
    let mut title = None;
    let mut text = String::new();
    let mut text_chars = 0usize;
    let mut truncated = tokens.iter().any(|token| token.truncated);
    for token in tokens {
        match token.kind {
            TokenKind::Start if token.name == "sp" => {
                in_shape = true;
                shape_is_title = false;
                shape_text.clear();
                shape_chars = 0;
            }
            TokenKind::Start | TokenKind::Empty if token.name == "ph" && in_shape => {
                shape_is_title = token.attributes.get("type").is_some_and(|value| {
                    matches!(value.to_ascii_lowercase().as_str(), "title" | "ctrtitle")
                });
            }
            TokenKind::Start if token.name == "p" && in_shape => {
                in_paragraph = true;
                paragraph.clear();
                paragraph_chars = 0;
            }
            TokenKind::Start if token.name == "t" && in_paragraph => in_text = true,
            TokenKind::Text if in_text => {
                append_limited(
                    &mut paragraph,
                    &mut paragraph_chars,
                    &token.text,
                    MAX_DOCUMENT_CHARS,
                    &mut truncated,
                );
            }
            TokenKind::End if token.name == "t" => in_text = false,
            TokenKind::End if token.name == "p" && in_paragraph => {
                if !paragraph.trim().is_empty() {
                    append_spaced(
                        &mut text,
                        &mut text_chars,
                        &paragraph,
                        MAX_DOCUMENT_CHARS,
                        &mut truncated,
                    );
                    append_limited(
                        &mut text,
                        &mut text_chars,
                        "\n",
                        MAX_DOCUMENT_CHARS,
                        &mut truncated,
                    );
                    append_spaced(
                        &mut shape_text,
                        &mut shape_chars,
                        &paragraph,
                        MAX_DOCUMENT_CHARS,
                        &mut truncated,
                    );
                }
                in_paragraph = false;
            }
            TokenKind::End if token.name == "sp" && in_shape => {
                if shape_is_title && title.is_none() && !shape_text.trim().is_empty() {
                    title = Some(shape_text.trim().to_string());
                }
                in_shape = false;
            }
            _ => {}
        }
    }
    (title, text.trim().to_string(), truncated)
}

fn format_from_content_types(content_types: &str) -> Option<DocumentFormat> {
    let lower = content_types.to_ascii_lowercase();
    if lower.contains("wordprocessingml.document.main+xml") {
        Some(DocumentFormat::Docx)
    } else if lower.contains("spreadsheetml.sheet.main+xml") {
        Some(DocumentFormat::Xlsx)
    } else if lower.contains("presentationml.presentation.main+xml") {
        Some(DocumentFormat::Pptx)
    } else {
        None
    }
}

fn read_preview(body: &[u8], name: &str, cap: usize) -> Option<String> {
    let mut archive = ZipArchive::new(Cursor::new(body)).ok()?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return None;
    }
    let entry = archive.by_name(name).ok()?;
    if entry.size() > cap as u64
        || (entry.size() > 0
            && (entry.compressed_size() == 0
                || entry.size() > entry.compressed_size().saturating_mul(MAX_INFLATION_RATIO)))
    {
        return None;
    }
    let mut bytes = Vec::new();
    entry.take(cap as u64 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > cap {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn preflight_zip(body: &[u8]) -> Result<(), FetchError> {
    let mut archive = ZipArchive::new(Cursor::new(body)).map_err(map_zip_error)?;
    if archive.is_empty() {
        return Err(FetchError::DocumentEmpty);
    }
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(FetchError::DocumentMalformed(
            "archive entry count bound exceeded".to_string(),
        ));
    }
    let mut inflated_total = 0u64;
    let mut buffer = [0u8; 8192];
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(map_zip_error)?;
        let entry_name = entry.name().map_err(map_zip_error)?;
        if !safe_entry_name(entry_name.as_ref()) {
            return Err(FetchError::DocumentMalformed(
                "unsafe archive entry path".to_string(),
            ));
        }
        let declared = entry.size();
        let compressed = entry.compressed_size();
        if declared > 0
            && (compressed == 0 || declared > compressed.saturating_mul(MAX_INFLATION_RATIO))
        {
            return Err(FetchError::DocumentMalformed(
                "archive inflation ratio bound exceeded".to_string(),
            ));
        }
        if entry.is_dir() {
            continue;
        }
        let mut inflated_entry = 0u64;
        loop {
            let count = entry.read(&mut buffer).map_err(map_io_error)?;
            if count == 0 {
                break;
            }
            inflated_entry = inflated_entry.saturating_add(count as u64);
            inflated_total = inflated_total.saturating_add(count as u64);
            if inflated_total > MAX_DECOMPRESSED_BYTES {
                return Err(FetchError::DocumentMalformed(
                    "decompressed size bound exceeded".to_string(),
                ));
            }
            if compressed == 0 || inflated_entry > compressed.saturating_mul(MAX_INFLATION_RATIO) {
                return Err(FetchError::DocumentMalformed(
                    "archive inflation ratio bound exceeded".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn safe_entry_name(name: &str) -> bool {
    !name.starts_with('/')
        && !name.starts_with('\\')
        && !name.contains(':')
        && !name.split(['/', '\\']).any(|part| part == "..")
}

fn read_zip_entry(body: &[u8], name: &str) -> Result<Vec<u8>, FetchError> {
    let mut archive = ZipArchive::new(Cursor::new(body)).map_err(map_zip_error)?;
    let entry = archive
        .by_name(name)
        .map_err(|error| map_missing_entry(name, error))?;
    if entry.size() > MAX_DECOMPRESSED_BYTES {
        return Err(FetchError::DocumentMalformed(
            "decompressed size bound exceeded".to_string(),
        ));
    }
    let mut output = Vec::with_capacity(entry.size().min(1024 * 1024) as usize);
    entry
        .take(MAX_DECOMPRESSED_BYTES + 1)
        .read_to_end(&mut output)
        .map_err(map_io_error)?;
    if output.len() as u64 > MAX_DECOMPRESSED_BYTES {
        return Err(FetchError::DocumentMalformed(
            "decompressed size bound exceeded".to_string(),
        ));
    }
    Ok(output)
}

fn map_missing_entry(name: &str, error: zip::result::ZipError) -> FetchError {
    match error {
        zip::result::ZipError::FileNotFound => {
            FetchError::DocumentMalformed(format!("required archive entry missing: {name}"))
        }
        other => map_zip_error(other),
    }
}

fn map_zip_error(error: zip::result::ZipError) -> FetchError {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("encrypt") {
        FetchError::DocumentEncrypted
    } else {
        FetchError::DocumentMalformed(message)
    }
}

fn map_io_error(error: std::io::Error) -> FetchError {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("encrypt") {
        FetchError::DocumentEncrypted
    } else {
        FetchError::DocumentMalformed(message)
    }
}

fn resolve_archive_path(base: &str, target: &str) -> Result<String, FetchError> {
    if target.starts_with('/') || target.contains('\\') || target.contains(':') {
        return Err(FetchError::DocumentMalformed(
            "unsafe package relationship target".to_string(),
        ));
    }
    let mut components: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for component in target.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(FetchError::DocumentMalformed(
                        "unsafe package relationship target".to_string(),
                    ));
                }
            }
            value => components.push(value),
        }
    }
    Ok(components.join("/"))
}

fn xml_tokens(bytes: &[u8]) -> Result<Vec<XmlToken>, FetchError> {
    let xml = std::str::from_utf8(bytes)
        .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut tokens: Vec<XmlToken> = Vec::new();
    let mut depth = 0usize;
    let mut total_text = 0usize;
    let mut truncated = false;
    loop {
        let token = match reader.read_event() {
            Ok(Event::Start(event)) => {
                depth += 1;
                if depth > 64 {
                    return Err(FetchError::DocumentMalformed(
                        "XML depth bound exceeded".to_string(),
                    ));
                }
                Some(XmlToken {
                    kind: TokenKind::Start,
                    name: local_name(event.local_name().as_ref()),
                    attributes: parse_attributes(&event)?,
                    text: String::new(),
                    truncated: false,
                })
            }
            Ok(Event::Empty(event)) => Some(XmlToken {
                kind: TokenKind::Empty,
                name: local_name(event.local_name().as_ref()),
                attributes: parse_attributes(&event)?,
                text: String::new(),
                truncated: false,
            }),
            Ok(Event::End(event)) => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    FetchError::DocumentMalformed("unbalanced XML end tag".to_string())
                })?;
                Some(XmlToken {
                    kind: TokenKind::End,
                    name: local_name(event.local_name().as_ref()),
                    attributes: HashMap::new(),
                    text: String::new(),
                    truncated: false,
                })
            }
            Ok(Event::Text(event)) => {
                let decoded = event
                    .decode()
                    .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
                let value = quick_xml::escape::unescape(&decoded)
                    .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
                text_token(&mut tokens, &value, &mut total_text, &mut truncated)
            }
            Ok(Event::CData(event)) => {
                let value = event
                    .decode()
                    .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
                text_token(&mut tokens, &value, &mut total_text, &mut truncated)
            }
            Ok(Event::DocType(event)) => {
                let body = event
                    .decode()
                    .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
                let upper = body.to_ascii_uppercase();
                if body.contains('[') || upper.contains("SYSTEM") || upper.contains("PUBLIC") {
                    return Err(FetchError::DocumentMalformed(
                        "DTD is not supported".to_string(),
                    ));
                }
                None
            }
            Ok(Event::GeneralRef(event)) => {
                let value = decode_general_ref(&event)?;
                text_token(&mut tokens, &value, &mut total_text, &mut truncated)
            }
            Ok(Event::Eof) => break,
            Ok(_) => None,
            Err(error) => return Err(FetchError::DocumentMalformed(error.to_string())),
        };
        if let Some(token) = token {
            tokens.push(token);
            if tokens.len() > MAX_XML_TOKENS {
                return Err(FetchError::DocumentMalformed(
                    "XML token bound exceeded".to_string(),
                ));
            }
        }
    }
    if depth != 0 {
        return Err(FetchError::DocumentMalformed(
            "unclosed XML element".to_string(),
        ));
    }
    if truncated {
        tokens.push(XmlToken {
            kind: TokenKind::Truncated,
            name: String::new(),
            attributes: HashMap::new(),
            text: String::new(),
            truncated: true,
        });
    }
    Ok(tokens)
}

fn text_token(
    tokens: &mut [XmlToken],
    value: &str,
    total_text: &mut usize,
    truncated: &mut bool,
) -> Option<XmlToken> {
    let remaining = MAX_DOCUMENT_CHARS.saturating_sub(*total_text);
    let text: String = value.chars().take(remaining).collect();
    let cut = text.chars().count() < value.chars().count();
    *total_text += text.chars().count();
    *truncated |= cut;
    if let Some(previous) = tokens.last_mut() {
        if previous.kind == TokenKind::Text {
            previous.text.push_str(&text);
            previous.truncated |= cut;
            return None;
        }
    }
    Some(XmlToken {
        kind: TokenKind::Text,
        name: String::new(),
        attributes: HashMap::new(),
        text,
        truncated: cut,
    })
}

fn parse_attributes(
    event: &quick_xml::events::BytesStart<'_>,
) -> Result<HashMap<String, String>, FetchError> {
    let mut attributes = HashMap::new();
    for attribute in event.attributes().with_checks(false) {
        let attribute =
            attribute.map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
        let name = String::from_utf8_lossy(attribute.key.as_ref()).to_ascii_lowercase();
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
        if value.chars().count() > 4096 {
            return Err(FetchError::DocumentMalformed(
                "XML attribute bound exceeded".to_string(),
            ));
        }
        attributes.insert(name, value.into_owned());
    }
    Ok(attributes)
}

fn local_name(value: &[u8]) -> String {
    String::from_utf8_lossy(value).to_ascii_lowercase()
}

fn attribute<'a>(
    attributes: &'a HashMap<String, String>,
    qualified: &str,
    local: &str,
) -> Option<&'a String> {
    attributes.get(qualified).or_else(|| attributes.get(local))
}

fn decode_general_ref(event: &quick_xml::events::BytesRef<'_>) -> Result<String, FetchError> {
    let name = event
        .decode()
        .map_err(|error| FetchError::DocumentMalformed(error.to_string()))?;
    let decoded = match &*name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "apos" => Some('\''),
        "quot" => Some('"'),
        _ => None,
    };
    if let Some(character) = decoded {
        return Ok(character.to_string());
    }
    match event.resolve_char_ref() {
        Ok(Some(character)) => Ok(character.to_string()),
        Ok(None) => Ok(format!("&{name};")),
        Err(error) => Err(FetchError::DocumentMalformed(error.to_string())),
    }
}

fn append_spaced(
    target: &mut String,
    target_chars: &mut usize,
    value: &str,
    limit: usize,
    truncated: &mut bool,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if *target_chars > 0 && !target.ends_with([' ', '\n', '\t']) {
        append_limited(target, target_chars, " ", limit, truncated);
    }
    append_limited(target, target_chars, value, limit, truncated);
}

fn append_limited(
    target: &mut String,
    target_chars: &mut usize,
    value: &str,
    limit: usize,
    truncated: &mut bool,
) {
    let remaining = limit.saturating_sub(*target_chars);
    let mut appended = 0usize;
    for character in value.chars() {
        if appended >= remaining {
            *truncated = true;
            break;
        }
        target.push(character);
        appended += 1;
    }
    *target_chars += appended;
    if appended < value.chars().count() {
        *truncated = true;
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
