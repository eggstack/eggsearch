//! Policy enforcement: gates tool execution based on configured mode.

use crate::core::config::Mode;

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
    fn policy_message_web_search() {
        let msg = policy_message("web_search");
        assert!(msg.contains("web_search"));
        assert!(msg.contains("disabled by policy"));
        assert!(msg.contains("mode = \"live\""));
    }

    #[test]
    fn policy_message_provider_status() {
        let msg = policy_message("provider_status");
        assert!(msg.contains("provider_status"));
    }
}
