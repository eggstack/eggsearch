#![allow(unused_imports, dead_code)]
//! MCP tool surface registration.
//!
//! Behavior-oriented suite partitioned from the historical `integration.rs` mega-suite.

use std::sync::Arc;

use eggsearch::core::config::{AppConfig, Mode};
use eggsearch::core::fetch::ExtractMode;
use eggsearch::core::workflow::RecipeDetail;
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{
    run_batch_fetch, run_provider_status, run_repo_fetch, run_repo_map, run_web_fetch,
    run_web_search, BatchFetchArgs, ProviderStatusArgs, RepoFetchArgs, RepoMapArgs, WebFetchArgs,
    WebSearchArgs,
};
#[cfg(feature = "mock")]
use eggsearch::mcp::tools::{
    run_repo_search, run_security_search, RepoSearchArgs, SecuritySearchArgs,
};
use rmcp::ServerHandler;

#[cfg(feature = "mock")]
use eggsearch::meta::mock::{
    mock_engines, MockEngine, MockFailure, MockResult, RecordingMockEngine,
};
#[cfg(feature = "mock")]
use eggsearch::meta::MetadataSearchAdapter;
use std::fs;
use std::time::Duration;

fn state_with_default() -> Arc<ServerState> {
    Arc::new(ServerState::build(AppConfig::default()).expect("default state"))
}

fn state_with_localhost() -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    Arc::new(ServerState::build(cfg).expect("state builds"))
}

fn state_with_mode_off() -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.search.mode = Mode::Off;
    Arc::new(ServerState::build(cfg).expect("off state"))
}

