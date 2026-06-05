//! File ingestion for the local index. Supports local Markdown, plain
//! text, and HTML files. The ingest path is intentionally network-free.

use chrono::Utc;
use sha2::{Digest, Sha256};
use std::path::Path;
use tracing::warn;
use walkdir::WalkDir;

use crate::schema::IndexedDocument;

#[derive(Clone, Debug, Default)]
pub struct IngestOptions {
    /// If true, follow symbolic links. Default: false.
    pub follow_symlinks: bool,
    /// File extensions to include. Empty means: all recognized types.
    pub include_exts: Vec<String>,
    /// Maximum file size in bytes; larger files are skipped.
    pub max_file_bytes: usize,
}

pub fn ingest_path(
    path: impl AsRef<Path>,
    opts: &IngestOptions,
) -> Vec<IndexedDocument> {
    let path = path.as_ref();
    if path.is_file() {
        match ingest_file(path) {
            Ok(d) => vec![d],
            Err(e) => {
                warn!("ingest failed for {}: {e}", path.display());
                vec![]
            }
        }
    } else if path.is_dir() {
        ingest_dir(path, opts)
    } else {
        vec![]
    }
}

fn ingest_dir(dir: &Path, opts: &IngestOptions) -> Vec<IndexedDocument> {
    let mut out = Vec::new();
    let root = dir.to_path_buf();
    let walker = WalkDir::new(dir)
        .follow_links(opts.follow_symlinks)
        .into_iter()
        .filter_entry(move |e| {
            // Always keep the root; for descendants, skip hidden.
            e.path() == root || !is_hidden(e.path())
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        if !ext_allowed(p, &opts.include_exts) {
            continue;
        }
        if let Ok(meta) = std::fs::metadata(p) {
            if opts.max_file_bytes > 0 && meta.len() as usize > opts.max_file_bytes {
                continue;
            }
        }
        match ingest_file(p) {
            Ok(d) => out.push(d),
            Err(e) => warn!("ingest failed for {}: {e}", p.display()),
        }
    }
    out
}

fn ext_allowed(path: &Path, include: &[String]) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };
    if include.is_empty() {
        matches!(ext.as_str(), "md" | "markdown" | "txt" | "html" | "htm" | "rst" | "adoc")
    } else {
        include.iter().any(|e| e.eq_ignore_ascii_case(&ext))
    }
}

fn is_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}

fn ingest_file(path: &Path) -> std::io::Result<IndexedDocument> {
    let bytes = std::fs::read(path)?;
    let text = decode(&bytes, path);
    let hash = hex::encode(Sha256::digest(&bytes));
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("(untitled)")
        .to_string();
    let body = text;
    Ok(IndexedDocument {
        id: format!("local:{hash}"),
        title,
        body,
        url: None,
        path: Some(path.to_path_buf()),
        source_kind: eggsearch_core::result::SourceKind::LocalFile,
        trust_level: eggsearch_core::result::TrustLevel::LocalTrusted,
        fetched_at: Some(Utc::now()),
        published_at: None,
        content_hash: hash,
        tags: Vec::new(),
    })
}

fn decode(bytes: &[u8], path: &Path) -> String {
    // Try UTF-8 first; otherwise lossy convert.
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let _ = path; // reserved for future per-extension decoders
            String::from_utf8_lossy(bytes).to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ingest_markdown_file() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("hello.md");
        std::fs::write(&f, "# Hello\n\nThis is a test.").unwrap();
        let docs = ingest_path(&f, &IngestOptions::default());
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].title, "hello");
        assert!(docs[0].body.contains("Hello"));
    }

    #[test]
    fn ingest_dir_skips_hidden() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "a").unwrap();
        std::fs::create_dir(dir.path().join(".hidden")).unwrap();
        std::fs::write(dir.path().join(".hidden").join("b.md"), "b").unwrap();
        let docs = ingest_path(dir.path(), &IngestOptions::default());
        assert_eq!(docs.len(), 1);
    }
}
