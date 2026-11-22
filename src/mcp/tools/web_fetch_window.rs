//! Address-window and page-chunk projection for `web_fetch`.
//!
//! These helpers reshape an already-extracted document for the caller's window
//! or first page. They hold no state and never perform I/O.

use super::common::{slice_chars, AddressWindow};

pub(crate) struct WindowedFetch {
    pub(crate) text: Option<String>,
    pub(crate) document: Option<crate::core::document::FetchDocument>,
    pub(crate) window: Option<AddressWindow>,
    pub(crate) doc_id: Option<String>,
}

fn addressable_text(document: &crate::core::document::FetchDocument) -> String {
    document
        .chunks
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn default_page_chunk_count(
    document: &crate::core::document::FetchDocument,
    capacity: usize,
) -> usize {
    let mut used = 0usize;
    let mut count = 0usize;
    for chunk in &document.chunks {
        let unit = chunk.text.chars().count().saturating_add(1);
        let next = used.saturating_add(unit);
        if count > 0 && next > capacity {
            break;
        }
        used = next;
        count += 1;
    }
    count
}

fn windowed_chunks(
    document: &crate::core::document::FetchDocument,
    window: AddressWindow,
) -> Vec<crate::core::document::DocumentChunk> {
    if window.len() == 0 {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut position = 0usize;
    for chunk in &document.chunks {
        let len = chunk.text.chars().count();
        let start = position;
        let end = start.saturating_add(len);
        if start < window.to && end > window.from {
            chunks.push(chunk.clone());
        }
        position = end.saturating_add(1);
    }
    chunks
}

pub(crate) fn windowed_fetch(
    resp: &crate::core::fetch::WebFetchResponse,
    capacity: usize,
    addressed: bool,
    offset: Option<usize>,
    sanitize_output: bool,
) -> WindowedFetch {
    let unchanged = || WindowedFetch {
        text: None,
        document: None,
        window: None,
        doc_id: None,
    };
    let Some(document) = resp.document.as_ref() else {
        return unchanged();
    };
    if document.chunks.is_empty() {
        return unchanged();
    }
    if !addressed {
        let count = default_page_chunk_count(document, capacity);
        if count == document.chunks.len() {
            return unchanged();
        }
        let page_chunks = document.chunks[..count].to_vec();
        let page = page_chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let page_chars = page.chars().count();
        let (stripped, _) = crate::core::sanitize::strip_control_chars(&page);
        let text = if sanitize_output {
            crate::core::sanitize::frame(&stripped, "text", &resp.final_url)
        } else {
            stripped
        };
        let mut paged_document = document.clone();
        paged_document.chunks = page_chunks;
        paged_document.text_chars_returned = page_chars;
        paged_document.text_truncated = count < document.chunks.len();
        return WindowedFetch {
            text: Some(text),
            document: Some(paged_document),
            window: None,
            doc_id: None,
        };
    }
    let joined = addressable_text(document);
    let total = joined.chars().count();
    let window = AddressWindow::resolve(total, offset, capacity);
    let slice = slice_chars(&joined, window.from, window.to);
    let (stripped, _) = crate::core::sanitize::strip_control_chars(slice);
    let text = if sanitize_output && !stripped.is_empty() {
        crate::core::sanitize::frame(&stripped, "text", &resp.final_url)
    } else {
        stripped
    };
    let mut paged_document = document.clone();
    paged_document.chunks = windowed_chunks(document, window);
    paged_document.text_chars_returned = window.len();
    paged_document.text_truncated = window.next_offset().is_some();
    WindowedFetch {
        text: Some(text),
        document: Some(paged_document),
        window: Some(window),
        doc_id: Some(document.doc_id.clone()),
    }
}
