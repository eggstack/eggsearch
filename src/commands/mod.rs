//! CLI subcommands.

#[cfg(feature = "browser")]
pub mod browser_login;
#[cfg(feature = "browser")]
pub mod browser_profiles;
pub mod doctor;
pub mod fetch;
pub mod integrate;
pub mod mcp;
pub mod providers;
pub mod search;
pub mod update;
