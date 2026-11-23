#![cfg(feature = "mock")]

//! Focused batch evidence retrieval contract.
//!
//! Regression provenance: phase-14 retrieval-ergonomics workstream.
//!
//! Deterministic, network-free tests (local `httpmock` servers and
//! temp workspace checkouts only) for:
//! - mixed focused/unfocused batch items;
//! - web and repo (workspace) items in one batch;
//! - aggregate truncation;
//! - UTF-8-safe boundaries;
//! - focus with cache hit/revalidation;
//! - focus rejection for metadata-only content;
//! - individual timeout/failure isolation;
//! - local/repo locator safety preservation;
//! - suggested-fetch -> batch-fetch round-trip fixtures.

use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::batch_fetch::BatchFetchItem;
use eggsearch::core::config::AppConfig;
use eggsearch::core::fetch::ExtractMode;
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{
    run_batch_fetch, run_repo_fetch, run_web_fetch, BatchFetchArgs, RepoFetchArgs, WebFetchArgs,
};
use eggsearch::meta::mock::{mock_engines, MockEngine};
use eggsearch::meta::MetadataSearchAdapter;

fn localhost_state() -> Arc<ServerState> {
    let defaults = AppConfig::default();
    localhost_state_with_brief_limits(
        defaults.mcp.brief_max_chars,
        defaults.fetch.batch_concurrency,
    )
}

fn localhost_state_with_brief_limits(
    brief_max_chars: usize,
    batch_concurrency: usize,
) -> Arc<ServerState> {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.fetch.batch_concurrency = batch_concurrency;
    cfg.mcp.brief_max_chars = brief_max_chars;
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

fn web_item(url: String) -> BatchFetchItem {
    BatchFetchItem::Web {
        url,
        extract_mode: None,
        include_links: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
        pdf: None,
        render: None,
        browser_profile: None,
        include_screenshot: None,
    }
}

fn focused_web_item(url: String, focus: &str) -> BatchFetchItem {
    BatchFetchItem::Web {
        url,
        extract_mode: None,
        include_links: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: Some(focus.to_string()),
        focus_max_chunks: Some(2),
        focus_max_chars: Some(500),
        pdf: None,
        render: None,
        browser_profile: None,
        include_screenshot: None,
    }
}

fn article_body() -> Vec<u8> {
    b"<!DOCTYPE html><html><head><title>Tokio Guide</title></head><body>\
      <h1>Tokio Runtime</h1>\
      <p>The tokio runtime drives async tasks to completion with a work stealing scheduler.</p>\
      <h2>Configuration</h2>\
      <p>Configure worker threads with Builder and enable all features for full runtime support.</p>\
      <h2>Cooking Recipes</h2>\
      <p>Unrelated dinner recipes follow in this final section about pasta.</p>\
      </body></html>"
        .to_vec()
}

fn batch_args(items: Vec<BatchFetchItem>) -> BatchFetchArgs {
    BatchFetchArgs {
        offset: None,
        limit: None,
        items,
        max_items: None,
        max_chars_per_item: None,
        max_total_chars: None,
        timeout_ms: None,
        continue_on_error: None,
        response_detail: None,
    }
}

fn workspace_state(root: &std::path::Path) -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.local.enabled = true;
    cfg.local.roots = vec![root.to_path_buf()];
    let backend = eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg.local.clone())
        .expect("backend");
    backend.get_or_build_inventory();
    let mut state = ServerState::build(cfg).expect("state builds");
    state.local_backend = Some(Arc::new(backend));
    Arc::new(state)
}

fn workspace_root_name(root: &std::path::Path) -> String {
    root.file_name().unwrap().to_str().unwrap().to_string()
}

fn workspace_repo_item(owner: &str, path: &str, max_chars: Option<usize>) -> BatchFetchItem {
    serde_json::from_value(serde_json::json!({
        "type": "repo",
        "host": "workspace",
        "owner": owner,
        "repo": path,
        "path": path,
        "max_chars": max_chars
    }))
    .expect("workspace repo item")
}

fn visible_string_chars(value: Option<&serde_json::Value>) -> usize {
    value
        .and_then(serde_json::Value::as_str)
        .map_or(0, |text| text.chars().count())
}

fn expected_batch_returned_chars(payload: &serde_json::Value, repo: bool) -> usize {
    let mut chars = ["text", "title", "description"]
        .iter()
        .map(|key| visible_string_chars(payload.get(key)))
        .sum::<usize>();
    chars += payload
        .get("links")
        .and_then(serde_json::Value::as_array)
        .map_or(0, |links| {
            links
                .iter()
                .map(|link| {
                    visible_string_chars(link.get("url")) + visible_string_chars(link.get("text"))
                })
                .sum()
        });
    if repo {
        chars += payload
            .get("locator")
            .and_then(serde_json::Value::as_object)
            .map_or(0, |locator| {
                locator
                    .values()
                    .map(|value| visible_string_chars(Some(value)))
                    .sum()
            });
        chars += [
            "browser_url",
            "raw_url",
            "permalink_url",
            "raw_permalink_url",
            "fetched_url",
        ]
        .iter()
        .map(|key| visible_string_chars(payload.get(key)))
        .sum::<usize>();
    }
    chars
}

#[tokio::test]
async fn mixed_focused_unfocused_batch_items() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/a");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    server.mock(|when, then| {
        when.method(GET).path("/b");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let state = localhost_state();
    let v = run_batch_fetch(
        state,
        batch_args(vec![
            focused_web_item(server.url("/a"), "tokio runtime scheduler"),
            web_item(server.url("/b")),
        ]),
    )
    .await
    .expect("batch works");
    assert_eq!(v["fetched"], 2);
    let focused = &v["results"][0]["response"]["focus"];
    assert!(focused.is_object(), "focused item must carry focus");
    let chunks = focused["chunks"].as_array().expect("chunks");
    assert!(!chunks.is_empty());
    assert!(chunks.len() <= 2);
    let unfocused = &v["results"][1]["response"]["focus"];
    assert!(
        unfocused.is_null(),
        "unfocused item must have null focus, got {unfocused}"
    );
    let telemetry = &v["telemetry"];
    assert_eq!(telemetry["focused_items"], 1);
    assert!(telemetry["focused_chunks_selected"].as_u64().unwrap_or(0) > 0);
}

