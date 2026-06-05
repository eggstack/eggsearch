//! MCP server adapter for eggsearch.
//!
//! Exposes a small set of tools to MCP clients:
//!
//! - `web_search`     — live metasearch (subject to policy mode).
//! - `web_fetch`      — fetch and extract a known URL.
//! - `local_search`   — search the local Tantivy index only (no network).
//! - `search_and_fetch` — run a search, then fetch the top N results.

pub mod policy;
pub mod server;
pub mod state;
pub mod tools;

pub use policy::Policy;
pub use server::EggsearchServer;
pub use state::ServerState;
