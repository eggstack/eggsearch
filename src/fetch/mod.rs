//! URL fetch and content extraction module.
//!
//! Fetches a single HTTP(S) URL, enforces size/time/content limits,
//! extracts readable text/metadata, and returns bounded structured output.

pub mod client;
pub mod detect;
pub mod extract;
pub mod limits;
/// HTML structural rendering (blocks, text, markdown).
pub mod render;
pub mod types;

pub use client::FetchClient;
pub use extract::{extract_content, HtmlExtractor};
pub use limits::{validate_fetch_target, FetchLimits};
pub use types::{FetchError, FetchErrorKind};