#[tokio::test]
async fn batch_marks_challenge_pages_per_item() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/wall");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(
                r#"<html><head><script id="anubis_challenge">{}</script></head><body><div id="app"></div></body></html>"#,
            );
    });
    server.mock(|when, then| {
        when.method(GET).path("/ok");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let state = localhost_state();
    let v = run_batch_fetch(
        state,
        batch_args(vec![
            web_item(server.url("/wall")),
            web_item(server.url("/ok")),
        ]),
    )
    .await
    .expect("batch works");

    let wall = &v["results"][0]["response"];
    assert_eq!(wall["manual_interaction_required"], true);
    let warnings = wall["structured_warnings"]
        .as_array()
        .expect("structured warnings");
    let challenge = warnings
        .iter()
        .find(|warning| warning["code"] == "fetch_challenge_page")
        .expect("challenge warning present");
    assert!(
        challenge["message"]
            .as_str()
            .unwrap_or_default()
            .contains("anubis"),
        "got: {}",
        challenge["message"]
    );
    assert_eq!(
        v["results"][1]["response"]["manual_interaction_required"],
        false
    );
}

const ANUBIS_PASS_COOKIE: &str = "pass-cookie-123";

fn anubis_fast_challenge_page() -> String {
    let payload = serde_json::json!({
        "rules": {"algorithm": "fast", "difficulty": 2},
        "challenge": {
            "issuedAt": "2026-11-11T00:00:00Z",
            "metadata": {"User-Agent": "eggsearch"},
            "id": "01a1cc17-0000-7000-8000-000000000000",
            "method": "fast",
            "randomData": "abc123def456",
            "redir": "/",
            "policyRuleHash": "00",
            "difficulty": 2,
            "spent": false
        }
    });
    format!(
        r#"<html><head><script id="anubis_challenge" type="application/json">{payload}</script></head><body><div id="app"></div></body></html>"#
    )
}

fn localhost_state_with_challenge() -> Arc<ServerState> {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.search.challenge.enabled = true;
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

#[tokio::test]
async fn batch_inherits_the_fetch_side_anubis_solve() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let replay = server.mock(|when, then| {
        when.method(GET)
            .path("/")
            .header("cookie", format!("spchal-auth={ANUBIS_PASS_COOKIE}"));
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let wall = server.mock(|when, then| {
        when.method(GET).path("/");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("set-cookie", "sp_pow=pow-1; Path=/")
            .body(anubis_fast_challenge_page());
    });
    let pass = server.mock(|when, then| {
        when.method(GET)
            .path("/.within.website/x/cmd/anubis/api/pass-challenge");
        then.status(200)
            .header(
                "set-cookie",
                format!("spchal-auth={ANUBIS_PASS_COOKIE}; Path=/; HttpOnly"),
            )
            .body("");
    });

    let state = localhost_state_with_challenge();
    let v = run_batch_fetch(state, batch_args(vec![web_item(server.url("/"))]))
        .await
        .expect("batch works");

    assert_eq!(wall.hits(), 1, "the wall is fetched once");
    assert_eq!(
        pass.hits(),
        1,
        "the batch path must attempt the published challenge"
    );
    assert_eq!(
        replay.hits(),
        1,
        "the batch path must replay with the issued cookie"
    );

    let item = &v["results"][0]["response"];
    assert_eq!(item["manual_interaction_required"], false);
    let warnings = item["structured_warnings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        !warnings
            .iter()
            .any(|warning| warning["code"] == "fetch_challenge_page"),
        "a solved wall must not keep the challenge warning: {warnings:?}"
    );
    let title = item["title"].as_str().unwrap_or_default();
    assert!(
        title.contains("Tokio Guide"),
        "the batch path must return the replayed document: {title}"
    );
}

#[tokio::test]
async fn batch_focus_validation_aligned_to_web_fetch() {
    let state = localhost_state();
    let bad_empty = BatchFetchItem::Web {
        url: "https://example.com".to_string(),
        extract_mode: None,
        include_links: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: Some("   ".to_string()),
        focus_max_chunks: None,
        focus_max_chars: None,
        pdf: None,
        render: None,
        browser_profile: None,
        include_screenshot: None,
    };
    let err = run_batch_fetch(state.clone(), batch_args(vec![bad_empty]))
        .await
        .expect_err("empty focus must fail");
    assert!(err.to_string().contains("focus must not be empty"));

    let bad_chunks = BatchFetchItem::Web {
        url: "https://example.com".to_string(),
        extract_mode: None,
        include_links: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: Some("tokio".to_string()),
        focus_max_chunks: Some(99),
        focus_max_chars: None,
        pdf: None,
        render: None,
        browser_profile: None,
        include_screenshot: None,
    };
    let err = run_batch_fetch(state.clone(), batch_args(vec![bad_chunks]))
        .await
        .expect_err("oversized chunks must fail");
    assert!(err.to_string().contains("focus_max_chunks"));

    let bad_chars = BatchFetchItem::Web {
        url: "https://example.com".to_string(),
        extract_mode: None,
        include_links: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: Some("tokio".to_string()),
        focus_max_chunks: None,
        focus_max_chars: Some(0),
        pdf: None,
        render: None,
        browser_profile: None,
        include_screenshot: None,
    };
    let err = run_batch_fetch(state, batch_args(vec![bad_chars]))
        .await
        .expect_err("zero focus chars must fail");
    assert!(err.to_string().contains("focus_max_chars"));
}

#[tokio::test]
async fn batch_prevalidates_target_options_before_fetching_any_item() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/must-not-fetch");
        then.status(200)
            .header("content-type", "text/plain")
            .body("should not be fetched");
    });
    let state = localhost_state();
    let mut invalid_render = web_item(server.url("/must-not-fetch"));
    if let BatchFetchItem::Web { ref mut render, .. } = invalid_render {
        *render = Some("invalid".to_string());
    }
    let error = run_batch_fetch(
        state.clone(),
        batch_args(vec![
            web_item(server.url("/must-not-fetch")),
            invalid_render,
        ]),
    )
    .await
    .expect_err("invalid render must fail batch prevalidation");
    assert!(error.to_string().contains("render policy"));
    assert_eq!(mock.hits(), 0);

    let invalid_repo = BatchFetchItem::Repo {
        host: Some("github".to_string()),
        target: eggsearch::core::repo_fetch::RepoTarget {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            ref_name: None,
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            symbol: Some("Thing".to_string()),
            symbol_kind: Some("constructor".to_string()),
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        },
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
    };
    let error = run_batch_fetch(
        state,
        batch_args(vec![web_item(server.url("/must-not-fetch")), invalid_repo]),
    )
    .await
    .expect_err("invalid repo selector must fail batch prevalidation");
    assert!(error.to_string().contains("symbol_kind"));
    assert_eq!(mock.hits(), 0);
}

#[tokio::test]
async fn batch_focus_rejected_for_metadata_only() {
    let state = localhost_state();
    let item = BatchFetchItem::Web {
        url: "https://example.com".to_string(),
        extract_mode: Some(ExtractMode::MetadataOnly),
        include_links: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: Some("tokio".to_string()),
        focus_max_chunks: None,
        focus_max_chars: None,
        pdf: None,
        render: None,
        browser_profile: None,
        include_screenshot: None,
    };
    let err = run_batch_fetch(state, batch_args(vec![item]))
        .await
        .expect_err("metadata-only focus must fail");
    assert!(err.to_string().contains("metadata_only"));
}

