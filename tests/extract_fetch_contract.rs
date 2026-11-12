//! Extractive excerpts and fetch/cache controls contract.
//!
//! Regression provenance: phase-2 extractive-evidence workstream.
//! Covers the extractive-evidence acceptance surface with deterministic,
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
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
        }
    };
    let run = |items: Vec<eggsearch::core::batch_fetch::BatchFetchItem>| {
        run_batch_fetch(
            state.clone(),
            eggsearch::mcp::tools::BatchFetchArgs {
                offset: None,
                limit: None,
                items,
                max_items: None,
                max_chars_per_item: None,
                max_total_chars: None,
                timeout_ms: None,
                continue_on_error: None,
                response_detail: None,
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
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
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
        response_detail: None,
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
            response_detail: None,
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

// ---------------------------------------------------------------------------
// Markdown-first content negotiation (M003)
// ---------------------------------------------------------------------------

fn markdown_document() -> &'static str {
    "# Retry Guide\n\n## Backoff\n\nUse an idempotency key for every retry so a duplicate request is safe.\n\n| Attempt | Delay |\n| --- | --- |\n| 1 | 100 ms |\n| 2 | 200 ms |\n\n```rust\nfn retry() {\n    let budget = 3;\n    println!(\"budget {budget}\");\n}\n```\n\n[the inline note](https://example.com/inline)\n\n[the reference note][ref]\n\n[ref]: https://example.com/reference\n"
}

fn html_document() -> &'static str {
    "<!DOCTYPE html><html><head><title>Retry Guide</title></head><body>\
     <h1>Retry Guide</h1>\
     <h2>Backoff</h2>\
     <p>Use an idempotency key for every retry so a duplicate request is safe.</p>\
     <table><tr><th>Attempt</th><th>Delay</th></tr>\
     <tr><td>1</td><td>100 ms</td></tr><tr><td>2</td><td>200 ms</td></tr></table>\
     <pre><code class=\"language-rust\">fn retry() {\n    let budget = 3;\n    println!(\"budget {budget}\");\n}</code></pre>\
     <p><a href=\"https://example.com/inline\">the inline note</a></p>\
     <p><a href=\"https://example.com/reference\">the reference note</a></p>\
     </body></html>"
}

fn block_text_chars(payload: &serde_json::Value) -> usize {
    payload["document"]["blocks"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block["text"].as_str())
                .map(|text| text.chars().count())
                .sum()
        })
        .unwrap_or(0)
}

#[tokio::test]
async fn markdown_policy_header_is_sent_for_html_targets() {
    use eggsearch::fetch::accept::MARKDOWN_ACCEPT_POLICY;
    use httpmock::prelude::*;

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/guide")
            .header("accept", MARKDOWN_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/markdown; charset=utf-8")
            .body("# Guide\n\nNegotiated body.\n");
    });

    let v = run_web_fetch(localhost_state(), fetch_args(server.url("/guide")))
        .await
        .expect("negotiated fetch works");

    mock.assert_hits(1);
    assert_eq!(v["representation"], "markdown");
    assert_eq!(v["attempt_count"], 1);
    assert!(v["text"]
        .as_str()
        .unwrap_or("")
        .contains("Negotiated body."));
}

#[tokio::test]
async fn non_html_targets_keep_the_html_policy() {
    use eggsearch::fetch::accept::MARKDOWN_ACCEPT_POLICY;
    use httpmock::prelude::*;

    let server = MockServer::start();
    let markdown_accept = server.mock(|when, then| {
        when.method(GET)
            .path("/data.json")
            .header("accept", MARKDOWN_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/markdown")
            .body("# must not be requested\n");
    });
    let data = server.mock(|when, then| {
        when.method(GET).path("/data.json");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"ok":true}"#);
    });

    let v = run_web_fetch(localhost_state(), fetch_args(server.url("/data.json")))
        .await
        .expect("json fetch works");

    data.assert_hits(1);
    assert_eq!(
        markdown_accept.hits(),
        0,
        "a .json target must never send the markdown policy"
    );
    assert!(v["representation"].is_null());
    assert!(v["text"].as_str().unwrap_or("").contains("\"ok\""));
}

