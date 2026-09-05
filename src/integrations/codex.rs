pub(crate) fn command(transport: super::Transport, executable: &str, url: &str) -> Vec<String> {
    match transport {
        super::Transport::Stdio => vec![
            "codex".to_string(),
            "mcp".to_string(),
            "add".to_string(),
            "eggsearch".to_string(),
            "--".to_string(),
            executable.to_string(),
            "mcp".to_string(),
            "stdio".to_string(),
        ],
        super::Transport::Http => vec![
            "codex".to_string(),
            "mcp".to_string(),
            "add".to_string(),
            "eggsearch".to_string(),
            "--url".to_string(),
            url.to_string(),
        ],
    }
}

pub(crate) fn remove_command() -> Vec<String> {
    vec![
        "codex".to_string(),
        "mcp".to_string(),
        "remove".to_string(),
        "eggsearch".to_string(),
    ]
}
