#![allow(unused_imports, dead_code)]
//! Evidence packaging and focused batch retrieval.
//!
//! Behavior-oriented suite partitioned from the historical `integration.rs` mega-suite.

mod support;

use support::*;

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

#[tokio::test]
async fn batch_fetch_returns_results_with_same_length_as_input() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    for i in 0..3 {
        server.mock(move |when, then| {
            when.method(GET).path(format!("/p{i}"));
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(format!(
                    "<!DOCTYPE html><html><body><p>Page {i}</p></body></html>"
                ));
        });
    }

    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    let state = Arc::new(ServerState::build(cfg).expect("state builds"));

    let items: Vec<eggsearch::core::batch_fetch::BatchFetchItem> = (0..3)
        .map(|i| eggsearch::core::batch_fetch::BatchFetchItem::Web {
            url: server.url(format!("/p{i}")),
            extract_mode: None,
            include_links: None,
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            max_chars: None,
            cache_policy: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
            snapshot: None,
            pdf: None,
            render: None,
            browser_profile: None,
            include_screenshot: None,
        })
        .collect();

    let v = run_batch_fetch(
        state,
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
        },
    )
    .await
    .expect("batch_fetch should succeed");

    let results = v["results"].as_array().expect("results is array");
    assert_eq!(
        results.len(),
        3,
        "results length must match input URL count"
    );
    assert_eq!(v["fetched"], 3);
    assert_eq!(v["failed"], 0);
}

#[tokio::test]
async fn batch_fetch_empty_items_returns_validation_not_empty_array() {
    let state = state_with_default();
    let res = run_batch_fetch(
        state,
        BatchFetchArgs {
            offset: None,
            limit: None,
            items: vec![],
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: None,
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        },
    )
    .await;
    let err = res.expect_err("empty items should be a validation error");
    assert!(
        err.to_string().contains("must not be empty"),
        "error should say 'must not be empty': {err}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn batch_fetch_result_stable_ids_are_deterministic() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<!DOCTYPE html><html><body><p>OK</p></body></html>");
    });

    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    let state = Arc::new(ServerState::build(cfg).expect("state builds"));

    let v1 = run_batch_fetch(
        state.clone(),
        BatchFetchArgs {
            offset: None,
            limit: None,
            items: vec![eggsearch::core::batch_fetch::BatchFetchItem::Web {
                url: server.url("/page"),
                extract_mode: None,
                include_links: None,
                include_selectors: None,
                exclude_selectors: None,
                attributes: None,
                max_chars: None,
                cache_policy: None,
                max_cache_age_seconds: None,
                focus: None,
                focus_max_chunks: None,
                focus_max_chars: None,
                snapshot: None,
                pdf: None,
                render: None,
                browser_profile: None,
                include_screenshot: None,
            }],
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: None,
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        },
    )
    .await
    .expect("ok");
    let id1 = v1["results"].as_array().unwrap()[0]["stable_id"]
        .as_str()
        .unwrap()
        .to_string();

    let v2 = run_batch_fetch(
        state,
        BatchFetchArgs {
            offset: None,
            limit: None,
            items: vec![eggsearch::core::batch_fetch::BatchFetchItem::Web {
                url: server.url("/page"),
                extract_mode: None,
                include_links: None,
                include_selectors: None,
                exclude_selectors: None,
                attributes: None,
                max_chars: None,
                cache_policy: None,
                max_cache_age_seconds: None,
                focus: None,
                focus_max_chunks: None,
                focus_max_chars: None,
                snapshot: None,
                pdf: None,
                render: None,
                browser_profile: None,
                include_screenshot: None,
            }],
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: None,
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        },
    )
    .await
    .expect("ok");
    let id2 = v2["results"].as_array().unwrap()[0]["stable_id"]
        .as_str()
        .unwrap()
        .to_string();

    assert_eq!(
        id1, id2,
        "batch_fetch stable_id must be deterministic across calls"
    );
    assert!(
        id1.starts_with("batch_"),
        "stable_id must use batch_ prefix: {id1}"
    );
}

