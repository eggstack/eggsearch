//! Provider implementations.

pub mod crates_io;
pub mod docs_rs;
pub mod duckduckgo_html;
pub mod mock;
pub mod wikipedia;

pub use mock::MockProvider;
