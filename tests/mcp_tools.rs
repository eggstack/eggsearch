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

#[test]
fn provider_status_pdf_capabilities_track_feature_and_config() {
    for pdf_enabled in [false, true] {
        let mut cfg = AppConfig::default();
        cfg.fetch.pdf_enabled = pdf_enabled;
        let state = Arc::new(ServerState::build(cfg).expect("state builds"));
        let v = run_provider_status(
            state,
            ProviderStatusArgs {
                probe: false,
                recipe_detail: None,
            },
        )
        .expect("provider status");

        let caps = &v["pdf_capabilities"];
        assert_eq!(
            caps["compiled"],
            cfg!(feature = "pdf"),
            "compiled must match the build feature"
        );
        assert_eq!(
            caps["configured"], pdf_enabled,
            "configured must match [fetch].pdf_enabled"
        );
        assert_eq!(
            caps["usable"],
            cfg!(feature = "pdf") && pdf_enabled,
            "usable must agree with feature and config"
        );
        assert_eq!(caps["layout"], "deferred");
        assert_eq!(caps["ocr"], "deferred");
    }
}

#[test]
fn provider_status_reports_active_mcp_advertisement() {
    let cases = [
        (
            eggsearch::core::config::McpToolPreset::Full,
            10usize,
            0usize,
        ),
        (eggsearch::core::config::McpToolPreset::Research, 7, 3),
        (eggsearch::core::config::McpToolPreset::Code, 8, 2),
        (eggsearch::core::config::McpToolPreset::Minimal, 4, 6),
    ];
    for (preset, advertised, hidden) in cases {
        let mut cfg = AppConfig::default();
        cfg.mcp.tool_preset = preset;
        let state = Arc::new(ServerState::build(cfg).expect("state builds"));
        let payload = run_provider_status(
            state,
            ProviderStatusArgs {
                probe: false,
                recipe_detail: None,
            },
        )
        .expect("provider status");
        let advertisement = &payload["mcp_advertisement"];
        assert_eq!(advertisement["tool_preset"], preset.as_str());
        assert_eq!(advertisement["schema_profile"], "full");
        assert_eq!(
            advertisement["advertised_count"].as_u64().unwrap(),
            advertised as u64,
            "{preset:?} advertised count"
        );
        assert_eq!(
            advertisement["advertised_tools"].as_array().unwrap().len(),
            advertised,
            "{preset:?} advertised names"
        );
        assert_eq!(
            advertisement["hidden_tools"].as_array().unwrap().len(),
            hidden,
            "{preset:?} hidden count"
        );
        for name in advertisement["advertised_tools"].as_array().unwrap() {
            assert!(
                eggsearch::mcp::tool_contract::is_known_tool(name.as_str().unwrap()),
                "{preset:?} advertises a non-canonical name: {name}"
            );
        }
        assert!(
            payload.get("next_actions_omitted").is_none(),
            "provider_status builds an unfiltered payload; the response seam filters"
        );
    }
}

// ---- M009 long-running progress and pagination ----

mod pagination {
    use super::*;
    use eggsearch::core::batch_fetch::BatchFetchItem;
    use eggsearch::mcp::cursor::{CursorTool, RetrievalCursor};
    use eggsearch::mcp::progress::ProgressSink;
    use httpmock::prelude::*;

    fn long_page_server(chars: usize) -> MockServer {
        let server = MockServer::start();
        let paragraph = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu ";
        let mut body = String::from("<html><body>");
        while body.len() < chars {
            body.push_str("<p>");
            body.push_str(paragraph);
            body.push_str(paragraph);
            body.push_str("</p>");
        }
        body.push_str("</body></html>");
        server.mock(move |when, then| {
            when.method(GET).path("/long");
            then.status(200)
                .header("content-type", "text/html")
                .body(body);
        });
        server
    }

