//! Container document extraction beyond PDF (requires the `documents` feature).

mod extract;

pub use extract::{
    detect_container_format, extract_document, unsupported_document_format, DocumentExtraction,
};
