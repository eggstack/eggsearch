#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::evidence_postprocess::resolve_workflow_model;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let tools = ["repo_search", "research_search", "security_search", "web_search", "unknown_tool"];
    let profiles = [None, Some("security"), Some("research"), Some("coding"), Some("generic")];
    let domains = [None, Some("architecture_decision"), Some("error_investigation"), Some("version_migration"), Some("security_review"), Some("unknown")];

    let tool_idx = (data[0] as usize) % tools.len();
    let profile_idx = (data.get(1).copied().unwrap_or(0) as usize) % profiles.len();
    let domain_idx = (data.get(2).copied().unwrap_or(0) as usize) % domains.len();
    let exact_error = data.get(3).copied().unwrap_or(0) % 2 == 1;

    let result = resolve_workflow_model(
        tools[tool_idx],
        profiles[profile_idx],
        domains[domain_idx],
        exact_error,
    );

    match tools[tool_idx] {
        "web_search" | "unknown_tool" => {
            assert!(result.is_none(), "non-security tools should return None");
        }
        _ => {
            if let Some(model) = &result {
                assert!(
                    !model.workflow_id.is_empty(),
                    "workflow_id must not be empty"
                );
                assert!(
                    !model.title.is_empty(),
                    "title must not be empty"
                );
                let total = model.required.len() + model.recommended.len() + model.optional.len();
                assert!(
                    total > 0,
                    "model must have at least one role across required/recommended/optional"
                );
            }
        }
    }
});
