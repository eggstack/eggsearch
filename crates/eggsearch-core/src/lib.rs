//! Core types, source card model, configuration, and URL canonicalization
//! for eggsearch. This crate is intentionally independent of any MCP,
//! HTTP, or search-engine implementation.

pub mod config;
pub mod error;
pub mod normalize;
pub mod query;
pub mod result;
pub mod source_card;

pub use config::{AppConfig, LiveConfig, Mode, SearchSection};
pub use error::{CoreError, CoreResult};
pub use query::{SafeSearch, WebSearchRequest};
pub use result::{SearchWarning, TrustLevel};
pub use source_card::SourceCard;
