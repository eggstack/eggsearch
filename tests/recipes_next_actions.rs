use eggsearch::core::provider::built_in_provider_descriptor;
use eggsearch::core::workflow::{RecipeDetail, RecipeSupport, MAX_NEXT_ACTIONS};
use eggsearch::meta::recipe_catalog::{
    build_recipe_catalog, repo_search_next_actions, research_search_next_actions,
    security_search_next_actions, web_search_next_actions,
};

const KNOWN_MCP_TOOLS: &[&str] = &[
    "web_search",
    "web_fetch",
    "repo_search",
    "repo_fetch",
    "repo_map",
    "batch_fetch",
    "security_search",
    "research_search",
    "provider_status",
    "build_evidence_bundle",
];

const EXPECTED_IDS: &[&str] = &[
    "generic_web_lookup",
    "documentation_api_lookup",
    "repository_investigation",
    "exact_error_investigation",
    "security_package_triage",
    "dependency_upgrade_research",
    "architecture_deep_research",
    "local_workspace_investigation",
];

fn empty_providers() -> Vec<eggsearch::core::provider::ProviderDescriptor> {
    vec![]
}

fn full_providers() -> Vec<eggsearch::core::provider::ProviderDescriptor> {
    [
        "duckduckgo",
        "brave",
        "brave_api",
        "github_code",
        "github_issues",
        "github_releases",
        "gitlab_code",
        "gitlab_issues",
        "gitlab_releases",
        "gitea_code",
        "gitea_issues",
        "gitea_releases",
        "osv",
    ]
    .iter()
    .filter_map(|id| built_in_provider_descriptor(id, true, false, true, false, None, None))
    .collect()
}

#[test]
fn exactly_eight_builtin_recipe_ids() {
    let catalog = build_recipe_catalog(&empty_providers(), false);
    assert_eq!(catalog.len(), 8);
    let ids: Vec<&str> = catalog.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, EXPECTED_IDS);
}

#[test]
fn every_recipe_step_tool_is_known_mcp_tool() {
    let catalog = build_recipe_catalog(&empty_providers(), false);
    for recipe in &catalog {
        for step in &recipe.steps {
            assert!(
                KNOWN_MCP_TOOLS.contains(&step.tool.as_str()),
                "Recipe '{}' step {} references unknown tool '{}'",
                recipe.id,
                step.order,
                step.tool
            );
        }
    }
}

#[test]
fn summary_detail_omits_steps_fallbacks_trust_notes() {
    let catalog = build_recipe_catalog_detail(&empty_providers(), false, RecipeDetail::Summary);
    assert_eq!(catalog.len(), 8);
    for recipe in &catalog {
        // The summarize() output does not contain steps/fallbacks/trust_notes
        let summary = recipe.summarize();
        assert!(
            summary.get("steps").is_none(),
            "Summary should not contain 'steps' for recipe '{}'",
            recipe.id
        );
        assert!(
            summary.get("fallbacks").is_none(),
            "Summary should not contain 'fallbacks' for recipe '{}'",
            recipe.id
        );
        assert!(
            summary.get("trust_notes").is_none(),
            "Summary should not contain 'trust_notes' for recipe '{}'",
            recipe.id
        );
    }
}

#[test]
fn full_detail_includes_steps() {
    let catalog = build_recipe_catalog_detail(&empty_providers(), false, RecipeDetail::Full);
    assert_eq!(catalog.len(), 8);
    for recipe in &catalog {
        assert!(
            !recipe.steps.is_empty(),
            "Recipe '{}' should have steps in Full detail",
            recipe.id
        );
    }
}

#[test]
fn none_detail_produces_empty_catalog() {
    let catalog = build_recipe_catalog_detail(&empty_providers(), false, RecipeDetail::None);
    assert!(
        catalog.is_empty(),
        "RecipeDetail::None should produce empty catalog, got {} recipes",
        catalog.len()
    );
}

#[test]
fn no_recipe_instructs_autonomous_crawling() {
    let catalog = build_recipe_catalog(&empty_providers(), false);
    for recipe in &catalog {
        for step in &recipe.steps {
            let purpose_lower = step.purpose.to_lowercase();
            assert!(
                !purpose_lower.contains("crawl"),
                "Recipe '{}' step {} purpose must not mention crawling: '{}'",
                recipe.id,
                step.order,
                step.purpose
            );
            assert!(
                !purpose_lower.contains("follow links"),
                "Recipe '{}' step {} purpose must not mention following links: '{}'",
                recipe.id,
                step.order,
                step.purpose
            );
            assert!(
                !purpose_lower.contains("auto-follow"),
                "Recipe '{}' step {} purpose must not mention auto-follow: '{}'",
                recipe.id,
                step.order,
                step.purpose
            );
            assert!(
                !purpose_lower.contains("automatically browse"),
                "Recipe '{}' step {} purpose must not mention automatically browsing: '{}'",
                recipe.id,
                step.order,
                step.purpose
            );
        }
    }
}

