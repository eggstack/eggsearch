//! Core types, source card model, configuration, and error types
//! for eggsearch. This module is intentionally independent of any MCP,
//! HTTP, or search-engine implementation.

pub mod config;
pub mod error;
pub mod fetch;
pub mod query;
pub mod result;
pub mod sanitize;
pub mod source_card;

pub use config::{AppConfig, LiveConfig, Mode, SearchSection};
pub use error::{CoreError, CoreResult};
pub use fetch::{ExtractMode, ExtractedLink, FetchTrust, WebFetchRequest, WebFetchResponse};
pub use query::{SafeSearch, WebSearchRequest};
pub use result::{SearchWarning, TrustLevel};
pub use sanitize::{
    bound_text, frame, scan_injection_markers, strip_control_chars, MarkerHit, TrustMarkers,
    TITLE_MAX_CHARS, SNIPPET_MAX_CHARS,
};
pub use source_card::SourceCard;
