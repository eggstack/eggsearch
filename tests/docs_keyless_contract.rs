use std::fs;

fn read_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

#[test]
fn readme_says_no_api_keys_required() {
    let text = read_file("README.md");
    assert!(
        text.contains("No API keys are required"),
        "README must prominently state that no API keys are required for the default installation"
    );
}

#[test]
fn readme_labels_native_adapter_credentials_as_maintainer_only() {
    let text = read_file("README.md");
    assert!(
        text.contains("maintainer-only") || text.contains("maintainer only"),
        "README must label native adapter credentials as maintainer-only, not user-facing"
    );
}

#[test]
fn provider_setup_identifies_keyless_and_optional_categories() {
    let text = read_file("docs/provider-setup.md");
    assert!(
        text.contains("Keyless defaults"),
        "provider-setup must have a 'Keyless defaults' category"
    );
    assert!(
        text.contains("Optional credentialed"),
        "provider-setup must have an 'Optional credentialed' category"
    );
    assert!(
        text.contains("No API keys required") || text.contains("none"),
        "provider-setup must indicate that keyless providers require no keys"
    );
}

#[test]
fn config_presents_keyless_examples_before_enhanced() {
    let text = read_file("docs/config.md");
    let keyless_pos = text
        .find("Keyless Default")
        .unwrap_or(text.find("Keyless coding").unwrap_or(0));
    let enhanced_pos = text
        .find("Enhanced Coding")
        .unwrap_or(text.find("Enhanced coding").unwrap_or(usize::MAX));
    assert!(
        keyless_pos < enhanced_pos,
        "config.md must present keyless profiles before enhanced profiles"
    );
}

#[test]
fn codegg_contract_contains_restored_sections() {
    let text = read_file("docs/architecture/codegg-contract.md");
    assert!(
        text.contains("### 8.3 Dirty State"),
        "codegg contract must contain section 8.3 Dirty State"
    );
    assert!(
        text.contains("### 8.4 File Classification Flags"),
        "codegg contract must contain section 8.4 File Classification Flags"
    );
    assert!(
        text.contains("### 8.5 Workspace ID"),
        "codegg contract must contain section 8.5 Workspace ID"
    );
}

#[test]
fn codegg_contract_state_tables_use_snake_case_wire_values() {
    let text = read_file("docs/architecture/codegg-contract.md");
    assert!(
        text.contains("`satisfied`"),
        "codegg contract state table must use snake_case wire value 'satisfied', not 'Satisfied'"
    );
    assert!(
        text.contains("`completed_no_match`"),
        "codegg contract state table must use snake_case wire value 'completed_no_match'"
    );
    assert!(
        text.contains("`skipped_by_policy`"),
        "codegg contract state table must use snake_case wire value 'skipped_by_policy'"
    );
    assert!(
        text.contains("`capability_unavailable`"),
        "codegg contract state table must use snake_case wire value 'capability_unavailable'"
    );
    assert!(
        text.contains("`not_applicable`"),
        "codegg contract state table must use snake_case wire value 'not_applicable'"
    );
}

#[test]
fn codegg_contract_contains_keyless_core_section() {
    let text = read_file("docs/architecture/codegg-contract.md");
    assert!(
        text.contains("Keyless-Core Invariant"),
        "codegg contract must contain keyless-core invariant section"
    );
    assert!(
        text.contains("Do Not Require Credentialed Providers"),
        "codegg contract must instruct harnesses not to require credentialed providers"
    );
    assert!(
        text.contains("Do Not Prompt for Keys"),
        "codegg contract must instruct harnesses not to prompt for keys on baseline operations"
    );
}

#[test]
fn no_default_install_command_includes_credential_setup() {
    let text = read_file("README.md");
    let install_start = text.find("## Install").unwrap_or(0);
    let install_end = text[install_start + 10..]
        .find("\n## ")
        .map(|pos| install_start + 10 + pos)
        .unwrap_or(text.len());
    let install_section = &text[install_start..install_end];
    assert!(
        !install_section.contains("GITHUB_TOKEN")
            && !install_section.contains("API key")
            && !install_section.contains("credential"),
        "default install command must not include credential setup"
    );
}

#[test]
fn tool_matrix_identifies_keyless_path_for_each_tool() {
    let text = read_file("docs/tool-matrix.md");
    assert!(
        text.contains("Keyless Baseline"),
        "tool-matrix must have a keyless baseline section"
    );
    assert!(
        text.contains("web_search") && text.contains("DuckDuckGo"),
        "tool-matrix must identify keyless path for web_search"
    );
    assert!(
        text.contains("security_search") && text.contains("OSV"),
        "tool-matrix must identify keyless path for security_search"
    );
    assert!(
        text.contains("research_search") && text.contains("OpenAlex"),
        "tool-matrix must identify keyless path for research_search"
    );
}
