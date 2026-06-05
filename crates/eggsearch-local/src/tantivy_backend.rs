//! Tantivy-backed implementation of the local index.

use eggsearch_core::result::TrustLevel;
use eggsearch_core::source_card::SourceCard;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};
use tracing::warn;
use uuid::Uuid;

use crate::schema::IndexedDocument;

/// Tantivy index wrapper. The index is thread-safe and supports concurrent
/// reads; writes go through an `IndexWriter` acquired per call.
pub struct TantivyIndex {
    schema: Schema,
    index: Index,
    reader: IndexReader,
    // Field handles
    f_id: Field,
    f_title: Field,
    f_body: Field,
    f_url: Field,
    f_path: Field,
    f_source_kind: Field,
    f_trust_level: Field,
    f_fetched_at: Field,
    f_published_at: Field,
    f_content_hash: Field,
    f_tags: Field,
}

#[allow(dead_code)]
const COMMIT_THRESHOLD: usize = 100;

impl TantivyIndex {
    pub fn open_or_create(path: impl AsRef<Path>) -> tantivy::Result<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).ok();
        let (schema, fields) = build_schema();
        let dir = tantivy::directory::MmapDirectory::open(path)?;
        let exists = Index::exists(&dir)?;
        let index = if exists {
            Index::open_in_dir(path)?
        } else {
            Index::create_in_dir(path, schema.clone())?
        };
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        Ok(Self {
            f_id: fields.id,
            f_title: fields.title,
            f_body: fields.body,
            f_url: fields.url,
            f_path: fields.path,
            f_source_kind: fields.source_kind,
            f_trust_level: fields.trust_level,
            f_fetched_at: fields.fetched_at,
            f_published_at: fields.published_at,
            f_content_hash: fields.content_hash,
            f_tags: fields.tags,
            schema,
            index,
            reader,
        })
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn writer(&self) -> tantivy::Result<IndexWriter> {
        self.index.writer(50_000_000)
    }

    /// Add or replace a single document. Commits every N writes to amortize cost.
    pub fn upsert(&self, doc: &IndexedDocument) -> tantivy::Result<()> {
        self.upsert_many(std::iter::once(doc))
    }

    pub fn upsert_many<'a, I: IntoIterator<Item = &'a IndexedDocument>>(
        &self,
        docs: I,
    ) -> tantivy::Result<()> {
        let mut writer = self.writer()?;
        for doc in docs {
            // delete existing by id
            let id_term = Term::from_field_text(self.f_id, &doc.id);
            writer.delete_term(id_term);
            let mut td = TantivyDocument::default();
            td.add_text(self.f_id, &doc.id);
            td.add_text(self.f_title, &doc.title);
            td.add_text(self.f_body, &doc.body);
            if let Some(u) = &doc.url {
                td.add_text(self.f_url, u);
            }
            if let Some(p) = &doc.path {
                td.add_text(self.f_path, p.display().to_string());
            }
            td.add_text(self.f_source_kind, doc.source_kind.as_str());
            td.add_text(self.f_trust_level, doc.trust_level.as_str());
            if let Some(ts) = doc.fetched_at {
                td.add_date(self.f_fetched_at, tantivy::DateTime::from_timestamp_secs(ts.timestamp()));
            }
            if let Some(ts) = doc.published_at {
                td.add_date(self.f_published_at, tantivy::DateTime::from_timestamp_secs(ts.timestamp()));
            }
            td.add_text(self.f_content_hash, &doc.content_hash);
            for tag in &doc.tags {
                td.add_text(self.f_tags, tag);
            }
            writer.add_document(td)?;
        }
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// Delete a document by id. Returns true if a term was deleted.
    pub fn delete(&self, id: &str) -> tantivy::Result<bool> {
        let mut writer = self.writer()?;
        let term = Term::from_field_text(self.f_id, id);
        writer.delete_term(term);
        writer.commit()?;
        self.reader.reload()?;
        Ok(true)
    }

    /// Search the local index. `tags`, if non-empty, restricts to documents
    /// containing all listed tags.
    pub fn search(
        &self,
        query: &str,
        max_results: usize,
        tags: &[String],
    ) -> tantivy::Result<Vec<SourceCard>> {
        let searcher = self.reader.searcher();
        let qp = QueryParser::for_index(
            &self.index,
            vec![self.f_title, self.f_body],
        );
        let parsed = match qp.parse_query(query) {
            Ok(q) => q,
            Err(e) => {
                warn!("tantivy parse failed for query '{query}': {e}");
                return Ok(Vec::new());
            }
        };

        let top = searcher.search(&parsed, &TopDocs::with_limit(max_results.max(1)))?;
        let mut out = Vec::new();
        for (_score, addr) in top {
            let doc: TantivyDocument = match searcher.doc(addr) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let id = get_text(&doc, self.f_id).unwrap_or_default();
            let title = get_text(&doc, self.f_title).unwrap_or_else(|| "(untitled)".to_string());
            let url = get_text(&doc, self.f_url);
            let path = get_text(&doc, self.f_path);
            let snippet_src = get_text(&doc, self.f_body).unwrap_or_default();
            let snippet = make_excerpt(&snippet_src, 280);
            let provider_id = "local".to_string();
            let trust_str = get_text(&doc, self.f_trust_level).unwrap_or_default();
            let trust = trust_from_str(&trust_str);
            let source_str = get_text(&doc, self.f_source_kind).unwrap_or_default();
            let kind = kind_from_str(&source_str);

            // Tag filtering (post-filter is fine for MVP scale).
            if !tags.is_empty() {
                let doc_tags: Vec<String> = doc
                    .get_all(self.f_tags)
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if !tags.iter().all(|t| doc_tags.contains(t)) {
                    continue;
                }
            }

            // Build a SourceCard directly.
            out.push(SourceCard {
                id: format!("src_{}", Uuid::new_v4().simple()),
                title,
                url,
                path,
                snippet: Some(snippet),
                provider_id,
                source_kind: kind,
                trust_level: trust,
                published_at: None,
                fetched_at: None,
                artifact_id: None,
                score: Some(_score),
                warnings: Vec::new(),
            });
            let _ = id; // not exposed in card; used only for dedup later
        }
        Ok(out)
    }

    pub fn count(&self) -> tantivy::Result<u64> {
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs())
    }
}