#[tokio::test]
async fn batch_aggregate_truncation_is_explicit() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    for i in 0..2 {
        server.mock(move |when, then| {
            when.method(GET).path(format!("/p{i}"));
            then.status(200)
                .header("content-type", "text/plain")
                .body("x".repeat(5000));
        });
    }
    let state = localhost_state();
    let items: Vec<BatchFetchItem> = (0..2)
        .map(|i| web_item(server.url(format!("/p{i}"))))
        .collect();
    let v = run_batch_fetch(
        state,
        BatchFetchArgs {
            offset: None,
            limit: None,
            items,
            max_items: None,
            max_chars_per_item: Some(4000),
            max_total_chars: Some(1000),
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        },
    )
    .await
    .expect("batch works");
    assert_eq!(v["telemetry"]["aggregate_budget_exhausted"], true);
    assert!(
        v["total_chars_returned"]
            .as_u64()
            .unwrap_or(usize::MAX as u64)
            <= 1000
    );
    let warnings = v["warnings"].as_array().expect("warnings");
    assert!(
        warnings.iter().any(|w| w
            .as_str()
            .unwrap_or("")
            .contains("batch_total_budget_exhausted")),
        "aggregate exhaustion must warn: {warnings:?}"
    );
}

#[tokio::test]
async fn batch_repo_large_workspace_file_is_clipped_to_default_total_budget() {
    let dir = tempfile::tempdir().expect("workspace root");
    let root_name = workspace_root_name(dir.path());
    let content = "pub fn routine() { let value = 1; }\n".repeat(3000);
    std::fs::write(dir.path().join("large.rs"), content).expect("write large workspace file");
    let item = workspace_repo_item(&root_name, "large.rs", Some(50_000));
    let mut args = batch_args(vec![item]);
    args.max_chars_per_item = Some(50_000);
    let batch = run_batch_fetch(workspace_state(dir.path()), args)
        .await
        .expect("large workspace batch");
    let result = &batch["results"][0];
    assert_eq!(result["ok"], true);
    assert_eq!(result["truncated"], true);
    assert!(!result["response"]["text"]
        .as_str()
        .unwrap_or_default()
        .is_empty());
    assert!(batch["total_chars_returned"].as_u64().unwrap() <= 50_000);
    let budget_warning = batch["structured_warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|warning| warning["code"] == "fetch_content_truncated")
        .expect("aggregate budget warning");
    assert!(budget_warning["message"]
        .as_str()
        .unwrap()
        .contains("large.rs"));
}

#[tokio::test]
async fn batch_repo_metadata_budget_omission_is_a_failed_item() {
    let dir = tempfile::tempdir().expect("workspace root");
    let root_name = workspace_root_name(dir.path());
    std::fs::write(dir.path().join("small.rs"), "pub fn ready() {}\n").expect("write");
    let mut args = batch_args(vec![workspace_repo_item(&root_name, "small.rs", None)]);
    args.max_chars_per_item = Some(1000);
    args.max_total_chars = Some(1);
    let batch = run_batch_fetch(workspace_state(dir.path()), args)
        .await
        .expect("small budget batch");
    let result = &batch["results"][0];
    assert_eq!(result["ok"], false);
    assert!(result.as_object().unwrap().contains_key("response"));
    assert!(result["response"].is_null());
    assert_eq!(result["chars_returned"], 0);
    assert_eq!(result["truncated"], true);
    let error = result["error"].as_str().unwrap();
    assert!(error.contains("small.rs") && error.contains("max_total_chars=1"));
    assert_eq!(batch["fetched"], 0);
    assert_eq!(batch["failed"], 1);
    assert_eq!(batch["telemetry"]["items_completed"], 0);
    assert_eq!(batch["telemetry"]["items_failed"], 1);
    let budget_warnings = batch["structured_warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|warning| warning["code"] == "fetch_content_truncated")
        .collect::<Vec<_>>();
    assert_eq!(budget_warnings.len(), 1);
    let warning = budget_warnings[0]["message"].as_str().unwrap();
    assert!(warning.contains(&root_name) && warning.contains("max_total_chars=1"));
}

#[tokio::test]
async fn batch_focus_utf8_boundaries() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let body = "<html><head><title>T</title></head><body><p>".to_string()
        + &"héllo wörld tokio runtime 🦀 ".repeat(50)
        + "</p></body></html>";
    server.mock(|when, then| {
        when.method(GET).path("/utf8");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(body.clone());
    });
    let state = localhost_state();
    let mut item = focused_web_item(server.url("/utf8"), "tokio runtime");
    if let BatchFetchItem::Web {
        ref mut focus_max_chars,
        ..
    } = item
    {
        *focus_max_chars = Some(10);
    }
    let v = run_batch_fetch(state, batch_args(vec![item]))
        .await
        .expect("batch works");
    let focus = &v["results"][0]["response"]["focus"];
    let chunks = focus["chunks"].as_array().expect("chunks");
    for c in chunks {
        let text = c["text"].as_str().unwrap_or("");
        assert!(text.chars().count() <= 10 || chunks.len() == 1);
        assert!(text.is_char_boundary(0) && text.is_char_boundary(text.len()));
    }
    let total = focus["total_chars"].as_u64().unwrap_or(0);
    let summed: u64 = chunks
        .iter()
        .map(|c| c["text"].as_str().unwrap_or("").chars().count() as u64)
        .sum();
    assert_eq!(total, summed);
}

#[tokio::test]
async fn batch_focus_with_cache_hit() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/c");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .body(article_body());
    });
    let state = localhost_state();
    let run = |focus: Option<String>| {
        let state = state.clone();
        let url = server.url("/c");
        async move {
            run_batch_fetch(
                state,
                batch_args(vec![BatchFetchItem::Web {
                    url,
                    extract_mode: None,
                    include_links: None,
                    max_chars: None,
                    cache_policy: None,
                    max_cache_age_seconds: None,
                    focus,
                    focus_max_chunks: Some(2),
                    focus_max_chars: Some(500),
                    pdf: None,
                    render: None,
                    browser_profile: None,
                    include_screenshot: None,
                }]),
            )
            .await
        }
    };
    let v = run(None).await.expect("prime works");
    assert_eq!(v["results"][0]["response"]["cache_status"], "miss");
    let v = run(Some("tokio runtime".to_string()))
        .await
        .expect("focused hit works");
    assert_eq!(v["results"][0]["response"]["cache_status"], "hit");
    assert!(v["results"][0]["response"]["focus"].is_object());
    assert_eq!(mock.hits(), 1, "focus must not cause extra fetch");
    assert_eq!(v["telemetry"]["cache_hits"], 1);
    assert_eq!(v["telemetry"]["focused_items"], 1);
}

