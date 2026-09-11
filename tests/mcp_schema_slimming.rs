use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::mcp::ServerState;

fn tool_definitions() -> Vec<rmcp::model::Tool> {
    let state = Arc::new(ServerState::build(AppConfig::default()).expect("default state"));
    let server = eggsearch::mcp::EggsearchServer::new(state);
    server.tool_definitions()
}

fn schema_for(name: &str) -> serde_json::Value {
    let tools = tool_definitions();
    let tool = tools
        .iter()
        .find(|t| t.name.as_ref() == name)
        .unwrap_or_else(|| panic!("tool {name} not found"));
    serde_json::to_value(&tool.input_schema).expect("schema serializes")
}

fn serialized_bytes(name: &str) -> usize {
    let tools = tool_definitions();
    let tool = tools
        .iter()
        .find(|t| t.name.as_ref() == name)
        .unwrap_or_else(|| panic!("tool {name} not found"));
    serde_json::to_string(&serde_json::to_value(tool).unwrap())
        .unwrap()
        .len()
}

fn prop_count(schema: &serde_json::Value) -> usize {
    schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|o| o.len())
        .unwrap_or(0)
}

fn has_prop(schema: &serde_json::Value, prop: &str) -> bool {
    schema.get("properties").and_then(|p| p.get(prop)).is_some()
}

#[test]
fn search_four_schema_bytes_stay_slim() {
    let names = [
        "web_search",
        "repo_search",
        "research_search",
        "security_search",
    ];
    let mut search_total = 0usize;
    for name in names {
        search_total += serialized_bytes(name);
    }
    assert!(
        search_total <= 12_500,
        "four search schemas total {search_total} bytes exceeds 12500 budget (pre-slim baseline 17723; 30% reduction target 12406)"
    );
    let total: usize = tool_definitions()
        .iter()
        .map(|t| {
            serde_json::to_string(&serde_json::to_value(t).unwrap())
                .unwrap()
                .len()
        })
        .sum();
    assert!(
        total <= 72_000,
        "all tool definitions total {total} bytes exceeds 72000 budget (pre-slim baseline 74694 input-only; +~4k for MCP 2026 outputSchema coverage added in 003 with compact stable envelopes)"
    );
}

#[test]
fn search_tool_property_counts_stay_bounded() {
    let web = schema_for("web_search");
    let repo = schema_for("repo_search");
    let research = schema_for("research_search");
    let security = schema_for("security_search");
    assert!(
        prop_count(&web) <= 13,
        "web_search props {} exceeds 13",
        prop_count(&web)
    );
    assert!(
        prop_count(&repo) <= 23,
        "repo_search props {} exceeds 23",
        prop_count(&repo)
    );
    assert!(
        prop_count(&research) <= 15,
        "research_search props {} exceeds 15",
        prop_count(&research)
    );
    assert!(
        prop_count(&security) <= 18,
        "security_search props {} exceeds 18",
        prop_count(&security)
    );
}

#[test]
fn web_search_hides_infrastructure_controls() {
    let schema = schema_for("web_search");
    assert!(
        !has_prop(&schema, "providers"),
        "providers must stay hidden"
    );
    assert!(
        !has_prop(&schema, "timeout_ms"),
        "timeout_ms must stay hidden"
    );
    assert!(has_prop(&schema, "query"), "query must stay advertised");
    assert!(
        has_prop(&schema, "max_results"),
        "max_results must stay advertised"
    );
}

#[test]
fn repo_search_advertises_goal_and_sources() {
    let schema = schema_for("repo_search");
    assert!(has_prop(&schema, "goal"), "goal must be advertised");
    assert!(has_prop(&schema, "sources"), "sources must be advertised");
    for legacy in [
        "profile",
        "mode",
        "workflow",
        "providers",
        "timeout_ms",
        "include_docs",
        "include_registry",
        "include_issues",
        "include_releases",
        "include_examples",
        "include_pull_requests",
        "include_changelog",
        "include_migration_guides",
        "include_security_context",
    ] {
        assert!(
            !has_prop(&schema, legacy),
            "legacy {legacy} must stay hidden from ordinary schema"
        );
    }
    for kept in [
        "query",
        "host",
        "owner",
        "repo",
        "ecosystem",
        "package",
        "include_local",
    ] {
        assert!(has_prop(&schema, kept), "{kept} must stay advertised");
    }
}

#[test]
fn research_search_advertises_goal_and_include() {
    let schema = schema_for("research_search");
    assert!(has_prop(&schema, "goal"), "goal must be advertised");
    assert!(has_prop(&schema, "include"), "include must be advertised");
    for legacy in [
        "include_counterpoints",
        "include_primary_sources",
        "include_recent_discussion",
        "include_security_considerations",
        "providers",
        "timeout_ms",
        "workflow",
    ] {
        assert!(
            !has_prop(&schema, legacy),
            "legacy {legacy} must stay hidden"
        );
    }
}