fn get_text(doc: &TantivyDocument, field: Field) -> Option<String> {
    doc.get_first(field)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn make_excerpt(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = String::new();
    for (i, c) in trimmed.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

fn trust_from_str(s: &str) -> TrustLevel {
    match s {
        "local_trusted" => TrustLevel::LocalTrusted,
        "local_cached_external" => TrustLevel::LocalCachedExternal,
        _ => TrustLevel::ExternalUntrusted,
    }
}

fn kind_from_str(s: &str) -> eggsearch_core::result::SourceKind {
    use eggsearch_core::result::SourceKind as K;
    match s {
        "web" => K::Web,
        "documentation" => K::Documentation,
        "package_registry" => K::PackageRegistry,
        "reference" => K::Reference,
        "news" => K::News,
        "local_file" => K::LocalFile,
        "local_artifact" => K::LocalArtifact,
        _ => K::Unknown,
    }
}

struct Fields {
    id: Field,
    title: Field,
    body: Field,
    url: Field,
    path: Field,
    source_kind: Field,
    trust_level: Field,
    fetched_at: Field,
    published_at: Field,
    content_hash: Field,
    tags: Field,
}

fn build_schema() -> (Schema, Fields) {
    let mut schema_builder = Schema::builder();
    let text_opts = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("default")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    let id = schema_builder.add_text_field("id", text_opts.clone());
    let title = schema_builder.add_text_field("title", text_opts.clone());
    let body = schema_builder.add_text_field("body", text_opts.clone());
    let url = schema_builder.add_text_field("url", TextOptions::default().set_stored());
    let path = schema_builder.add_text_field("path", TextOptions::default().set_stored());
    let source_kind = schema_builder.add_text_field("source_kind", TextOptions::default().set_stored());
    let trust_level = schema_builder.add_text_field("trust_level", TextOptions::default().set_stored());
    let fetched_at = schema_builder.add_date_field("fetched_at", tantivy::schema::DateOptions::default().set_stored().set_indexed());
    let published_at = schema_builder.add_date_field("published_at", tantivy::schema::DateOptions::default().set_stored().set_indexed());
    let content_hash = schema_builder.add_text_field("content_hash", TextOptions::default().set_stored());
    let tags = schema_builder.add_text_field("tags", TextOptions::default().set_stored());
    let schema = schema_builder.build();
    let fields = Fields {
        id,
        title,
        body,
        url,
        path,
        source_kind,
        trust_level,
        fetched_at,
        published_at,
        content_hash,
        tags,
    };
    (schema, fields)
}

impl std::fmt::Debug for TantivyIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TantivyIndex")
            .field("schema", &"<tantivy schema>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::IndexedDocument;
    use tempfile::tempdir;

    fn doc(id: &str, title: &str, body: &str) -> IndexedDocument {
        IndexedDocument {
            id: id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            url: None,
            path: None,
            source_kind: eggsearch_core::result::SourceKind::LocalFile,
            trust_level: eggsearch_core::result::TrustLevel::LocalTrusted,
            fetched_at: None,
            published_at: None,
            content_hash: id.to_string(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn end_to_end_index_and_search() {
        let dir = tempdir().unwrap();
        let index = TantivyIndex::open_or_create(dir.path()).unwrap();
        index
            .upsert_many(&[
                doc("1", "Axum middleware", "tower-http middleware and utilities"),
                doc("2", "Tokio runtime", "asynchronous runtime for rust"),
                doc("3", "Serde JSON", "serialization and deserialization"),
            ])
            .unwrap();
        let results = index.search("middleware", 5, &[]).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].title.to_lowercase().contains("axum"));
    }

    #[test]
    fn tag_filter() {
        let dir = tempdir().unwrap();
        let index = TantivyIndex::open_or_create(dir.path()).unwrap();
        let mut a = doc("1", "alpha", "alpha body");
        a.tags = vec!["rust".into()];
        let mut b = doc("2", "alpha two", "second alpha body");
        b.tags = vec!["python".into()];
        index.upsert_many(&[a, b]).unwrap();
        let only_rust = index.search("alpha", 10, &["rust".to_string()]).unwrap();
        assert_eq!(only_rust.len(), 1);
        assert!(only_rust[0].title == "alpha");
    }
}

