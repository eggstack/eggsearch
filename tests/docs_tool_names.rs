use std::fs;

const FILES: &[&str] = &[
    "README.md",
    "docs/config.md",
    "docs/codegg-integration.md",
    "docs/tool-matrix.md",
    "docs/agent-workflows.md",
    "AGENTS.md",
];

fn tool_names_from_server() -> Vec<String> {
    let source = fs::read_to_string("src/mcp/server.rs").expect("read server.rs");
    let mut names = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name = \"") {
            if let Some(end) = rest.find('"') {
                names.push(rest[..end].to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

fn read_all_docs() -> String {
    let mut combined = String::new();
    for &file in FILES {
        if let Ok(text) = fs::read_to_string(file) {
            combined.push_str(&text);
            combined.push('\n');
        }
    }
    combined
}

#[test]
fn tool_names_in_docs() {
    let tools = tool_names_from_server();
    assert_eq!(
        tools.len(),
        10,
        "server.rs must register exactly ten stable tools, got: {tools:?}"
    );
    let text = read_all_docs();
    let mut missing = Vec::new();

    for tool in &tools {
        if !text.contains(tool.as_str()) {
            missing.push(tool.clone());
        }
    }

    if !missing.is_empty() {
        panic!("the following MCP tools are not mentioned in any scanned docs: {missing:?}");
    }
}
