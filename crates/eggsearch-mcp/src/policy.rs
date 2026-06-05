//! Policy enforcement: gates tool execution based on configured mode.

use eggsearch_core::config::Mode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy {
    /// Tool execution is allowed.
    Allow,
    /// Tool execution is denied; return a structured message.
    Deny,
}

pub fn live_allowed(mode: Mode) -> Policy {
    match mode {
        Mode::Live => Policy::Allow,
        Mode::Ask => Policy::Allow, // host-mediated; we trust the host.
        Mode::Off | Mode::LocalOnly => Policy::Deny,
    }
}

pub fn local_allowed(mode: Mode) -> Policy {
    match mode {
        Mode::Off => Policy::Deny,
        Mode::LocalOnly | Mode::Live | Mode::Ask => Policy::Allow,
    }
}

pub fn fetch_allowed(mode: Mode) -> Policy {
    live_allowed(mode)
}

pub fn policy_message(kind: &str) -> String {
    format!(
        "Tool '{kind}' is disabled by policy. Configure [search].mode to 'live', 'local_only', or 'ask' in your eggsearch config to enable it."
    )
}
