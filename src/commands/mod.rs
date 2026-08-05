//! CLI subcommands.

#[cfg(feature = "browser")]
pub mod browser_login;
#[cfg(feature = "browser")]
pub mod browser_profiles;
pub mod doctor;
pub mod fetch;
pub mod mcp;
pub mod providers;
pub mod search;
