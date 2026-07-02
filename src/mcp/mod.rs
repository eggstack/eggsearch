//! MCP server adapter for eggsearch.
//!
//! Exposes a stable tool surface:
//!
//! - `web_search`          — live metasearch over configured upstream providers.
//! - `web_fetch`           — bounded extraction of one explicit HTTP(S) URL.
//! - `batch_fetch`         — bounded batch fetch over explicit URLs or structured repo locators.
//! - `provider_status`     — diagnostic report of configured providers and server capabilities.
//! - `repo_search`         — structured repository evidence discovery with grouped result bundles.
//! - `repo_fetch`          — structured repository file fetch by locator with line ranges and symbols.
//! - `repo_map`            — bounded repository-structure discovery for coding agents.
//! - `security_search`     — security-oriented retrieval with normalized vulnerability metadata.
//! - `research_search`     — research-oriented multi-source evidence discovery with grouped bundles.
//! - `build_evidence_bundle` — packages already-selected evidence into a portable container.
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