#[cfg(feature = "mock")]
fn state_with_engines(
    cfg: AppConfig,
    engines: Vec<MockEngine>,
    timeout: Duration,
) -> Arc<ServerState> {
    let adapter = MetadataSearchAdapter::from_engines(mock_engines(engines), timeout);
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

#[cfg(feature = "mock")]
fn state_with_arc_engines(
    cfg: AppConfig,
    engines: Vec<Arc<dyn eggsearch::meta::engines::SearchEngine>>,
    timeout: Duration,
) -> Arc<ServerState> {
    let adapter = MetadataSearchAdapter::from_engines(engines, timeout);
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

#[cfg(feature = "mock")]
fn state_with_engines_sanitize(
    cfg: AppConfig,
    engines: Vec<MockEngine>,
    timeout: Duration,
    sanitize: bool,
) -> Arc<ServerState> {
    let adapter =
        MetadataSearchAdapter::from_engines_with_sanitize(mock_engines(engines), timeout, sanitize);
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

#[cfg(feature = "mock")]
fn test_cfg() -> AppConfig {
    let mut cfg = AppConfig::default();
    cfg.search.timeout_ms = 2_000;
    cfg.search.max_query_chars = 256;
    cfg.search.default_max_results = 10;
    cfg.search.max_results_cap = 50;
    // Register mock provider ids so resolve_providers() accepts them.
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.search.providers.insert("mock_b".to_string(), true);
    cfg
}

/// Build a `WebSearchArgs` that uses the given mock provider ids
/// instead of the configured defaults.
#[cfg(feature = "mock")]
fn args_for(providers: &[&'static str], query: &'static str) -> WebSearchArgs {
    WebSearchArgs {
        query: query.into(),
        max_results: None,
        providers: providers.iter().map(|s| s.to_string()).collect(),
        safe_search: None,
        timeout_ms: None,
        intent: None,
        freshness: None,
        date_range: None,
        include_domains: Vec::new(),
        exclude_domains: Vec::new(),
        language: None,
        region: None,
        excerpt_count: None,
        response_detail: None,
    }
}

#[test]
fn mcp_server_get_info() {
    let state = state_with_default();
    let server = eggsearch::mcp::EggsearchServer::new(state);
    let info = server.get_info();
    assert_eq!(info.server_info.name, "eggsearch");
    assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    assert!(
        info.capabilities.tools.is_some(),
        "tools capability must be enabled"
    );
    // Server instructions should mention both tools by name so a host
    // agent can discover them from the initialize handshake.
    let instructions = info.instructions.unwrap_or_default();
    assert!(
        instructions.contains("web_search"),
        "instructions should mention web_search: {instructions}"
    );
    assert!(
        instructions.contains("provider_status"),
        "instructions should mention provider_status: {instructions}"
    );
    // Instructions must not suggest crawling is conditionally allowed.
    assert!(
        instructions.contains("Do not use web_fetch as a crawler"),
        "instructions should contain anti-crawling wording: {instructions}"
    );
    assert!(
        instructions.contains("one explicit HTTP(S) URL"),
        "instructions should mention one explicit URL: {instructions}"
    );
    assert!(
        !instructions.contains("unless the user explicitly asks for research"),
        "instructions must not contain crawling-permissive wording: {instructions}"
    );
}

#[test]
fn mcp_server_lists_three_tools() {
    let state = state_with_default();
    let server = eggsearch::mcp::EggsearchServer::new(state);
    let tools = server.tool_definitions();
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert!(
        names.contains(&"web_search".to_string()),
        "tools: {names:?}"
    );
    assert!(names.contains(&"web_fetch".to_string()), "tools: {names:?}");
    assert!(
        names.contains(&"provider_status".to_string()),
        "tools: {names:?}"
    );
    // Legacy tools must not be exposed.
    for legacy in ["local_search", "search_and_fetch"] {
        assert!(
            !names.contains(&legacy.to_string()),
            "legacy tool {legacy} must not be exposed: {names:?}"
        );
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_fetch_tool_listed() {
    let state = state_with_default();
    let server = eggsearch::mcp::EggsearchServer::new(state);
    let tools = server.tool_definitions();
    let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert!(
        tool_names.contains(&"web_fetch".to_string()),
        "web_fetch should be in tools list: {tool_names:?}"
    );
}

#[tokio::test]
#[cfg(feature = "mock")]
async fn all_ten_stable_tools_registered() {
    let state = state_with_default();
    let server = eggsearch::mcp::EggsearchServer::new(state);
    let tools = server.tool_definitions();
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();

    let expected = [
        "web_search",
        "web_fetch",
        "batch_fetch",
        "provider_status",
        "repo_search",
        "repo_fetch",
        "repo_map",
        "security_search",
        "research_search",
        "build_evidence_bundle",
    ];

    for name in &expected {
        assert!(
            names.contains(&name.to_string()),
            "stable tool `{name}` not found in tool_definitions(); registered tools: {names:?}"
        );
    }
    assert_eq!(
        names.len(),
        expected.len(),
        "expected exactly {} stable tools, got {}: {names:?}",
        expected.len(),
        names.len()
    );
}

fn fetch_disabled_state() -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.fetch.enabled = false;
    Arc::new(ServerState::build(cfg).expect("state with fetch disabled"))
}

#[cfg(feature = "mock")]
#[test]
fn mcp_tool_surface_all_ten_tools_with_mock_state() {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));
    let server = eggsearch::mcp::EggsearchServer::new(state);
    let tools = server.tool_definitions();
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();

    assert_eq!(names.len(), 10, "expected exactly 10 tools, got: {names:?}");
    assert!(
        names.contains(&"web_search".to_string()),
        "missing web_search: {names:?}"
    );
    assert!(
        names.contains(&"web_fetch".to_string()),
        "missing web_fetch: {names:?}"
    );
    assert!(
        names.contains(&"provider_status".to_string()),
        "missing provider_status: {names:?}"
    );
    assert!(
        names.contains(&"repo_search".to_string()),
        "missing repo_search: {names:?}"
    );
    assert!(
        names.contains(&"repo_fetch".to_string()),
        "missing repo_fetch: {names:?}"
    );
    assert!(
        names.contains(&"repo_map".to_string()),
        "missing repo_map: {names:?}"
    );
    assert!(
        names.contains(&"security_search".to_string()),
        "missing security_search: {names:?}"
    );
    assert!(
        names.contains(&"research_search".to_string()),
        "missing research_search: {names:?}"
    );
    assert!(
        names.contains(&"batch_fetch".to_string()),
        "missing batch_fetch: {names:?}"
    );
    assert!(
        names.contains(&"build_evidence_bundle".to_string()),
        "missing build_evidence_bundle: {names:?}"
    );

    // Verify the tools have non-empty descriptions (MCP contract).
    for tool in &tools {
        assert!(
            !tool.description.as_deref().unwrap_or("").is_empty(),
            "tool '{}' should have a non-empty description",
            tool.name
        );
    }
}

// ---------------------------------------------------------------------------
// Task 8: MCP-Level web_fetch Test With Local HTTP Server
//
// Verifies the web_fetch tool works end-to-end through the MCP layer
// against a local HTTP server, checking response shape, trust label,
// trust_markers, and minimal sanitize/framing behavior.
// ---------------------------------------------------------------------------

fn repo_fetch_state() -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.fetch.sanitize_output = false;
    Arc::new(ServerState::build(cfg).expect("repo_fetch state"))
}

fn git_cmd() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "safe.directory")
        .env("GIT_CONFIG_VALUE_0", "*");
    cmd
}

