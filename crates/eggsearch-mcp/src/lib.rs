//! MCP server adapter for eggsearch.
//!
//! Exposes a minimal, stable tool surface:
//!
//! - `web_search`     — live metasearch over configured upstream providers.
//! - `provider_status` — diagnostic report of configured providers.

pub mod policy;
pub mod server;
pub mod state;
pub mod tools;

pub use policy::Policy;
pub use server::EggsearchServer;
pub use state::ServerState;