#[test]
fn security_search_advertises_goal_and_include() {
    let schema = schema_for("security_search");
    assert!(has_prop(&schema, "goal"), "goal must be advertised");
    assert!(has_prop(&schema, "include"), "include must be advertised");
    for legacy in [
        "include_kev",
        "include_exploit_context",
        "include_defensive_guidance",
        "include_vendor_advisories",
        "providers",
        "timeout_ms",
        "workflow",
    ] {
        assert!(
            !has_prop(&schema, legacy),
            "legacy {legacy} must stay hidden"
        );
    }
}

#[test]
fn legacy_repo_fields_still_deserialize() {
    let json = r#"{
        "query": "Router::layer",
        "host": "github",
        "owner": "tokio-rs",
        "repo": "axum",
        "profile": "coding",
        "mode": "exact_error",
        "workflow": "error_investigation",
        "include_docs": true,
        "providers": ["github_code"],
        "timeout_ms": 5000
    }"#;
    let args: eggsearch::mcp::tools::RepoSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.profile.as_deref(), Some("coding"));
    assert_eq!(args.mode.as_deref(), Some("exact_error"));
    assert_eq!(args.workflow.as_deref(), Some("error_investigation"));
    assert_eq!(args.include_docs, Some(true));
}

#[test]
fn canonical_repo_goal_still_deserializes() {
    let json = r#"{"query": "auth middleware", "goal": "security", "sources": ["docs", "issues"]}"#;
    let args: eggsearch::mcp::tools::RepoSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.goal.as_deref(), Some("security"));
    assert_eq!(args.sources, vec!["docs", "issues"]);
}

#[test]
fn legacy_web_fields_still_deserialize() {
    let json = r#"{"query": "axum", "providers": ["duckduckgo"], "timeout_ms": 5000}"#;
    let args: eggsearch::mcp::tools::WebSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.providers, vec!["duckduckgo"]);
    assert_eq!(args.timeout_ms, Some(5000));
}

#[test]
fn legacy_research_fields_still_deserialize() {
    let json = r#"{"query": "compare", "workflow": "library_comparison", "include_counterpoints": true, "providers": ["x"], "timeout_ms": 1000}"#;
    let args: eggsearch::mcp::tools::ResearchSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.workflow.as_deref(), Some("library_comparison"));
    assert_eq!(args.include_counterpoints, Some(true));
}

#[test]
fn legacy_security_fields_still_deserialize() {
    let json = r#"{"query": "CVE-2024-0001", "workflow": "security_review", "include_kev": true, "providers": ["x"], "timeout_ms": 1000}"#;
    let args: eggsearch::mcp::tools::SecuritySearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.workflow.as_deref(), Some("security_review"));
    assert_eq!(args.include_kev, Some(true));
}

#[test]
fn repo_goal_conflict_reports_repair() {
    let r = eggsearch::mcp::tools::canonical::resolve_repo_semantics(
        Some("debug"),
        None,
        None,
        Some("security_review"),
    );
    let err = r.unwrap_err().to_string();
    assert!(err.contains("conflicting goal"), "got: {err}");
    assert!(err.contains("Repair"), "got: {err}");
}

#[test]
fn repo_unknown_goal_enumerates_canonical_values() {
    let r =
        eggsearch::mcp::tools::canonical::resolve_repo_semantics(Some("bogus"), None, None, None);
    let err = r.unwrap_err().to_string();
    assert!(err.contains("understand"), "got: {err}");
    assert!(err.contains("Repair"), "got: {err}");
}

#[test]
fn repo_sources_conflict_reports_repair() {
    let r = eggsearch::mcp::tools::canonical::resolve_repo_sources(
        &["docs".to_string()],
        Some(false),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let err = r.unwrap_err().to_string();
    assert!(err.contains("Repair"), "got: {err}");
}

#[test]
fn research_goal_conflict_reports_repair() {
    let r = eggsearch::mcp::tools::canonical::resolve_research_workflow(
        Some("architecture"),
        Some("security_review"),
    );
    let err = r.unwrap_err().to_string();
    assert!(err.contains("conflicting"), "got: {err}");
    assert!(err.contains("Repair"), "got: {err}");
}

#[test]
fn security_include_conflict_reports_repair() {
    let r = eggsearch::mcp::tools::canonical::resolve_security_includes(
        &["kev".to_string()],
        Some(false),
        None,
        None,
        None,
    );
    let err = r.unwrap_err().to_string();
    assert!(err.contains("Repair"), "got: {err}");
}
