//! On-disk artifact store keyed by content hash.
//!
//! Layout:
//! ```text
//! <root>/
//!   <aa>/<bb>/<fullhex>      # the raw text
//!   <aa>/<bb>/<fullhex>.json # metadata sidecar
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

use crate::error::FetchResult;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    pub artifact_id: String,
    pub url: String,
    pub canonical_url: String,
    pub title: Option<String>,
    pub content_type: String,
    pub content_hash: String,
    pub fetched_at: DateTime<Utc>,
    pub trust_level: String,
    pub extractor_version: String,
    pub raw_length: usize,
}

pub struct ArtifactStore {
    root: PathBuf,
    lock: Mutex<()>,
}

impl std::fmt::Debug for ArtifactStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactStore").field("root", &self.root).finish()
    }
}

impl ArtifactStore {
    pub fn new(root: impl Into<PathBuf>) -> FetchResult<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root, lock: Mutex::new(()) })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, hash: &str) -> PathBuf {
        if hash.len() < 4 {
            return self.root.join(hash);
        }
        self.root.join(&hash[..2]).join(&hash[2..4]).join(hash)
    }

    /// Store `value` (any JSON-serializable structure) under `hash`.
    /// The artifact_id is the hash itself.
    pub async fn put(&self, hash: &str, value: &Value) -> FetchResult<String> {
        let _g = self.lock.lock().await;
        let path = self.path_for(hash);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(value)?;
        std::fs::write(&path, text)?;
        Ok(hash.to_string())
    }

    pub async fn put_with_meta(&self, hash: &str, value: &Value, meta: &ArtifactMetadata) -> FetchResult<String> {
        let _g = self.lock.lock().await;
        let path = self.path_for(hash);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(value)?;
        std::fs::write(&path, text)?;
        let meta_path = path.with_extension("json");
        let meta_text = serde_json::to_string_pretty(meta)?;
        std::fs::write(&meta_path, meta_text)?;
        Ok(hash.to_string())
    }

    pub fn get_path(&self, hash: &str) -> PathBuf {
        self.path_for(hash)
    }

    pub fn exists(&self, hash: &str) -> bool {
        self.path_for(hash).exists()
    }

    pub fn read(&self, hash: &str) -> FetchResult<Value> {
        let path = self.path_for(hash);
        let text = std::fs::read_to_string(&path)?;
        let v: Value = serde_json::from_str(&text)?;
        Ok(v)
    }

    pub fn delete(&self, hash: &str) -> FetchResult<()> {
        let path = self.path_for(hash);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

/// Compute a content hash from a byte slice.
pub fn hash_bytes(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn put_and_read() {
        let dir = tempdir().unwrap();
        let store = ArtifactStore::new(dir.path()).unwrap();
        let v = serde_json::json!({"hello": "world"});
        let rt = tokio::runtime::Runtime::new().unwrap();
        let id = rt.block_on(async { store.put("abc123", &v).await.unwrap() });
        assert_eq!(id, "abc123");
        let read = store.read("abc123").unwrap();
        assert_eq!(read["hello"], "world");
    }

    #[test]
    fn hashing() {
        assert_eq!(hash_bytes(b"hello").len(), 64);
    }
}