#[tokio::test]
async fn batch_failure_isolation() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/ok");
        then.status(200)
            .header("content-type", "text/plain")
            .body("ok content here");
    });
    server.mock(|when, then| {
        when.method(GET).path("/fail");
        then.status(500).body("boom");
    });
    let state = localhost_state();
    let v = run_batch_fetch(
        state,
        batch_args(vec![
            web_item(server.url("/ok")),
            web_item(server.url("/fail")),
            focused_web_item(server.url("/ok"), "ok content"),
        ]),
    )
    .await
    .expect("batch works");
    assert_eq!(v["fetched"], 2);
    assert_eq!(v["failed"], 1);
    assert_eq!(v["results"][0]["ok"], true);
    assert_eq!(v["results"][1]["ok"], false);
    assert_eq!(v["results"][2]["ok"], true);
    assert!(v["results"][2]["response"]["focus"].is_object());
}

#[tokio::test]
async fn batch_locator_safety_preserved() {
    let state = localhost_state();
    let traversal = BatchFetchItem::Repo {
        host: Some("github".to_string()),
        target: eggsearch::core::repo_fetch::RepoTarget {
            owner: "o".to_string(),
            repo: "r".to_string(),
            ref_name: None,
            commit_sha: None,
            path: "../evil.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        },
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
    };
    let err = run_batch_fetch(state.clone(), batch_args(vec![traversal]))
        .await
        .expect_err("traversal must fail");
    assert!(err.to_string().contains(".."));

    let absolute = BatchFetchItem::Repo {
        host: Some("github".to_string()),
        target: eggsearch::core::repo_fetch::RepoTarget {
            owner: "o".to_string(),
            repo: "r".to_string(),
            ref_name: None,
            commit_sha: None,
            path: "/etc/passwd".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        },
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
    };
    let err = run_batch_fetch(state.clone(), batch_args(vec![absolute]))
        .await
        .expect_err("absolute must fail");
    assert!(err.to_string().contains("absolute"));

    let bad_host = BatchFetchItem::Repo {
        host: Some("bitbucket".to_string()),
        target: eggsearch::core::repo_fetch::RepoTarget {
            owner: "o".to_string(),
            repo: "r".to_string(),
            ref_name: None,
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        },
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
    };
    let err = run_batch_fetch(state, batch_args(vec![bad_host]))
        .await
        .expect_err("bad host must fail");
    assert!(err.to_string().contains("unknown host"));
}

#[tokio::test]
async fn batch_web_and_workspace_repo_in_one_batch() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/doc");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let dir = tempfile::tempdir().expect("tempdir");
    let root_name = workspace_root_name(dir.path());
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn tokio_runtime() {}\n// tokio runtime scheduler drives tasks\npub fn other() {}\n",
    )
    .expect("write");
    let state = workspace_state(dir.path());

    let v = run_batch_fetch(
        state,
        batch_args(vec![
            focused_web_item(server.url("/doc"), "tokio runtime"),
            BatchFetchItem::Repo {
                host: Some("workspace".to_string()),
                target: eggsearch::core::repo_fetch::RepoTarget {
                    owner: root_name.clone(),
                    repo: "lib.rs".to_string(),
                    ref_name: None,
                    commit_sha: None,
                    path: "lib.rs".to_string(),
                    line_start: None,
                    line_end: None,
                    context_before: None,
                    context_after: None,
                    max_chars: None,
                    symbol: None,
                    symbol_kind: None,
                    match_text: None,
                    expand_to_block: None,
                    max_block_lines: None,
                    prefer_local: None,
                },
                focus: Some("tokio_runtime".to_string()),
                focus_max_chunks: Some(3),
                focus_max_chars: Some(1000),
            },
        ]),
    )
    .await
    .expect("mixed batch works");
    assert_eq!(v["fetched"], 2);
    let web_response = &v["results"][0]["response"];
    let repo_result = &v["results"][1];
    let repo_response = &repo_result["response"];
    assert!(repo_response["text"]
        .as_str()
        .unwrap()
        .contains("tokio_runtime"));
    assert_eq!(repo_response["trust"], "local_trusted");
    let expected_web_chars = expected_batch_returned_chars(web_response, false) as u64;
    assert_eq!(
        v["results"][0]["chars_returned"].as_u64(),
        Some(expected_web_chars)
    );
    let expected_repo_chars = expected_batch_returned_chars(repo_response, true) as u64;
    assert_eq!(
        repo_result["chars_returned"].as_u64(),
        Some(expected_repo_chars)
    );
    assert!(v["results"][0]["response"]["focus"].is_object());
    assert!(v["results"][1]["response"]["focus"].is_object());
    assert_eq!(v["telemetry"]["focused_items"], 2);
    assert!(
        v["telemetry"]["focused_chunks_selected"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
}

#[tokio::test]
async fn batch_web_target_options_match_web_fetch() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/parity");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let state = localhost_state();
    let url = server.url("/parity");
    let direct = run_web_fetch(
        state.clone(),
        WebFetchArgs {
            url: url.clone(),
            max_chars: Some(5000),
            extract_mode: Some(ExtractMode::Markdown),
            include_links: Some(true),
            cache_policy: Some(eggsearch::core::fetch::FetchCachePolicy::Bypass),
            focus: Some("tokio runtime scheduler".to_string()),
            focus_max_chunks: Some(2),
            focus_max_chars: Some(500),
            render: Some("http_only".to_string()),
            response_detail: Some(eggsearch::mcp::projection::ResponseDetail::Diagnostic),
            ..Default::default()
        },
    )
    .await
    .expect("direct web fetch works");
    let batch = run_batch_fetch(
        state,
        batch_args(vec![BatchFetchItem::Web {
            url,
            extract_mode: Some(ExtractMode::Markdown),
            include_links: Some(true),
            max_chars: Some(5000),
            cache_policy: Some(eggsearch::core::fetch::FetchCachePolicy::Bypass),
            max_cache_age_seconds: None,
            focus: Some("tokio runtime scheduler".to_string()),
            focus_max_chunks: Some(2),
            focus_max_chars: Some(500),
            pdf: None,
            render: Some("http_only".to_string()),
            browser_profile: None,
            include_screenshot: None,
        }]),
    )
    .await
    .expect("batch web fetch works");
    let batched = &batch["results"][0]["response"];
    for key in [
        "url",
        "final_url",
        "status",
        "fetched",
        "text",
        "document",
        "focus",
        "trust",
        "trust_markers",
        "warnings",
        "structured_warnings",
        "cache_status",
        "representation",
    ] {
        assert_eq!(direct[key], batched[key], "field {key} differs");
    }
    assert_eq!(mock.hits(), 2);
}

