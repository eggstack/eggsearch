//! Metasearch adapter wrapping `metadata-search-engine-rs`.
//!
//! This crate exposes a thin boundary around the upstream metasearch
//! library. It does not leak upstream types beyond the `MetadataSearchAdapter`
//! and the small response/payload types defined here. Callers receive
//! `eggsearch_core::SourceCard` values.

pub mod adapter;
#[cfg(feature = "metasearch")]
pub mod engine;
pub mod response;

pub use adapter::{ErrorClass, MetadataSearchAdapter};
pub use response::{ProviderFailure, ProviderStatus, WebSearchResponse};
