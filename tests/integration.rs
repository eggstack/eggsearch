//! Integration tests for the MCP server tool surface.
//!
//! These tests build a `ServerState` either with the real default
//! engines (no network) or with a set of mock engines injected via
//! the `mock` feature. They verify the on-the-wire behavior of the
//! `web_search` and `provider_status` tools without requiring any
//! network access.
//!
//! Covered behavior:
//!
//! - Server `initialize` returns the documented server info and
//!   capabilities.
//! - `tools/list` returns exactly `web_search`, `web_fetch`, and
//!   `provider_status` and never returns the legacy `local_search` or
//!   `search_and_fetch` tools.
//! - `web_search` happy path returns a structured payload with
//!   deduplicated cards and the documented trust label.
//! - `web_search` with an empty / whitespace-only query returns a
//!   validation error.
//! - `web_search` with a query longer than `max_query_chars` returns
//!   a validation error.
//! - `web_search` with `max_results = 0` returns a validation error.
//! - `web_search` with `max_results > cap` returns a validation
//!   error.
//! - `web_search` with an unknown provider id returns an error.
//! - `web_search` when `mode = "off"` is denied by policy.
//! - `web_search` with one failing provider returns partial results
//!   and a non-empty `providers_failed`.
//! - `web_search` with all providers failing returns a structured
//!   error.
//! - `web_search` with a global timeout (mock engines that hang)
//!   reports the timeout per provider.
//! - `web_search` cards are deduplicated when the same URL appears
//!   in multiple engines.
//! - `web_search` cards carry a per-card `id` of the form `src_<uuid>`
//!   and the id is unique within a response.
//! - `provider_status` returns one entry per configured provider with
//!   the documented field shape.

use std::sync::Arc;

use eggsearch::core::config::{AppConfig, Mode};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{
    run_provider_status, run_web_search, ProviderStatusArgs, WebSearchArgs,
};
use rmcp::ServerHandler;

#[cfg(feature = "mock")]
use eggsearch::meta::mock::{mock_engines, MockEngine, MockFailure, MockResult};
#[cfg(feature = "mock")]
use eggsearch::meta::MetadataSearchAdapter;
#[cfg(feature = "mock")]
use std::time::Duration;