#[cfg(feature = "pdf")]
#[tokio::test]
async fn batch_pdf_page_options_do_not_cross_derived_cache_entries() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let body = include_bytes!("fixtures/pdf/clean_text/single_column_notes.pdf");
    let mock = server.mock(|when, then| {
        when.method(GET).path("/pages.pdf");
        then.status(200)
            .header("content-type", "application/pdf")
            .header("cache-control", "max-age=300")
            .body(body.as_slice());
    });
    let state = localhost_state();
    let url = server.url("/pages.pdf");
    let direct_page_one = run_web_fetch(
        state.clone(),
        WebFetchArgs {
            url: url.clone(),
            max_chars: Some(5000),
            pdf: Some(eggsearch::core::fetch::PdfFetchOptions {
                pages: Some("1".to_string()),
                pdf_password: None,
                include_media: None,
                pdf_ocr: None,
            }),
            render: Some("http_only".to_string()),
            response_detail: Some(eggsearch::mcp::projection::ResponseDetail::Diagnostic),
            ..Default::default()
        },
    )
    .await
    .expect("page one fetch works");
    let page_two = run_batch_fetch(
        state.clone(),
        batch_args(vec![BatchFetchItem::Web {
            url: url.clone(),
            extract_mode: None,
            include_links: None,
            max_chars: Some(5000),
            cache_policy: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
            pdf: Some(eggsearch::core::fetch::PdfFetchOptions {
                pages: Some("2".to_string()),
                pdf_password: None,
                include_media: None,
                pdf_ocr: None,
            }),
            render: Some("http_only".to_string()),
            browser_profile: None,
            include_screenshot: None,
        }]),
    )
    .await
    .expect("page two batch fetch works");
    let page_one_text = direct_page_one["text"].as_str().expect("page one text");
    let page_two_response = &page_two["results"][0]["response"];
    let page_two_text = page_two_response["text"].as_str().expect("page two text");
    assert!(page_one_text.contains("Clean text sample"));
    assert_ne!(page_one_text, page_two_text);
    assert!(page_two_response["document"]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|block| block["page"] == 2));
    assert_eq!(page_two_response["cache_status"], "hit");
    assert_eq!(mock.hits(), 1, "raw PDF bytes should be reused");
}

#[cfg(feature = "pdf")]
#[tokio::test]
async fn batch_pdf_invalid_page_syntax_fails_before_any_fetch() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/invalid.pdf");
        then.status(200)
            .header("content-type", "application/pdf")
            .body(include_bytes!("fixtures/pdf/clean_text/single_column_notes.pdf").as_slice());
    });
    let mut item = web_item(server.url("/invalid.pdf"));
    if let BatchFetchItem::Web { pdf, .. } = &mut item {
        *pdf = Some(eggsearch::core::fetch::PdfFetchOptions {
            pages: Some("0".to_string()),
            pdf_password: None,
            include_media: None,
            pdf_ocr: None,
        });
    }
    let error = run_batch_fetch(localhost_state(), batch_args(vec![item]))
        .await
        .expect_err("invalid PDF page syntax must fail preflight");
    assert!(error.to_string().contains("item 0"));
    assert_eq!(mock.hits(), 0);
}

#[cfg(feature = "pdf")]
#[tokio::test]
async fn batch_pdf_out_of_range_is_item_local_and_preserves_mixed_order() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/out-of-range.pdf");
        then.status(200)
            .header("content-type", "application/pdf")
            .body(include_bytes!("fixtures/pdf/clean_text/single_column_notes.pdf").as_slice());
    });
    server.mock(|when, then| {
        when.method(GET).path("/html");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let mut pdf_item = web_item(server.url("/out-of-range.pdf"));
    if let BatchFetchItem::Web { pdf, .. } = &mut pdf_item {
        *pdf = Some(eggsearch::core::fetch::PdfFetchOptions {
            pages: Some("99".to_string()),
            pdf_password: None,
            include_media: None,
            pdf_ocr: None,
        });
    }
    let batch = run_batch_fetch(
        localhost_state(),
        batch_args(vec![pdf_item, web_item(server.url("/html"))]),
    )
    .await
    .expect("out-of-range page selection stays item-local");
    assert_eq!(batch["results"][0]["index"], 0);
    assert_eq!(batch["results"][1]["index"], 1);
    assert_eq!(batch["results"][0]["ok"], false);
    assert!(batch["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("pdf_page_out_of_range"));
    assert_eq!(batch["results"][1]["ok"], true);
    assert!(batch["results"][1]["response"]["text"]
        .as_str()
        .unwrap()
        .contains("Tokio"));
    assert_eq!(batch["telemetry"]["items_completed"], 1);
    assert_eq!(batch["telemetry"]["items_failed"], 1);
}

#[cfg(feature = "pdf")]
#[tokio::test]
async fn batch_pdf_options_are_ignored_for_html_and_metadata_only_matches_web_fetch() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/html");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    server.mock(|when, then| {
        when.method(GET).path("/document.pdf");
        then.status(200)
            .header("content-type", "application/pdf")
            .body(include_bytes!("fixtures/pdf/clean_text/single_column_notes.pdf").as_slice());
    });
    let state = localhost_state();
    let url = server.url("/document.pdf");
    let direct = run_web_fetch(
        state.clone(),
        WebFetchArgs {
            url: url.clone(),
            max_chars: Some(5000),
            extract_mode: Some(ExtractMode::MetadataOnly),
            pdf: Some(eggsearch::core::fetch::PdfFetchOptions {
                pages: Some("1".to_string()),
                pdf_password: None,
                include_media: None,
                pdf_ocr: None,
            }),
            cache_policy: Some(eggsearch::core::fetch::FetchCachePolicy::Bypass),
            render: Some("http_only".to_string()),
            response_detail: Some(eggsearch::mcp::projection::ResponseDetail::Diagnostic),
            ..Default::default()
        },
    )
    .await
    .expect("metadata-only web fetch");
    let mut html_item = web_item(server.url("/html"));
    if let BatchFetchItem::Web {
        pdf, cache_policy, ..
    } = &mut html_item
    {
        *pdf = Some(eggsearch::core::fetch::PdfFetchOptions {
            pages: Some("1".to_string()),
            pdf_password: None,
            include_media: None,
            pdf_ocr: None,
        });
        *cache_policy = Some(eggsearch::core::fetch::FetchCachePolicy::Bypass);
    }
    let mut metadata_item = web_item(url);
    if let BatchFetchItem::Web {
        extract_mode,
        pdf,
        cache_policy,
        ..
    } = &mut metadata_item
    {
        *extract_mode = Some(ExtractMode::MetadataOnly);
        *pdf = Some(eggsearch::core::fetch::PdfFetchOptions {
            pages: Some("1".to_string()),
            pdf_password: None,
            include_media: None,
            pdf_ocr: None,
        });
        *cache_policy = Some(eggsearch::core::fetch::FetchCachePolicy::Bypass);
    }
    let batch = run_batch_fetch(state, batch_args(vec![html_item, metadata_item]))
        .await
        .expect("batch web items ignore irrelevant PDF options");
    let html = &batch["results"][0]["response"];
    assert!(html["text"].as_str().unwrap().contains("Tokio"));
    let metadata_only = &batch["results"][1]["response"];
    for key in ["title", "description", "text", "document", "warnings"] {
        assert_eq!(direct[key], metadata_only[key], "field {key} differs");
    }
}

