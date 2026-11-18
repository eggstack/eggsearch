use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::core::config::McpSchemaProfile;
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

fn compact_tool_definitions() -> Vec<rmcp::model::Tool> {
    let mut config = AppConfig::default();
    config.mcp.schema_profile = McpSchemaProfile::Compact;
    let state = Arc::new(ServerState::build(config).expect("compact state"));
    let server = eggsearch::mcp::EggsearchServer::new(state);
    server.tool_definitions()
}

fn compact_tool_for(name: &str) -> rmcp::model::Tool {
    compact_tool_definitions()
        .into_iter()
        .find(|t| t.name.as_ref() == name)
        .unwrap_or_else(|| panic!("tool {name} not found"))
}

fn compact_schema_for(name: &str) -> serde_json::Value {
    serde_json::to_value(&compact_tool_for(name).input_schema).expect("schema serializes")
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

fn accepts_boolean(schema: &serde_json::Value) -> bool {
    schema.get("type").and_then(serde_json::Value::as_str) == Some("boolean")
        || schema
            .get("type")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|types| types.iter().any(|kind| kind.as_str() == Some("boolean")))
        || ["anyOf", "oneOf"]
            .iter()
            .filter_map(|key| schema.get(*key).and_then(serde_json::Value::as_array))
            .flatten()
            .any(accepts_boolean)
}

fn find_target_variant<'a>(
    value: &'a serde_json::Value,
    root: &'a serde_json::Value,
    kind: &str,
    active_refs: &mut Vec<String>,
) -> Option<&'a serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => {
            if value
                .pointer("/properties/type/const")
                .and_then(serde_json::Value::as_str)
                == Some(kind)
            {
                if let Some(reference) = value.get("$ref").and_then(serde_json::Value::as_str) {
                    if let Some(pointer) = reference.strip_prefix('#') {
                        return root.pointer(pointer).or(Some(value));
                    }
                }
                return Some(value);
            }
            if let Some(reference) = value.get("$ref").and_then(serde_json::Value::as_str) {
                if let Some(pointer) = reference.strip_prefix('#') {
                    if !active_refs.iter().any(|active| active == reference) {
                        active_refs.push(reference.to_string());
                        let result = root.pointer(pointer).and_then(|target| {
                            find_target_variant(target, root, kind, active_refs)
                        });
                        active_refs.pop();
                        if result.is_some() {
                            return result;
                        }
                    }
                }
            }
            object
                .values()
                .find_map(|child| find_target_variant(child, root, kind, active_refs))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|child| find_target_variant(child, root, kind, active_refs)),
        _ => None,
    }
}

fn batch_variant<'a>(schema: &'a serde_json::Value, kind: &str) -> &'a serde_json::Value {
    find_target_variant(schema, schema, kind, &mut Vec::new())
        .unwrap_or_else(|| panic!("fetch schema has no {kind} variant"))
}

fn has_nonempty_array(
    value: &serde_json::Value,
    root: &serde_json::Value,
    active_refs: &mut Vec<String>,
) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            if value.get("type").and_then(serde_json::Value::as_str) == Some("array")
                && value.get("minItems").and_then(serde_json::Value::as_u64) == Some(1)
            {
                return true;
            }
            if let Some(reference) = value.get("$ref").and_then(serde_json::Value::as_str) {
                if let Some(pointer) = reference.strip_prefix('#') {
                    if !active_refs.iter().any(|active| active == reference) {
                        active_refs.push(reference.to_string());
                        let found = root
                            .pointer(pointer)
                            .is_some_and(|target| has_nonempty_array(target, root, active_refs));
                        active_refs.pop();
                        if found {
                            return true;
                        }
                    }
                }
            }
            object
                .iter()
                .filter(|(key, _)| key.as_str() != "$ref")
                .any(|(_, child)| has_nonempty_array(child, root, active_refs))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .any(|child| has_nonempty_array(child, root, active_refs)),
        _ => false,
    }
}

fn collect_enum_strings(value: &serde_json::Value, output: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(values) = object.get("enum").and_then(serde_json::Value::as_array) {
                output.extend(
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned),
                );
            }
            for child in object.values() {
                collect_enum_strings(child, output);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                collect_enum_strings(child, output);
            }
        }
        _ => {}
    }
}

