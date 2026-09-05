use serde_json::json;

use super::Transport;

pub(crate) fn command(transport: Transport, executable: &str, url: &str) -> Vec<String> {
    let entry = match transport {
        Transport::Stdio => json!({
            "name": "eggsearch",
            "type": "stdio",
            "command": executable,
            "args": ["mcp", "stdio"]
        }),
        Transport::Http => json!({
            "name": "eggsearch",
            "type": "http",
            "url": url
        }),
    };
    vec![
        "code".to_string(),
        "--add-mcp".to_string(),
        serde_json::to_string(&entry).expect("VS Code MCP entry is serializable"),
    ]
}
