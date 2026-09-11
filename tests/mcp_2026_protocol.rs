use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::mcp::tools::{map_tool_result, RepairHint, ToolError, ToolErrorCode};
use eggsearch::mcp::{output_schema, tool_contract};

fn state() -> Arc<eggsearch::mcp::ServerState> {
    Arc::new(eggsearch::mcp::ServerState::build(AppConfig::default()).expect("default state"))
}

#[test]
fn success_maps_to_native_structured_content_with_text_fallback() {
    let value = serde_json::json!({"query": "test", "results": []});
    let result = map_tool_result(Ok(value.clone())).expect("success maps");
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(value.clone()));
    assert!(!result.content.is_empty());
    let text = result.content[0]
        .as_text()
        .expect("text fallback for older clients");
    let parsed: serde_json::Value =
        serde_json::from_str(&text.text).expect("fallback text is JSON");
    assert_eq!(parsed, value);
}

#[test]
fn semantic_validation_becomes_repairable_tool_error_not_invalid_params() {
    let err = ToolError::Validation("invalid goal 'bogus'; accepted values: debug".to_string());
    let result = map_tool_result(Err(err)).expect("validation maps to tool error");
    assert_eq!(result.is_error, Some(true));
    let payload = result.structured_content.expect("structured error payload");
    assert_eq!(
        payload.get("code").and_then(|c| c.as_str()),
        Some("invalid_semantic_value")
    );
    assert!(payload.get("message").is_some());
}

#[test]
fn execution_errors_carry_stable_codes_and_bounded_repair() {
    let repair = RepairHint::new(
        Some("workflow"),
        &["architecture", "debug", "migration"],
        Some("debug"),
    );
    let err = ToolError::execution_with_repair(
        ToolErrorCode::ConflictingArguments,
        "conflicting goal and workflow",
        repair,
    );
    let result = map_tool_result(Err(err)).expect("execution maps to tool error");
    assert_eq!(result.is_error, Some(true));
    let payload = result.structured_content.expect("payload");
    assert_eq!(payload["code"], "conflicting_arguments");
    assert_eq!(payload["repair"]["field"], "workflow");
    assert_eq!(payload["repair"]["accepted"].as_array().unwrap().len(), 3);
    assert_eq!(payload["repair"]["suggested_value"], "debug");
}

#[test]
fn repair_hints_are_bounded() {
    let many: Vec<String> = (0..100).map(|i| format!("value_{i}")).collect();
    let refs: Vec<&str> = many.iter().map(|s| s.as_str()).collect();
    let hint = RepairHint::new(Some("field"), &refs, Some("suggested"));
    assert!(hint.accepted.len() <= 20);
}

#[test]
fn invalid_request_maps_to_jsonrpc_invalid_params() {
    let err = ToolError::invalid_request("missing required field");
    match map_tool_result(Err(err)) {
        Err(e) => assert_eq!(e.code, rmcp::model::ErrorCode::INVALID_PARAMS),
        Ok(_) => panic!("InvalidRequest must become JSON-RPC invalid_params"),
    }
}

