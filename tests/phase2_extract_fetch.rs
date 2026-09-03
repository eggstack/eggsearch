//! Phase 2 focused tests: extractive excerpts and fetch/cache controls.
//!
//! Covers the phase-2 acceptance surface with deterministic,
//! network-free tests (local `httpmock` servers only):
//!
//! - focused fetch projection causes no additional URL traversal;
//! - focus output respects chunk/character caps and validation;
//! - `cache_policy=default` preserves fresh-hit behavior;
//! - `bypass` skips cache reads but keeps URL/SSRF policy;
//! - `refresh` revalidates with validators when available;
//! - caller `max_cache_age_seconds` tightens reuse but cannot
//!   override `no-store` restrictions;
//! - omitted new fields preserve prior behavior;
//! - batch per-item cache controls are explicit and bounded.

use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::core::fetch::ExtractMode;
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_batch_fetch, run_web_fetch, run_web_search, WebFetchArgs};

fn localhost_state() -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    Arc::new(ServerState::build(cfg).expect("state builds"))
}

fn fetch_args(url: String) -> WebFetchArgs {
    WebFetchArgs {
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

#[tokio::test]
async fn focus_projection_causes_no_additional_fetch() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/article");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let state = localhost_state();
    let mut args = fetch_args(server.url("/article"));
    args.focus = Some("tokio runtime scheduler".to_string());
    let v = run_web_fetch(state, args).await.expect("focus fetch works");
    assert_eq!(mock.hits(), 1, "focus must not cause extra URL fetches");
    assert_eq!(v["status"], 200);
    let focus = &v["focus"];
    assert!(focus.is_object(), "focus selection must be present");
    let chunks = focus["chunks"].as_array().expect("chunks array");
    assert!(!chunks.is_empty());
    assert!(chunks.len() <= 5);
    let total = focus["total_chars"].as_u64().expect("total_chars");
    let summed: u64 = chunks
        .iter()
        .map(|c| c["text"].as_str().unwrap_or("").chars().count() as u64)
        .sum();
    assert_eq!(total, summed);
    let doc_chunks = v["document"]["chunks"].as_array().expect("doc chunks");
    let doc_texts: Vec<&str> = doc_chunks
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect();
    for c in chunks {
        let text = c["text"].as_str().expect("chunk text");
        assert!(
            doc_texts.iter().any(|d| *d == text || d.starts_with(text)),
            "focused text must come from stored document chunks: {text:?}"
        );
        assert!(!c["chunk_id"].as_str().unwrap_or("").is_empty());
    }
    assert!(v["text"].as_str().is_some(), "ordinary text is unchanged");
}

#[tokio::test]
async fn focus_output_respects_chunk_and_char_caps() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/article");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let state = localhost_state();
    let mut args = fetch_args(server.url("/article"));
    args.focus = Some("tokio".to_string());
    args.focus_max_chunks = Some(1);
    args.focus_max_chars = Some(40);
    let v = run_web_fetch(state, args).await.expect("focus fetch works");
    let focus = &v["focus"];
    let chunks = focus["chunks"].as_array().expect("chunks array");
    assert!(chunks.len() <= 1);
    assert!(focus["total_chars"].as_u64().unwrap_or(u64::MAX) <= 40);
}

