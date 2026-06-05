//! Simple in-memory + on-disk fetch cache.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::fetch::FetchedDocument;

#[derive(Debug)]
struct Entry {
    doc: FetchedDocument,
    inserted_at: Instant,
}

#[derive(Clone, Debug)]
pub struct FetchCache {
    inner: Arc<Mutex<Vec<(String, Entry)>>>,
    max_entries: usize,
    ttl: Duration,
    disk_path: Option<PathBuf>,
}

impl Default for FetchCache {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
            max_entries: 128,
            ttl: Duration::from_secs(60 * 30),
            disk_path: None,
        }
    }
}

impl FetchCache {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
            max_entries,
            ttl,
            disk_path: None,
        }
    }

    pub fn with_disk_path(mut self, p: impl Into<PathBuf>) -> Self {
        self.disk_path = Some(p.into());
        self
    }

    pub async fn get(&self, key: &str) -> Option<FetchedDocument> {
        let mut g = self.inner.lock().await;
        // scan and find
        for (k, e) in g.iter() {
            if k == &key && e.inserted_at.elapsed() < self.ttl {
                return Some(e.doc.clone());
            }
        }
        // evict expired
        g.retain(|(_, e)| e.inserted_at.elapsed() < self.ttl);
        None
    }

    pub async fn put(&self, key: String, doc: FetchedDocument) {
        let mut g = self.inner.lock().await;
        g.retain(|(_, e)| e.inserted_at.elapsed() < self.ttl);
        g.push((key, Entry { doc, inserted_at: Instant::now() }));
        if g.len() > self.max_entries {
            let overflow = g.len() - self.max_entries;
            g.drain(0..overflow);
        }
    }

    pub async fn clear(&self) {
        let mut g = self.inner.lock().await;
        g.clear();
    }
}
