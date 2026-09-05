pub(crate) fn command(transport: super::Transport, executable: &str, url: &str) -> Vec<String> {
    match transport {
        super::Transport::Stdio => vec![
            "claude".to_string(),
            "mcp".to_string(),
            "add".to_string(),
            "--scope".to_string(),
            "user".to_string(),
            "eggsearch".to_string(),
            "--".to_string(),
            executable.to_string(),
            "mcp".to_string(),
            "stdio".to_string(),
        ],
        super::Transport::Http => vec![
            "claude".to_string(),
            "mcp".to_string(),
            "add".to_string(),
            "--scope".to_string(),
            "user".to_string(),
            "--transport".to_string(),
            "http".to_string(),
            "eggsearch".to_string(),
            url.to_string(),
        ],
    }
}

pub(crate) fn remove_command() -> Vec<String> {
    vec![
        "claude".to_string(),
        "mcp".to_string(),
        "remove".to_string(),
        "--scope".to_string(),
        "user".to_string(),
        "eggsearch".to_string(),
    ]
}
