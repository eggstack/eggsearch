//! Browser rendering for provider result pages.
//!
//! Sits above `meta` and `fetch` the way `challenge_replay` does: the engine
//! layer depends only on the `SerpRenderer` contract, and the browser stays an
//! optional implementation detail behind the `browser` feature.

#[cfg(feature = "browser")]
pub mod browser;