    fn small_page_server() -> MockServer {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/small");
            then.status(200)
                .header("content-type", "text/plain")
                .body("short document body");
        });
        server
    }

    fn fetch_args(url: String) -> WebFetchArgs {
        WebFetchArgs {
            cursor: None,
            url,
            max_chars: None,
            timeout_ms: None,
            extract_mode: None,
            include_links: None,
            pdf: None,
            cache_policy: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
            render: None,
            browser_profile: None,
            response_detail: None,
        }
    }

    fn chunk_ids(value: &serde_json::Value) -> Vec<String> {
        value["document"]["chunks"]
            .as_array()
            .map(|chunks| {
                chunks
                    .iter()
                    .filter_map(|chunk| chunk["chunk_id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn legacy_client_payload_has_no_cursor_keys() {
        let server = small_page_server();
        let state = state_with_localhost();
        let value = run_web_fetch(state, fetch_args(server.url("/small")))
            .await
            .expect("fetch works");
        assert!(value.get("next_cursor").is_none());
        assert!(value.get("cursor_status").is_none());
        assert!(
            value["text"]
                .as_str()
                .unwrap_or_default()
                .contains("short document body"),
            "legacy text: {}",
            value["text"]
        );
        assert_eq!(value["fetched"], true);
    }

    #[tokio::test]
    async fn web_fetch_pages_cover_every_chunk_once() {
        let server = long_page_server(40_000);
        let state = state_with_localhost();
        let first = run_web_fetch(state.clone(), fetch_args(server.url("/long")))
            .await
            .expect("first page");
        let next = first["next_cursor"]
            .as_str()
            .expect("first page must offer a cursor")
            .to_string();
        let decoded = RetrievalCursor::decode(&next).expect("cursor round-trips");
        assert_eq!(decoded.t, CursorTool::WebFetch);
        assert_eq!(decoded.timeout_ms(), state.config.fetch.timeout_ms);

        let mut args = fetch_args(server.url("/long"));
        args.cursor = Some(next);
        let second = run_web_fetch(state.clone(), args)
            .await
            .expect("second page");
        assert_eq!(second["cursor_status"], "ok");
        assert!(second.get("next_cursor").is_none(), "chain must terminate");

        let mut first_ids = chunk_ids(&first);
        let second_ids = chunk_ids(&second);
        assert!(!first_ids.is_empty() && !second_ids.is_empty());
        assert_eq!(first_ids.len() as u64, u64::from(decoded.p.ord));
        let before = first_ids.len();
        first_ids.extend(second_ids);
        first_ids.sort();
        first_ids.dedup();
        assert_eq!(
            first_ids.len(),
            before + chunk_ids(&second).len(),
            "pages must not repeat a chunk"
        );
    }

    #[tokio::test]
    async fn stale_cursor_returns_the_complete_first_page() {
        let server = long_page_server(40_000);
        let state = state_with_localhost();
        let first = run_web_fetch(state.clone(), fetch_args(server.url("/long")))
            .await
            .expect("first page");
        let next = first["next_cursor"].as_str().unwrap().to_string();
        let mut forged = RetrievalCursor::decode(&next).unwrap();
        forged.p.id = "chunk_ffffffffffffffff".to_string();
        let mut args = fetch_args(server.url("/long"));
        args.cursor = Some(forged.encode().unwrap());
        let stale = run_web_fetch(state, args)
            .await
            .expect("stale is not an error");
        assert_eq!(stale["cursor_status"], "stale");
        assert!(stale.get("next_cursor").is_none());
        assert_eq!(chunk_ids(&stale), chunk_ids(&first));
    }

    #[tokio::test]
    async fn cursor_from_another_tool_is_rejected() {
        let server = long_page_server(40_000);
        let state = state_with_localhost();
        let first = run_web_fetch(state.clone(), fetch_args(server.url("/long")))
            .await
            .expect("first page");
        let next = first["next_cursor"].as_str().unwrap().to_string();
        let mut forged = RetrievalCursor::decode(&next).unwrap();
        forged.t = CursorTool::BatchFetch;
        let mut args = fetch_args(server.url("/long"));
        args.cursor = Some(forged.encode().unwrap());
        let error = run_web_fetch(state, args).await.expect_err("mismatch");
        assert_eq!(error.code().as_str(), "cursor_mismatch");
    }

    #[tokio::test]
    async fn cursor_for_another_request_is_rejected() {
        let server = long_page_server(40_000);
        let state = state_with_localhost();
        let first = run_web_fetch(state.clone(), fetch_args(server.url("/long")))
            .await
            .expect("first page");
        let next = first["next_cursor"].as_str().unwrap().to_string();
        let mut args = fetch_args(server.url("/long"));
        args.max_chars = Some(4_096);
        args.cursor = Some(next);
        let error = run_web_fetch(state, args)
            .await
            .expect_err("plan key mismatch");
        assert_eq!(error.code().as_str(), "cursor_mismatch");
    }

    #[tokio::test]
    async fn oversized_and_malformed_cursors_are_rejected() {
        let server = small_page_server();
        let state = state_with_localhost();
        let mut args = fetch_args(server.url("/small"));
        args.cursor = Some("A".repeat(600));
        let error = run_web_fetch(state.clone(), args)
            .await
            .expect_err("oversize");
        assert_eq!(error.code().as_str(), "budget_invalid");

        let mut args = fetch_args(server.url("/small"));
        args.cursor = Some("e1.not-base64!!".to_string());
        let error = run_web_fetch(state, args).await.expect_err("malformed");
        assert_eq!(error.code().as_str(), "budget_invalid");
    }

    #[tokio::test]
    async fn progress_token_never_changes_the_payload() {
        let server = long_page_server(40_000);
        let state = state_with_localhost();
        let mut bypass = fetch_args(server.url("/long"));
        bypass.cache_policy = Some(eggsearch::core::fetch::FetchCachePolicy::Bypass);
        let plain = run_web_fetch(state.clone(), bypass.clone())
            .await
            .expect("plain fetch");
        let mut sink = ProgressSink::for_test(rmcp::model::ProgressToken(
            rmcp::model::NumberOrString::String("progress-token".into()),
        ));
        let observed = eggsearch::mcp::tools::run_web_fetch_with_progress(state, bypass, &mut sink)
            .await
            .expect("fetch with progress");
        assert!(sink.is_enabled());
        assert!(sink.emitted_count() >= 1);
        assert_eq!(plain, observed);
    }

    fn batch_items(server: &MockServer, count: usize) -> Vec<BatchFetchItem> {
        (0..count)
            .map(|index| BatchFetchItem::Web {
                url: server.url(format!("/item{index}")),
                extract_mode: None,
                include_links: None,
                max_chars: None,
                cache_policy: None,
                max_cache_age_seconds: None,
                focus: None,
                focus_max_chunks: None,
                focus_max_chars: None,
            })
            .collect()
    }

    #[tokio::test]
    async fn batch_fetch_pages_every_item_once() {
        let server = MockServer::start();
        for index in 0..10 {
            server.mock(move |when, then| {
                when.method(GET).path(format!("/item{index}"));
                then.status(200)
                    .header("content-type", "text/plain")
                    .body(format!("body {index}"));
            });
        }
        let state = state_with_localhost();
        let mut args = BatchFetchArgs {
            cursor: None,
            items: batch_items(&server, 10),
            max_items: Some(4),
            max_chars_per_item: None,
            max_total_chars: None,
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        };
        let mut seen: Vec<u64> = Vec::new();
        let mut pages = 0;
        loop {
            let value = run_batch_fetch(state.clone(), args.clone())
                .await
                .expect("batch page");
            pages += 1;
            for result in value["results"].as_array().expect("results") {
                seen.push(result["index"].as_u64().expect("index"));
            }
            let Some(next) = value["next_cursor"].as_str().map(str::to_string) else {
                break;
            };
            args.cursor = Some(next);
        }
        assert_eq!(seen, (0..10u64).collect::<Vec<_>>());
        assert_eq!(pages, 3);
    }

    #[tokio::test]
    async fn batch_fetch_above_page_capacity_still_terminates() {
        let server = MockServer::start();
        for index in 0..20 {
            server.mock(move |when, then| {
                when.method(GET).path(format!("/item{index}"));
                then.status(200)
                    .header("content-type", "text/plain")
                    .body(format!("body {index}"));
            });
        }
        let state = state_with_localhost();
        let mut args = BatchFetchArgs {
            cursor: None,
            items: batch_items(&server, 20),
            max_items: Some(3),
            max_chars_per_item: None,
            max_total_chars: None,
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        };
        let mut seen: Vec<u64> = Vec::new();
        for _ in 0..16 {
            let value = run_batch_fetch(state.clone(), args.clone())
                .await
                .expect("batch page");
            for result in value["results"].as_array().expect("results") {
                seen.push(result["index"].as_u64().expect("index"));
            }
            match value["next_cursor"].as_str() {
                Some(next) => args.cursor = Some(next.to_string()),
                None => break,
            }
        }
        assert_eq!(seen, (0..20u64).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn batch_fetch_cursor_mismatch_is_typed() {
        let server = long_page_server(40_000);
        let state = state_with_localhost();
        let first = run_web_fetch(state.clone(), fetch_args(server.url("/long")))
            .await
            .expect("first page");
        let cursor = first["next_cursor"].as_str().unwrap().to_string();
        let small = small_page_server();
        let args = BatchFetchArgs {
            cursor: Some(cursor),
            items: batch_items(&small, 1),
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: None,
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        };
        let error = run_batch_fetch(state, args).await.expect_err("mismatch");
        assert_eq!(error.code().as_str(), "cursor_mismatch");
    }
}

#[cfg(feature = "mock")]
mod research_pagination {
    use super::*;
    use eggsearch::mcp::cursor::RetrievalCursor;
    use eggsearch::mcp::tools::{run_research_search, ResearchSearchArgs};

    fn research_args(query: &str, max_groups: Option<usize>) -> ResearchSearchArgs {
        ResearchSearchArgs {
            cursor: None,
            query: query.to_string(),
            max_groups,
            providers: vec!["mock_a".to_string()],
            ..ResearchSearchArgs::default()
        }
    }

    fn multi_group_state() -> (Arc<ServerState>, Vec<&'static str>) {
        let results = vec![
            MockResult::new(
                "Tokio docs",
                "https://docs.rs/tokio/latest/tokio/",
                "mock_a",
            ),
            MockResult::new(
                "Tokio source",
                "https://github.com/tokio-rs/tokio/blob/master/README.md",
                "mock_a",
            ),
            MockResult::new(
                "Tokio issue",
                "https://github.com/tokio-rs/tokio/issues/1234",
                "mock_a",
            ),
            MockResult::new(
                "Tokio discussion",
                "https://news.ycombinator.com/item?id=1",
                "mock_a",
            ),
            MockResult::new(
                "HTTP semantics",
                "https://www.ietf.org/rfc/rfc9110.html",
                "mock_a",
            ),
        ];
        let engines = vec![MockEngine::success("mock_a", results)];
        let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        (state, Vec::new())
    }

    #[tokio::test]
    async fn research_search_pages_groups_without_gap_or_duplication() {
        let (state, _) = multi_group_state();
        let mut args = research_args("tokio runtime", Some(2));
        let mut labels: Vec<String> = Vec::new();
        let mut pages = 0;
        for _ in 0..8 {
            let value = run_research_search(state.clone(), args.clone())
                .await
                .expect("research page");
            pages += 1;
            for group in value["groups"].as_array().expect("groups") {
                labels.push(group["label"].as_str().unwrap_or_default().to_string());
            }
            match value["next_cursor"].as_str() {
                Some(next) => {
                    RetrievalCursor::decode(next).expect("cursor round-trips");
                    args.cursor = Some(next.to_string());
                }
                None => break,
            }
        }
        assert!(pages > 1, "multiple groups must paginate: {labels:?}");
        let unique: std::collections::BTreeSet<&String> = labels.iter().collect();
        assert_eq!(
            labels.len(),
            unique.len(),
            "no group may repeat: {labels:?}"
        );
    }

    #[tokio::test]
    async fn research_search_wide_max_groups_is_single_page() {
        let (state, _) = multi_group_state();
        let value = run_research_search(state, research_args("tokio runtime", None))
            .await
            .expect("research page");
        assert!(value.get("next_cursor").is_none());
    }
}
