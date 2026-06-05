//! Policy enforcement: gates tool execution based on configured mode.

use eggsearch_core::config::Mode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy {
    Allow,
    Deny,
}

/// Whether the live metasearch tool is allowed under the given mode.
pub fn live_allowed(mode: Mode) -> Policy {
    match mode {
        Mode::Live => Policy::Allow,
        Mode::Off => Policy::Deny,
    }
}

pub fn policy_message(kind: &str) -> String {
    format!(
        "Tool '{kind}' is disabled by policy. Set [search].mode = \"live\" in your eggsearch config to enable it."
    )
}
