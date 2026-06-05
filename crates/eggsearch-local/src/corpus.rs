//! High-level corpus management: ties together a Tantivy index and a list
//! of indexed documents, providing a single entry point for `local_search`.

use std::path::{Path, PathBuf};

use crate::schema::IndexedDocument;
use crate::tantivy_backend::TantivyIndex;

#[derive(Debug)]
pub struct LocalCorpus {
    pub index: TantivyIndex,
    pub dir: PathBuf,
}

impl LocalCorpus {
    pub fn open_or_create(dir: impl AsRef<Path>) -> tantivy::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let index = TantivyIndex::open_or_create(&dir)?;
        Ok(Self { index, dir })
    }

    /// Add a single document.
    pub fn add(&self, doc: &IndexedDocument) -> tantivy::Result<()> {
        self.index.upsert(doc)
    }

    /// Add many documents.
    pub fn add_many(&self, docs: &[IndexedDocument]) -> tantivy::Result<()> {
        self.index.upsert_many(docs)
    }

    /// Search the corpus.
    pub fn search(
        &self,
        query: &str,
        max_results: usize,
        tags: &[String],
    ) -> tantivy::Result<Vec<eggsearch_core::source_card::SourceCard>> {
        self.index.search(query, max_results, tags)
    }

    pub fn count(&self) -> tantivy::Result<u64> {
        self.index.count()
    }
}
