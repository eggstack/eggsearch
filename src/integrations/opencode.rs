use serde_json::json;

use super::Transport;

pub(crate) fn entry(transport: Transport, command: &str, url: &str) -> serde_json::Value {
    match transport {
        Transport::Stdio => json!({
            "type": "local",
            "command": [command, "mcp", "stdio"]
        }),
        Transport::Http => json!({
            "type": "remote",
            "url": url,
            "oauth": false
        }),
    }
}