#[tokio::test]
async fn batch_fetch_with_single_empty_url_returns_validation_error() {
    let state = state_with_default();
    let res = run_batch_fetch(
        state,
        BatchFetchArgs {
            offset: None,
            limit: None,
            items: vec![eggsearch::core::batch_fetch::BatchFetchItem::Web {
                url: "   ".to_string(),
                extract_mode: None,
                include_links: None,
                include_selectors: None,
                exclude_selectors: None,
                attributes: None,
                max_chars: None,
                cache_policy: None,
                max_cache_age_seconds: None,
                focus: None,
                focus_max_chunks: None,
                focus_max_chars: None,
                snapshot: None,
                pdf: None,
                render: None,
                browser_profile: None,
                include_screenshot: None,
            }],
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: None,
            timeout_ms: None,
            continue_on_error: None,
            response_detail: None,
        },
    )
    .await;
    let err = res.expect_err("blank URL should error");
    assert!(
        err.to_string().contains("url must not be empty"),
        "error should mention empty URL: {err}"
    );
}

#[tokio::test]
async fn batch_fetch_shares_cache_with_web_fetch() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method(GET).path("/shared-cache");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("cache-control", "max-age=3600")
            .body("<html><body>cached</body></html>");
    });

    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.fetch.retry_max_attempts = 1;
    let state = Arc::new(ServerState::build(cfg).expect("state builds"));

    let v1 = run_web_fetch(
        state.clone(),
        WebFetchArgs {
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            url: server.url("/shared-cache"),
            ..Default::default()
        },
    )
    .await
    .expect("web_fetch ok");
    assert_eq!(v1["cache_status"], "miss");

    let batch_result = run_batch_fetch(
        state.clone(),
        eggsearch::mcp::tools::BatchFetchArgs {
            offset: None,
            limit: None,
            items: vec![eggsearch::core::batch_fetch::BatchFetchItem::Web {
                url: server.url("/shared-cache"),
                extract_mode: Some(ExtractMode::Text),
                include_links: Some(false),
                include_selectors: None,
                exclude_selectors: None,
                attributes: None,
                max_chars: Some(12000),
                cache_policy: None,
                max_cache_age_seconds: None,
                focus: None,
                focus_max_chunks: None,
                focus_max_chars: None,
                snapshot: None,
                pdf: None,
                render: None,
                browser_profile: None,
                include_screenshot: None,
            }],
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: Some(50000),
            continue_on_error: Some(true),
            timeout_ms: None,
            response_detail: None,
        },
    )
    .await
    .expect("batch_fetch ok");

    let results = batch_result["results"]
        .as_array()
        .expect("results is array");
    assert_eq!(results.len(), 1);
    let item = &results[0];
    assert!(item["ok"].as_bool().unwrap_or(false));

    mock.assert_calls(1);
}

#[tokio::test]
async fn batch_fetch_respects_per_origin_concurrency() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/origin-a");
        then.status(200)
            .header("content-type", "text/plain")
            .body("a")
            .delay(Duration::from_millis(50));
    });
    server.mock(|when, then| {
        when.method(GET).path("/origin-b");
        then.status(200)
            .header("content-type", "text/plain")
            .body("b")
            .delay(Duration::from_millis(50));
    });

    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.fetch.retry_max_attempts = 1;
    cfg.fetch.origin_http_concurrency = 1;
    let state = Arc::new(ServerState::build(cfg).expect("state builds"));

    let items: Vec<eggsearch::core::batch_fetch::BatchFetchItem> = (0..4)
        .map(|i| {
            let path = if i % 2 == 0 { "/origin-a" } else { "/origin-b" };
            eggsearch::core::batch_fetch::BatchFetchItem::Web {
                url: server.url(path),
                extract_mode: Some(ExtractMode::Text),
                include_links: Some(false),
                include_selectors: None,
                exclude_selectors: None,
                attributes: None,
                max_chars: Some(12000),
                cache_policy: None,
                max_cache_age_seconds: None,
                focus: None,
                focus_max_chunks: None,
                focus_max_chars: None,
                snapshot: None,
                pdf: None,
                render: None,
                browser_profile: None,
                include_screenshot: None,
            }
        })
        .collect();

    let result = run_batch_fetch(
        state.clone(),
        eggsearch::mcp::tools::BatchFetchArgs {
            offset: None,
            limit: None,
            items,
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: Some(120000),
            continue_on_error: Some(true),
            timeout_ms: None,
            response_detail: None,
        },
    )
    .await
    .expect("batch_fetch ok");

    let arr = result["results"].as_array().expect("results is array");
    assert_eq!(arr.len(), 4);
    let successes = arr
        .iter()
        .filter(|item| item["ok"].as_bool().unwrap_or(false))
        .count();
    assert_eq!(
        successes, 4,
        "all items should succeed despite concurrency=1"
    );
}