#[cfg(feature = "mock")]
fn state_with_local_backend(temp_dir: &std::path::Path) -> Arc<ServerState> {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter = MetadataSearchAdapter::from_engines(
        eggsearch::meta::mock::mock_engines(engines),
        Duration::from_secs(5),
    );
    let mut cfg = AppConfig::default();
    cfg.search.timeout_ms = 30_000;
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.local.enabled = true;
    cfg.local.roots = vec![temp_dir.to_path_buf()];
    let backend = eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg.local.clone())
        .expect("backend builds");
    backend.get_or_build_inventory();
    let mut state = ServerState::with_adapter(cfg, Arc::new(adapter));
    state.local_backend = Some(Arc::new(backend));
    Arc::new(state)
}

#[cfg(feature = "mock")]
fn state_with_local_backend_sanitize(
    temp_dir: &std::path::Path,
    sanitize: bool,
) -> Arc<ServerState> {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter = MetadataSearchAdapter::from_engines_with_sanitize(
        mock_engines(engines),
        Duration::from_secs(5),
        sanitize,
    );
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.local.enabled = true;
    cfg.local.roots = vec![temp_dir.to_path_buf()];
    cfg.fetch.sanitize_output = sanitize;
    let backend = eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg.local.clone())
        .expect("backend builds");
    backend.get_or_build_inventory();
    let mut state = ServerState::with_adapter(cfg, Arc::new(adapter));
    state.local_backend = Some(Arc::new(backend));
    Arc::new(state)
}

#[cfg(feature = "mock")]
fn state_with_local_backend_mode_off(temp_dir: &std::path::Path) -> Arc<ServerState> {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter = MetadataSearchAdapter::from_engines(
        eggsearch::meta::mock::mock_engines(engines),
        Duration::from_secs(5),
    );
    let mut cfg = AppConfig::default();
    cfg.search.mode = Mode::Off;
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.local.enabled = true;
    cfg.local.roots = vec![temp_dir.to_path_buf()];
    let backend = eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg.local.clone())
        .expect("backend builds");
    backend.get_or_build_inventory();
    let mut state = ServerState::with_adapter(cfg, Arc::new(adapter));
    state.local_backend = Some(Arc::new(backend));
    Arc::new(state)
}

#[cfg(feature = "mock")]
fn state_with_local_backend_mode_off_for_repo_map(temp_dir: &std::path::Path) -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.search.mode = Mode::Off;
    cfg.local.enabled = true;
    cfg.local.roots = vec![temp_dir.to_path_buf()];
    let backend = eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg.local.clone())
        .expect("backend builds");
    backend.get_or_build_inventory();
    let state = ServerState::build(cfg).expect("state");
    let mut state = state;
    state.local_backend = Some(Arc::new(backend));
    Arc::new(state)
}

#[cfg(feature = "mock")]
fn run_git_checked(cmd: &mut std::process::Command, operation: &str) {
    let output = cmd
        .output()
        .unwrap_or_else(|error| panic!("{operation} could not start: {error}"));
    assert!(
        output.status.success(),
        "{operation} failed with status {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

#[cfg(feature = "mock")]
fn setup_git_repo_with_remote(root: &std::path::Path, remote_url: &str, _owner: &str, _repo: &str) {
    fs::write(root.join("main.rs"), "fn main() {}").unwrap();

    git_cmd().arg("init").arg(root).output().ok();

    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg(remote_url)
        .output()
        .ok();

    // Create initial commit so dirty state detection works
    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("add")
        .arg(".")
        .output()
        .ok();
    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("-c")
        .arg("user.name=ci")
        .arg("-c")
        .arg("user.email=ci@test.com")
        .arg("commit")
        .arg("-m")
        .arg("init")
        .arg("--allow-empty")
        .output()
        .ok();
}