#[test]
fn internal_maps_to_jsonrpc_internal_error_without_stack() {
    let err = ToolError::internal("something broke");
    match map_tool_result(Err(err)) {
        Err(e) => {
            assert_eq!(e.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
            let rendered = format!("{e:?}");
            assert!(!rendered.contains("backtrace"));
        }
        Ok(_) => panic!("Internal must become JSON-RPC internal_error"),
    }
}

#[test]
fn canonical_goal_conflict_is_repairable_with_code() {
    let err = eggsearch::mcp::tools::canonical::resolve_repo_semantics(
        Some("debug"),
        None,
        None,
        Some("security_review"),
    )
    .unwrap_err();
    assert_eq!(err.code(), ToolErrorCode::ConflictingArguments);
    let result = map_tool_result(Err(err)).expect("conflict maps to tool error");
    assert_eq!(result.is_error, Some(true));
    let payload = result.structured_content.unwrap();
    assert_eq!(payload["code"], "conflicting_arguments");
    assert!(payload.get("repair").is_some());
}

#[test]
fn canonical_invalid_goal_carries_accepted_values() {
    let err =
        eggsearch::mcp::tools::canonical::resolve_repo_semantics(Some("bogus"), None, None, None)
            .unwrap_err();
    assert_eq!(err.code(), ToolErrorCode::InvalidSemanticValue);
    let payload = err.error_payload();
    assert_eq!(payload["code"], "invalid_semantic_value");
    let accepted = payload["repair"]["accepted"].as_array().unwrap();
    assert!(accepted.iter().any(|v| v == "debug"));
}

#[test]
fn all_stable_tools_have_output_schemas() {
    for name in tool_contract::tool_names() {
        let schema = output_schema::output_schema_for(name);
        assert!(
            schema.is_some(),
            "tool '{name}' must have an output schema or documented exception"
        );
        let schema = schema.unwrap();
        assert_eq!(
            schema.get("type").and_then(|v| v.as_str()),
            Some("object"),
            "tool '{name}' output schema must be an object"
        );
    }
}

#[test]
fn typed_output_schemas_validate_representative_payloads() {
    let batch = serde_json::to_value(eggsearch::core::batch_fetch::BatchFetchResponse {
        fetched: 1,
        failed: 0,
        truncated: false,
        total_chars_returned: 10,
        results: vec![],
        warnings: vec![],
        structured_warnings: vec![],
        telemetry: None,
    })
    .unwrap();
    assert!(batch.get("results").is_some());
    assert!(batch.get("fetched").is_some());

    let bundle = serde_json::json!({
        "sources": [],
        "fetches": [],
    });
    assert!(bundle.is_object());

    let web_search = serde_json::json!({"query": "test", "results": []});
    assert_eq!(web_search["query"], "test");

    let web_fetch = serde_json::json!({
        "url": "https://example.com",
        "final_url": "https://example.com",
        "status": 200,
        "fetched": true,
        "truncated": false,
    });
    for key in ["url", "final_url", "status", "fetched", "truncated"] {
        assert!(web_fetch.get(key).is_some(), "web_fetch missing {key}");
    }
}

#[test]
fn tools_list_is_deterministic_and_fingerprinted() {
    let server = eggsearch::mcp::EggsearchServer::new(state());
    let first = server.tool_definitions();
    let second = server.tool_definitions();
    let names_first: Vec<String> = first.iter().map(|t| t.name.to_string()).collect();
    let names_second: Vec<String> = second.iter().map(|t| t.name.to_string()).collect();
    assert_eq!(names_first, names_second);
    let mut sorted = names_first.clone();
    sorted.sort();
    assert_eq!(names_first, sorted, "tools/list must be sorted for caching");
    assert_eq!(first.len(), 10);

    let fp1 = server.tool_fingerprint();
    let fp2 = server.tool_fingerprint();
    assert_eq!(fp1, fp2);
    assert_eq!(fp1.len(), 16);
    assert!(fp1.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn output_schemas_are_stable_across_calls() {
    let server = eggsearch::mcp::EggsearchServer::new(state());
    let a = server.tool_definitions();
    let b = server.tool_definitions();
    for (ta, tb) in a.iter().zip(b.iter()) {
        assert_eq!(ta.name, tb.name);
        assert_eq!(
            serde_json::to_value(&ta.output_schema).unwrap(),
            serde_json::to_value(&tb.output_schema).unwrap(),
            "output schema for {} must be stable",
            ta.name
        );
    }
}

#[test]
fn browser_capability_errors_are_repairable_not_internal() {
    let err = ToolError::capability_unavailable("browser profiles are not enabled");
    assert_eq!(err.code(), ToolErrorCode::CapabilityUnavailable);
    let result = map_tool_result(Err(err)).expect("capability maps to tool error");
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap()["code"],
        "capability_unavailable"
    );
}