#[tokio::test]
async fn focus_validation_rejects_bad_input() {
    let state = localhost_state();
    let mut args = fetch_args("https://example.com/".to_string());
    args.focus = Some("   ".to_string());
    let err = run_web_fetch(state.clone(), args)
        .await
        .expect_err("empty focus must fail");
    assert!(
        err.to_string().contains("focus must not be empty"),
        "got: {err}"
    );

    let mut args = fetch_args("https://example.com/".to_string());
    args.focus = Some("x".repeat(513));
    let err = run_web_fetch(state.clone(), args)
        .await
        .expect_err("oversized focus must fail");
    assert!(err.to_string().contains("focus must be <="), "got: {err}");

    let mut args = fetch_args("https://example.com/".to_string());
    args.focus = Some("tokio".to_string());
    args.focus_max_chunks = Some(0);
    let err = run_web_fetch(state.clone(), args)
        .await
        .expect_err("zero focus_max_chunks must fail");
    assert!(err.to_string().contains("focus_max_chunks"), "got: {err}");

    let mut args = fetch_args("https://example.com/".to_string());
    args.focus = Some("tokio".to_string());
    args.focus_max_chunks = Some(99);
    let err = run_web_fetch(state.clone(), args)
        .await
        .expect_err("oversized focus_max_chunks must fail");
    assert!(err.to_string().contains("focus_max_chunks"), "got: {err}");

    let mut args = fetch_args("https://example.com/".to_string());
    args.focus = Some("tokio".to_string());
    args.focus_max_chars = Some(0);
    let err = run_web_fetch(state.clone(), args)
        .await
        .expect_err("zero focus_max_chars must fail");
    assert!(err.to_string().contains("focus_max_chars"), "got: {err}");

    let mut args = fetch_args("https://example.com/".to_string());
    args.focus = Some("tokio".to_string());
    args.extract_mode = Some(ExtractMode::MetadataOnly);
    let err = run_web_fetch(state.clone(), args)
        .await
        .expect_err("focus with metadata_only must fail");
    assert!(err.to_string().contains("metadata_only"), "got: {err}");

    let mut args = fetch_args("https://example.com/".to_string());
    args.max_cache_age_seconds = Some(2_592_001);
    let err = run_web_fetch(state, args)
        .await
        .expect_err("oversized max_cache_age_seconds must fail");
    assert!(
        err.to_string().contains("max_cache_age_seconds"),
        "got: {err}"
    );
}

#[tokio::test]
async fn focus_no_match_returns_empty_selection() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/article");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let state = localhost_state();
    let mut args = fetch_args(server.url("/article"));
    args.focus = Some("zzzznomatchqqqq".to_string());
    let v = run_web_fetch(state, args).await.expect("fetch works");
    assert_eq!(v["focus"]["chunks"].as_array().map(|a| a.len()), Some(0));
    assert_eq!(v["focus"]["truncated"], false);
}

#[tokio::test]
async fn omitted_focus_leaves_response_unchanged() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/article");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(article_body());
    });
    let state = localhost_state();
    let v = run_web_fetch(state, fetch_args(server.url("/article")))
        .await
        .expect("fetch works");
    assert!(v["focus"].is_null(), "omitted focus must stay null");
    assert!(v["text"].as_str().is_some());
}

#[tokio::test]
async fn cache_default_preserves_fresh_hit() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .body(b"<html><head><title>T</title></head><body><p>hello</p></body></html>");
    });
    let state = localhost_state();
    let first = run_web_fetch(state.clone(), fetch_args(server.url("/page")))
        .await
        .expect("first fetch works");
    assert_eq!(first["cache_status"], "miss");
    let second = run_web_fetch(state, fetch_args(server.url("/page")))
        .await
        .expect("second fetch works");
    assert_eq!(second["cache_status"], "hit");
    assert_eq!(mock.hits(), 1, "fresh hit must not refetch");
}

#[tokio::test]
async fn cache_bypass_skips_read_but_keeps_policy() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mut mock = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .body(b"<html><head><title>T</title></head><body><p>hello</p></body></html>");
    });
    let state = localhost_state();
    run_web_fetch(state.clone(), fetch_args(server.url("/page")))
        .await
        .expect("prime works");
    assert_eq!(mock.hits(), 1);
    mock.delete();
    let v2 = server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .body(b"<html><head><title>T</title></head><body><p>version two</p></body></html>");
    });
    let mut args = fetch_args(server.url("/page"));
    args.cache_policy = Some(eggsearch::core::fetch::FetchCachePolicy::Bypass);
    let v = run_web_fetch(state.clone(), args)
        .await
        .expect("bypass refetches");
    assert_eq!(v["cache_status"], "bypassed");
    assert!(
        v["text"].as_str().unwrap_or("").contains("version two"),
        "bypass must serve fresh content, not the cached entry"
    );
    assert_eq!(v2.hits(), 1);
    let mut blocked = fetch_args("http://127.0.0.1:9/blocked".to_string());
    blocked.cache_policy = Some(eggsearch::core::fetch::FetchCachePolicy::Bypass);
    let denied = AppConfig::default();
    let denied_state = Arc::new(ServerState::build(denied).expect("state builds"));
    let err = run_web_fetch(denied_state, blocked)
        .await
        .expect_err("bypass must not skip URL/SSRF policy");
    assert!(
        err.to_string().contains("private")
            || err.to_string().contains("loopback")
            || err.to_string().contains("blocked")
            || err.to_string().contains("not allowed")
            || err.to_string().contains("denied"),
        "bypass must keep origin policy, got: {err}"
    );
}

