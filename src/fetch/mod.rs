//! URL fetch and content extraction module.
//!
//! Fetches a single HTTP(S) URL, enforces size/time/content limits,
//! extracts readable text/metadata, and returns bounded structured output.

pub mod client;
pub mod extract;
pub mod limits;
pub mod types;

pub use client::FetchClient;
pub use extract::{extract_content, HtmlExtractor};
pub use limits::FetchLimits;
pub use types::{FetchError, FetchErrorKind};
