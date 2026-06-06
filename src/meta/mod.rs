//! Metasearch adapter wrapping vendored search engines.
//!
//! This module exposes a thin boundary around the vendored search engine
//! implementations. It does not leak engine types beyond the
//! `MetadataSearchAdapter` and the small response/payload types defined
//! here. Callers receive `crate::core::SourceCard` values.

pub mod adapter;
/// Vendored HTML search engines. Internal; not part of the stable
/// public API.
pub mod engines;
#[cfg(feature = "mock")]
pub mod mock;
pub mod response;

pub use adapter::{ErrorClass, MetadataSearchAdapter};
pub use response::{ProviderFailure, ProviderStatus, WebSearchResponse};