#[test]
fn build_evidence_bundle_returns_expected_structure() {
    use eggsearch::mcp::tools::{run_build_evidence_bundle, EvidenceBundleArgs};

    let args = EvidenceBundleArgs {
        goal: Some("test evidence bundle".to_string()),
        sources: vec![eggsearch::core::evidence_bundle::EvidenceSourceInput {
            id: Some("src_test123".to_string()),
            url: Some("https://example.com/source".to_string()),
            title: Some("Test Source".to_string()),
            snippet: Some("A test snippet".to_string()),
            providers: vec!["mock".to_string()],
            score: Some(0.95),
            trust: Some(eggsearch::core::result::TrustLevel::ExternalUntrusted),
            trust_markers: None,
            metadata: None,
            quality: None,
        }],
        fetches: vec![],
        include_unfetched_sources: None,
        max_sources: None,
        max_fetched_items: None,
        max_total_chars: None,
        response_detail: None,
    };

    let v = run_build_evidence_bundle(args).expect("bundle should succeed");

    assert!(
        v["bundle_id"].as_str().is_some(),
        "bundle must have bundle_id"
    );
    assert!(
        v["bundle_id"].as_str().unwrap().starts_with("bundle_"),
        "bundle_id must use bundle_ prefix"
    );
    assert!(
        v["created_at"].as_str().is_some(),
        "bundle must have created_at"
    );

    let sources = v["sources"].as_array().expect("sources is array");
    assert_eq!(sources.len(), 1, "should have 1 source");
    assert_eq!(sources[0]["title"], "Test Source");
    assert_eq!(
        sources[0]["trust"], "external_untrusted",
        "trust label on source"
    );

    let trust_summary = v["trust_summary"]
        .as_object()
        .expect("trust_summary is object");
    assert!(
        trust_summary.get("external_untrusted_count").is_some(),
        "trust_summary must have external_untrusted_count"
    );

    let provider_summary = v["provider_summary"]
        .as_object()
        .expect("provider_summary is object");
    assert!(
        provider_summary.get("providers_used").is_some(),
        "provider_summary must have providers_used"
    );
    assert!(
        provider_summary.get("per_provider_counts").is_some(),
        "provider_summary must have per_provider_counts"
    );

    let limits = v["limits"].as_object().expect("limits is object");
    assert!(
        limits.get("max_sources").is_some(),
        "limits must have max_sources"
    );
}

