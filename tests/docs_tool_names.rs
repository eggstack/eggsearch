use std::fs;

const KNOWN_TOOLS: &[&str] = &[
    "web_search",
    "web_fetch",
    "batch_fetch",
    "provider_status",
    "repo_search",
    "repo_fetch",
    "repo_map",
    "security_search",
    "research_search",
    "build_evidence_bundle",
];

const FILES: &[&str] = &[
    "README.md",
    "docs/config.md",
    "docs/codegg-integration.md",
    "docs/tool-matrix.md",
    "docs/agent-workflows.md",
    "AGENTS.md",
];

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
    let text = read_all_docs();
    let mut missing = Vec::new();

    for &tool in KNOWN_TOOLS {
        if !text.contains(tool) {
            missing.push(tool);
        }
    }

    if !missing.is_empty() {
        panic!("the following MCP tools are not mentioned in any scanned docs: {missing:?}");
    }
}