#[tokio::test]
async fn cache_refresh_revalidates_with_validators() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mut prime = server.mock(|when, then| {
        when.method(GET).path("/doc");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .header("etag", "\"v1\"")
            .body(b"<html><head><title>T</title></head><body><p>v1 body</p></body></html>");
    });
    let state = localhost_state();
    run_web_fetch(state.clone(), fetch_args(server.url("/doc")))
        .await
        .expect("prime works");
    assert_eq!(prime.hits(), 1);
    prime.delete();
    let conditional = server.mock(|when, then| {
        when.method(GET)
            .path("/doc")
            .header("If-None-Match", "\"v1\"");
        then.status(304)
            .header("cache-control", "max-age=300")
            .header("etag", "\"v1\"");
    });
    let mut args = fetch_args(server.url("/doc"));
    args.cache_policy = Some(eggsearch::core::fetch::FetchCachePolicy::Refresh);
    let v = run_web_fetch(state, args).await.expect("refresh works");
    assert_eq!(v["cache_status"], "revalidated");
    assert_eq!(
        conditional.hits(),
        1,
        "refresh must revalidate conditionally"
    );
}

#[tokio::test]
async fn cache_max_age_forces_revalidation_of_fresh_entry() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/fresh");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .body(b"<html><head><title>T</title></head><body><p>hi</p></body></html>");
    });
    let state = localhost_state();
    run_web_fetch(state.clone(), fetch_args(server.url("/fresh")))
        .await
        .expect("prime works");
    let mut strict = fetch_args(server.url("/fresh"));
    strict.max_cache_age_seconds = Some(0);
    let v = run_web_fetch(state.clone(), strict)
        .await
        .expect("strict max-age fetch works");
    assert_ne!(v["cache_status"], "hit", "max-age=0 must not report fresh");
    assert_eq!(mock.hits(), 2, "stricter max-age must refetch");
    let v = run_web_fetch(state, fetch_args(server.url("/fresh")))
        .await
        .expect("default fetch works");
    assert_eq!(v["cache_status"], "hit");
}

#[tokio::test]
async fn cache_max_age_cannot_override_no_store() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/ephemeral");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "no-store")
            .body(b"<html><head><title>T</title></head><body><p>hi</p></body></html>");
    });
    let state = localhost_state();
    let first = run_web_fetch(state.clone(), fetch_args(server.url("/ephemeral")))
        .await
        .expect("first fetch works");
    assert_eq!(first["cache_status"], "not_cacheable");
    let mut args = fetch_args(server.url("/ephemeral"));
    args.max_cache_age_seconds = Some(3600);
    let second = run_web_fetch(state, args).await.expect("second works");
    assert_ne!(second["cache_status"], "hit");
    assert_eq!(mock.hits(), 2, "no-store must never be served from cache");
}

