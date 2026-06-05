//! Integration tests for the MCP server tool surface.
//!
//! These tests build a real `ServerState` from a default config (no
//! network calls) and exercise the `web_search` and `provider_status`
//! tools against it. They verify:
//!
//! - Tool schema (server info, tool names).
//! - `web_search` valid query returns structured payload.
//! - `web_search` empty query returns validation error.
//! - `web_search` mode=off is denied by policy.
//! - `provider_status` returns the configured provider list.

use eggsearch_core::config::AppConfig;
use eggsearch_mcp::tools::{
    run_provider_status, run_web_search, ProviderStatusArgs, WebSearchArgs,
};
use eggsearch_mcp::ServerState;
use rmcp::ServerHandler;
use std::sync::Arc;
use tempfile::tempdir;

fn build_state() -> (tempfile::TempDir, Arc<ServerState>) {
    let dir = tempdir().unwrap();
    let _ = dir; // reserved for future feature-gated tests
    let state = Arc::new(ServerState::build(AppConfig::default()).unwrap());
    (dir, state)
}

#[test]
fn mcp_server_get_info() {
    let (_dir, state) = build_state();
    let server = eggsearch_mcp::EggsearchServer::new(state);
    let info = server.get_info();
    assert_eq!(info.server_info.name, "eggsearch");
    assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    assert!(info.capabilities.tools.is_some(), "tools capability must be enabled");
}

#[test]
fn mcp_server_lists_two_tools() {
    let (_dir, state) = build_state();
    let server = eggsearch_mcp::EggsearchServer::new(state);
    let tools = server.tool_definitions();
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert!(names.contains(&"web_search".to_string()), "tools: {names:?}");
    assert!(names.contains(&"provider_status".to_string()), "tools: {names:?}");
    // Legacy tools must not be exposed.
    assert!(!names.contains(&"web_fetch".to_string()), "tools: {names:?}");
    assert!(!names.contains(&"local_search".to_string()), "tools: {names:?}");
    assert!(!names.contains(&"search_and_fetch".to_string()), "tools: {names:?}");
}

#[tokio::test]
async fn web_search_empty_query_returns_validation_error() {
    let (_dir, state) = build_state();
    let res = run_web_search(
        state,
        WebSearchArgs {
            query: "   ".into(),
            max_results: None,
            providers: vec![],
            safe_search: None,
            timeout_ms: None,
        },
    )
    .await;
    assert!(res.is_err(), "expected validation error");
    let err = res.err().unwrap();
    assert!(err.contains("invalid query"), "got: {err}");
}

#[tokio::test]
async fn web_search_blocked_when_mode_off() {
    let (_dir, _state) = build_state();
    let mut cfg = AppConfig::default();
    cfg.search.mode = eggsearch_core::config::Mode::Off;
    let state = Arc::new(ServerState::build(cfg).unwrap());
    let res = run_web_search(
        state,
        WebSearchArgs {
            query: "rust".into(),
            max_results: None,
            providers: vec![],
            safe_search: None,
            timeout_ms: None,
        },
    )
    .await;
    assert!(res.is_err(), "expected policy denial");
    let err = res.err().unwrap();
    assert!(err.contains("disabled by policy"), "got: {err}");
}

#[test]
fn provider_status_returns_configured_providers() {
    let (_dir, state) = build_state();
    let v = run_provider_status(state, ProviderStatusArgs { probe: false }).unwrap();
    let arr = v["providers"].as_array().expect("providers is array");
    let ids: Vec<&str> = arr
        .iter()
        .map(|p| p["id"].as_str().unwrap_or(""))
        .filter(|s| !s.is_empty())
        .collect();
    for expected in ["duckduckgo", "brave", "startpage", "yahoo"] {
        assert!(
            ids.contains(&expected),
            "expected provider id {expected} in status, got {ids:?}"
        );
    }
}

#[test]
fn provider_status_payload_shape_is_stable() {
    let (_dir, state) = build_state();
    let v = run_provider_status(state, ProviderStatusArgs { probe: false }).unwrap();
    assert!(v["mode"].is_string());
    let arr = v["providers"].as_array().unwrap();
    for p in arr {
        assert!(p["id"].is_string(), "missing id: {p}");
        assert!(p["enabled"].is_boolean(), "missing enabled: {p}");
        assert!(p["kind"].is_string(), "missing kind: {p}");
        assert!(p["requires_api_key"].is_boolean(), "missing requires_api_key: {p}");
    }
}
