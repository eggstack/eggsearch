use eggsearch_core::config::{AppConfig, Mode};
use eggsearch_mcp::{EggsearchServer, ServerState};
use rmcp::ServerHandler;
use std::sync::Arc;
use tempfile::tempdir;

fn build_test_state(mode: Mode) -> (tempfile::TempDir, Arc<ServerState>) {
    let dir = tempdir().unwrap();
    let mut cfg = AppConfig::default();
    cfg.search.mode = mode;
    cfg.search.local.index_dir = dir.path().to_path_buf();
    let state = Arc::new(ServerState::build(cfg).unwrap());
    (dir, state)
}

#[test]
fn mcp_server_get_info() {
    let (_dir, state) = build_test_state(Mode::default());
    let server = EggsearchServer::new(state);
    let info = server.get_info();
    assert_eq!(info.server_info.name, "eggsearch");
    assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    let caps = info.capabilities;
    assert!(caps.tools.is_some(), "tools capability must be enabled");
}

#[test]
fn mcp_server_lists_four_tools() {
    let (_dir, state) = build_test_state(Mode::default());
    let server = EggsearchServer::new(state);
    let tools = server.tool_definitions();
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert!(names.contains(&"web_search".to_string()));
    assert!(names.contains(&"web_fetch".to_string()));
    assert!(names.contains(&"local_search".to_string()));
    assert!(names.contains(&"search_and_fetch".to_string()));
}

#[tokio::test]
async fn web_search_blocked_when_mode_off() {
    let (_dir, state) = build_test_state(Mode::Off);
    let res = eggsearch_mcp::tools::run_web_search(
        state,
        eggsearch_mcp::tools::WebSearchArgs {
            query: "rust".into(),
            max_results: Some(3),
            providers: vec![],
            fetch: false,
            max_excerpt_chars: None,
        },
    )
    .await;
    assert!(res.is_err(), "expected policy denial");
}

#[tokio::test]
async fn local_search_returns_empty_when_index_empty() {
    let (_dir, state) = build_test_state(Mode::LocalOnly);
    let result = eggsearch_mcp::tools::run_local_search(
        state,
        eggsearch_mcp::tools::LocalSearchArgs {
            query: "anything".into(),
            max_results: Some(5),
            tags: vec![],
        },
    )
    .await
    .unwrap();
    let v = result.as_object().unwrap();
    assert_eq!(v["mode"], "local_only");
    let results = v["results"].as_array().unwrap();
    assert!(results.is_empty());
    let warnings = v["warnings"].as_array().unwrap();
    assert!(!warnings.is_empty(), "expected warning about empty index");
}

#[tokio::test]
async fn local_search_blocked_when_mode_off() {
    let (_dir, state) = build_test_state(Mode::Off);
    let res = eggsearch_mcp::tools::run_local_search(
        state,
        eggsearch_mcp::tools::LocalSearchArgs {
            query: "anything".into(),
            max_results: Some(5),
            tags: vec![],
        },
    )
    .await;
    assert!(res.is_err());
}