#[cfg(feature = "pdf")]
#[tokio::test]
async fn batch_workspace_pdf_matches_repo_fetch() {
    let root = tempfile::tempdir().expect("workspace root");
    let path = "paper.pdf";
    std::fs::write(
        root.path().join(path),
        include_bytes!("fixtures/pdf/clean_text/single_column_notes.pdf"),
    )
    .expect("write workspace PDF");
    let root_name = root
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("root name")
        .to_string();
    let mut config = AppConfig::default();
    config.local.enabled = true;
    config.local.roots = vec![root.path().to_path_buf()];
    let state = Arc::new(ServerState::build(config).expect("server state"));
    let args = RepoFetchArgs {
        host: Some("workspace".to_string()),
        target: eggsearch::core::repo_fetch::RepoTarget {
            owner: root_name.clone(),
            repo: path.to_string(),
            ref_name: None,
            commit_sha: None,
            path: path.to_string(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: Some(5000),
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        },
        timeout_ms: None,
        test_fetch_url: None,
        response_detail: None,
    };
    let direct = run_repo_fetch(state.clone(), args)
        .await
        .expect("direct workspace PDF");
    let batch = run_batch_fetch(
        state,
        batch_args(vec![BatchFetchItem::Repo {
            host: Some("workspace".to_string()),
            target: eggsearch::core::repo_fetch::RepoTarget {
                owner: root_name,
                repo: path.to_string(),
                ref_name: None,
                commit_sha: None,
                path: path.to_string(),
                line_start: None,
                line_end: None,
                context_before: None,
                context_after: None,
                max_chars: Some(5000),
                symbol: None,
                symbol_kind: None,
                match_text: None,
                expand_to_block: None,
                max_block_lines: None,
                prefer_local: None,
            },
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
        }]),
    )
    .await
    .expect("batch workspace PDF");
    let batched = &batch["results"][0]["response"];
    for key in ["content_type", "document", "text", "trust", "warnings"] {
        assert_eq!(direct[key], batched[key], "field {key} differs");
    }
}

#[tokio::test]
async fn batch_repo_span_options_match_workspace_repo_fetch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root_name = workspace_root_name(dir.path());
    let source = "pub struct Router {\n    pub enabled: bool,\n}\n\npub fn unrelated() {}\n";
    std::fs::write(dir.path().join("lib.rs"), source).expect("write source");
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.local.enabled = true;
    cfg.local.roots = vec![dir.path().to_path_buf()];
    let backend = eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg.local.clone())
        .expect("backend");
    let mut state = ServerState::with_adapter(cfg, Arc::new(adapter));
    state.local_backend = Some(Arc::new(backend));
    let state = Arc::new(state);
    let direct = run_repo_fetch(
        state.clone(),
        RepoFetchArgs {
            host: Some("workspace".to_string()),
            target: eggsearch::core::repo_fetch::RepoTarget {
                owner: root_name.clone(),
                repo: "".to_string(),
                ref_name: None,
                commit_sha: None,
                path: "lib.rs".to_string(),
                line_start: None,
                line_end: None,
                context_before: None,
                context_after: None,
                max_chars: Some(3000),
                symbol: Some("Router".to_string()),
                symbol_kind: Some("struct".to_string()),
                match_text: None,
                expand_to_block: Some(true),
                max_block_lines: Some(20),
                prefer_local: None,
            },
            timeout_ms: None,
            test_fetch_url: None,
            response_detail: None,
        },
    )
    .await
    .expect("direct workspace repo fetch works");
    let batch = run_batch_fetch(
        state,
        batch_args(vec![BatchFetchItem::Repo {
            host: Some("workspace".to_string()),
            target: eggsearch::core::repo_fetch::RepoTarget {
                owner: root_name,
                repo: "".to_string(),
                ref_name: None,
                commit_sha: None,
                path: "lib.rs".to_string(),
                line_start: None,
                line_end: None,
                context_before: None,
                context_after: None,
                max_chars: Some(3000),
                symbol: Some("Router".to_string()),
                symbol_kind: Some("struct".to_string()),
                match_text: None,
                expand_to_block: Some(true),
                max_block_lines: Some(20),
                prefer_local: None,
            },
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
        }]),
    )
    .await
    .expect("batch workspace repo fetch works");
    let batched = &batch["results"][0]["response"];
    for key in [
        "fetched",
        "document",
        "text",
        "lines",
        "selected_span",
        "code_span",
        "trust",
        "trust_markers",
        "warnings",
        "structured_warnings",
    ] {
        assert_eq!(direct[key], batched[key], "field {key} differs");
    }
    assert_eq!(batched["trust"], "local_trusted");
}

