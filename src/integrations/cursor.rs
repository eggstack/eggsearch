use serde_json::json;

use super::Transport;

pub(crate) fn entry(transport: Transport, command: &str, url: &str) -> serde_json::Value {
    match transport {
        Transport::Stdio => json!({
            "command": command,
            "args": ["mcp", "stdio"]
        }),
        Transport::Http => json!({ "url": url }),
    }
}