#[test]
fn build_evidence_bundle_empty_sources_and_fetches_errors() {
    use eggsearch::mcp::tools::{run_build_evidence_bundle, EvidenceBundleArgs};

    let args = EvidenceBundleArgs {
        goal: None,
        sources: vec![],
        fetches: vec![],
        include_unfetched_sources: None,
        max_sources: None,
        max_fetched_items: None,
        max_total_chars: None,
        response_detail: None,
    };

    let err = run_build_evidence_bundle(args).expect_err("empty bundle should error");
    assert!(
        err.to_string().contains("at least one"),
        "error should mention at least one source/fetch: {err}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn build_evidence_bundle_with_sources_and_fetches() {
    use eggsearch::mcp::tools::{run_build_evidence_bundle, EvidenceBundleArgs};

    let args = EvidenceBundleArgs {
        goal: Some("comprehensive evidence".to_string()),
        sources: vec![
            eggsearch::core::evidence_bundle::EvidenceSourceInput {
                id: Some("src_abc".to_string()),
                url: Some("https://example.com/doc1".to_string()),
                title: Some("Documentation".to_string()),
                snippet: Some("Official docs".to_string()),
                providers: vec!["mock".to_string()],
                score: Some(0.9),
                trust: Some(eggsearch::core::result::TrustLevel::ExternalUntrusted),
                trust_markers: None,
                metadata: None,
                quality: None,
            },
            eggsearch::core::evidence_bundle::EvidenceSourceInput {
                id: Some("src_def".to_string()),
                url: Some("https://example.com/doc2".to_string()),
                title: Some("Blog Post".to_string()),
                snippet: Some("Community discussion".to_string()),
                providers: vec!["mock".to_string()],
                score: Some(0.7),
                trust: Some(eggsearch::core::result::TrustLevel::ExternalUntrusted),
                trust_markers: None,
                metadata: None,
                quality: None,
            },
        ],
        fetches: vec![eggsearch::core::evidence_bundle::EvidenceFetchInput {
            source_id: Some("src_abc".to_string()),
            url: Some("https://example.com/doc1".to_string()),
            locator: None,
            fetched: true,
            content_type: Some("text/html".to_string()),
            language: None,
            selected_span: None,
            code_span_id: None,
            line_start: None,
            line_end: None,
            text: Some("Full fetched content here".to_string()),
            truncated: false,
            trust: Some(eggsearch::core::FetchTrust::ExternalUntrusted),
            trust_markers: None,
            warnings: vec![],
        }],
        include_unfetched_sources: None,
        max_sources: None,
        max_fetched_items: None,
        max_total_chars: None,
        response_detail: None,
    };

    let v = run_build_evidence_bundle(args).expect("bundle should succeed");

    let sources = v["sources"].as_array().expect("sources is array");
    assert_eq!(sources.len(), 2, "should have 2 sources");

    let fetched = v["fetched_items"]
        .as_array()
        .expect("fetched_items is array");
    assert_eq!(fetched.len(), 1, "should have 1 fetched item");
    assert_eq!(fetched[0]["fetched"], true);

    let links = v["source_links"].as_array().expect("source_links is array");
    assert!(
        !links.is_empty(),
        "should have links between sources and fetches"
    );

    let ts = v["trust_summary"].as_object().expect("trust_summary");
    let total_trust: i64 = ["external_untrusted_count", "local_trusted_count"]
        .iter()
        .filter_map(|k| ts.get(*k).and_then(|v| v.as_i64()))
        .sum();
    assert!(
        total_trust >= 2,
        "trust_summary total should be >= number of sources: {ts:?}"
    );

    assert_eq!(v["goal"], "comprehensive evidence");
}

#[test]
fn build_evidence_bundle_deterministic_bundle_id() {
    use eggsearch::mcp::tools::{run_build_evidence_bundle, EvidenceBundleArgs};

    let make_args = || EvidenceBundleArgs {
        goal: Some("determinism test".to_string()),
        sources: vec![eggsearch::core::evidence_bundle::EvidenceSourceInput {
            id: Some("src_det".to_string()),
            url: Some("https://example.com/det".to_string()),
            title: Some("Det Source".to_string()),
            snippet: Some("snippet".to_string()),
            providers: vec!["mock".to_string()],
            score: Some(1.0),
            trust: Some(eggsearch::core::result::TrustLevel::ExternalUntrusted),
            trust_markers: None,
            metadata: None,
            quality: None,
        }],
        fetches: vec![],
        include_unfetched_sources: None,
        max_sources: None,
        max_fetched_items: None,
        max_total_chars: None,
        response_detail: None,
    };

    let v1 = run_build_evidence_bundle(make_args()).expect("ok");
    let v2 = run_build_evidence_bundle(make_args()).expect("ok");

    let id1 = v1["bundle_id"].as_str().unwrap();
    let id2 = v2["bundle_id"].as_str().unwrap();

    assert_eq!(
        id1, id2,
        "bundle_id must be deterministic for identical inputs"
    );
    assert!(
        id1.starts_with("bundle_"),
        "bundle_id must use bundle_ prefix: {id1}"
    );
}

#[test]
fn build_evidence_bundle_with_fetches_populates_limits() {
    use eggsearch::mcp::tools::{run_build_evidence_bundle, EvidenceBundleArgs};

    let args = EvidenceBundleArgs {
        goal: None,
        sources: vec![],
        fetches: vec![eggsearch::core::evidence_bundle::EvidenceFetchInput {
            source_id: None,
            url: Some("https://example.com".to_string()),
            locator: None,
            fetched: true,
            content_type: None,
            language: None,
            selected_span: None,
            code_span_id: None,
            line_start: None,
            line_end: None,
            text: Some("hello".to_string()),
            truncated: false,
            trust: Some(eggsearch::core::FetchTrust::ExternalUntrusted),
            trust_markers: None,
            warnings: vec![],
        }],
        include_unfetched_sources: None,
        max_sources: None,
        max_fetched_items: None,
        max_total_chars: None,
        response_detail: None,
    };

    let v = run_build_evidence_bundle(args).expect("ok");

    let limits = v["limits"].as_object().expect("limits is object");
    assert!(
        limits.get("max_sources").is_some(),
        "limits must have max_sources"
    );
    assert!(
        limits.get("max_fetched_items").is_some(),
        "limits must have max_fetched_items"
    );
    assert!(
        limits.get("max_total_chars").is_some(),
        "limits must have max_total_chars"
    );
    assert!(
        limits.get("sources_truncated").is_some(),
        "limits must have sources_truncated"
    );
    assert!(
        limits.get("fetched_items_truncated").is_some(),
        "limits must have fetched_items_truncated"
    );
    assert!(
        limits.get("total_chars_exceeded").is_some(),
        "limits must have total_chars_exceeded"
    );
}

#[cfg(feature = "mock")]
#[test]
fn evidence_bundle_identity_is_stable_under_bounded_input_schema() {
    use eggsearch::mcp::tools::{run_build_evidence_bundle, EvidenceBundleArgs};

    let request = serde_json::json!({
        "goal": "identity snapshot",
        "sources": [{
            "id": "src_captured",
            "url": "https://docs.example.test/guide",
            "title": "Guide",
            "snippet": "Captured search card",
            "providers": ["duckduckgo"],
            "trust": "external_untrusted",
            "metadata": {
                "source_kind": "official_docs",
                "domain": "docs.example.test"
            }
        }],
        "fetches": [{
            "source_id": "src_captured",
            "url": "https://raw.example.test/owner/repo/main/src/lib.rs",
            "locator": {
                "kind": "remote",
                "owner": "owner",
                "repo": "repo",
                "ref_name": "main",
                "path": "src/lib.rs"
            },
            "fetched": true,
            "content_type": "text/plain",
            "line_start": 10,
            "line_end": 20,
            "text": "fn captured() {}",
            "trust": "external_untrusted",
            "warnings": []
        }]
    });
    let args: EvidenceBundleArgs =
        serde_json::from_value(request).expect("captured request parses");
    let value = run_build_evidence_bundle(args).expect("bundle builds");
    assert_eq!(value["bundle_id"], "bundle_27becfbe6b86541c");
    assert_eq!(
        value["sources"][0]["source_id"], "src_673bbf9ba38b7620",
        "source identity is content-derived and must not move"
    );
    assert_eq!(value["sources"][0]["source_kind"], "official_docs");
    assert_eq!(
        value["sources"][0]["metadata"]["source_kind"], "official_docs",
        "nested metadata must still reach the builder"
    );
    assert_eq!(
        value["fetched_items"][0]["fetch_id"], "fetch_6609ffa1b98e12f7",
        "fetch identity is content-derived and must not move"
    );
    assert_eq!(value["fetched_items"][0]["locator"]["path"], "src/lib.rs");
    assert_eq!(value["fetched_items"][0]["line_start"], 10);
    assert_eq!(value["fetched_items"][0]["line_end"], 20);
    assert_eq!(value["trust_summary"]["external_untrusted_count"], 1);
}

fn fetch_disabled_state() -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.fetch.enabled = false;
    Arc::new(ServerState::build(cfg).expect("state with fetch disabled"))
}

