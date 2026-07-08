use eggsearch::core::provider::KNOWN_PROVIDER_IDS;
use std::collections::BTreeSet;
use std::fs;

const FILES: &[&str] = &[
    "README.md",
    "docs/config.md",
    "docs/codegg-integration.md",
    "docs/provider-setup.md",
    "docs/tool-matrix.md",
    "docs/agent-workflows.md",
    "docs/safety.md",
    "AGENTS.md",
];

fn extract_toml_blocks(path: &str) -> Vec<String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].starts_with("```toml") {
            i += 1;
            let mut block = String::new();
            while i < lines.len() && lines[i] != "```" {
                block.push_str(lines[i]);
                block.push('\n');
                i += 1;
            }
            blocks.push(block);
        }
        i += 1;
    }

    blocks
}

fn extract_provider_ids_from_toml(block: &str) -> Vec<String> {
    let mut ids = Vec::new();

    for line in block.lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("default_providers") {
            if rest.starts_with('=') {
                let list_part = rest.split_once('=').unwrap().1.trim();
                if list_part.starts_with('[') {
                    let inner = &list_part[1..list_part.rfind(']').unwrap_or(list_part.len())];
                    for item in inner.split(',') {
                        let id = item.trim().trim_matches('"').trim_matches('\'');
                        if !id.is_empty() {
                            ids.push(id.to_string());
                        }
                    }
                }
            }
        }
    }

    ids
}

fn extract_providers_from_sections(block: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut in_providers_section = false;

    for line in block.lines() {
        let trimmed = line.trim();

        if trimmed == "[search.providers]" || trimmed.starts_with("[search.providers.") {
            in_providers_section = true;
            continue;
        }
        if trimmed.starts_with('[') && !trimmed.starts_with("[search.providers") {
            in_providers_section = false;
            continue;
        }

        if in_providers_section {
            if let Some((key, _)) = trimmed.split_once('=') {
                let key = key.trim();
                if key.starts_with('"') && key.ends_with('"') {
                    ids.push(key[1..key.len() - 1].to_string());
                } else if !key.is_empty()
                    && (key.chars().next().unwrap().is_alphabetic() || key.starts_with('_'))
                {
                    ids.push(key.to_string());
                }
            }
        }
    }

    ids
}

fn extract_api_provider_sections(block: &str) -> Vec<String> {
    let mut ids = Vec::new();

    for line in block.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("[search.api.") {
            if let Some(id) = rest.strip_suffix(']') {
                let id = id.trim();
                if !id.is_empty() {
                    ids.push(id.to_string());
                }
            }
        }
    }

    ids
}

#[test]
fn provider_inventory_in_docs() {
    let known: BTreeSet<&str> = KNOWN_PROVIDER_IDS.iter().copied().collect();
    let mut errors: Vec<String> = Vec::new();

    for &file in FILES {
        let blocks = extract_toml_blocks(file);
        for (idx, block) in blocks.iter().enumerate() {
            let mut found_ids = extract_provider_ids_from_toml(block);
            found_ids.extend(extract_providers_from_sections(block));
            found_ids.extend(extract_api_provider_sections(block));
            for id in &found_ids {
                if !known.contains(id.as_str()) {
                    errors.push(format!(
                        "{file}: TOML block {idx} references unknown provider id '{id}'"
                    ));
                }
            }
        }
    }

    if !errors.is_empty() {
        panic!("provider inventory errors:\n{}", errors.join("\n"));
    }
}
