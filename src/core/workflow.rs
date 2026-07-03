//! Agent workflow recipes and next-action hints.
//!
//! Recipes are compact, machine-readable retrieval playbooks that teach
//! agent harnesses when to use which eggsearch tools. They are guidance
//! only — hosts remain in control of tool sequencing.
//!
//! Next-action hints are lightweight suggestions appended to tool
//! responses so agents can chain tools without prompt-level reasoning.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Verbosity level for workflow recipes in `provider_status` responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecipeDetail {
    /// Omit workflow_recipes from the response entirely.
    None,
    /// Return compact recipe summaries (id, title, goal, support, step_tools).
    #[default]
    Summary,
    /// Return full recipe objects with steps, fallbacks, trust_notes.
    Full,
}

/// Whether a recipe is fully supported, partially supported, or
/// unsupported given the current provider configuration.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecipeSupport {
    /// All required capabilities are available.
    Available,
    /// Some required capabilities are available but the recipe will
    /// operate with degraded coverage.
    Partial,
    /// Required capabilities are not available.
    Unavailable,
}

/// A single step in a workflow recipe.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentWorkflowStep {
    /// Step order (1-indexed).
    pub order: u8,
    /// Tool name (must match a registered MCP tool).
    pub tool: String,
    /// Why this step exists.
    pub purpose: String,
    /// Hints for constructing tool inputs.
    pub input_hints: Vec<String>,
    /// Response fields to inspect before proceeding.
    pub inspect_fields: Vec<String>,
    /// Optional rule describing when to advance or branch.
    pub next_action_rule: Option<String>,
}

/// A fallback strategy when a recipe's preferred path is unavailable.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentWorkflowFallback {
    /// Short description of the fallback.
    pub description: String,
    /// Tool to use in the fallback path.
    pub tool: String,
    /// When this fallback applies.
    pub when: String,
}

/// Machine-readable workflow recipe for a common agent task.
///
/// Recipes are deterministic guidance derived from provider capabilities.
/// They never instruct autonomous crawling or automatic link following.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentWorkflowRecipe {
    /// Stable recipe id (snake_case).
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// What this recipe accomplishes.
    pub goal: String,
    /// When this recipe is the right choice.
    pub suitable_when: Vec<String>,
    /// When this recipe should NOT be used.
    pub avoid_when: Vec<String>,
    /// Capability strings that must be present for full support.
    pub required_capabilities: Vec<String>,
    /// Capability strings that improve coverage but are not required.
    pub optional_capabilities: Vec<String>,
    /// Ordered steps to execute.
    pub steps: Vec<AgentWorkflowStep>,
    /// Fallback strategies.
    pub fallbacks: Vec<AgentWorkflowFallback>,
    /// What the recipe produces when followed.
    pub expected_outputs: Vec<String>,
    /// Trust and safety notes for the agent.
    pub trust_notes: Vec<String>,
    /// Current support level given provider configuration.
    pub support: RecipeSupport,
}

/// A lightweight next-action hint appended to tool responses.
///
/// Hints are compact, explicit, and bounded. They suggest the most
/// productive follow-up tool call based on the response content.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentNextAction {
    /// Target tool name.
    pub tool: String,
    /// Machine-readable reason code.
    pub reason_code: String,
    /// Priority (1 = highest). Bounded to 1..=5.
    pub priority: u8,
    /// Suggested input template for the target tool.
    pub input_template: serde_json::Value,
    /// Source card IDs this action relates to.
    pub source_ids: Vec<String>,
}

/// Maximum number of next-action hints per response.
pub const MAX_NEXT_ACTIONS: usize = 5;

impl AgentWorkflowRecipe {
    /// Return a compact summary suitable for `recipe_detail = "summary"`.
    pub fn summarize(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "title": self.title,
            "goal": self.goal,
            "support": self.support,
            "required_capabilities": self.required_capabilities,
            "optional_capabilities": self.optional_capabilities,
            "step_tools": self.steps.iter().map(|s| s.tool.as_str()).collect::<Vec<_>>(),
        })
    }
}

