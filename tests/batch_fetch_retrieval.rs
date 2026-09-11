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
use eggsearch::mcp::tools::{run_batch_fetch, BatchFetchArgs};
use eggsearch::meta::mock::{mock_engines, MockEngine};
use eggsearch::meta::MetadataSearchAdapter;

fn localhost_state() -> Arc<ServerState> {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
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
        items,
        max_items: None,
        max_chars_per_item: None,
        max_total_chars: None,
        timeout_ms: None,
        continue_on_error: None,
        response_detail: None,
    }
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
    };
    let err = run_batch_fetch(state, batch_args(vec![bad_chars]))
        .await
        .expect_err("zero focus chars must fail");
    assert!(err.to_string().contains("focus_max_chars"));
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
    let root_name = dir
        .path()
        .file_name()
        .and_then(|n| n.to_str())
        .expect("root name")
        .to_string();
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn tokio_runtime() {}\n// tokio runtime scheduler drives tasks\npub fn other() {}\n",
    )
    .expect("write");
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.local.enabled = true;
    cfg.local.roots = vec![dir.path().to_path_buf()];
    let backend = eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg.local.clone())
        .expect("backend");
    backend.get_or_build_inventory();
    let mut state = ServerState::with_adapter(cfg, Arc::new(adapter));
    state.local_backend = Some(Arc::new(backend));
    let state = Arc::new(state);

    let v = run_batch_fetch(
        state,
        batch_args(vec![
            focused_web_item(server.url("/doc"), "tokio runtime"),
            BatchFetchItem::Repo {
                host: Some("workspace".to_string()),
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
                focus: Some("tokio_runtime".to_string()),
                focus_max_chunks: Some(3),
                focus_max_chars: Some(1000),
            },
        ]),
    )
    .await
    .expect("mixed batch works");
    assert_eq!(v["fetched"], 2);
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

#[test]
fn suggested_fetch_to_batch_round_trip() {
    use eggsearch::core::batch_fetch::BatchFetchItem;
    use eggsearch::core::repo_fetch::RepoFetchRequest;

    let req = RepoFetchRequest {
        host: Some(eggsearch::core::code_metadata::CodeHost::Github),
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
        timeout_ms: None,
        symbol: Some("Router".to_string()),
        symbol_kind: None,
        match_text: None,
        expand_to_block: None,
        max_block_lines: None,
        prefer_local: None,
    };
    let item = eggsearch::core::fetch_locator::structured_repo_fetch_to_batch_item(&req);
    match &item {
        BatchFetchItem::Repo {
            owner, repo, path, ..
        } => {
            assert_eq!(owner, "tokio-rs");
            assert_eq!(repo, "axum");
            assert_eq!(path, "src/lib.rs");
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
    assert_eq!(action.tool, "batch_fetch");
    assert_eq!(action.reason_code, "fetch_multiple_focused");
    assert!(action.input_template.get("items").is_some());

    let web = web_search_next_actions(&ids, true);
    assert!(web.iter().any(|a| a.tool == "batch_fetch"));
    let sec = security_search_next_actions(&ids, true);
    assert!(sec.iter().any(|a| a.tool == "batch_fetch"));
    let res = research_search_next_actions(&ids, true);
    assert!(res.iter().any(|a| a.tool == "batch_fetch"));
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

#[test]
fn focus_policy_utf8_safe() {
    let s = "a🦀b🦀c";
    assert_eq!(
        eggsearch::core::fetch_policy::truncate_utf8_safe(s, 2),
        "a🦀"
    );
}
