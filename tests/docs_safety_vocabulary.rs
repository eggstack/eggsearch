use std::fs;

const SAFETY_DOCS: &[&str] = &["docs/threat-model.md", "docs/safety.md"];

const REQUIRED_HEADINGS: &[&str] = &[
    "Trust Boundaries",
    "Fetch Safety Model",
    "Prompt-Injection",
    "Local Workspace",
];

const REQUIRED_TERMS: &[&str] = &[
    "external_untrusted",
    "local_trusted",
    "allow_private_network",
    "allow_localhost",
    "sanitize_output",
    "raw_text",
    "provider_status",
];

fn read_all_docs() -> String {
    let mut combined = String::new();
    for &file in SAFETY_DOCS {
        if let Ok(text) = fs::read_to_string(file) {
            combined.push_str(&text);
            combined.push('\n');
        }
    }
    combined
}

#[test]
fn safety_headings_present() {
    let text = read_all_docs();
    let mut missing = Vec::new();

    for &heading in REQUIRED_HEADINGS {
        if !text.contains(heading) {
            missing.push(heading);
        }
    }

    if !missing.is_empty() {
        panic!("the following safety headings are missing from docs: {missing:?}");
    }
}

#[test]
fn safety_terms_present() {
    let text = read_all_docs();
    let mut missing = Vec::new();

    for &term in REQUIRED_TERMS {
        if !text.contains(term) {
            missing.push(term);
        }
    }

    if !missing.is_empty() {
        panic!("the following safety terms are missing from docs: {missing:?}");
    }
}

#[test]
fn threat_model_links_to_safety() {
    let text =
        fs::read_to_string("docs/threat-model.md").expect("failed to read docs/threat-model.md");
    assert!(
        text.contains("safety.md"),
        "threat-model.md should cross-link to safety.md"
    );
}

#[test]
fn safety_links_to_threat_model() {
    let text = fs::read_to_string("docs/safety.md").expect("failed to read docs/safety.md");
    assert!(
        text.contains("threat-model.md"),
        "safety.md should cross-link to threat-model.md"
    );
}
