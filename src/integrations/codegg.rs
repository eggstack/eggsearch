use serde_json::json;

use super::Transport;

pub(crate) fn entry(transport: Transport, _command: &str, url: &str) -> serde_json::Value {
    match transport {
        Transport::Stdio => json!({ "search": { "backend": "eggsearch" } }),
        Transport::Http => json!({
            "search": { "backend": "eggsearch" },
            "mcp": { "eggsearch": {
                "type": "remote",
                "url": url,
                "enabled": true
            }}
        }),
    }
}
