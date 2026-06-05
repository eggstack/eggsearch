//! Live metasearch providers for eggsearch.
//!
//! Each provider implements `eggsearch_core::SearchProvider`. Providers
//! should:
//!
//! - Never panic on malformed upstream HTML.
//! - Emit parser warnings when suspicious / empty parses occur.
//! - Return at most `query.max_results` entries.

pub mod providers;
pub mod registry;

pub use providers::MockProvider;
pub use registry::ProviderRegistry;
