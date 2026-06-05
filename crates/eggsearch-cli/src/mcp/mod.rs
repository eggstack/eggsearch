//! MCP server adapter for eggsearch.
//!
//! Exposes a minimal, stable tool surface:
//!
//! - `web_search`     — live metasearch over configured upstream providers.
//! - `provider_status` — diagnostic report of configured providers.
//!
//! The public API ([`EggsearchServer`], [`ServerState`], [`Policy`]) is
//! re-exported at the crate root of this module. The submodule types are
//! internal and not part of the stable surface.

#![allow(missing_docs)]

pub mod policy;
pub mod server;
pub mod state;
pub mod tools;

pub use policy::Policy;
pub use server::EggsearchServer;
pub use state::ServerState;