#[test]
fn suggested_fetch_to_batch_round_trip() {
    use eggsearch::core::batch_fetch::BatchFetchItem;
    use eggsearch::core::repo_fetch::RepoFetchRequest;

    let req = RepoFetchRequest {
        host: Some(eggsearch::core::code_metadata::CodeHost::Github),
        target: eggsearch::core::repo_fetch::RepoTarget {
            owner: "tokio-rs".to_string(),
            repo: "axum".to_string(),
            ref_name: Some("main".to_string()),
            commit_sha: None,
            path: "src/lib.rs".to_string(),
            line_start: Some(1),
            line_end: Some(10),
            context_before: None,
            context_after: None,
            max_chars: None,
            symbol: Some("Router".to_string()),
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
        },
        timeout_ms: None,
    };
    let item = eggsearch::core::fetch_locator::structured_repo_fetch_to_batch_item(&req);
    match &item {
        BatchFetchItem::Repo { target, .. } => {
            assert_eq!(target.owner, "tokio-rs");
            assert_eq!(target.repo, "axum");
            assert_eq!(target.path, "src/lib.rs");
        }
        other => panic!("expected repo item, got {other:?}"),
    }

    let web = eggsearch::core::fetch_locator::url_to_batch_web_item(
        "https://example.com/docs",
        Some(ExtractMode::Markdown),
    );
    match &web {
        BatchFetchItem::Web {
            url, extract_mode, ..
        } => {
            assert_eq!(url, "https://example.com/docs");
            assert_eq!(*extract_mode, Some(ExtractMode::Markdown));
        }
        other => panic!("expected web item, got {other:?}"),
    }

    let repo_suggested = eggsearch::core::repo_search::RepoSuggestedFetch {
        url: "https://raw.githubusercontent.com/tokio-rs/axum/main/src/lib.rs".to_string(),
        reason: "source_evidence".to_string(),
        group: eggsearch::core::repo_search::RepoResultGroupKind::SourceFiles,
        expected_kind: eggsearch::core::source_card::SourceKind::SourceFile,
        recommended_extract_mode: None,
        priority: 1,
        structured_repo_fetch: Some(req.clone()),
        stable_id: None,
        source_id: None,
        score: None,
        reason_code: None,
        rank_reasons: Vec::new(),
        information_gain: None,
        stable: None,
        preferred_tool: Some("repo_fetch".to_string()),
        recommended_focus_query: Some("Router".to_string()),
        batch_item: None,
    };
    let round = repo_suggested.to_batch_item();
    match round {
        BatchFetchItem::Repo { focus, .. } => {
            assert_eq!(focus.as_deref(), Some("Router"));
        }
        other => panic!("expected repo handoff, got {other:?}"),
    }

    let research = eggsearch::core::research::ResearchSuggestedFetch {
        url: "https://example.com/paper".to_string(),
        group: eggsearch::core::research::ResearchResultGroupKind::PrimarySources,
        expected_kind: eggsearch::core::source_card::SourceKind::OfficialDocs,
        evidence_quality: eggsearch::core::research::EvidenceQuality::OfficialPrimary,
        reason: "primary".to_string(),
        recommended_extract_mode: Some(ExtractMode::Markdown),
        priority: 1,
        stable_id: None,
        source_id: None,
        score: None,
        rank_reasons: Vec::new(),
        information_gain: None,
        source_class: None,
        reason_code: None,
        recommended_focus_query: Some("axum router".to_string()),
        batch_item: None,
    };
    let round = research.to_batch_item();
    match round {
        BatchFetchItem::Web { url, focus, .. } => {
            assert_eq!(url, "https://example.com/paper");
            assert_eq!(focus.as_deref(), Some("axum router"));
        }
        other => panic!("expected web handoff, got {other:?}"),
    }

    let security = eggsearch::core::security::SecuritySuggestedFetch {
        url: "https://osv.dev/vulnerability/GHSA-xxxx".to_string(),
        reason: "primary_advisory".to_string(),
        group: eggsearch::core::security::SecurityResultGroupKind::AuthoritativeAdvisories,
        priority: 1,
        stable_id: None,
        source_id: None,
        score: None,
        rank_reasons: Vec::new(),
        information_gain: None,
        reason_code: None,
        advisory_ids: vec!["GHSA-xxxx".to_string()],
        package: None,
        version: None,
        recommended_extract_mode: None,
        recommended_focus_query: None,
        batch_item: None,
    };
    let round = security.to_batch_item();
    match round {
        BatchFetchItem::Web { url, .. } => {
            assert_eq!(url, "https://osv.dev/vulnerability/GHSA-xxxx");
        }
        other => panic!("expected web handoff, got {other:?}"),
    }
}

#[test]
fn focused_batch_next_action_is_bounded_and_typed() {
    use eggsearch::meta::recipe_catalog::{
        focused_batch_next_action, research_search_next_actions, security_search_next_actions,
        web_search_next_actions,
    };
    let ids: Vec<String> = (0..3).map(|i| format!("src_{i}")).collect();
    let action = focused_batch_next_action(&ids, "fetch_multiple_focused", 2);
    assert_eq!(action.tool, "fetch");
    assert_eq!(action.reason_code, "fetch_multiple_focused");
    assert!(action.input_template.get("items").is_some());

    let web = web_search_next_actions(&ids, true);
    assert!(web.iter().any(|a| a.tool == "fetch"));
    let sec = security_search_next_actions(&ids, true);
    assert!(sec.iter().any(|a| a.tool == "fetch"));
    let res = research_search_next_actions(&ids, true);
    assert!(res.iter().any(|a| a.tool == "fetch"));
    for actions in [web, sec, res] {
        assert!(actions.len() <= eggsearch::core::workflow::MAX_NEXT_ACTIONS);
    }
}

#[test]
fn locator_helpers_preserve_safety() {
    assert!(eggsearch::core::fetch_locator::validate_web_url("https://example.com").is_ok());
    assert!(eggsearch::core::fetch_locator::validate_web_url("ftp://example.com").is_err());
    assert!(eggsearch::core::fetch_locator::validate_repo_path("src/lib.rs").is_ok());
    assert!(eggsearch::core::fetch_locator::validate_repo_path("../x").is_err());
    assert!(eggsearch::core::fetch_locator::validate_repo_path("/abs").is_err());
    let loc = eggsearch::core::fetch_locator::batch_repo_item_to_locator(
        Some("workspace"),
        "root",
        "ignored",
        None,
        None,
        "a/b.rs",
    )
    .expect("workspace locator");
    assert!(matches!(
        loc,
        eggsearch::core::fetch_locator::FetchLocator::Local(_)
    ));
}

#[tokio::test]
async fn batch_web_budget_uses_projected_payload_and_drops_structure_before_text() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let requested_text =
        "The requested passage stays intact across a response projection. ".repeat(24);
    let html = format!(
        r#"<!doctype html><html><head><title>Projection page</title><meta name="description" content="A projected batch fixture"></head><body><main><p>{requested_text}</p></main></body></html>"#
    );
    let page = server.mock(|when, then| {
        when.method(GET).path("/");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(html.clone());
    });

    let mut diagnostic_args = batch_args(vec![web_item(server.url("/"))]);
    diagnostic_args.max_chars_per_item = Some(10_000);
    diagnostic_args.max_total_chars = Some(50_000);
    diagnostic_args.response_detail = Some(eggsearch::mcp::projection::ResponseDetail::Diagnostic);
    let diagnostic = run_batch_fetch(localhost_state(), diagnostic_args)
        .await
        .expect("diagnostic batch");
    let original = diagnostic["results"][0]["response"].clone();
    assert!(original["document"]["blocks"].is_array());
    assert!(original["document"]["chunks"].is_array());

    let standard = eggsearch::mcp::projection::project(
        "web_fetch",
        original.clone(),
        eggsearch::mcp::projection::ResponseDetail::Standard,
    );
    let standard_full_chars = serde_json::to_string(&standard).unwrap().chars().count();
    let mut standard_after_structure = standard.clone();
    let standard_document = standard_after_structure["document"]
        .as_object_mut()
        .expect("standard document");
    assert!(standard_document.remove("blocks").is_some());
    assert!(standard_document.remove("chunks").is_some());
    let standard_fitted_chars = serde_json::to_string(&standard_after_structure)
        .unwrap()
        .chars()
        .count();
    let standard_cap = standard_fitted_chars + 128;
    assert!(standard_full_chars > standard_cap);

    let mut standard_args = batch_args(vec![web_item(server.url("/"))]);
    standard_args.max_chars_per_item = Some(10_000);
    standard_args.max_total_chars = Some(standard_cap);
    standard_args.response_detail = Some(eggsearch::mcp::projection::ResponseDetail::Standard);
    let standard_batch = run_batch_fetch(localhost_state(), standard_args)
        .await
        .expect("standard batch");
    page.assert_hits(2);
    let standard_result = &standard_batch["results"][0];
    assert_eq!(standard_result["ok"], true);
    assert_eq!(standard_result["response"]["text"], original["text"]);
    assert!(standard_result["response"]["document"]
        .get("blocks")
        .is_none());
    assert!(standard_result["response"]["document"]
        .get("chunks")
        .is_none());
    assert_eq!(standard_result["truncated"], false);
    assert_eq!(
        standard_result["degraded_fields"],
        serde_json::json!(["document.blocks", "document.chunks"])
    );
    let standard_payload_chars = serde_json::to_string(&standard_result["response"])
        .unwrap()
        .chars()
        .count() as u64;
    assert_eq!(
        standard_result["payload_chars"].as_u64(),
        Some(standard_payload_chars)
    );
    assert_eq!(
        standard_batch["total_payload_chars"].as_u64(),
        Some(standard_payload_chars)
    );
    assert!(standard_payload_chars as usize <= standard_cap);

    let compact = eggsearch::mcp::projection::project(
        "web_fetch",
        original.clone(),
        eggsearch::mcp::projection::ResponseDetail::Compact,
    );
    let compact_chars = serde_json::to_string(&compact).unwrap().chars().count();
    let compact_cap = compact_chars + 128;
    assert!(serde_json::to_string(&original).unwrap().chars().count() > compact_cap);

    let mut compact_args = batch_args(vec![web_item(server.url("/"))]);
    compact_args.max_chars_per_item = Some(10_000);
    compact_args.max_total_chars = Some(compact_cap);
    compact_args.response_detail = Some(eggsearch::mcp::projection::ResponseDetail::Compact);
    let compact_batch = run_batch_fetch(localhost_state(), compact_args)
        .await
        .expect("compact batch");
    let compact_result = &compact_batch["results"][0];
    assert_eq!(compact_result["ok"], true);
    assert_eq!(compact_result["response"]["text"], original["text"]);
    assert!(compact_result["response"].get("document").is_none());
    assert!(compact_result["degraded_fields"]
        .as_array()
        .is_none_or(|fields| fields.is_empty()));
    let compact_payload_chars = serde_json::to_string(&compact_result["response"])
        .unwrap()
        .chars()
        .count() as u64;
    assert_eq!(
        compact_result["payload_chars"].as_u64(),
        Some(compact_payload_chars)
    );
    assert_eq!(
        compact_batch["total_payload_chars"].as_u64(),
        Some(compact_payload_chars)
    );
    assert!(compact_payload_chars as usize <= compact_cap);
}