fn state_with_default() -> Arc<ServerState> {
    Arc::new(ServerState::build(AppConfig::default()).expect("default state"))
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
fn test_cfg() -> AppConfig {
    let mut cfg = AppConfig::default();
    cfg.search.timeout_ms = 2_000;
    cfg.search.max_query_chars = 256;
    cfg.search.max_results = 10;
    cfg.search.max_results_cap = 50;
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

#[tokio::test]
async fn web_search_empty_query_returns_validation_error() {
    let state = state_with_default();
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
    let err = res.expect_err("expected validation error");
    assert!(err.to_string().contains("invalid query"), "got: {err}");
}

#[tokio::test]
async fn web_search_oversized_query_returns_validation_error() {
    let state = state_with_default();
    let too_long = "a".repeat(2_000);
    let res = run_web_search(
        state,
        WebSearchArgs {
            query: too_long,
            max_results: None,
            providers: vec![],
            safe_search: None,
            timeout_ms: None,
        },
    )
    .await;
    let err = res.expect_err("expected validation error");
    assert!(err.to_string().contains("invalid query"), "got: {err}");
    assert!(err.to_string().contains("characters"), "got: {err}");
}

#[tokio::test]
async fn web_search_zero_max_results_returns_validation_error() {
    let state = state_with_default();
    let res = run_web_search(
        state,
        WebSearchArgs {
            query: "rust".into(),
            max_results: Some(0),
            providers: vec![],
            safe_search: None,
            timeout_ms: None,
        },
    )
    .await;
    let err = res.expect_err("expected validation error");
    assert!(
        err.to_string().contains("max_results must be > 0"),
        "got: {err}"
    );
}

#[tokio::test]
async fn web_search_oversized_max_results_returns_validation_error() {
    let state = state_with_default();
    let res = run_web_search(
        state,
        WebSearchArgs {
            query: "rust".into(),
            max_results: Some(10_000),
            providers: vec![],
            safe_search: None,
            timeout_ms: None,
        },
    )
    .await;
    let err = res.expect_err("expected validation error");
    assert!(
        err.to_string().contains("max_results must be <="),
        "got: {err}"
    );
}

#[tokio::test]
async fn web_search_blocked_when_mode_off() {
    let state = state_with_mode_off();
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
    let err = res.expect_err("expected policy denial");
    assert!(err.to_string().contains("disabled by policy"), "got: {err}");
}

#[tokio::test]
async fn web_search_unknown_provider_returns_error() {
    let state = state_with_default();
    let res = run_web_search(
        state,
        WebSearchArgs {
            query: "rust".into(),
            max_results: None,
            providers: vec!["nope".into()],
            safe_search: None,
            timeout_ms: None,
        },
    )
    .await;
    let err = res.expect_err("expected unknown provider error");
    assert!(err.to_string().contains("unknown provider"), "got: {err}");
    assert!(err.to_string().contains("nope"), "got: {err}");
}

#[test]
fn provider_status_returns_configured_providers() {
    let state = state_with_default();
    let v = run_provider_status(state, ProviderStatusArgs { probe: false }).expect("ok");
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
    let state = state_with_default();
    let v = run_provider_status(state, ProviderStatusArgs { probe: false }).expect("ok");
    assert!(v["mode"].is_string());
    let arr = v["providers"].as_array().unwrap();
    for p in arr {
        assert!(p["id"].is_string(), "missing id: {p}");
        assert!(p["enabled"].is_boolean(), "missing enabled: {p}");
        assert!(p["kind"].is_string(), "missing kind: {p}");
        assert!(
            p["requires_api_key"].is_boolean(),
            "missing requires_api_key: {p}"
        );
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_happy_path_dedupes_across_engines() {
    let engines = vec![
        MockEngine::success(
            "mock_a",
            vec![
                MockResult::new("Title A", "https://example.com/a", "mock_a"),
                MockResult::new("Title B", "https://example.com/b", "mock_a"),
            ],
        ),
        MockEngine::success(
            "mock_b",
            vec![MockResult::new(
                "Title A",
                "https://example.com/a",
                "mock_b",
            )],
        ),
    ];
    let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));
    let v = run_web_search(state, args_for(&["mock_a", "mock_b"], "rust"))
        .await
        .expect("ok");

    assert_eq!(v["query"], "rust");
    assert_eq!(v["mode"], "live_metasearch");
    let results = v["results"].as_array().expect("results is array");
    // Two unique URLs across the two engines; one of them appears in
    // both, so we expect 2 cards, not 3.
    assert_eq!(results.len(), 2, "results: {results:?}");

    // Card for https://example.com/a should be present in both
    // providers.
    let a_card = results
        .iter()
        .find(|c| c["url"] == "https://example.com/a")
        .expect("card a");
    let providers = a_card["providers"].as_array().unwrap();
    let provider_ids: Vec<&str> = providers.iter().filter_map(|v| v.as_str()).collect();
    assert!(provider_ids.contains(&"mock_a"));
    assert!(provider_ids.contains(&"mock_b"));
    assert_eq!(a_card["trust"], "external_untrusted");
    assert_eq!(a_card["fetched"], false);

    // Each card must have a unique id of the form src_<uuid>.
    let ids: Vec<&str> = results.iter().filter_map(|c| c["id"].as_str()).collect();
    for id in &ids {
        assert!(id.starts_with("src_"), "id format: {id}");
    }
    let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "ids must be unique: {ids:?}");

    // Warnings array should contain the untrusted-content warning.
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap_or("").contains("untrusted")),
        "warnings: {warnings:?}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_partial_failure_returns_results_and_failures() {
    let engines = vec![
        MockEngine::success(
            "mock_a",
            vec![MockResult::new("A", "https://example.com/a", "mock_a")],
        ),
        MockEngine::failure("mock_b", MockFailure::Parse),
    ];
    let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));
    let v = run_web_search(state, args_for(&["mock_a", "mock_b"], "rust"))
        .await
        .expect("ok");

    let results = v["results"].as_array().unwrap();
    assert_eq!(results.len(), 1, "partial results: {results:?}");

    let failed = v["providers_failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1, "failed: {failed:?}");
    assert_eq!(failed[0]["id"], "mock_b");
    assert_eq!(failed[0]["error_class"], "parse_error");
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_all_providers_fail_returns_error() {
    let engines = vec![
        MockEngine::failure("mock_a", MockFailure::HttpStatus(503)),
        MockEngine::failure("mock_b", MockFailure::Network),
    ];
    let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));
    let err = run_web_search(state, args_for(&["mock_a", "mock_b"], "rust"))
        .await
        .expect_err("expected all-fail error");
    assert!(
        err.to_string().contains("all providers failed"),
        "expected all-fail error, got: {err}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_global_timeout_returns_all_fail_error() {
    // Both engines hang forever; adapter timeout is 200 ms. With all
    // providers timing out, the tool surface returns a structured
    // "all providers failed" error rather than a soft partial result.
    let engines = vec![MockEngine::hang("mock_a"), MockEngine::hang("mock_b")];
    let state = state_with_engines(test_cfg(), engines, Duration::from_millis(200));
    let err = run_web_search(state, args_for(&["mock_a", "mock_b"], "rust"))
        .await
        .expect_err("expected all-fail error after global timeout");
    assert!(
        err.to_string().contains("all providers failed"),
        "expected all-fail error, got: {err}"
    );
    assert!(
        err.to_string().contains("timed out"),
        "error should mention the timeout: {err}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_provider_override_queries_only_requested_providers() {
    // mock_b is enabled in the adapter but we explicitly ask for
    // mock_a only. mock_b must not appear in providers_queried.
    let engines = vec![
        MockEngine::success(
            "mock_a",
            vec![MockResult::new("A", "https://example.com/a", "mock_a")],
        ),
        MockEngine::success(
            "mock_b",
            vec![MockResult::new("B", "https://example.com/b", "mock_b")],
        ),
    ];
    let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));
    let v = run_web_search(state, args_for(&["mock_a"], "rust"))
        .await
        .expect("ok");

    let queried = v["providers_queried"].as_array().unwrap();
    let queried_ids: Vec<&str> = queried.iter().filter_map(|q| q.as_str()).collect();
    assert_eq!(queried_ids, vec!["mock_a"]);
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_provider_override_with_unknown_id_errors() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new("A", "https://example.com/a", "mock_a")],
    )];
    let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));
    let err = run_web_search(state, args_for(&["mock_a", "mock_does_not_exist"], "rust"))
        .await
        .expect_err("expected unknown provider error");
    assert!(err.to_string().contains("unknown provider"), "got: {err}");
    assert!(
        err.to_string().contains("mock_does_not_exist"),
        "unknown id should be named in error: {err}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_partial_timeout_preserves_successful_results() {
    // mock_a returns instantly, mock_b hangs forever. With a tight
    // global timeout, mock_a's results must still be returned.
    let engines = vec![
        MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Fast",
                "https://example.com/fast",
                "mock_a",
            )],
        ),
        MockEngine::hang("mock_b"),
    ];
    let state = state_with_engines(test_cfg(), engines, Duration::from_millis(200));
    let v = run_web_search(state, args_for(&["mock_a", "mock_b"], "rust"))
        .await
        .expect("ok");

    let results = v["results"].as_array().unwrap();
    assert_eq!(
        results.len(),
        1,
        "should have 1 result from mock_a: {results:?}"
    );
    assert_eq!(results[0]["title"], "Fast");

    // mock_b should appear in providers_failed as timed out.
    let failed = v["providers_failed"].as_array().unwrap();
    let failed_ids: Vec<&str> = failed.iter().filter_map(|f| f["id"].as_str()).collect();
    assert!(
        failed_ids.contains(&"mock_b"),
        "mock_b should be in providers_failed: {failed:?}"
    );
    assert!(
        !failed_ids.contains(&"mock_a"),
        "mock_a should NOT be in providers_failed: {failed:?}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_per_request_timeout_ms_shorter_than_global() {
    // Global timeout is 5s but per-request timeout_ms is 100ms.
    // Both engines hang. The per-request timeout should trigger.
    let engines = vec![MockEngine::hang("mock_a"), MockEngine::hang("mock_b")];
    let mut cfg = test_cfg();
    cfg.search.timeout_ms = 5_000;
    let state = state_with_engines(cfg, engines, Duration::from_secs(5));
    let mut args = args_for(&["mock_a", "mock_b"], "rust");
    args.timeout_ms = Some(100);
    let err = run_web_search(state, args)
        .await
        .expect_err("expected timeout error");
    assert!(
        err.to_string().contains("all providers failed"),
        "expected all-fail error, got: {err}"
    );
    assert!(
        err.to_string().contains("timed out"),
        "error should mention timeout: {err}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_all_providers_fail_returns_error_when_no_results() {
    // Both providers report failure with no results, so the tool
    // surface returns a structured "all providers failed" error.
    let engines = vec![
        MockEngine::failure("mock_a", MockFailure::Parse),
        MockEngine::failure("mock_b", MockFailure::HttpStatus(503)),
    ];
    let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));
    let err = run_web_search(state, args_for(&["mock_a", "mock_b"], "rust"))
        .await
        .expect_err("expected all-fail error");
    assert!(
        err.to_string().contains("all providers failed"),
        "got: {err}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn provider_status_with_mixed_enabled_disabled() {
    use eggsearch::core::config::{AppConfig, Mode};

    let engines = vec![
        MockEngine::success("mock_a", vec![]),
        MockEngine::success("mock_b", vec![]),
    ];
    let mut cfg = AppConfig::default();
    cfg.search.mode = Mode::Live;
    cfg.search.providers.clear();
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.search.providers.insert("mock_b".to_string(), false);
    let adapter = eggsearch::meta::MetadataSearchAdapter::from_engines(
        eggsearch::meta::mock::mock_engines(engines),
        Duration::from_secs(5),
    );
    let state = Arc::new(eggsearch::mcp::state::ServerState::with_adapter(
        cfg,
        Arc::new(adapter),
    ));
    let v = run_provider_status(state, ProviderStatusArgs { probe: false }).expect("ok");
    // provider_status lists KNOWN_PROVIDERS (duckduckgo, brave, startpage, yahoo),
    // not mock engine names. The mock engines aren't in that list.
    let arr = v["providers"].as_array().unwrap();
    let ids: Vec<&str> = arr.iter().filter_map(|p| p["id"].as_str()).collect();
    assert!(ids.contains(&"duckduckgo"));
    assert!(ids.contains(&"brave"));
    assert!(ids.contains(&"startpage"));
    assert!(ids.contains(&"yahoo"));
    // All four should be listed, even though only mock_a and mock_b
    // are loaded in the adapter.
    assert_eq!(ids.len(), 4);
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
        "web_fetch should be in tools list: {:?}",
        tool_names
    );
}
