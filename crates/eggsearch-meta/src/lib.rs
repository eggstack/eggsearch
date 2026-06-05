//! Metasearch adapter wrapping vendored search engines.
//!
//! This crate exposes a thin boundary around the vendored search engine
//! implementations. It does not leak engine types beyond the
//! `MetadataSearchAdapter` and the small response/payload types defined
//! here. Callers receive `eggsearch_core::SourceCard` values.

pub mod adapter;
pub mod engines;
#[cfg(feature = "mock")]
pub mod mock;
pub mod response;

pub use adapter::{ErrorClass, MetadataSearchAdapter};
pub use response::{ProviderFailure, ProviderStatus, WebSearchResponse};