#[tokio::test]
async fn brief_batch_budget_waits_for_final_projection() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let text =
        "A requested passage whose final brief response is intentionally bounded. ".repeat(100);
    let html = format!(
        r#"<!doctype html><html><head><title>Brief budget fixture</title></head><body><main><p>{text}</p></main></body></html>"#
    );
    let first = server.mock(|when, then| {
        when.method(GET).path("/first");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(html.clone());
    });
    let second = server.mock(|when, then| {
        when.method(GET).path("/second");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(html.clone());
    });

    let mut args = batch_args(vec![
        web_item(server.url("/first")),
        web_item(server.url("/second")),
    ]);
    args.max_chars_per_item = Some(10_000);
    args.max_total_chars = Some(512);
    args.response_detail = Some(eggsearch::mcp::projection::ResponseDetail::Brief);
    let result = run_batch_fetch(localhost_state_with_brief_limits(32_000, 1), args)
        .await
        .expect("brief batch");

    first.assert_hits(1);
    second.assert_hits(1);
    assert!(
        result["total_payload_chars"].as_u64().unwrap() <= 512,
        "{result}"
    );
    assert!(
        !result["results"].as_array().unwrap().is_empty(),
        "brief projection removed every item: {result}"
    );
    assert!(result["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_str)
        .all(|warning| !warning.contains("was skipped")));
}

#[tokio::test]
async fn batch_repo_degradation_drops_structure_before_text() {
    let dir = tempfile::tempdir().expect("workspace root");
    let root_name = workspace_root_name(dir.path());
    let content = "pub fn routine() { let value = 1; }\n".repeat(900);
    std::fs::write(dir.path().join("structured.rs"), &content).expect("write structured file");
    let mut args = batch_args(vec![workspace_repo_item(&root_name, "structured.rs", None)]);
    args.max_total_chars = Some(25_000);
    let batch = run_batch_fetch(workspace_state(dir.path()), args)
        .await
        .expect("structured batch");
    let result = &batch["results"][0];
    assert_eq!(result["ok"], true);
    let returned_text = result["response"]["text"].as_str().unwrap_or_default();
    assert!(!returned_text.is_empty());
    assert!(content.starts_with(returned_text));
    let degraded: Vec<&str> = result["degraded_fields"]
        .as_array()
        .expect("degraded markers")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert_eq!(degraded, vec!["lines"]);
    let payload_chars = result["payload_chars"].as_u64().expect("payload chars");
    assert!(payload_chars > 0 && payload_chars <= 25_000);
    assert_eq!(batch["total_payload_chars"].as_u64(), Some(payload_chars));
    let warning = batch["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .filter_map(|value| value.as_str())
        .find(|warning| warning.starts_with("batch_payload_budget_degraded"))
        .expect("degradation warning");
    assert!(warning.contains("structured.rs"));
    assert!(warning.contains("lines"));
    assert!(warning.contains("payload_chars="));
    let structured = batch["structured_warnings"]
        .as_array()
        .expect("structured warnings")
        .iter()
        .find(|warning| {
            warning["message"]
                .as_str()
                .is_some_and(|message| message.contains("dropped lines"))
        })
        .expect("structured degradation warning");
    assert_eq!(structured["code"], "fetch_content_truncated");
}

#[tokio::test]
async fn batch_total_payload_chars_bounds_three_item_batch() {
    let dir = tempfile::tempdir().expect("workspace root");
    let root_name = workspace_root_name(dir.path());
    for name in ["alpha.rs", "beta.rs", "gamma.rs"] {
        let content = "pub fn routine() { let value = 1; }\n".repeat(200);
        std::fs::write(dir.path().join(name), content).expect("write workspace file");
    }
    let items = ["alpha.rs", "beta.rs", "gamma.rs"]
        .iter()
        .map(|name| workspace_repo_item(&root_name, name, None))
        .collect();
    let args = batch_args(items);
    let batch = run_batch_fetch(workspace_state(dir.path()), args)
        .await
        .expect("three item batch");
    let mut summed = 0u64;
    for result in batch["results"].as_array().expect("results") {
        assert_eq!(result["ok"], true);
        assert!(!result["response"]["text"]
            .as_str()
            .unwrap_or_default()
            .is_empty());
        summed += result["payload_chars"]
            .as_u64()
            .expect("item payload chars");
    }
    assert_eq!(batch["total_payload_chars"].as_u64(), Some(summed));
    assert!(summed <= 50_000);
}

#[test]
fn focus_policy_utf8_safe() {
    let s = "a🦀b🦀c";
    assert_eq!(
        eggsearch::core::fetch_policy::truncate_utf8_safe(s, 2),
        "a🦀"
    );
}