fn contains_string(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains(needle),
        serde_json::Value::Object(object) => {
            object.values().any(|child| contains_string(child, needle))
        }
        serde_json::Value::Array(values) => {
            values.iter().any(|child| contains_string(child, needle))
        }
        _ => false,
    }
}

fn collect_unresolved_local_refs(
    value: &serde_json::Value,
    root: &serde_json::Value,
    path: &str,
    unresolved: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(serde_json::Value::as_str) {
                if let Some(pointer) = reference.strip_prefix('#') {
                    if root.pointer(pointer).is_none() {
                        unresolved.push(format!("{path} -> {reference}"));
                    }
                }
            }
            for (key, child) in object {
                collect_unresolved_local_refs(child, root, &format!("{path}/{key}"), unresolved);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                collect_unresolved_local_refs(child, root, &format!("{path}/{index}"), unresolved);
            }
        }
        _ => {}
    }
}

#[test]
fn every_compact_schema_resolves_local_references() {
    let tools = compact_tool_definitions();
    assert_eq!(tools.len(), 9);
    for tool in tools {
        let schema = serde_json::to_value(tool.input_schema).expect("schema serializes");
        let mut unresolved = Vec::new();
        collect_unresolved_local_refs(&schema, &schema, "$", &mut unresolved);
        assert!(
            unresolved.is_empty(),
            "{} has unresolved compact schema references: {unresolved:?}",
            tool.name
        );
    }
}

#[test]
fn fetch_schema_advertises_single_and_nonempty_multi_target_inputs() {
    let full = schema_for("fetch");
    let compact = compact_schema_for("fetch");
    for schema in [&full, &compact] {
        assert!(has_prop(schema, "items"));
        for property in [
            "max_items",
            "max_chars_per_item",
            "max_total_chars",
            "timeout_ms",
            "continue_on_error",
            "offset",
            "limit",
            "response_detail",
        ] {
            assert!(
                has_prop(schema, property),
                "fetch must advertise {property}"
            );
        }
        let items = schema.pointer("/properties/items").expect("items schema");
        assert!(
            has_nonempty_array(items, schema, &mut Vec::new()),
            "items must accept a non-empty array: {items}"
        );
        let web = batch_variant(schema, "web");
        for property in ["offset", "limit"] {
            assert!(
                web.pointer(&format!("/properties/{property}")).is_some(),
                "document window must be scoped to each web fetch item: {web}"
            );
        }
        for property in [
            "url",
            "pdf",
            "render",
            "browser_profile",
            "include_screenshot",
        ] {
            assert!(
                web.pointer(&format!("/properties/{property}")).is_some(),
                "web fetch item must advertise {property}"
            );
        }
        let repo = batch_variant(schema, "repo");
        assert!(repo.pointer("/properties/offset").is_none());
        assert!(repo.pointer("/properties/limit").is_none());
        for property in ["owner", "repo", "path", "symbol", "prefer_local"] {
            assert!(
                repo.pointer(&format!("/properties/{property}")).is_some(),
                "repo fetch item must advertise {property}"
            );
        }
        assert!(!contains_string(schema, "web_fetch"));
        assert!(!contains_string(schema, "batch_fetch"));
    }
}