#[tokio::test]
async fn batch_item_cache_controls_are_explicit_and_bounded() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/b");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .body(b"<html><head><title>T</title></head><body><p>hi</p></body></html>");
    });
    let state = localhost_state();
    let item = |policy: Option<eggsearch::core::fetch::FetchCachePolicy>| {
        eggsearch::core::batch_fetch::BatchFetchItem::Web {
            url: server.url("/b"),
            extract_mode: None,
            include_links: None,
            max_chars: None,
            cache_policy: policy,
            max_cache_age_seconds: None,
        }
    };
    let run = |items: Vec<eggsearch::core::batch_fetch::BatchFetchItem>| {
        run_batch_fetch(
            state.clone(),
            eggsearch::mcp::tools::BatchFetchArgs {
                items,
                max_items: None,
                max_chars_per_item: None,
                max_total_chars: None,
                timeout_ms: None,
                continue_on_error: None,
            },
        )
    };
    let v = run(vec![item(None)]).await.expect("batch prime works");
    assert_eq!(v["results"][0]["ok"], true);
    let v = run(vec![item(None)]).await.expect("batch hit works");
    assert_eq!(v["results"][0]["response"]["cache_status"], "hit");
    assert_eq!(mock.hits(), 1);
    let v = run(vec![item(Some(
        eggsearch::core::fetch::FetchCachePolicy::Bypass,
    ))])
    .await
    .expect("batch bypass works");
    assert_eq!(v["results"][0]["response"]["cache_status"], "bypassed");
    assert_eq!(mock.hits(), 2);
    let bad = eggsearch::core::batch_fetch::BatchFetchItem::Web {
        url: server.url("/b"),
        extract_mode: None,
        include_links: None,
        max_chars: None,
        cache_policy: None,
        max_cache_age_seconds: Some(9_999_999),
    };
    let v = run(vec![bad]).await.expect("batch runs with item error");
    assert_eq!(v["results"][0]["ok"], false);
    assert!(
        v["results"][0]["error"]
            .as_str()
            .unwrap_or("")
            .contains("max_cache_age_seconds"),
        "oversized batch max-age must be an item error: {}",
        v["results"][0]
    );
}

#[tokio::test]
async fn web_search_rejects_oversized_excerpt_count() {
    let state = Arc::new(ServerState::build(AppConfig::default()).expect("state builds"));
    let args = eggsearch::mcp::tools::WebSearchArgs {
        query: "rust".to_string(),
        max_results: None,
        providers: Vec::new(),
        safe_search: None,
        timeout_ms: None,
        intent: None,
        freshness: None,
        date_range: None,
        include_domains: Vec::new(),
        exclude_domains: Vec::new(),
        language: None,
        region: None,
        excerpt_count: Some(99),
    };
    let err = run_web_search(state, args)
        .await
        .expect_err("oversized excerpt_count must fail");
    assert!(err.to_string().contains("excerpt_count"), "got: {err}");
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_excerpt_demand_is_additive() {
    use eggsearch::meta::mock::{mock_engines, MockEngine, MockResult};
    use eggsearch::meta::MetadataSearchAdapter;
    use std::time::Duration;

    let excerpt = || eggsearch::core::source_card::SourceExcerpt {
        text: "a source-derived passage".to_string(),
        score: Some(0.7),
        provenance: eggsearch::core::source_card::ExcerptProvenance::ProviderSnippet,
    };
    let results = || {
        vec![MockResult::new("Title", "https://example.com/a", "mock_a")
            .with_snippet("primary snippet")
            .with_excerpts(vec![excerpt()])
            .with_published_at("2024-02-01")]
    };
    let run = |excerpt_count: Option<usize>| {
        let engines = vec![MockEngine::success("mock_a", results())];
        let mut cfg = AppConfig::default();
        cfg.search.providers.insert("mock_a".to_string(), true);
        let adapter =
            MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
        let state = Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)));
        let args = eggsearch::mcp::tools::WebSearchArgs {
            query: "test".to_string(),
            max_results: None,
            providers: vec!["mock_a".to_string()],
            safe_search: None,
            timeout_ms: None,
            intent: None,
            freshness: None,
            date_range: None,
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            language: None,
            region: None,
            excerpt_count,
        };
        run_web_search(state, args)
    };
    let plain = run(None).await.expect("plain search works");
    let card = &plain["results"][0];
    assert_eq!(
        card["excerpts"].as_array().map(|a| a.len()).unwrap_or(0),
        0,
        "default search must not emit excerpts"
    );
    assert!(card["metadata"]["published_at"].is_string());
    let plain_id = card["stable_id"].as_str().expect("stable id").to_string();

    let with = run(Some(2)).await.expect("excerpt search works");
    let card = &with["results"][0];
    let excerpts = card["excerpts"].as_array().expect("excerpts array");
    assert_eq!(excerpts.len(), 1);
    assert!(excerpts[0]["text"]
        .as_str()
        .unwrap()
        .contains("source-derived"));
    assert_eq!(
        card["stable_id"].as_str(),
        Some(plain_id.as_str()),
        "excerpts must not alter stable IDs"
    );
}
