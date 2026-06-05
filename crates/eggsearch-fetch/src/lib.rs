//! URL fetching, content extraction, cache, and artifact store for eggsearch.
//!
//! The fetch layer is intentionally simple: it bounds response size and
//! time, extracts readable text, and persists the full extracted document
//! to an on-disk artifact store keyed by content hash.

pub mod artifact;
pub mod cache;
pub mod error;
pub mod extract;
pub mod fetch;
pub mod html;
pub mod markdown;
pub mod provider;
pub mod robots;

pub use artifact::{hash_bytes, ArtifactMetadata, ArtifactStore};
pub use cache::FetchCache;
pub use error::{FetchError, FetchResult};
pub use extract::{extract_html, extract_text, ExtractMode};
pub use fetch::{FetchProvider, FetchRequest, FetchedDocument};
pub use provider::ReqwestFetchProvider;
pub use robots::RobotsCache;
