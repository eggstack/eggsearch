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

    fn multibyte_page_server(chars: usize) -> MockServer {
        let server = MockServer::start();
        let paragraph = "héllo wörld — ünïcode ✓ 日本語 текст ";
        let mut body = String::from("<html><body>");
        while body.len() < chars {
            body.push_str("<p>");
            body.push_str(paragraph);
            body.push_str(paragraph);
            body.push_str("</p>");
        }
        body.push_str("</body></html>");
        server.mock(move |when, then| {
            when.method(GET).path("/uni");
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
            offset: None,
            limit: None,
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
        for key in [
            "text_from",
            "text_to",
            "text_total",
            "next_offset",
            "doc_id",
        ] {
            assert!(
                value.get(key).is_none(),
                "unaddressed payload carries {key}"
            );
        }
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
    async fn web_fetch_offset_windows_compose_to_the_whole_document() {
        let server = long_page_server(40_000);
        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.fetch.sanitize_output = false;
        let state = Arc::new(ServerState::build(cfg).expect("state builds"));
        let limit = 2_000usize;
        let mut offset = 0usize;
        let mut composed = String::new();
        let mut total: Option<usize> = None;
        let mut doc_id: Option<String> = None;
        let mut windows = 0usize;
        loop {
            let mut args = fetch_args(server.url("/long"));
            args.offset = Some(offset);
            args.limit = Some(limit);
            let page = run_web_fetch(state.clone(), args).await.expect("window");
            windows += 1;
            assert!(windows < 60, "the window chain must terminate");
            let from = page["text_from"].as_u64().expect("text_from") as usize;
            let to = page["text_to"].as_u64().expect("text_to") as usize;
            let reported = page["text_total"].as_u64().expect("text_total") as usize;
            assert_eq!(from, offset, "windows must start where the last ended");
            assert!(to > from, "every window must return addressable text");
            assert!(to <= reported);
            assert_eq!(*total.get_or_insert(reported), reported);
            let id = page["doc_id"].as_str().expect("doc_id").to_string();
            assert_eq!(*doc_id.get_or_insert(id), page["doc_id"].as_str().unwrap());
            composed.push_str(page["text"].as_str().unwrap_or_default());
            match page["next_offset"].as_u64() {
                Some(next) => {
                    assert_eq!(next as usize, to, "next_offset is the window end");
                    offset = next as usize;
                }
                None => break,
            }
        }
        assert!(windows > 2, "a 40k document must span several windows");
        assert_eq!(composed.chars().count(), total.expect("total"));
    }

    #[tokio::test]
    async fn addressed_web_fetch_rejects_a_clamped_plan_field() {
        let server = small_page_server();
        let state = state_with_localhost();
        let mut args = fetch_args(server.url("/small"));
        args.offset = Some(0);
        args.focus = Some("body".to_string());
        args.focus_max_chars = Some(state.config.fetch.max_chars_cap + 1);
        let error = run_web_fetch(state, args)
            .await
            .expect_err("a clamped plan field is a mismatch");
        assert_eq!(error.code().as_str(), "plan_mismatch");
    }

    #[tokio::test]
    async fn web_fetch_accepts_reference_compatibility_aliases() {
        let server = small_page_server();
        let state = state_with_localhost();
        let args: WebFetchArgs = serde_json::from_value(serde_json::json!({
            "url": server.url("/small"),
            "start_index": 6,
            "max_length": 8,
            "question": "document",
        }))
        .expect("reference-style fetch aliases deserialize");
        assert_eq!(args.offset, Some(6));
        assert_eq!(args.max_chars, Some(8));
        assert_eq!(args.focus.as_deref(), Some("document"));

        let page = run_web_fetch(state, args)
            .await
            .expect("aliased fetch succeeds");
        assert_eq!(page["text_from"], 6);
        assert_eq!(page["text_to"], 14);
        assert!(
            page["focus"]["chunks"].is_array(),
            "the question alias reaches focus selection: {page}"
        );
    }

    #[test]
    fn web_fetch_rejects_a_canonical_field_and_its_alias_together() {
        let error = serde_json::from_value::<WebFetchArgs>(serde_json::json!({
            "url": "https://example.com",
            "offset": 1,
            "start_index": 2,
        }))
        .expect_err("sending both spellings is ambiguous");
        assert!(
            error.to_string().contains("duplicate field"),
            "got: {error}"
        );
    }

    #[tokio::test]
    async fn windows_are_scalar_safe_on_multibyte_text() {
        let server = multibyte_page_server(20_000);
        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.fetch.sanitize_output = false;
        let state = Arc::new(ServerState::build(cfg).expect("state builds"));

        let mut whole = fetch_args(server.url("/uni"));
        whole.offset = Some(0);
        whole.limit = Some(50_000);
        let whole = run_web_fetch(state.clone(), whole).await.expect("whole");
        let total = whole["text_total"].as_u64().expect("text_total") as usize;
        assert!(total > 1_000, "fixture must exceed one small window");
        let whole_text = whole["text"].as_str().unwrap_or_default().to_string();
        assert_eq!(whole_text.chars().count(), total);
        assert!(
            !whole_text.is_ascii(),
            "the fixture must contain multi-byte scalars"
        );

        let limit = 1_001usize;
        let mut offset = 0usize;
        let mut composed = String::new();
        let mut windows = 0usize;
        loop {
            let mut args = fetch_args(server.url("/uni"));
            args.offset = Some(offset);
            args.limit = Some(limit);
            let page = run_web_fetch(state.clone(), args).await.expect("window");
            windows += 1;
            assert!(windows < 200, "the window chain must terminate");
            let from = page["text_from"].as_u64().expect("text_from") as usize;
            let to = page["text_to"].as_u64().expect("text_to") as usize;
            assert_eq!(from, offset);
            assert_eq!(to - from, limit.min(total - from));
            let text = page["text"].as_str().unwrap_or_default();
            assert_eq!(text.chars().count(), to - from);
            assert!(!text.contains('\u{FFFD}'), "a scalar must never be split");
            composed.push_str(text);
            match page["next_offset"].as_u64() {
                Some(next) => {
                    assert_eq!(next as usize, to);
                    offset = next as usize;
                }
                None => break,
            }
        }
        assert!(windows > 2, "the fixture must span several windows");
        assert_eq!(composed, whole_text);
    }

    #[tokio::test]
    async fn trust_framing_never_moves_an_offset() {
        let server = long_page_server(40_000);
        let mut plain_cfg = AppConfig::default();
        plain_cfg.fetch.allow_localhost = true;
        plain_cfg.fetch.allow_private_network = true;
        plain_cfg.fetch.sanitize_output = false;
        let plain_state = Arc::new(ServerState::build(plain_cfg).expect("state builds"));
        let framed_state = state_with_localhost();

        let mut plain_args = fetch_args(server.url("/long"));
        plain_args.offset = Some(1_000);
        plain_args.limit = Some(500);
        let plain = run_web_fetch(plain_state, plain_args)
            .await
            .expect("plain window");
        let mut framed_args = fetch_args(server.url("/long"));
        framed_args.offset = Some(1_000);
        framed_args.limit = Some(500);
        let framed = run_web_fetch(framed_state.clone(), framed_args)
            .await
            .expect("framed window");

        for key in [
            "text_from",
            "text_to",
            "text_total",
            "next_offset",
            "doc_id",
        ] {
            assert_eq!(plain[key], framed[key], "framing moved {key}");
        }
        let plain_text = plain["text"].as_str().unwrap_or_default();
        let framed_text = framed["text"].as_str().unwrap_or_default();
        assert_ne!(plain_text, framed_text, "framing must wrap the slice");
        assert!(
            framed_text.contains(plain_text),
            "framing must wrap the same slice, not a different one"
        );

        let total = framed["text_total"].as_u64().expect("text_total") as usize;
        let mut empty_args = fetch_args(server.url("/long"));
        empty_args.offset = Some(total);
        let empty = run_web_fetch(framed_state, empty_args)
            .await
            .expect("empty window");
        assert_eq!(empty["text"].as_str(), Some(""));
        assert_eq!(empty["text_from"].as_u64(), Some(total as u64));
        assert_eq!(empty["text_to"].as_u64(), Some(total as u64));
        assert!(empty.get("next_offset").is_none());

        let mut past_args = fetch_args(server.url("/long"));
        past_args.offset = Some(total + 50);
        past_args.limit = Some(10);
        let past = run_web_fetch(state_with_localhost(), past_args)
            .await
            .expect("an offset past the end clamps rather than failing");
        assert_eq!(past["text"].as_str(), Some(""));
        assert_eq!(past["text_from"].as_u64(), Some(total as u64));
        assert_eq!(past["text_to"].as_u64(), Some(total as u64));
        assert!(past.get("next_offset").is_none());
    }

    #[tokio::test]
    async fn addressed_reads_report_distinguishable_codes() {
        let server = small_page_server();
        let state = state_with_localhost();
        let mut args = fetch_args(server.url("/small"));
        args.offset = Some(0);
        args.extract_mode = Some(eggsearch::core::fetch::ExtractMode::MetadataOnly);
        let error = run_web_fetch(state.clone(), args)
            .await
            .expect_err("metadata_only has no addressable text");
        assert_eq!(error.code().as_str(), "document_unavailable");

        let mut args = fetch_args("http://127.0.0.1:1/refused".to_string());
        args.offset = Some(0);
        let error = run_web_fetch(state, args)
            .await
            .expect_err("connection refused");
        assert_eq!(error.code().as_str(), "upstream_failed");
    }

    #[tokio::test]
    async fn zero_limit_is_a_repairable_rejection() {
        let server = small_page_server();
        let state = state_with_localhost();
        let mut args = fetch_args(server.url("/small"));
        args.offset = Some(0);
        args.limit = Some(0);
        let error = run_web_fetch(state, args).await.expect_err("zero limit");
        assert_eq!(error.code().as_str(), "invalid_semantic_value");
        let result = eggsearch::mcp::tools::map_tool_result(Err(error)).expect("maps");
        let payload = result.structured_content.expect("payload");
        assert_eq!(payload["repair"]["field"], "limit");
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
            offset: Some(0),
            limit: None,
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
            let Some(next) = value["next_offset"].as_u64() else {
                break;
            };
            args.offset = Some(next as usize);
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
            offset: Some(0),
            limit: None,
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
            match value["next_offset"].as_u64() {
                Some(next) => args.offset = Some(next as usize),
                None => break,
            }
        }
        assert_eq!(seen, (0..20u64).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn addressed_batch_fetch_rejects_a_clamped_plan_bound() {
        let server = MockServer::start();
        let state = state_with_localhost();
        let cap = state.config.fetch.batch_max_items_cap;
        let args = BatchFetchArgs {
            offset: Some(0),
            limit: None,
            items: batch_items(&server, 1),
            max_items: Some(cap + 1),
            max_chars_per_item: None,
            max_total_chars: None,
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        };
        let error = run_batch_fetch(state, args)
            .await
            .expect_err("a clamped plan bound is a mismatch");
        assert_eq!(error.code().as_str(), "plan_mismatch");
    }
}

#[cfg(feature = "mock")]
mod research_pagination {
    use super::*;
    use eggsearch::mcp::tools::{run_research_search, ResearchSearchArgs};

    fn research_args(query: &str, max_groups: Option<usize>) -> ResearchSearchArgs {
        ResearchSearchArgs {
            offset: None,
            limit: None,
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
        args.offset = Some(0);
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
            match value["next_offset"].as_u64() {
                Some(next) => args.offset = Some(next as usize),
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
        assert!(value.get("next_offset").is_none());
        assert!(value.get("group_from").is_none());
    }
}

mod document_budget {
    use super::*;
    use httpmock::prelude::*;

    fn windowed_page_server(chars: usize) -> MockServer {
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

    fn plain_state() -> Arc<ServerState> {
        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.fetch.sanitize_output = false;
        Arc::new(ServerState::build(cfg).expect("state builds"))
    }

    fn capped_state(cap: usize) -> Arc<ServerState> {
        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.fetch.sanitize_output = false;
        cfg.fetch.max_chars_default = cap;
        cfg.fetch.max_chars_cap = cap;
        Arc::new(ServerState::build(cfg).expect("capped state builds"))
    }

    fn args(url: String, max_chars: Option<usize>) -> WebFetchArgs {
        WebFetchArgs {
            offset: None,
            limit: None,
            url,
            max_chars,
            timeout_ms: None,
            extract_mode: None,
            include_links: None,
            pdf: None,
            cache_policy: None,
            render: None,
            browser_profile: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
            response_detail: None,
        }
    }

    fn page_chars(document: &serde_json::Value) -> usize {
        document["chunks"]
            .as_array()
            .map(|chunks| {
                chunks
                    .iter()
                    .map(|chunk| chunk["text"].as_str().unwrap_or_default().chars().count())
                    .sum()
            })
            .unwrap_or(0)
    }

    fn chunk_count(document: &serde_json::Value) -> usize {
        document["chunks"].as_array().map_or(0, Vec::len)
    }

    async fn walk_document(
        state: Arc<ServerState>,
        url: String,
        max_chars: Option<usize>,
    ) -> (Vec<serde_json::Value>, usize, usize) {
        let mut pages = Vec::new();
        let mut chars = 0usize;
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut offset: Option<usize> = Some(0);
        while let Some(from) = offset {
            let mut request = args(url.clone(), max_chars);
            request.offset = Some(from);
            let page = run_web_fetch(state.clone(), request).await.expect("window");
            let to = page["text_to"].as_u64().expect("text_to") as usize;
            chars += to.saturating_sub(from);
            for chunk in page["document"]["chunks"].as_array().expect("chunks") {
                seen.insert(chunk["chunk_id"].as_str().unwrap_or_default().to_string());
            }
            offset = page["next_offset"].as_u64().map(|next| next as usize);
            pages.push(page);
            assert!(pages.len() < 40, "the window chain must terminate");
        }
        (pages, chars, seen.len())
    }

    fn windowed_text_server(chars: usize) -> MockServer {
        let server = MockServer::start();
        let paragraph = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu ";
        let mut body = String::new();
        while body.len() < chars {
            body.push_str(paragraph);
            body.push_str(paragraph);
            body.push_str("\n\n");
        }
        server.mock(move |when, then| {
            when.method(GET).path("/text");
            then.status(200)
                .header("content-type", "text/plain")
                .body(body);
        });
        server
    }

    #[tokio::test]
    async fn plain_text_total_does_not_vary_with_the_caller_window() {
        let server = windowed_text_server(20_000);
        let state = plain_state();
        let mut totals = Vec::new();
        for window in [2_000usize, 8_000, 12_000] {
            let mut request = args(server.url("/text"), Some(window));
            request.offset = Some(0);
            request.limit = Some(50_000);
            let value = run_web_fetch(state.clone(), request)
                .await
                .expect("wide read");
            assert_eq!(value["text_from"].as_u64(), Some(0));
            totals.push(value["text_total"].as_u64().expect("text_total"));
        }
        assert!(
            totals.iter().all(|total| *total == totals[0]),
            "text_total must not vary with the caller window: {totals:?}"
        );
        assert!(
            totals[0] > 12_000,
            "the document must exceed the default window, saw {}",
            totals[0]
        );
    }

    #[tokio::test]
    async fn plain_text_windows_reach_the_tail_and_compose() {
        let server = windowed_text_server(20_000);
        let state = plain_state();
        let url = server.url("/text");

        let mut wide_args = args(url.clone(), None);
        wide_args.offset = Some(0);
        wide_args.limit = Some(50_000);
        let wide = run_web_fetch(state.clone(), wide_args)
            .await
            .expect("wide read");
        let total = wide["text_total"].as_u64().expect("text_total") as usize;
        let whole = wide["text"].as_str().unwrap_or_default().to_string();
        assert_eq!(whole.chars().count(), total);

        let limit = 2_000usize;
        let mut offset = 0usize;
        let mut composed = String::new();
        let mut windows = 0usize;
        loop {
            let mut request = args(url.clone(), None);
            request.offset = Some(offset);
            request.limit = Some(limit);
            let page = run_web_fetch(state.clone(), request).await.expect("window");
            windows += 1;
            assert!(windows < 40, "the window chain must terminate");
            let from = page["text_from"].as_u64().expect("text_from") as usize;
            let to = page["text_to"].as_u64().expect("text_to") as usize;
            assert_eq!(from, offset);
            assert_eq!(page["text_total"].as_u64(), Some(total as u64));
            composed.push_str(page["text"].as_str().unwrap_or_default());
            match page["next_offset"].as_u64() {
                Some(next) => {
                    assert_eq!(next as usize, to);
                    offset = next as usize;
                }
                None => break,
            }
        }
        assert!(windows > 5, "a 20k document must span several windows");
        assert_eq!(composed, whole);

        let plain = run_web_fetch(state.clone(), args(url.clone(), None))
            .await
            .expect("unaddressed");
        assert_eq!(plain["document"]["text_truncated"], serde_json::json!(true));
        assert!(plain.get("text_total").is_none());
        assert!(plain.get("next_offset").is_none());

        let mut first_args = args(url, None);
        first_args.offset = Some(0);
        first_args.limit = Some(limit);
        let first = run_web_fetch(state, first_args)
            .await
            .expect("first window");
        assert_eq!(first["document"]["text_truncated"], serde_json::json!(true));
        assert!(first["next_offset"].as_u64().is_some());
    }

    #[tokio::test]
    async fn a_document_larger_than_one_window_is_still_served() {
        let server = windowed_page_server(40_000);
        let (pages, chars, chunks) =
            walk_document(plain_state(), server.url("/long"), Some(2_000)).await;

        assert!(pages.len() > 1, "a 40k page cannot fit one 2k window");
        assert!(
            chars > 20_000,
            "the document must cover most of the page, saw {chars}"
        );
        assert!(chunks > 10, "the document must be chunked, saw {chunks}");
        for page in &pages {
            for chunk in page["document"]["chunks"].as_array().expect("chunks") {
                let chars = chunk["text"].as_str().unwrap_or_default().chars().count();
                assert!(
                    chars <= 4_096,
                    "a chunk must stay inside the chunk budget, saw {chars}"
                );
            }
        }
        let first_text = pages[0]["text"].as_str().unwrap_or_default();
        assert!(!first_text.is_empty());
    }

    #[tokio::test]
    async fn truncation_flags_never_claim_completeness_below_the_document() {
        let long = windowed_page_server(40_000);
        let small = windowed_page_server(400);

        let cases = [
            (
                "small page, no window",
                run_web_fetch(plain_state(), args(small.url("/long"), None)).await,
            ),
            (
                "small page, explicit window",
                run_web_fetch(plain_state(), args(small.url("/long"), Some(4_000))).await,
            ),
            (
                "long page, default window",
                run_web_fetch(plain_state(), args(long.url("/long"), None)).await,
            ),
            (
                "long page, cap reached",
                run_web_fetch(capped_state(2_000), args(long.url("/long"), None)).await,
            ),
        ];

        for (label, result) in cases {
            let value = result.expect(label);
            let document = &value["document"];
            let text_truncated = document["text_truncated"].as_bool().unwrap_or_default();
            let document_truncated = document
                .get("document_truncated")
                .and_then(|flag| flag.as_bool())
                .unwrap_or(false);
            let returned = document["text_chars_returned"].as_u64().unwrap_or_default() as usize;
            let addressable = page_chars(document);
            let boundaries = chunk_count(document);

            if !text_truncated && !document_truncated {
                assert!(
                    returned + boundaries >= addressable,
                    "{label}: both flags are false but only {returned} of {addressable} characters are returned"
                );
            }

            match label {
                "small page, no window" | "small page, explicit window" => {
                    assert!(!text_truncated, "{label}: a 400-byte page fits the window");
                    assert!(
                        !document_truncated,
                        "{label}: nothing was extracted past a cap"
                    );
                }
                "long page, default window" => {
                    assert!(text_truncated, "{label}: the window clipped the document");
                    assert!(
                        !document_truncated,
                        "{label}: extraction reached the whole page"
                    );
                }
                _ => {
                    assert!(
                        document_truncated,
                        "{label}: extraction stopped at the configured cap"
                    );
                    assert!(
                        returned <= 2_000,
                        "{label}: the returned text must stay inside the cap-sized window"
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn document_total_is_caller_independent() {
        let server = windowed_page_server(40_000);
        let state = plain_state();
        let mut totals = Vec::new();
        for window in [2_000usize, 8_000, 12_000] {
            let (pages, chars, chunks) =
                walk_document(state.clone(), server.url("/long"), Some(window)).await;
            totals.push(chars);
            assert!(chunks > 0);

            let mut seen = std::collections::HashSet::new();
            for page in &pages {
                let mut per_page = std::collections::HashSet::new();
                for chunk in page["document"]["chunks"].as_array().expect("chunks") {
                    let id = chunk["chunk_id"].as_str().unwrap_or_default().to_string();
                    assert!(
                        per_page.insert(id.clone()),
                        "a chunk must appear once per window"
                    );
                    seen.insert(id);
                }
            }
            assert_eq!(seen.len(), chunks, "no chunk may be lost across windows");
            assert!(chunks > 10, "the document must be chunked, saw {chunks}");

            let blocks = pages[0]["document"]["blocks"].as_array().expect("blocks");
            let mut expected_start = 0usize;
            for chunk in pages[0]["document"]["chunks"].as_array().expect("chunks") {
                let start = chunk["block_start"].as_u64().unwrap_or_default() as usize;
                let end = chunk["block_end"].as_u64().unwrap_or_default() as usize;
                assert_eq!(start, expected_start, "chunk coverage must be contiguous");
                assert!(end >= start);
                expected_start = end + 1;
            }
            assert!(expected_start <= blocks.len());
        }
        assert_eq!(totals[0], totals[1]);
        assert_eq!(totals[1], totals[2]);
    }

    #[tokio::test]
    async fn cache_hit_matches_miss_and_reprojects_per_window() {
        let server = windowed_page_server(40_000);
        let url = server.url("/long");

        let cold_4k = run_web_fetch(plain_state(), args(url.clone(), Some(4_000)))
            .await
            .expect("cold 4k");
        assert_eq!(cold_4k["cache_status"], serde_json::json!("miss"));

        let warm_state = plain_state();
        let wide = run_web_fetch(warm_state.clone(), args(url.clone(), Some(8_000)))
            .await
            .expect("wide");
        let hit_4k = run_web_fetch(warm_state.clone(), args(url.clone(), Some(4_000)))
            .await
            .expect("hit 4k");
        assert_eq!(wide["cache_status"], serde_json::json!("miss"));
        assert_eq!(hit_4k["cache_status"], serde_json::json!("hit"));

        let mut left = cold_4k.clone();
        let mut right = hit_4k.clone();
        left["cache_status"] = serde_json::Value::Null;
        right["cache_status"] = serde_json::Value::Null;
        assert_eq!(
            left, right,
            "a cache hit must return the bytes a cold fetch of the same window returns"
        );

        let narrow_chars = hit_4k["text"].as_str().unwrap_or_default().chars().count();
        let wide_chars = wide["text"].as_str().unwrap_or_default().chars().count();
        assert!(
            narrow_chars <= 4_000,
            "the narrow window must bound its own text, saw {narrow_chars}"
        );
        assert!(
            narrow_chars < wide_chars,
            "a shared document must still project per caller window"
        );
        assert_eq!(
            hit_4k["document"]["text_truncated"],
            serde_json::json!(true)
        );
    }
}
