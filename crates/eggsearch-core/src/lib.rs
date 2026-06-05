//! Core types, traits, normalization, ranking, source cards, and errors for eggsearch.
//!
//! This crate is intentionally independent of any MCP, HTTP, or search-engine
//! implementation. It only defines the abstract data model and the algorithms
//! required to combine, dedupe, and rank provider output.

pub mod config;
pub mod dedupe;
pub mod error;
pub mod normalize;
pub mod provider;
pub mod query;
pub mod rank;
pub mod result;
pub mod source_card;

pub use config::{AppConfig, LiveConfig, LocalConfig, Mode, ProviderConfig, SearchSection};
pub use error::{CoreError, CoreResult};
pub use provider::{NetworkMode, SearchContext, SearchProvider, SearchProviderResponse};
pub use query::{Freshness, SafeSearch, SearchCategory, SearchQuery};
pub use result::{SearchResult, SearchWarning, SourceKind, TrustLevel};
pub use source_card::SourceCard;