#[tokio::test]
async fn metadata_only_keeps_the_html_policy() {
    use eggsearch::fetch::accept::MARKDOWN_ACCEPT_POLICY;
    use httpmock::prelude::*;

    let server = MockServer::start();
    let markdown_accept = server.mock(|when, then| {
        when.method(GET)
            .path("/meta-page")
            .header("accept", MARKDOWN_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/markdown")
            .body("# head metadata would be lost\n");
    });
    let html = server.mock(|when, then| {
        when.method(GET).path("/meta-page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><head><title>Meta Title</title><meta name=\"description\" content=\"Meta Desc\"></head><body><p>body</p></body></html>");
    });

    let mut args = fetch_args(server.url("/meta-page"));
    args.extract_mode = Some(ExtractMode::MetadataOnly);
    let v = run_web_fetch(localhost_state(), args)
        .await
        .expect("metadata fetch works");

    html.assert_hits(1);
    assert_eq!(
        markdown_accept.hits(),
        0,
        "metadata_only must never negotiate away the HTML head"
    );
    assert!(v["title"].as_str().unwrap_or("").contains("Meta Title"));
    assert!(v["text"].is_null());
}

#[tokio::test]
async fn ignored_markdown_preference_does_not_trigger_a_second_request() {
    use eggsearch::fetch::accept::{HTML_ACCEPT_POLICY, MARKDOWN_ACCEPT_POLICY};
    use httpmock::prelude::*;

    let server = MockServer::start();
    let ignoring = server.mock(|when, then| {
        when.method(GET)
            .path("/ignoring")
            .header("accept", MARKDOWN_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>primary html body</p></body></html>");
    });
    let html_policy = server.mock(|when, then| {
        when.method(GET)
            .path("/ignoring")
            .header("accept", HTML_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>second html body</p></body></html>");
    });

    let v = run_web_fetch(localhost_state(), fetch_args(server.url("/ignoring")))
        .await
        .expect("fetch works");

    ignoring.assert_hits(1);
    assert_eq!(
        html_policy.hits(),
        0,
        "an origin that ignored the header already served a usable representation"
    );
    assert_eq!(v["attempt_count"], 1);
    assert_eq!(v["representation"], "html");
    assert!(v["text"]
        .as_str()
        .unwrap_or("")
        .contains("primary html body"));
}

#[tokio::test]
async fn markdown_fallback_on_406_uses_one_extra_request() {
    use eggsearch::fetch::accept::{HTML_ACCEPT_POLICY, MARKDOWN_ACCEPT_POLICY};
    use httpmock::prelude::*;

    let server = MockServer::start();
    let rejecting = server.mock(|when, then| {
        when.method(GET)
            .path("/not-acceptable")
            .header("accept", MARKDOWN_ACCEPT_POLICY);
        then.status(406).body("not acceptable");
    });
    let fallback = server.mock(|when, then| {
        when.method(GET)
            .path("/not-acceptable")
            .header("accept", HTML_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>html fallback content</p></body></html>");
    });

    let v = run_web_fetch(localhost_state(), fetch_args(server.url("/not-acceptable")))
        .await
        .expect("fallback returns usable content");

    rejecting.assert_hits(1);
    fallback.assert_hits(1);
    assert_eq!(v["attempt_count"], 2);
    assert_eq!(v["status"], 200);
    assert_eq!(v["representation"], "html");
    assert!(v["text"]
        .as_str()
        .unwrap_or("")
        .contains("html fallback content"));
}

#[tokio::test]
async fn markdown_fallback_on_empty_body_uses_the_html_representation() {
    use eggsearch::fetch::accept::{HTML_ACCEPT_POLICY, MARKDOWN_ACCEPT_POLICY};
    use httpmock::prelude::*;

    let server = MockServer::start();
    let empty = server.mock(|when, then| {
        when.method(GET)
            .path("/empty-markdown")
            .header("accept", MARKDOWN_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/markdown; charset=utf-8")
            .body("");
    });
    let fallback = server.mock(|when, then| {
        when.method(GET)
            .path("/empty-markdown")
            .header("accept", HTML_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><head><title>Recovered</title></head><body><p>recovered body</p></body></html>");
    });

    let v = run_web_fetch(localhost_state(), fetch_args(server.url("/empty-markdown")))
        .await
        .expect("empty markdown falls back");

    empty.assert_hits(1);
    fallback.assert_hits(1);
    assert_eq!(v["attempt_count"], 2);
    assert!(v["text"].as_str().unwrap_or("").contains("recovered body"));
}

#[tokio::test]
async fn markdown_representation_extracts_inline_and_reference_links() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/linked-md");
        then.status(200)
            .header("content-type", "text/markdown; charset=utf-8")
            .body(markdown_document());
    });

    let mut args = fetch_args(server.url("/linked-md"));
    args.include_links = Some(true);
    let v = run_web_fetch(localhost_state(), args)
        .await
        .expect("markdown link fetch works");

    let links = v["links"].as_array().expect("links array");
    let mut urls: Vec<&str> = links.iter().filter_map(|l| l["url"].as_str()).collect();
    urls.sort_unstable();
    assert_eq!(
        urls,
        vec![
            "https://example.com/inline",
            "https://example.com/reference"
        ],
        "inline and reference links must both be extracted"
    );
    assert_eq!(v["links_seen"], 2);
    assert_eq!(v["links_truncated"], false);
}

#[tokio::test]
async fn markdown_and_html_policy_cache_entries_stay_isolated() {
    use eggsearch::fetch::accept::{HTML_ACCEPT_POLICY, MARKDOWN_ACCEPT_POLICY};
    use httpmock::prelude::*;

    let server = MockServer::start();
    let markdown = server.mock(|when, then| {
        when.method(GET)
            .path("/isolated")
            .header("accept", MARKDOWN_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/markdown; charset=utf-8")
            .header("cache-control", "max-age=300")
            .header("etag", "\"md\"")
            .body("# Markdown body\n\nmarkdown-only text\n");
    });
    let html = server.mock(|when, then| {
        when.method(GET)
            .path("/isolated")
            .header("accept", HTML_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .header("etag", "\"html\"")
            .body("<html><head><title>Html Title</title></head><body><p>html-only text</p></body></html>");
    });

    let state = localhost_state();
    let first = run_web_fetch(state.clone(), fetch_args(server.url("/isolated")))
        .await
        .expect("markdown fetch works");
    assert_eq!(first["cache_status"], "miss");
    assert_eq!(first["representation"], "markdown");
    assert_eq!(markdown.hits(), 1);
    assert_eq!(html.hits(), 0);

    let metadata_args = || {
        let mut args = fetch_args(server.url("/isolated"));
        args.extract_mode = Some(ExtractMode::MetadataOnly);
        args
    };
    let metadata = run_web_fetch(state.clone(), metadata_args())
        .await
        .expect("html policy fetch works");
    assert_eq!(
        html.hits(),
        1,
        "the html key must not reuse the markdown entry"
    );
    assert_eq!(
        markdown.hits(),
        1,
        "the html request must not repeat the markdown request"
    );
    assert!(
        metadata["title"]
            .as_str()
            .unwrap_or("")
            .contains("Html Title"),
        "a markdown body must never be served for the html key"
    );

    let metadata_again = run_web_fetch(state.clone(), metadata_args())
        .await
        .expect("html policy cache hit");
    assert_eq!(metadata_again["cache_status"], "hit");
    assert_eq!(html.hits(), 1, "the html key caches under its own policy");

    let markdown_again = run_web_fetch(state, fetch_args(server.url("/isolated")))
        .await
        .expect("markdown cache hit");
    assert_eq!(markdown_again["cache_status"], "hit");
    assert_eq!(markdown.hits(), 1);
    assert_eq!(
        markdown_again["representation"], "markdown",
        "an html body must never be served for the markdown key"
    );
    assert!(markdown_again["text"]
        .as_str()
        .unwrap_or("")
        .contains("markdown-only text"));
}

#[tokio::test]
async fn conditional_revalidation_replays_the_markdown_policy() {
    use eggsearch::core::fetch::FetchCachePolicy;
    use eggsearch::fetch::accept::{HTML_ACCEPT_POLICY, MARKDOWN_ACCEPT_POLICY};
    use httpmock::prelude::*;

    let server = MockServer::start();
    let mut prime = server.mock(|when, then| {
        when.method(GET)
            .path("/revalidate")
            .header("accept", MARKDOWN_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/markdown; charset=utf-8")
            .header("cache-control", "max-age=300")
            .header("etag", "\"v1\"")
            .body("# Revalidated\n\nstill markdown\n");
    });
    let state = localhost_state();
    run_web_fetch(state.clone(), fetch_args(server.url("/revalidate")))
        .await
        .expect("prime works");
    prime.delete();

    let conditional = server.mock(|when, then| {
        when.method(GET)
            .path("/revalidate")
            .header("accept", MARKDOWN_ACCEPT_POLICY)
            .header("If-None-Match", "\"v1\"");
        then.status(304)
            .header("cache-control", "max-age=300")
            .header("etag", "\"v1\"");
    });
    let wrong_policy = server.mock(|when, then| {
        when.method(GET)
            .path("/revalidate")
            .header("accept", HTML_ACCEPT_POLICY);
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=300")
            .body("<html><head><title>Html Only</title></head><body><p>html replacement body</p></body></html>");
    });

    let mut args = fetch_args(server.url("/revalidate"));
    args.cache_policy = Some(FetchCachePolicy::Refresh);
    let v = run_web_fetch(state.clone(), args)
        .await
        .expect("revalidation works");

    conditional.assert_hits(1);
    assert_eq!(
        wrong_policy.hits(),
        0,
        "revalidation must replay the policy that produced the entry"
    );
    assert_eq!(v["cache_status"], "revalidated");
    assert_eq!(v["representation"], "markdown");
    assert!(v["text"].as_str().unwrap_or("").contains("still markdown"));

    let mut metadata_args = fetch_args(server.url("/revalidate"));
    metadata_args.extract_mode = Some(ExtractMode::MetadataOnly);
    let metadata = run_web_fetch(state, metadata_args)
        .await
        .expect("html policy fetch after revalidation works");
    assert_eq!(
        wrong_policy.hits(),
        1,
        "the html key must fetch its own representation after a 304 refresh"
    );
    assert!(
        metadata["title"]
            .as_str()
            .unwrap_or("")
            .contains("Html Only"),
        "a revalidated markdown body must never answer the html key"
    );
}

#[tokio::test]
async fn markdown_representation_ab_reduces_bytes_and_keeps_structure() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/ab-markdown");
        then.status(200)
            .header("content-type", "text/markdown; charset=utf-8")
            .header("x-markdown-tokens", "64")
            .header("x-original-tokens", "180")
            .body(markdown_document());
    });
    server.mock(|when, then| {
        when.method(GET).path("/ab-html");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(html_document());
    });

    let state = localhost_state();
    let mut markdown_args = fetch_args(server.url("/ab-markdown"));
    markdown_args.include_links = Some(true);
    markdown_args.focus = Some("idempotency key".to_string());
    let markdown = run_web_fetch(state.clone(), markdown_args)
        .await
        .expect("markdown arm works");

    let mut html_args = fetch_args(server.url("/ab-html"));
    html_args.include_links = Some(true);
    html_args.focus = Some("idempotency key".to_string());
    let html = run_web_fetch(state, html_args)
        .await
        .expect("html arm works");

    assert_eq!(markdown["representation"], "markdown");
    assert_eq!(html["representation"], "html");

    let markdown_bytes = markdown["document"]["metadata"]["bytes_read"]
        .as_u64()
        .expect("markdown bytes_read");
    let html_bytes = html["document"]["metadata"]["bytes_read"]
        .as_u64()
        .expect("html bytes_read");
    assert!(
        markdown_bytes < html_bytes,
        "markdown representation must return fewer body characters than html: {markdown_bytes} vs {html_bytes}"
    );

    let markdown_block_chars = block_text_chars(&markdown);
    let html_block_chars = block_text_chars(&html);
    assert!(
        markdown_block_chars <= html_block_chars,
        "extracted markdown text must not exceed html text: {markdown_block_chars} vs {html_block_chars}"
    );

    let outline_titles = |payload: &serde_json::Value| -> Vec<String> {
        payload["document"]["outline"]
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|e| e["title"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(outline_titles(&markdown), vec!["Retry Guide", "Backoff"]);
    assert_eq!(outline_titles(&markdown), outline_titles(&html));

    let table_text = |payload: &serde_json::Value| -> String {
        payload["document"]["blocks"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["kind"] == "table")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    };
    for payload in [&markdown, &html] {
        let table = table_text(payload);
        assert!(table.contains("| Attempt | Delay |"), "{table}");
        assert!(table.contains("| 1 | 100 ms |"), "{table}");
        assert!(table.contains("| 2 | 200 ms |"), "{table}");
    }

    let code_text = |payload: &serde_json::Value| -> String {
        payload["document"]["blocks"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["kind"] == "code")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    };
    for payload in [&markdown, &html] {
        let code = code_text(payload);
        assert!(code.contains("fn retry() {"), "{code}");
        assert!(code.contains("let budget = 3;"), "{code}");
        assert!(code.contains("println!(\"budget {budget}\");"), "{code}");
        assert_eq!(code.lines().count(), 4, "{code}");
    }

    let link_urls = |payload: &serde_json::Value| -> Vec<String> {
        let mut urls: Vec<String> = payload["links"]
            .as_array()
            .map(|links| {
                links
                    .iter()
                    .filter_map(|l| l["url"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        urls.sort_unstable();
        urls
    };
    assert_eq!(link_urls(&markdown).len(), 2);
    assert_eq!(link_urls(&markdown), link_urls(&html));

    for payload in [&markdown, &html] {
        let chunks = payload["focus"]["chunks"].as_array().expect("focus chunks");
        assert!(!chunks.is_empty(), "focus query must hit a chunk");
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk["text"].as_str().unwrap_or("").contains("idempotency")),
            "focus chunk must carry the matched paragraph"
        );
    }

    assert_eq!(
        markdown["document"]["metadata"]["x-markdown-tokens"], 64,
        "origin token estimate must be surfaced"
    );
    assert_eq!(
        markdown["document"]["metadata"]["x-original-tokens"], 180,
        "origin original-token estimate must be surfaced"
    );

    eprintln!(
        "M003 A/B: markdown body_chars={markdown_bytes} block_chars={markdown_block_chars} \
         approx_tokens={} | html body_chars={html_bytes} block_chars={html_block_chars} \
         approx_tokens={}",
        markdown_block_chars.div_ceil(4),
        html_block_chars.div_ceil(4)
    );
}

#[tokio::test]
async fn unparseable_token_headers_are_omitted() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/bad-tokens");
        then.status(200)
            .header("content-type", "text/markdown; charset=utf-8")
            .header("x-markdown-tokens", "not-a-number")
            .header("x-original-tokens", "-5")
            .body("# Tokens\n\nbody\n");
    });

    let v = run_web_fetch(localhost_state(), fetch_args(server.url("/bad-tokens")))
        .await
        .expect("fetch works");
    assert!(v["document"]["metadata"]["x-markdown-tokens"].is_null());
    assert!(v["document"]["metadata"]["x-original-tokens"].is_null());
}

#[cfg(feature = "mock")]
fn compact_search_args(
    query: &str,
    response_detail: eggsearch::mcp::projection::ResponseDetail,
) -> eggsearch::mcp::tools::WebSearchArgs {
    eggsearch::mcp::tools::WebSearchArgs {
        query: query.to_string(),
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
        excerpt_count: None,
        response_detail: Some(response_detail),
    }
}

#[cfg(feature = "mock")]
async fn run_mock_search(
    results: Vec<eggsearch::meta::mock::MockResult>,
    detail: eggsearch::mcp::projection::ResponseDetail,
) -> serde_json::Value {
    use eggsearch::meta::mock::{mock_engines, MockEngine};
    use eggsearch::meta::MetadataSearchAdapter;
    use std::time::Duration;

    let engines = vec![MockEngine::success("mock_a", results)];
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    let state = Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)));
    run_web_search(state, compact_search_args("tokio runtime", detail))
        .await
        .expect("mock search works")
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_compact_collapses_cross_result_duplicate_passages() {
    use eggsearch::mcp::projection::ResponseDetail;
    use eggsearch::meta::mock::MockResult;

    let shared =
        "The tokio runtime drives async tasks to completion with a work stealing scheduler";
    let results = vec![
        MockResult::new("Title A", "https://example.com/a", "mock_a").with_snippet(shared),
        MockResult::new("Title B", "https://example.com/b", "mock_a").with_snippet(shared),
        MockResult::new("Title C", "https://example.com/c", "mock_a")
            .with_snippet("An unrelated sentence about pasta recipes and dinner plans"),
    ];
    let diagnostic = run_mock_search(results.clone(), ResponseDetail::Diagnostic).await;
    let compact = run_mock_search(results, ResponseDetail::Compact).await;

    let economy = &compact["excerpt_economy"];
    assert!(economy.is_object(), "compact must report excerpt_economy");
    let admitted = economy["admitted_count"].as_u64().unwrap();
    let redundant = economy["redundant_count"].as_u64().unwrap();
    let over_budget = economy["over_budget_count"].as_u64().unwrap();
    let considered = economy["considered_count"].as_u64().unwrap();
    let chars_before = economy["chars_before"].as_u64().unwrap();
    let chars_after = economy["chars_after"].as_u64().unwrap();
    assert_eq!(admitted + redundant + over_budget, considered);
    assert!(redundant >= 1, "duplicate passage must collapse: {economy}");
    assert!(chars_after < chars_before);
    println!(
        "excerpt-economy duplicate-fixture considered={considered} admitted={admitted} redundant={redundant} over_budget={over_budget} chars_before={chars_before} chars_after={chars_after} compact_bytes={} diagnostic_bytes={}",
        serde_json::to_string(&compact).unwrap().len(),
        serde_json::to_string(&diagnostic).unwrap().len()
    );

    let compact_results = compact["results"].as_array().unwrap();
    let diagnostic_results = diagnostic["results"].as_array().unwrap();
    assert_eq!(compact_results.len(), diagnostic_results.len());
    for (index, card) in compact_results.iter().enumerate() {
        assert_eq!(card["stable_id"], diagnostic_results[index]["stable_id"]);
        assert_eq!(card["url"], diagnostic_results[index]["url"]);
    }
    assert!(compact_results[0]["snippet"].is_string());
    assert!(compact_results[1].get("snippet").is_none());
    assert_eq!(compact_results[1]["excerpts_suppressed"], 1);
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_compact_retains_distinct_passages() {
    use eggsearch::mcp::projection::ResponseDetail;
    use eggsearch::meta::mock::MockResult;

    let results = vec![
        MockResult::new("Title A", "https://example.com/a", "mock_a")
            .with_snippet("The tokio runtime drives async tasks to completion"),
        MockResult::new("Title B", "https://example.com/b", "mock_a")
            .with_snippet("Cargo resolves dependencies from the registry index"),
        MockResult::new("Title C", "https://example.com/c", "mock_a")
            .with_snippet("An unrelated sentence about pasta recipes and dinner plans"),
    ];
    let compact = run_mock_search(results, ResponseDetail::Compact).await;
    let economy = &compact["excerpt_economy"];
    assert_eq!(economy["redundant_count"].as_u64(), Some(0));
    assert_eq!(economy["considered_count"].as_u64(), Some(3));
    assert_eq!(economy["admitted_count"].as_u64(), Some(3));
    let chars_before = economy["chars_before"].as_u64().unwrap();
    let chars_after = economy["chars_after"].as_u64().unwrap();
    assert_eq!(chars_before, chars_after);
    println!(
        "excerpt-economy distinct-fixture chars_before={chars_before} chars_after={chars_after} compact_bytes={}",
        serde_json::to_string(&compact).unwrap().len()
    );
    for card in compact["results"].as_array().unwrap() {
        assert!(card["snippet"].is_string());
        assert!(card.get("excerpts_suppressed").is_none());
    }
}

#[derive(serde::Deserialize)]
struct FetchWindowCase {
    id: String,
    query: Option<String>,
    content_type: String,
    body: String,
    answer_start: usize,
    answer_end: usize,
}

#[derive(serde::Deserialize)]
struct FetchWindowCorpus {
    schema_version: u32,
    cases: Vec<FetchWindowCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FetchWindowRecord {
    id: String,
    contained: bool,
    answer_start: usize,
    text_from: usize,
    text_to: usize,
    text_total: usize,
}

struct FetchWindowMeasurement {
    records: Vec<FetchWindowRecord>,
    doc_ids: Vec<String>,
}

fn fetch_window_corpus() -> FetchWindowCorpus {
    let corpus: FetchWindowCorpus =
        serde_json::from_str(include_str!("fixtures/fetch_windows/corpus.json"))
            .expect("fetch-window corpus parses");
    assert_eq!(
        corpus.schema_version, 1,
        "unsupported corpus schema version"
    );
    assert!(
        (8..=12).contains(&corpus.cases.len()),
        "corpus must hold 8 to 12 cases, found {}",
        corpus.cases.len()
    );
    corpus
}

async fn measure_fetch_windows(corpus: &FetchWindowCorpus) -> FetchWindowMeasurement {
    use httpmock::prelude::*;

    let server = MockServer::start();
    for case in &corpus.cases {
        let path = format!("/{}", case.id);
        let body = case.body.clone();
        let content_type = case.content_type.clone();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200)
                .header("content-type", content_type.as_str())
                .body(body.as_str());
        });
    }
    let state = localhost_state();
    let mut records = Vec::with_capacity(corpus.cases.len());
    let mut doc_ids = Vec::with_capacity(corpus.cases.len());
    for case in &corpus.cases {
        let mut args = fetch_args(server.url(format!("/{}", case.id)));
        args.offset = Some(0);
        args.focus = case.query.clone();
        let payload = run_web_fetch(state.clone(), args)
            .await
            .unwrap_or_else(|error| panic!("fetch-window case {} failed: {error:?}", case.id));
        let field = |name: &str| -> usize {
            payload[name]
                .as_u64()
                .unwrap_or_else(|| panic!("fetch-window case {} lost {name}", case.id))
                as usize
        };
        let text_from = field("text_from");
        let text_to = field("text_to");
        let text_total = field("text_total");
        doc_ids.push(
            payload["doc_id"]
                .as_str()
                .unwrap_or_else(|| panic!("fetch-window case {} lost doc_id", case.id))
                .to_string(),
        );
        records.push(FetchWindowRecord {
            id: case.id.clone(),
            contained: case.answer_start >= text_from && case.answer_end <= text_to,
            answer_start: case.answer_start,
            text_from,
            text_to,
            text_total,
        });
    }
    FetchWindowMeasurement { records, doc_ids }
}

fn median_chars_to_answer(values: &[usize]) -> usize {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    match sorted.len() {
        0 => 0,
        len if len % 2 == 1 => sorted[len / 2],
        len => (sorted[len / 2 - 1] + sorted[len / 2]) / 2,
    }
}

#[tokio::test]
async fn fetch_window_first_slice_precision_is_recorded() {
    let corpus = fetch_window_corpus();
    for case in &corpus.cases {
        assert!(
            case.answer_start < case.answer_end,
            "fetch-window case {} has an empty answer span",
            case.id
        );
    }

    let first = measure_fetch_windows(&corpus).await;
    let second = measure_fetch_windows(&corpus).await;
    assert_eq!(
        first.records, second.records,
        "the fetch-window measurement must be repeatable"
    );
    assert_eq!(first.records.len(), corpus.cases.len());

    for (record, case) in first.records.iter().zip(&corpus.cases) {
        assert_eq!(record.id, case.id);
        assert_eq!(
            record.text_from, 0,
            "fetch-window case {} must be addressed from the document origin",
            record.id
        );
        assert!(
            record.text_to > 0,
            "fetch-window case {} returned an empty window",
            record.id
        );
        assert!(
            record.text_to <= record.text_total,
            "fetch-window case {} window ends past the addressable total",
            record.id
        );
        assert!(
            case.answer_end <= record.text_total,
            "fetch-window case {} span ends past the addressable total",
            record.id
        );
    }
    for (record, doc_id) in first.records.iter().zip(&first.doc_ids) {
        assert!(
            !doc_id.is_empty(),
            "fetch-window case {} returned an empty doc_id",
            record.id
        );
    }

    let contained = first.records.iter().filter(|r| r.contained).count();
    let total = first.records.len();
    let starts: Vec<usize> = first.records.iter().map(|r| r.answer_start).collect();
    println!(
        "fetch-window first-slice measurement: default window (offset 0, no limit), {total} cases"
    );
    println!(
        "{:<46} {:>9} {:>13} {:>11} {:>9}",
        "id", "contained", "answer_start", "text_total", "text_to"
    );
    for record in &first.records {
        println!(
            "{:<46} {:>9} {:>13} {:>11} {:>9}",
            record.id,
            if record.contained { "yes" } else { "no" },
            record.answer_start,
            record.text_total,
            record.text_to
        );
    }
    println!(
        "first-window recall: {contained}/{total} = {:.1}%",
        contained as f64 * 100.0 / total as f64
    );
    println!(
        "median chars-to-answer: {}",
        median_chars_to_answer(&starts)
    );
    assert!(
        first.records.iter().any(|r| r.contained) && first.records.iter().any(|r| !r.contained),
        "the corpus must exercise both containment outcomes so the record is not vacuous"
    );
}
