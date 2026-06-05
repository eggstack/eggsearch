use eggsearch_core::config::{AppConfig, Mode, ProviderConfig};
use eggsearch_meta::registry::DiagnosticStatus;
use eggsearch_mcp::{EggsearchServer, ServerState};
use rmcp::ServerHandler;
use std::sync::Arc;
use tempfile::tempdir;

fn build_test_state(mode: Mode) -> (tempfile::TempDir, Arc<ServerState>) {
    build_test_state_with(mode, |_| {})
}

fn build_test_state_with<F>(mode: Mode, mut f: F) -> (tempfile::TempDir, Arc<ServerState>)
where
    F: FnMut(&mut AppConfig),
{
    let dir = tempdir().unwrap();
    let mut cfg = AppConfig::default();
    cfg.search.mode = mode;
    cfg.search.local.index_dir = dir.path().to_path_buf();
    f(&mut cfg);
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

#[test]
fn server_state_collects_provider_diagnostics() {
    let (_dir, state) = build_test_state_with(Mode::default(), |cfg| {
        cfg.search.providers.insert(
            "searxng".into(),
            ProviderConfig {
                enabled: true,
                ..Default::default()
            },
        );
        cfg.search.providers.insert(
            "brave".into(),
            ProviderConfig {
                enabled: true,
                api_key_env: Some("EGGSEARCH_TEST_BRAVE_MCP".into()),
                ..Default::default()
            },
        );
    });
    let diags = &state.diagnostics.diagnostics;
    let searxng = diags.iter().find(|d| d.id == "searxng").unwrap();
    assert_eq!(searxng.status, DiagnosticStatus::Misconfigured);
    assert!(searxng
        .message
        .as_deref()
        .unwrap()
        .contains("base_url"));
    let brave = diags.iter().find(|d| d.id == "brave").unwrap();
    assert_eq!(brave.status, DiagnosticStatus::Misconfigured);
    assert!(brave
        .message
        .as_deref()
        .unwrap()
        .contains("EGGSEARCH_TEST_BRAVE_MCP"));
    // Misconfigured providers must NOT be in the loaded list.
    assert!(!state.diagnostics.loaded.iter().any(|p| p == "searxng"));
    assert!(!state.diagnostics.loaded.iter().any(|p| p == "brave"));
    // But the no-key providers should still load.
    for id in ["duckduckgo_html", "wikipedia", "crates_io", "docs_rs"] {
        assert!(
            state.diagnostics.loaded.iter().any(|p| p == id),
            "expected {id} to be loaded, got {:?}",
            state.diagnostics.loaded
        );
    }
}

#[test]
fn server_state_with_searxng_base_url_loads_it() {
    let (_dir, state) = build_test_state_with(Mode::default(), |cfg| {
        cfg.search.providers.insert(
            "searxng".into(),
            ProviderConfig {
                enabled: true,
                base_url: Some("http://127.0.0.1:8080".into()),
                ..Default::default()
            },
        );
    });
    assert!(state.diagnostics.loaded.iter().any(|p| p == "searxng"));
    let searxng = state
        .diagnostics
        .diagnostics
        .iter()
        .find(|d| d.id == "searxng")
        .unwrap();
    assert_eq!(searxng.status, DiagnosticStatus::Loaded);
}

#[test]
fn default_providers_appear_in_diagnostics() {
    // Regression: the default config registers 8 known provider ids.
    let (_dir, state) = build_test_state(Mode::default());
    let ids: std::collections::BTreeSet<&str> = state
        .diagnostics
        .diagnostics
        .iter()
        .map(|d| d.id.as_str())
        .collect();
    for expected in [
        "duckduckgo_html",
        "wikipedia",
        "crates_io",
        "docs_rs",
        "searxng",
        "brave",
        "tavily",
        "exa",
    ] {
        assert!(ids.contains(expected), "missing provider id {expected} in diagnostics");
    }
}
