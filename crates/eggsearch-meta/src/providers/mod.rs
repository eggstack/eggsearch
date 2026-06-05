//! Provider implementations.

pub mod brave;
pub mod crates_io;
pub mod docs_rs;
pub mod duckduckgo_html;
pub mod exa;
pub mod mock;
pub mod searxng;
pub mod tavily;
pub mod wikipedia;

pub use mock::MockProvider;
