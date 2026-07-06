use eggsearch::core::config::AppConfig;
use std::fs;

fn extract_config_snippets(path: &str) -> Vec<(String, String)> {
    let text = fs::read_to_string(path).expect("failed to read file");
    let lines: Vec<&str> = text.lines().collect();
    let mut snippets = Vec::new();
    let mut i = 0;
    let mut heading = String::new();

    while i < lines.len() {
        let line = lines[i];
        if let Some(h) = line.strip_prefix("# ") {
            heading = h.to_string();
        } else if let Some(h) = line.strip_prefix("## ") {
            heading = h.to_string();
        } else if let Some(h) = line.strip_prefix("### ") {
            heading = h.to_string();
        }

        if line.starts_with("```toml eggsearch-config") && !line.contains("parse-only") {
            i += 1;
            let mut block = String::new();
            while i < lines.len() && lines[i] != "```" {
                block.push_str(lines[i]);
                block.push('\n');
                i += 1;
            }
            let slug: String = heading
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            snippets.push((slug, block));
        }
        i += 1;
    }

    snippets
}

#[test]
fn config_snippets_docs_config() {
    let snippets = extract_config_snippets("docs/config.md");
    assert!(
        !snippets.is_empty(),
        "no config snippets found in docs/config.md"
    );
    for (slug, toml_str) in &snippets {
        let cfg: AppConfig = toml::from_str(toml_str)
            .unwrap_or_else(|e| panic!("TOML parse failed for snippet '{slug}': {e}"));
        cfg.validate()
            .unwrap_or_else(|e| panic!("validate() failed for snippet '{slug}': {e}"));
    }
}

#[test]
fn config_snippets_docs_codegg_integration() {
    let snippets = extract_config_snippets("docs/codegg-integration.md");
    assert!(
        !snippets.is_empty(),
        "no config snippets found in docs/codegg-integration.md"
    );
    for (slug, toml_str) in &snippets {
        let cfg: AppConfig = toml::from_str(toml_str)
            .unwrap_or_else(|e| panic!("TOML parse failed for snippet '{slug}': {e}"));
        cfg.validate()
            .unwrap_or_else(|e| panic!("validate() failed for snippet '{slug}': {e}"));
    }
}