#[test]
fn full_and_compact_advertisements_are_smaller_than_the_ten_tool_baseline() {
    let full_bytes = serde_json::to_string(&tool_definitions()).unwrap().len();
    let compact_bytes = serde_json::to_string(&compact_tool_definitions())
        .unwrap()
        .len();
    println!("m029-advertisement-bytes full={full_bytes} compact={compact_bytes} baseline_full=78072 baseline_compact=22540");
    assert!(
        full_bytes < 78_072,
        "full tools/list {full_bytes} must be below the exact ten-tool baseline 78072"
    );
    assert!(
        compact_bytes < 22_540,
        "compact tools/list {compact_bytes} must be below the exact ten-tool baseline 22540"
    );
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
        search_total <= 13_100,
        "four search schemas total {search_total} bytes exceeds 13100 budget (pre-slim baseline 17723; 30% reduction target 12406; +318 for M020 offset addressing on research_search; +184 for explicit destructive/idempotent hints on the four search tools)"
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
        total <= 78_250,
        "all tool definitions total {total} bytes exceeds 78250 budget (pre-slim baseline 74694 input-only; +~4k for MCP 2026 outputSchema coverage added in 003 with compact stable envelopes; +~1k for 004 response_detail projection control; +411 for M020 offset addressing on web_fetch, batch_fetch and research_search; +37 for M015 provider_status challenge_solver; +632 for explicit behavior hints on all ten tools and the web_search evidence-only sentence; +2,532 for M002 documented per-target batch controls; +272 for M003 screenshot request properties)"
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
        prop_count(&research) <= 16,
        "research_search props {} exceeds 16",
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
fn compact_fetch_preserves_browser_render_modes() {
    let full = schema_for("fetch");
    let compact = compact_schema_for("fetch");
    for (name, schema) in [("full", &full), ("compact", &compact)] {
        let web = batch_variant(schema, "web");
        let screenshot = web
            .pointer("/properties/include_screenshot")
            .unwrap_or_else(|| panic!("{name} fetch web items expose include_screenshot"));
        assert!(
            accepts_boolean(screenshot),
            "{name} screenshot field must accept boolean: {screenshot}"
        );
    }
    let web = batch_variant(&compact, "web");
    assert!(
        web.pointer("/properties/render").is_some(),
        "compact fetch web items expose render"
    );
    let render_policy = compact
        .pointer("/$defs/RenderPolicy")
        .or_else(|| compact.pointer("/definitions/RenderPolicy"))
        .expect("render policy enum schema");
    let mut values = Vec::new();
    collect_enum_strings(render_policy, &mut values);
    assert_eq!(values, ["http_only", "auto", "browser"]);
}

#[test]
fn compact_fetch_explains_bounded_web_and_repo_targets() {
    let description = compact_tool_for("fetch")
        .description
        .expect("compact fetch description")
        .to_string();
    assert!(description.contains("bounded results"));
    assert!(description.contains("do not search or crawl"));
}

#[test]
fn fetch_advertises_per_target_controls_in_full_and_compact_schemas() {
    let full = schema_for("fetch");
    let compact = compact_schema_for("fetch");

    for schema in [&full, &compact] {
        let web = batch_variant(schema, "web");
        for property in ["pdf", "render", "browser_profile", "include_screenshot"] {
            let field = web.pointer(&format!("/properties/{property}"));
            assert!(field.is_some(), "web batch items must advertise {property}");
            if property == "include_screenshot" {
                assert!(
                    accepts_boolean(field.unwrap()),
                    "{property} must accept boolean"
                );
            }
        }
        let repo = batch_variant(schema, "repo");
        for property in [
            "symbol",
            "symbol_kind",
            "match_text",
            "expand_to_block",
            "max_block_lines",
            "prefer_local",
        ] {
            assert!(
                repo.pointer(&format!("/properties/{property}")).is_some(),
                "repo fetch items must advertise {property}"
            );
        }
        assert!(!contains_string(schema, "batch_fetch"));
    }
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
    let compact_schema = compact_schema_for("security_search");
    let goal_values = compact_schema["properties"]["goal"]["enum"]
        .as_array()
        .expect("compact security goal must advertise an enum")
        .iter()
        .map(|value| value.as_str().expect("goal enum value is a string"))
        .collect::<Vec<_>>();
    assert_eq!(
        goal_values,
        eggsearch::mcp::tools::canonical::SECURITY_GOAL_VALUES
    );
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
fn repo_map_hides_infrastructure_controls() {
    // `providers` was advertised but never read, so an agent could target a
    // self-hosted forge and silently receive another host's tree. `timeout_ms`
    // is the one tuning knob repo_map legitimately advertises.
    let schema = schema_for("repo_map");
    assert!(
        !has_prop(&schema, "providers"),
        "repo_map providers must stay hidden"
    );
    assert!(has_prop(&schema, "owner"), "owner must stay advertised");
    assert!(has_prop(&schema, "repo"), "repo must stay advertised");
    assert!(has_prop(&schema, "host"), "host must stay advertised");
    assert!(
        has_prop(&schema, "timeout_ms"),
        "repo_map timeout_ms is an advertised knob"
    );
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