impl AgentNextAction {
    /// Create a new next-action hint, clamping priority to 1..=5.
    pub fn new(
        tool: impl Into<String>,
        reason_code: impl Into<String>,
        priority: u8,
        input_template: serde_json::Value,
        source_ids: Vec<String>,
    ) -> Self {
        Self {
            tool: tool.into(),
            reason_code: reason_code.into(),
            priority: priority.clamp(1, 5),
            input_template,
            source_ids,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_serde_roundtrip() {
        let recipe = AgentWorkflowRecipe {
            id: "generic_lookup".into(),
            title: "Generic Web Lookup".into(),
            goal: "Discover and fetch evidence for ordinary web questions".into(),
            suitable_when: vec!["General information needed".into()],
            avoid_when: vec!["Repo-specific code search".into()],
            required_capabilities: vec!["generic_search".into()],
            optional_capabilities: vec![],
            steps: vec![AgentWorkflowStep {
                order: 1,
                tool: "provider_status".into(),
                purpose: "Check available providers".into(),
                input_hints: vec!["No arguments needed".into()],
                inspect_fields: vec!["providers".into()],
                next_action_rule: None,
            }],
            fallbacks: vec![AgentWorkflowFallback {
                description: "No providers available".into(),
                tool: "provider_status".into(),
                when: "All providers disabled".into(),
            }],
            expected_outputs: vec!["SourceCards with web evidence".into()],
            trust_notes: vec!["All results are external_untrusted".into()],
            support: RecipeSupport::Available,
        };
        let json = serde_json::to_string(&recipe).unwrap();
        let parsed: AgentWorkflowRecipe = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "generic_lookup");
        assert_eq!(parsed.steps.len(), 1);
        assert_eq!(parsed.support, RecipeSupport::Available);
    }

    #[test]
    fn next_action_serde_roundtrip() {
        let action = AgentNextAction::new(
            "web_fetch",
            "inspect_source",
            1,
            serde_json::json!({"url": "https://example.com"}),
            vec!["src_abc".into()],
        );
        let json = serde_json::to_string(&action).unwrap();
        let parsed: AgentNextAction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tool, "web_fetch");
        assert_eq!(parsed.priority, 1);
    }

    #[test]
    fn next_action_priority_clamped() {
        let action = AgentNextAction::new("web_fetch", "test", 99, serde_json::json!(null), vec![]);
        assert_eq!(action.priority, 5);
        let action = AgentNextAction::new("web_fetch", "test", 0, serde_json::json!(null), vec![]);
        assert_eq!(action.priority, 1);
    }

    #[test]
    fn support_serde_variants() {
        for (json, expected) in [
            (r#""available""#, RecipeSupport::Available),
            (r#""partial""#, RecipeSupport::Partial),
            (r#""unavailable""#, RecipeSupport::Unavailable),
        ] {
            let parsed: RecipeSupport = serde_json::from_str(json).unwrap();
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn recipe_ids_are_snake_case() {
        let recipe = AgentWorkflowRecipe {
            id: "repo_investigation".into(),
            title: "Repository Investigation".into(),
            goal: "Understand a repo".into(),
            suitable_when: vec![],
            avoid_when: vec![],
            required_capabilities: vec![],
            optional_capabilities: vec![],
            steps: vec![],
            fallbacks: vec![],
            expected_outputs: vec![],
            trust_notes: vec![],
            support: RecipeSupport::Available,
        };
        assert!(recipe.id.chars().all(|c| c.is_alphanumeric() || c == '_'));
    }

    #[test]
    fn recipe_detail_default_is_summary() {
        assert_eq!(RecipeDetail::default(), RecipeDetail::Summary);
    }

    #[test]
    fn recipe_detail_serde_variants() {
        for (json, expected) in [
            (r#""none""#, RecipeDetail::None),
            (r#""summary""#, RecipeDetail::Summary),
            (r#""full""#, RecipeDetail::Full),
        ] {
            let parsed: RecipeDetail = serde_json::from_str(json).unwrap();
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn summarize_produces_expected_compact_output() {
        let recipe = AgentWorkflowRecipe {
            id: "generic_lookup".into(),
            title: "Generic Web Lookup".into(),
            goal: "Discover and fetch evidence".into(),
            suitable_when: vec![],
            avoid_when: vec![],
            required_capabilities: vec!["generic_search".into()],
            optional_capabilities: vec![],
            steps: vec![
                AgentWorkflowStep {
                    order: 1,
                    tool: "web_search".into(),
                    purpose: "Search".into(),
                    input_hints: vec![],
                    inspect_fields: vec![],
                    next_action_rule: None,
                },
                AgentWorkflowStep {
                    order: 2,
                    tool: "web_fetch".into(),
                    purpose: "Fetch".into(),
                    input_hints: vec![],
                    inspect_fields: vec![],
                    next_action_rule: None,
                },
            ],
            fallbacks: vec![],
            expected_outputs: vec![],
            trust_notes: vec![],
            support: RecipeSupport::Available,
        };
        let summary = recipe.summarize();
        assert_eq!(summary["id"], "generic_lookup");
        assert_eq!(summary["title"], "Generic Web Lookup");
        assert_eq!(summary["support"], "available");
        assert_eq!(
            summary["step_tools"],
            serde_json::json!(["web_search", "web_fetch"])
        );
        // Should NOT contain steps, fallbacks, trust_notes
        assert!(summary.get("steps").is_none());
        assert!(summary.get("fallbacks").is_none());
        assert!(summary.get("trust_notes").is_none());
    }
}
