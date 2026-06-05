//! Local embedded search index for eggsearch.
//!
//! Uses Tantivy as the default backend. The local index MUST NOT perform
//! any network access. All inputs come from local files, local corpora,
//! or previously fetched artifacts.

pub mod corpus;
pub mod ingest;
pub mod schema;
pub mod tantivy_backend;

pub use corpus::LocalCorpus;
pub use ingest::{ingest_path, IngestOptions};
pub use schema::IndexedDocument;
pub use tantivy_backend::TantivyIndex;