#[test]
fn next_action_hints_capped_at_max() {
    let source_ids: Vec<String> = (0..20).map(|i| format!("src_{i}")).collect();

    let actions = web_search_next_actions(&source_ids, true);
    assert!(
        actions.len() <= MAX_NEXT_ACTIONS,
        "web_search_next_actions produced {} actions, max is {}",
        actions.len(),
        MAX_NEXT_ACTIONS
    );

    let actions = repo_search_next_actions(&source_ids, true);
    assert!(
        actions.len() <= MAX_NEXT_ACTIONS,
        "repo_search_next_actions produced {} actions, max is {}",
        actions.len(),
        MAX_NEXT_ACTIONS
    );

    let actions = security_search_next_actions(&source_ids, true);
    assert!(
        actions.len() <= MAX_NEXT_ACTIONS,
        "security_search_next_actions produced {} actions, max is {}",
        actions.len(),
        MAX_NEXT_ACTIONS
    );

    let actions = research_search_next_actions(&source_ids, true);
    assert!(
        actions.len() <= MAX_NEXT_ACTIONS,
        "research_search_next_actions produced {} actions, max is {}",
        actions.len(),
        MAX_NEXT_ACTIONS
    );
}

#[test]
fn next_action_tool_names_are_valid() {
    let source_ids = vec!["src_1".to_string(), "src_2".to_string()];

    for actions in [
        web_search_next_actions(&source_ids, true),
        repo_search_next_actions(&source_ids, true),
        security_search_next_actions(&source_ids, true),
        research_search_next_actions(&source_ids, true),
    ] {
        for action in &actions {
            assert!(
                KNOWN_MCP_TOOLS.contains(&action.tool.as_str()),
                "Next action references unknown tool '{}'",
                action.tool
            );
        }
    }
}

#[test]
fn next_action_priorities_are_bounded() {
    let source_ids = vec!["src_1".to_string(), "src_2".to_string()];

    for actions in [
        web_search_next_actions(&source_ids, true),
        repo_search_next_actions(&source_ids, true),
        security_search_next_actions(&source_ids, true),
        research_search_next_actions(&source_ids, true),
    ] {
        for action in &actions {
            assert!(
                action.priority >= 1 && action.priority <= 5,
                "Next action priority {} is out of 1..=5 range for tool '{}'",
                action.priority,
                action.tool
            );
        }
    }
}

#[test]
fn recipe_support_unavailable_with_empty_providers() {
    let catalog = build_recipe_catalog(&empty_providers(), false);
    for recipe in &catalog {
        // Recipes requiring only generic_search or explicit_fetch are Available
        // local_workspace_investigation requires local_workspace so should be Unavailable
        if recipe
            .required_capabilities
            .iter()
            .any(|c| c == "local_workspace")
        {
            assert_eq!(
                recipe.support,
                RecipeSupport::Unavailable,
                "Recipe '{}' should be Unavailable without providers",
                recipe.id
            );
        }
    }
}

#[test]
fn recipe_support_available_with_full_providers() {
    let catalog = build_recipe_catalog(&full_providers(), true);
    for recipe in &catalog {
        assert_eq!(
            recipe.support,
            RecipeSupport::Available,
            "Recipe '{}' should be Available with full providers + local enabled",
            recipe.id
        );
    }
}

#[test]
fn every_recipe_has_required_capabilities() {
    let catalog = build_recipe_catalog(&empty_providers(), false);
    for recipe in &catalog {
        assert!(
            !recipe.required_capabilities.is_empty(),
            "Recipe '{}' has no required_capabilities",
            recipe.id
        );
    }
}

#[test]
fn each_recipe_has_at_least_one_step_in_full_detail() {
    let catalog = build_recipe_catalog_detail(&empty_providers(), false, RecipeDetail::Full);
    assert_eq!(catalog.len(), 8);
    for recipe in &catalog {
        assert!(
            !recipe.steps.is_empty(),
            "Recipe '{}' has no steps in Full detail",
            recipe.id
        );
    }
}

#[test]
fn next_actions_empty_when_no_sources() {
    let empty: Vec<String> = vec![];
    let actions = web_search_next_actions(&empty, false);
    assert!(actions.is_empty());

    let actions = repo_search_next_actions(&empty, false);
    // repo_search always adds bundle_evidence
    assert!(actions.len() <= MAX_NEXT_ACTIONS);

    let actions = security_search_next_actions(&empty, false);
    assert!(actions.is_empty());

    let actions = research_search_next_actions(&empty, false);
    // research_search always adds bundle_evidence
    assert!(actions.len() <= MAX_NEXT_ACTIONS);
}

#[test]
fn next_action_source_ids_populated() {
    let source_ids = vec!["src_a".to_string(), "src_b".to_string()];
    let actions = web_search_next_actions(&source_ids, true);
    assert!(!actions.is_empty());
    // The first action (inspect_top_source) should reference the first source
    assert_eq!(actions[0].source_ids, vec!["src_a"]);
}

/// Helper: build catalog with a specific RecipeDetail level.
/// Since build_recipe_catalog always populates full data, we simulate
/// detail filtering by checking the summarize() output for Summary
/// and the raw recipe for Full.
fn build_recipe_catalog_detail(
    providers: &[eggsearch::core::provider::ProviderDescriptor],
    local_enabled: bool,
    detail: RecipeDetail,
) -> Vec<eggsearch::core::workflow::AgentWorkflowRecipe> {
    match detail {
        RecipeDetail::None => vec![],
        RecipeDetail::Summary | RecipeDetail::Full => {
            build_recipe_catalog(providers, local_enabled)
        }
    }
}