#[tokio::test]
async fn batch_fetch_results_follow_input_order_when_budget_skips_items() {
    use httpmock::prelude::*;

    // A one-character total budget leaves room for exactly one item, so item 1
    // is skipped during the spawn loop while item 0 lands after the JoinSet
    // drains. Pushing skips first reversed the whole results vector and
    // mis-attributed every telemetry counter.
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/first");
        then.status(200)
            .header("content-type", "text/plain")
            .body("first item body");
    });
    server.mock(|when, then| {
        when.method(GET).path("/second");
        then.status(200)
            .header("content-type", "text/plain")
            .body("second item body");
    });

    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.fetch.retry_max_attempts = 1;
    let state = Arc::new(ServerState::build(cfg).expect("state builds"));

    let items: Vec<eggsearch::core::batch_fetch::BatchFetchItem> = ["/first", "/second"]
        .into_iter()
        .map(|path| eggsearch::core::batch_fetch::BatchFetchItem::Web {
            url: server.url(path),
            extract_mode: Some(ExtractMode::Text),
            include_links: Some(false),
            include_selectors: None,
            exclude_selectors: None,
            attributes: None,
            max_chars: Some(12000),
            cache_policy: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
            snapshot: None,
            pdf: None,
            render: None,
            browser_profile: None,
            include_screenshot: None,
        })
        .collect();

    let probe = run_batch_fetch(
        state.clone(),
        eggsearch::mcp::tools::BatchFetchArgs {
            offset: None,
            limit: None,
            items: items.clone(),
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: Some(50_000),
            continue_on_error: Some(true),
            timeout_ms: None,
            response_detail: None,
        },
    )
    .await
    .expect("budget probe");
    let first_payload_chars = probe["results"][0]["payload_chars"]
        .as_u64()
        .expect("measured payload") as usize;

    let result = run_batch_fetch(
        state.clone(),
        eggsearch::mcp::tools::BatchFetchArgs {
            offset: None,
            limit: None,
            items,
            max_items: None,
            max_chars_per_item: None,
            max_total_chars: Some(first_payload_chars),
            continue_on_error: Some(true),
            timeout_ms: None,
            response_detail: None,
        },
    )
    .await
    .expect("batch_fetch ok");

    let arr = result["results"].as_array().expect("results is array");
    assert_eq!(arr.len(), 2, "one result per input item");
    for (expected_index, entry) in arr.iter().enumerate() {
        assert_eq!(
            entry["index"].as_u64(),
            Some(expected_index as u64),
            "result {expected_index} is out of input order: {arr:?}"
        );
    }
    // The skipped item must still be the last one, and the launched item the
    // first — with labels carrying the URL each result describes.
    assert_eq!(arr[0]["ok"].as_bool(), Some(true), "item 0 must be fetched");
    assert!(
        arr[0]["label"]
            .as_str()
            .is_some_and(|l| l.contains("/first")),
        "result 0 must describe item 0: {arr:?}"
    );
    assert_eq!(
        arr[1]["ok"].as_bool(),
        Some(false),
        "item 1 must be skipped once the budget is spent"
    );
    assert!(
        arr[1]["label"]
            .as_str()
            .is_some_and(|l| l.contains("/second")),
        "result 1 must describe item 1: {arr:?}"
    );
}
