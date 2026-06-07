//! Policy enforcement: gates tool execution based on configured mode.

use crate::core::config::{AppConfig, Mode};

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

/// Whether the `web_fetch` tool is allowed given the `[fetch].enabled` flag.
pub fn fetch_allowed(fetch_enabled: bool) -> Policy {
    if fetch_enabled {
        Policy::Allow
    } else {
        Policy::Deny
    }
}

/// Build the human-readable denial message for a disabled tool.
///
/// `kind` is the tool name (e.g. `"web_search"`, `"web_fetch"`).
/// `config_key` is the operator-facing config knob that must be flipped
/// to enable the tool (e.g. `"[search].mode = \"live\""` or
/// `"[fetch].enabled = true"`).
pub fn policy_message(kind: &str, config_key: &str) -> String {
    format!(
        "Tool '{kind}' is disabled by policy. Set {config_key} in your eggsearch config to enable it."
    )
}

/// Convenience wrapper for the `web_search` policy message.
pub fn web_search_denied_message() -> String {
    policy_message("web_search", "[search].mode = \"live\"")
}

/// Convenience wrapper for the `web_fetch` policy message.
pub fn web_fetch_denied_message() -> String {
    policy_message("web_fetch", "[fetch].enabled = true")
}

#[allow(dead_code)]
pub(crate) fn policy_for_web_search(cfg: &AppConfig) -> Policy {
    live_allowed(cfg.search.mode)
}

#[allow(dead_code)]
pub(crate) fn policy_for_web_fetch(cfg: &AppConfig) -> Policy {
    fetch_allowed(cfg.fetch.enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_allowed_live_mode() {
        assert_eq!(live_allowed(Mode::Live), Policy::Allow);
    }

    #[test]
    fn live_allowed_off_mode() {
        assert_eq!(live_allowed(Mode::Off), Policy::Deny);
    }

    #[test]
    fn fetch_allowed_when_enabled() {
        assert_eq!(fetch_allowed(true), Policy::Allow);
    }

    #[test]
    fn fetch_allowed_when_disabled() {
        assert_eq!(fetch_allowed(false), Policy::Deny);
    }

    #[test]
    fn policy_message_web_search() {
        let msg = policy_message("web_search", "[search].mode = \"live\"");
        assert!(msg.contains("web_search"));
        assert!(msg.contains("disabled by policy"));
        assert!(msg.contains("[search].mode = \"live\""));
    }

    #[test]
    fn policy_message_web_fetch() {
        let msg = policy_message("web_fetch", "[fetch].enabled = true");
        assert!(msg.contains("web_fetch"));
        assert!(msg.contains("disabled by policy"));
        assert!(msg.contains("[fetch].enabled = true"));
    }

    #[test]
    fn policy_message_provider_status() {
        let msg = policy_message("provider_status", "[search].mode = \"live\"");
        assert!(msg.contains("provider_status"));
    }

    #[test]
    fn web_search_denied_message_is_stable() {
        let msg = web_search_denied_message();
        assert!(msg.contains("web_search"));
        assert!(msg.contains("disabled by policy"));
    }

    #[test]
    fn web_fetch_denied_message_is_stable() {
        let msg = web_fetch_denied_message();
        assert!(msg.contains("web_fetch"));
        assert!(msg.contains("disabled by policy"));
        assert!(msg.contains("[fetch].enabled = true"));
    }
}
