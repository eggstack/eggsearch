use super::batch_fetch::{
    batch_returned_chars, prepare_batch_fetch_client, truncate_batch_result_to_budget,
};
use super::repo_fetch::workspace_relative_path_arg;
use super::*;
use crate::core::config::AppConfig;
use crate::core::fetch::ExtractMode;
use crate::core::sanitize::TrustMarkers;
use crate::fetch::{FetchClient, FetchLimits};
use crate::mcp::state::ServerState;
use std::sync::Arc;

#[cfg(feature = "mock")]
fn mock_web_state() -> Arc<ServerState> {
    use crate::meta::mock::{mock_engines, MockEngine, MockResult};

    let cfg = AppConfig::default();
    let engine = MockEngine::success(
        "duckduckgo",
        vec![MockResult::new(
            "Example result",
            "https://example.com/result",
            "duckduckgo",
        )],
    );
    let adapter = crate::meta::MetadataSearchAdapter::from_engines(
        mock_engines(vec![engine]),
        std::time::Duration::from_millis(cfg.search.timeout_ms),
    );
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

#[test]
fn batch_timeout_client_is_prepared_at_batch_scope() {
    let client = Arc::new(
        FetchClient::new(FetchLimits::default(), "eggsearch/test".to_string(), false)
            .expect("client builds"),
    );
    let shared = prepare_batch_fetch_client(client.clone(), None).expect("shared client");
    assert!(Arc::ptr_eq(&shared, &client));

    let adjusted = prepare_batch_fetch_client(client, Some(12_000)).expect("adjusted client");
    assert!(!Arc::ptr_eq(&adjusted, &shared));
}

#[test]
fn evidence_bundle_limits_reject_values_above_caps() {
    let error = run_build_evidence_bundle(EvidenceBundleArgs {
        goal: None,
        sources: vec![],
        fetches: vec![],
        include_unfetched_sources: None,
        max_sources: Some(crate::core::evidence_bundle::MAX_SOURCES_CAP + 1),
        max_fetched_items: None,
        max_total_chars: None,
        response_detail: None,
    })
    .expect_err("oversized max_sources should be rejected");
    assert!(error.to_string().contains("max_sources"));

    let error = run_build_evidence_bundle(EvidenceBundleArgs {
        goal: None,
        sources: vec![],
        fetches: vec![],
        include_unfetched_sources: None,
        max_sources: None,
        max_fetched_items: None,
        max_total_chars: Some(crate::core::evidence_bundle::MAX_TOTAL_CHARS_CAP + 1),
        response_detail: None,
    })
    .expect_err("oversized max_total_chars should be rejected");
    assert!(error.to_string().contains("max_total_chars"));
}

#[tokio::test]
async fn invalid_unicode_url_scheme_returns_validation_errors() {
    let url = format!("ftp://{}", "é".repeat(12));
    let state = Arc::new(ServerState::build(AppConfig::default()).unwrap());

    let web_error = run_web_fetch(
        state.clone(),
        WebFetchArgs {
            offset: None,
            limit: None,
            url: url.clone(),
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
            include_screenshot: None,
            response_detail: None,
        },
    )
    .await
    .expect_err("invalid URL scheme should fail validation");
    assert!(web_error
        .to_string()
        .contains("url scheme must be http or https"));

    let batch_error = run_batch_fetch(
        state,
        BatchFetchArgs {
            offset: None,
            limit: None,
            items: vec![crate::core::batch_fetch::BatchFetchItem::Web {
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
    .expect_err("invalid URL scheme should fail batch validation");
    assert!(batch_error
        .to_string()
        .contains("url scheme must be http or https"));
}

#[test]
fn repo_batch_budget_counts_readable_metadata() {
    let payload = serde_json::json!({
        "text": "x",
        "raw_url": "https://example.com/a-very-long-path"
    });
    let result = crate::core::batch_fetch::BatchFetchResult {
        index: 0,
        item_type: crate::core::batch_fetch::BatchFetchItemType::Repo,
        label: "repo".to_string(),
        stable_id: None,
        ok: true,
        response: Some(payload.clone()),
        error: None,
        chars_returned: batch_returned_chars(
            &payload,
            crate::core::batch_fetch::BatchFetchItemType::Repo,
        ),
        payload_chars: 0,
        degraded_fields: Vec::new(),
        truncated: false,
    };
    assert!(result.chars_returned > 1);

    let bounded = truncate_batch_result_to_budget(result, 5, 5);
    assert!(bounded.chars_returned <= 5);
    assert!(bounded.truncated);
}

#[test]
fn repo_batch_degradation_ladder_drops_structure_before_text() {
    let payload = serde_json::json!({
        "text": "a".repeat(100),
        "document": {
            "kind": "file",
            "text_chars_returned": 100,
            "metadata": {"path": "src/ladder.rs"},
            "blocks": [{"text": "b".repeat(50_000)}],
            "chunks": [{"text": "c".repeat(50_000)}]
        },
        "lines": [{"number": 1, "text": "l".repeat(50_000)}]
    });
    let full = serde_json::to_string(&payload).unwrap().chars().count();
    let base = crate::core::batch_fetch::BatchFetchResult {
        index: 0,
        item_type: crate::core::batch_fetch::BatchFetchItemType::Repo,
        label: "ladder".to_string(),
        stable_id: None,
        ok: true,
        response: Some(payload),
        error: None,
        chars_returned: 0,
        payload_chars: 0,
        degraded_fields: Vec::new(),
        truncated: false,
    };

    let first = truncate_batch_result_to_budget(base.clone(), full - 1, full);
    assert_eq!(
        first.degraded_fields,
        vec!["document.blocks", "document.chunks"]
    );
    assert!(first.ok);
    assert_eq!(
        first.response.as_ref().unwrap()["text"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        100
    );

    let second = truncate_batch_result_to_budget(base.clone(), first.payload_chars - 1, full);
    assert_eq!(
        second.degraded_fields,
        vec!["document.blocks", "document.chunks", "document"]
    );

    let third = truncate_batch_result_to_budget(base.clone(), second.payload_chars - 1, full);
    assert_eq!(
        third.degraded_fields,
        vec!["document.blocks", "document.chunks", "document", "lines"]
    );
    assert!(!third.truncated);

    let fourth = truncate_batch_result_to_budget(base.clone(), third.payload_chars - 1, full);
    assert_eq!(
        fourth.degraded_fields,
        vec![
            "document.blocks",
            "document.chunks",
            "document",
            "lines",
            "text"
        ]
    );
    assert!(fourth.truncated);
    assert!(fourth.payload_chars < third.payload_chars);

    let omitted = truncate_batch_result_to_budget(base, 1, full);
    assert!(!omitted.ok);
    assert_eq!(omitted.response, Some(serde_json::Value::Null));
    assert_eq!(omitted.chars_returned, 0);
    assert_eq!(omitted.payload_chars, 0);
    assert!(omitted.degraded_fields.is_empty());
    assert!(omitted.truncated);
}

#[test]
fn workspace_fetch_path_is_trimmed_before_use() {
    let args = RepoFetchArgs {
        host: Some("workspace".to_string()),
        target: crate::core::repo_fetch::RepoTarget {
            owner: "root".to_string(),
            repo: " legacy.rs ".to_string(),
            ref_name: None,
            commit_sha: None,
            path: " file.rs ".to_string(),
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
        timeout_ms: None,
        test_fetch_url: None,
        response_detail: None,
    };

    assert_eq!(workspace_relative_path_arg(&args).unwrap(), "file.rs");
}

#[test]
fn repo_target_envelopes_preserve_the_flat_field_set() {
    use crate::core::batch_fetch::BatchFetchItem;
    use crate::core::code_metadata::CodeHost;
    use crate::core::repo_fetch::{RepoFetchRequest, RepoTarget};
    use crate::mcp::tools::fetch::FetchItem;
    use crate::mcp::tools::repo_fetch::RepoFetchArgs;
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    fn assert_lossless<T>(value: serde_json::Value, target_keys: &[String], extras: &[&str])
    where
        T: DeserializeOwned + Serialize,
    {
        let mut expected = target_keys.to_vec();
        expected.extend(extras.iter().map(|key| (*key).to_string()));
        expected.sort();
        let mut actual = value
            .as_object()
            .expect("envelope object")
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        actual.sort();
        assert_eq!(actual, expected);
        let parsed: T = serde_json::from_value(value.clone()).expect("envelope deserializes");
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    }

    let target = RepoTarget {
        owner: "tokio-rs".to_string(),
        repo: "axum".to_string(),
        ref_name: Some("main".to_string()),
        commit_sha: Some("0123456789abcdef".to_string()),
        path: "src/lib.rs".to_string(),
        line_start: Some(2),
        line_end: Some(20),
        context_before: Some(1),
        context_after: Some(2),
        max_chars: Some(2400),
        symbol: Some("Router".to_string()),
        symbol_kind: Some("struct".to_string()),
        match_text: Some("pub struct Router".to_string()),
        expand_to_block: Some(true),
        max_block_lines: Some(80),
        prefer_local: Some(false),
    };
    let target_keys = serde_json::to_value(&target)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let args = RepoFetchArgs {
        host: Some("gh".to_string()),
        target: target.clone(),
        timeout_ms: Some(5000),
        test_fetch_url: Some("https://example.test/src/lib.rs".to_string()),
        response_detail: Some(crate::mcp::projection::ResponseDetail::Diagnostic),
    };
    assert_lossless::<RepoFetchArgs>(
        serde_json::to_value(&args).unwrap(),
        &target_keys,
        &["host", "timeout_ms", "test_fetch_url", "response_detail"],
    );

    let batch = BatchFetchItem::Repo {
        host: Some("gh".to_string()),
        target: target.clone(),
        focus: Some("Router".to_string()),
        focus_max_chunks: Some(3),
        focus_max_chars: Some(1200),
    };
    assert_lossless::<BatchFetchItem>(
        serde_json::to_value(&batch).unwrap(),
        &target_keys,
        &[
            "type",
            "host",
            "focus",
            "focus_max_chunks",
            "focus_max_chars",
        ],
    );

    let request = RepoFetchRequest {
        host: Some(CodeHost::Github),
        target: target.clone(),
        timeout_ms: Some(7000),
    };
    assert_lossless::<RepoFetchRequest>(
        serde_json::to_value(&request).unwrap(),
        &target_keys,
        &["host", "timeout_ms"],
    );

    let fetch_item = FetchItem::Repo {
        args,
        focus: Some("Router".to_string()),
        focus_max_chunks: Some(3),
        focus_max_chars: Some(1200),
    };
    assert_lossless::<FetchItem>(
        serde_json::to_value(&fetch_item).unwrap(),
        &target_keys,
        &[
            "type",
            "host",
            "timeout_ms",
            "test_fetch_url",
            "response_detail",
            "focus",
            "focus_max_chunks",
            "focus_max_chars",
        ],
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn safe_search_warning_emitted_when_requested() {
    let state = mock_web_state();

    let args = WebSearchArgs {
        query: "test query".to_string(),
        max_results: Some(5),
        providers: vec![],
        safe_search: Some(crate::core::SafeSearch::Strict),
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
    };

    let result = run_web_search(state, args).await;
    assert!(result.is_ok());
    let value = result.unwrap();
    let warnings = value.get("warnings").unwrap().as_array().unwrap();
    assert!(warnings
        .iter()
        .any(|w| w.as_str().unwrap().contains("safe_search_unenforced")));
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_payload_includes_top_level_trust_markers() {
    let state = mock_web_state();

    let args = WebSearchArgs {
        query: "test".to_string(),
        max_results: Some(3),
        providers: vec![],
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
    };

    let result = run_web_search(state, args).await;
    assert!(result.is_ok());
    let value = result.unwrap();
    // The payload must include a top-level `trust_markers` object.
    let markers = value
        .get("trust_markers")
        .expect("trust_markers should be on payload");
    // It must deserialize back to TrustMarkers (or at least
    // expose the documented boolean/numeric fields).
    assert!(markers.get("text_sanitized").is_some());
    assert!(markers.get("text_truncated").is_some());
    assert!(markers.get("text_framed").is_some());
    assert!(markers.get("control_chars_removed").is_some());
    assert!(markers.get("injection_hits").is_some());
}

#[test]
fn trust_markers_payload_shape_matches_struct() {
    // Sanity: the JSON we emit for `trust_markers` is the same
    // shape as the TrustMarkers struct, so a host agent can
    // deserialize it.
    let m = TrustMarkers {
        text_sanitized: true,
        text_truncated: false,
        text_framed: true,
        control_chars_removed: 3,
        injection_hits: 2,
    };
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["text_sanitized"], serde_json::json!(true));
    assert_eq!(v["text_truncated"], serde_json::json!(false));
    assert_eq!(v["text_framed"], serde_json::json!(true));
    assert_eq!(v["control_chars_removed"], serde_json::json!(3));
    assert_eq!(v["injection_hits"], serde_json::json!(2));
}

#[tokio::test]
async fn repo_search_host_github_accepted() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

    let args = RepoSearchArgs {
        query: "repo:tokio-rs/axum".to_string(),
        host: Some("github".to_string()),
        ..Default::default()
    };

    let result = run_repo_search(state, args).await;
    // Should not fail with a validation error about the host.
    // It may fail for other reasons (e.g. no providers), but
    // the host itself is valid.
    match &result {
        Err(ToolError::Validation(msg)) if msg.contains("unknown host") => {
            panic!("github host should be accepted, got: {msg}");
        }
        _ => {}
    }
}

#[tokio::test]
async fn repo_search_host_gh_alias_accepted() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

    let args = RepoSearchArgs {
        query: "repo:tokio-rs/axum".to_string(),
        host: Some("gh".to_string()),
        ..Default::default()
    };

    let result = run_repo_search(state, args).await;
    match &result {
        Err(ToolError::Validation(msg)) if msg.contains("unknown host") => {
            panic!("gh alias should be accepted, got: {msg}");
        }
        _ => {}
    }
}

#[tokio::test]
async fn repo_search_host_unknown_rejected() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

    let args = RepoSearchArgs {
        query: "some query".to_string(),
        host: Some("unknownhost".to_string()),
        ..Default::default()
    };

    let result = run_repo_search(state, args).await;
    match result {
        Err(ToolError::Validation(msg)) => {
            assert!(
                msg.contains("unknown host 'unknownhost'"),
                "unexpected validation message: {msg}"
            );
        }
        other => panic!("expected validation error, got: {other:?}"),
    }
}

#[tokio::test]
async fn repo_search_host_none_accepted() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

    let args = RepoSearchArgs {
        query: "some query".to_string(),
        host: None,
        ..Default::default()
    };

    let result = run_repo_search(state, args).await;
    // host=None should not produce a validation error about host.
    match &result {
        Err(ToolError::Validation(msg)) if msg.contains("unknown host") => {
            panic!("None host should be accepted, got: {msg}");
        }
        _ => {}
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_structured_warnings_safe_search_unenforced() {
    let state = mock_web_state();

    let args = WebSearchArgs {
        query: "test".to_string(),
        max_results: Some(3),
        providers: vec![],
        safe_search: Some(crate::core::SafeSearch::Strict),
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
    };

    let value = run_web_search(state, args).await.unwrap();
    let sw = value
        .get("structured_warnings")
        .expect("structured_warnings should be present");
    let arr = sw.as_array().expect("structured_warnings should be array");
    assert!(
        arr.iter().any(|w| w["code"] == "safe_search_unenforced"),
        "should contain safe_search_unenforced code: {arr:?}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_structured_warnings_present_alongside_legacy() {
    let state = mock_web_state();

    let args = WebSearchArgs {
        query: "test".to_string(),
        max_results: Some(3),
        providers: vec![],
        safe_search: Some(crate::core::SafeSearch::Strict),
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
    };

    let value = run_web_search(state, args).await.unwrap();
    // Both legacy and structured warnings must be present.
    assert!(
        value.get("warnings").is_some(),
        "legacy warnings must be present"
    );
    assert!(
        value.get("structured_warnings").is_some(),
        "structured_warnings must be present"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn web_search_structured_warnings_empty_for_clean_search() {
    let state = mock_web_state();

    let args = WebSearchArgs {
        query: "test".to_string(),
        max_results: Some(3),
        providers: vec![],
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
    };

    let value = run_web_search(state, args).await.unwrap();
    let sw = value
        .get("structured_warnings")
        .expect("structured_warnings field");
    let arr = sw.as_array().unwrap();
    // Clean search should not have capability-enforcement warnings.
    // The generic_context_untrusted advisory is always present.
    assert!(
        !arr.iter().any(|w| w["code"] == "safe_search_unenforced"),
        "clean search should not have safe_search_unenforced: {arr:?}"
    );
    assert!(
        !arr.iter().any(|w| w["code"] == "freshness_unenforced"),
        "clean search should not have freshness_unenforced: {arr:?}"
    );
}

#[tokio::test]
async fn web_fetch_structured_warnings_present() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(GET).path("/get");
        then.status(200)
            .header("content-type", "text/plain")
            .body("mock fetch body");
    });

    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    let state = Arc::new(ServerState::build(cfg).unwrap());
    let args = WebFetchArgs {
        offset: None,
        limit: None,
        url: server.url("/get"),
        max_chars: Some(1000),
        timeout_ms: Some(1000),
        extract_mode: Some(ExtractMode::Text),
        include_links: Some(false),
        pdf: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
        render: None,
        browser_profile: None,
        include_screenshot: None,
        response_detail: None,
    };
    let value = run_web_fetch(state, args).await.unwrap();
    // structured_warnings must always be in the payload (even if empty).
    assert!(
        value.get("structured_warnings").is_some(),
        "web_fetch response must always include structured_warnings"
    );
}

#[tokio::test]
async fn web_fetch_rejects_zero_timeout() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());
    let args = WebFetchArgs {
        offset: None,
        limit: None,
        url: "https://example.com/".to_string(),
        max_chars: Some(1000),
        timeout_ms: Some(0),
        extract_mode: Some(ExtractMode::Text),
        include_links: Some(false),
        pdf: None,
        cache_policy: None,
        max_cache_age_seconds: None,
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
        render: None,
        browser_profile: None,
        include_screenshot: None,
        response_detail: None,
    };

    let err = run_web_fetch(state, args)
        .await
        .expect_err("zero timeout should fail validation");
    assert!(
        err.to_string().contains("timeout_ms must be > 0"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn repo_map_structured_warnings_present() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-org".to_string(),
        repo: "test-repo".to_string(),
        ref_name: None,
        commit_sha: None,
        max_entries: None,
        max_depth: None,
        include_files: None,
        include_directories: None,
        include_ci: None,
        include_security: None,
        timeout_ms: None,
        providers: vec![],
        response_detail: None,
    };
    let value = run_repo_map(state, args).await.unwrap();
    // structured_warnings must always be in the payload (even if empty).
    assert!(
        value.get("structured_warnings").is_some(),
        "repo_map response must always include structured_warnings"
    );
    // The fallback response should include a no_native_tree_provider warning.
    let structured = value
        .get("structured_warnings")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        !structured.is_empty(),
        "repo_map structured_warnings should not be empty (fallback emits warnings)"
    );
    // Verify legacy warnings are also present alongside.
    assert!(
        value.get("warnings").is_some(),
        "repo_map response must also include legacy warnings"
    );
}

#[test]
fn agent_workflows_repo_search_example_deserializes() {
    let json = r#"{
        "query": "Router::layer middleware",
        "host": "github",
        "owner": "tokio-rs",
        "repo": "axum",
        "profile": "coding"
    }"#;
    let args: RepoSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.query, "Router::layer middleware");
    assert_eq!(args.host.as_deref(), Some("github"));
    assert_eq!(args.owner.as_deref(), Some("tokio-rs"));
    assert_eq!(args.repo.as_deref(), Some("axum"));
    assert_eq!(args.profile.as_deref(), Some("coding"));
}

#[test]
fn agent_workflows_repo_search_exact_error_deserializes() {
    let json = r#"{
        "query": "error[E0308]: mismatched types - expected `String`, found `i32`",
        "host": "github",
        "owner": "tokio-rs",
        "repo": "axum",
        "mode": "exact_error",
        "profile": "coding"
    }"#;
    let args: RepoSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.mode.as_deref(), Some("exact_error"));
}

#[test]
fn agent_workflows_research_search_example_deserializes() {
    let json = r#"{
        "query": "axum vs actix-web for high-performance REST API",
        "research_domain": "software_architecture",
        "workflow": "library_comparison",
        "depth": "standard",
        "compare_targets": ["axum", "actix-web"],
        "include_counterpoints": true,
        "include_primary_sources": true,
        "desired_source_types": ["benchmarks"]
    }"#;
    let args: ResearchSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(
        args.query,
        "axum vs actix-web for high-performance REST API"
    );
    assert_eq!(
        args.research_domain.as_deref(),
        Some("software_architecture")
    );
    assert_eq!(args.workflow.as_deref(), Some("library_comparison"));
    assert_eq!(args.depth.as_deref(), Some("standard"));
    assert_eq!(args.compare_targets, vec!["axum", "actix-web"]);
    assert_eq!(args.include_counterpoints, Some(true));
    assert_eq!(args.include_primary_sources, Some(true));
    assert_eq!(args.desired_source_types, vec!["benchmarks"]);
}

#[test]
fn agent_workflows_security_search_example_deserializes() {
    let json = r#"{
        "query": "axum",
        "ecosystem": "crates.io",
        "package": "axum",
        "version": "0.7.0",
        "include_kev": true,
        "include_defensive_guidance": true,
        "assess_applicability": true,
        "dependency_files": ["Cargo.lock"]
    }"#;
    let args: SecuritySearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.query.as_deref(), Some("axum"));
    assert_eq!(args.ecosystem.as_deref(), Some("crates.io"));
    assert_eq!(args.package.as_deref(), Some("axum"));
    assert_eq!(args.version.as_deref(), Some("0.7.0"));
    assert_eq!(args.include_kev, Some(true));
    assert_eq!(args.include_defensive_guidance, Some(true));
    assert_eq!(args.assess_applicability, Some(true));
    assert_eq!(args.dependency_files, vec!["Cargo.lock"]);
}

#[test]
fn repo_search_slash_form_deserializes() {
    let json = r#"{"query": "repo:tokio-rs/axum", "repo": "tokio-rs/axum"}"#;
    let args: RepoSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.repo.as_deref(), Some("tokio-rs/axum"));
}

#[test]
fn research_search_full_options_deserializes() {
    let json = r#"{
        "query": "compare QUIC vs WebSocket IPC for a coding agent daemon",
        "research_domain": "software_architecture",
        "desired_source_types": ["specifications", "official_docs", "reference_implementations", "benchmarks", "security_considerations"],
        "include_counterpoints": true,
        "freshness": "year",
        "max_results": 32,
        "max_groups": 10,
        "max_per_group": 5
    }"#;
    let args: ResearchSearchArgs = serde_json::from_str(json).unwrap();
    assert_eq!(
        args.query,
        "compare QUIC vs WebSocket IPC for a coding agent daemon"
    );
    assert_eq!(
        args.research_domain.as_deref(),
        Some("software_architecture")
    );
    assert_eq!(
        args.desired_source_types,
        vec![
            "specifications",
            "official_docs",
            "reference_implementations",
            "benchmarks",
            "security_considerations"
        ]
    );
    assert_eq!(args.max_results, Some(32));
    assert_eq!(args.max_groups, Some(10));
    assert_eq!(args.max_per_group, Some(5));
}

#[tokio::test]
async fn batch_workspace_fetch_honors_host_local_scope_without_widening() {
    struct TokenRestore(Option<std::ffi::OsString>);
    impl Drop for TokenRestore {
        fn drop(&mut self) {
            match self.0.take() {
                Some(value) => std::env::set_var("CODEGG_EGGSEARCH_HOST_SCOPE_TOKEN", value),
                None => std::env::remove_var("CODEGG_EGGSEARCH_HOST_SCOPE_TOKEN"),
            }
        }
    }

    let _lock = crate::mcp::state::HOST_LOCAL_SCOPE_TEST_LOCK.lock().await;
    let previous = std::env::var_os("CODEGG_EGGSEARCH_HOST_SCOPE_TOKEN");
    let _restore = TokenRestore(previous);
    let token = "batch-scope-test-token";
    std::env::set_var("CODEGG_EGGSEARCH_HOST_SCOPE_TOKEN", token);

    let dir = tempfile::tempdir().expect("tempdir");
    let authorized_root = dir.path().join("authorized");
    let configured_root = dir.path().join("configured");
    std::fs::create_dir_all(&authorized_root).expect("authorized root");
    std::fs::create_dir_all(&configured_root).expect("configured root");
    std::fs::write(authorized_root.join("lib.rs"), "pub fn authorized() {}\n")
        .expect("authorized source");
    std::fs::write(configured_root.join("lib.rs"), "pub fn configured() {}\n")
        .expect("configured source");
    let authorized_name = authorized_root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("authorized root name")
        .to_string();
    let configured_name = configured_root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("configured root name")
        .to_string();
    let mut config = AppConfig::default();
    config.local.enabled = true;
    config.local.roots = vec![configured_root];
    let state = Arc::new(ServerState::build(config).expect("state builds"));
    let scope = serde_json::json!({
        "root": authorized_root.to_string_lossy(),
        "token": token
    });
    let args_for_root = |root: String| BatchFetchArgs {
        items: vec![crate::core::batch_fetch::BatchFetchItem::Repo {
            host: Some("workspace".to_string()),
            target: crate::core::repo_fetch::RepoTarget {
                owner: root,
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
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
        }],
        max_items: None,
        max_chars_per_item: None,
        max_total_chars: None,
        timeout_ms: None,
        continue_on_error: None,
        offset: None,
        limit: None,
        response_detail: None,
    };

    let mut progress = crate::mcp::progress::ProgressSink::disabled();
    let authorized = run_batch_fetch_with_host_scope_and_progress(
        state.clone(),
        args_for_root(authorized_name),
        Some(scope.clone()),
        &mut progress,
    )
    .await
    .expect("authorized batch workspace fetch");
    assert!(authorized.value["results"][0]["response"]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("authorized"));

    let mut progress = crate::mcp::progress::ProgressSink::disabled();
    let rejected = run_batch_fetch_with_host_scope_and_progress(
        state.clone(),
        args_for_root(configured_name.clone()),
        Some(scope),
        &mut progress,
    )
    .await
    .expect_err("out-of-scope root must be rejected");
    assert!(rejected.to_string().contains("unknown workspace root"));

    let configured = run_batch_fetch(state, args_for_root(configured_name))
        .await
        .expect("internal unscoped calls retain configured roots");
    assert!(configured["results"][0]["response"]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("configured"));
}

#[cfg(all(feature = "mock", feature = "browser"))]
fn synthetic_png(seed: u8) -> Vec<u8> {
    let mut png = vec![seed; 40];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&1280_u32.to_be_bytes());
    png[20..24].copy_from_slice(&720_u32.to_be_bytes());
    png
}

#[cfg(all(feature = "mock", feature = "browser"))]
fn synthetic_browser_outcome(
    url: &str,
    screenshot: Option<Vec<u8>>,
) -> crate::fetch::browser::BrowserFetchOutcome {
    use crate::fetch::browser::{
        BrowserFetchOutcome, BrowserFetchResult, FetchDisposition, FetchTransportKind,
        TransportResponse, TransportTiming,
    };

    BrowserFetchOutcome {
        result: BrowserFetchResult {
            response: TransportResponse {
                transport: FetchTransportKind::Browser,
                requested_url: url.to_string(),
                final_url: url.to_string(),
                status: Some(200),
                headers: Vec::new(),
                body: b"<html><body>synthetic browser content</body></html>".to_vec(),
                content_type: Some("text/html".to_string()),
                redirects: Vec::new(),
                timing: TransportTiming::default(),
                classification: Some(FetchDisposition::UsefulContent),
            },
            extracted_title: Some("Synthetic page".to_string()),
            extracted_text: "synthetic browser content".to_string(),
            trust_markers: TrustMarkers::default(),
            warnings: Vec::new(),
            challenge_completed: false,
            challenge: None,
        },
        screenshot: screenshot.map(|png| {
            crate::fetch::browser::ScreenshotCapture::try_new(png).expect("synthetic PNG")
        }),
    }
}

#[cfg(all(feature = "mock", feature = "browser"))]
fn synthetic_browser_state(
    browser_fetch_override: crate::mcp::state::BrowserFetchOverride,
) -> Arc<ServerState> {
    use crate::fetch::browser::{
        BrowserConfig, BrowserDiscovery, BrowserDiscoveryState, BrowserFamily, BrowserLifecycle,
        BrowserSource, RenderPolicy,
    };

    let mut config = AppConfig::default();
    config.fetch.allow_localhost = true;
    config.fetch.allow_private_network = true;
    config.fetch.browser.enabled = true;
    let mut state = Arc::new(ServerState::build(config).expect("state builds"));
    let discovery = BrowserDiscovery {
        path: std::path::PathBuf::from("/synthetic/chrome"),
        family: BrowserFamily::Chrome,
        version: "Chrome 120.0.0.0".to_string(),
        source: BrowserSource::AutoDiscovered,
    };
    let browser_config = BrowserConfig {
        enabled: true,
        policy: RenderPolicy::Browser,
        ..BrowserConfig::default()
    };
    let state_mut = Arc::get_mut(&mut state).expect("unique state");
    state_mut.browser_discovery_state = BrowserDiscoveryState::Available(discovery.clone());
    state_mut.browser_lifecycle = Some(Arc::new(BrowserLifecycle::new(
        Some(discovery),
        browser_config,
    )));
    state_mut.browser_fetch_override = Some(browser_fetch_override);
    state
}

#[cfg(all(feature = "mock", feature = "browser"))]
fn screenshot_web_args(url: &str, include_screenshot: Option<bool>) -> WebFetchArgs {
    WebFetchArgs {
        offset: None,
        limit: None,
        url: url.to_string(),
        max_chars: Some(1000),
        timeout_ms: Some(5000),
        extract_mode: Some(ExtractMode::Text),
        include_links: Some(false),
        pdf: None,
        cache_policy: Some(crate::core::fetch::FetchCachePolicy::Default),
        render: None,
        browser_profile: None,
        max_cache_age_seconds: None,
        focus: None,
        focus_max_chunks: None,
        focus_max_chars: None,
        response_detail: None,
        include_screenshot,
    }
}

#[cfg(all(feature = "mock", feature = "browser"))]
#[tokio::test]
async fn screenshot_web_fetch_uses_browser_and_keeps_image_out_of_json_and_cache() {
    use crate::mcp::state::MockBrowserRequest;
    use base64::Engine;

    let png = synthetic_png(37);
    let requests = Arc::new(std::sync::Mutex::new(Vec::<MockBrowserRequest>::new()));
    let seen = requests.clone();
    let capture_png = png.clone();
    let state = synthetic_browser_state(Arc::new(move |request| {
        seen.lock().expect("request lock").push(request.clone());
        Ok(synthetic_browser_outcome(
            &request.url,
            request.capture.then(|| capture_png.clone()),
        ))
    }));
    let mut progress = crate::mcp::progress::ProgressSink::disabled();
    let outcome = run_web_fetch_outcome_with_progress(
        state.clone(),
        screenshot_web_args("https://example.test/static", Some(true)),
        &mut progress,
    )
    .await
    .expect("screenshot fetch");

    assert_eq!(outcome.images.len(), 1);
    assert_eq!(outcome.images[0].png, png);
    {
        let requests = requests.lock().expect("request lock");
        assert_eq!(requests.len(), 1);
        assert!(requests[0].capture);
        assert_eq!(requests[0].url, "https://example.test/static");
    }

    let json = serde_json::to_string(&outcome.value).expect("structured JSON");
    let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
    assert!(!json.contains(&encoded));
    let cache = state
        .fetch_cache
        .as_ref()
        .expect("fetch cache")
        .stats()
        .await;
    assert_eq!(cache.raw_entries, 0);
    assert_eq!(cache.derived_entries, 0);

    let mapped = map_tool_outcome(Ok(outcome)).expect("MCP image result");
    let content = serde_json::to_value(&mapped.content).expect("content serialization");
    assert_eq!(content.as_array().map(Vec::len), Some(2));
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["mimeType"], "image/png");
    assert_eq!(content[1]["data"], encoded);
    let structured = serde_json::to_string(
        mapped
            .structured_content
            .as_ref()
            .expect("structured content"),
    )
    .expect("structured payload serialization");
    assert!(!structured.contains(&encoded));
}

#[cfg(all(feature = "mock", feature = "browser"))]
#[tokio::test]
async fn screenshot_conflict_fails_before_browser_io() {
    use crate::mcp::state::MockBrowserRequest;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let state = synthetic_browser_state(Arc::new(move |_request: MockBrowserRequest| {
        seen.fetch_add(1, Ordering::SeqCst);
        Err(crate::fetch::browser::BrowserFetchError::CaptureFailed(
            "unexpected capture".to_string(),
        ))
    }));
    let mut args = screenshot_web_args("https://example.test/conflict", Some(true));
    args.render = Some("http_only".to_string());
    let error = run_web_fetch(state, args)
        .await
        .expect_err("conflicting route options");
    assert_eq!(error.code().as_str(), "conflicting_arguments");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[cfg(all(feature = "mock", feature = "browser"))]
#[tokio::test]
async fn screenshot_capture_failures_do_not_return_text_only_success() {
    use crate::mcp::state::MockBrowserRequest;
    use std::sync::atomic::{AtomicBool, Ordering};

    let captured = Arc::new(AtomicBool::new(false));
    let seen = captured.clone();
    let state = synthetic_browser_state(Arc::new(move |request: MockBrowserRequest| {
        seen.store(request.capture, Ordering::SeqCst);
        Err(crate::fetch::browser::BrowserFetchError::CaptureFailed(
            "synthetic cap rejection".to_string(),
        ))
    }));
    let error = run_web_fetch(
        state,
        screenshot_web_args("https://example.test/capture-failure", Some(true)),
    )
    .await
    .expect_err("capture failure must fail the fetch");
    assert!(captured.load(Ordering::SeqCst));
    assert!(error.to_string().contains("synthetic cap rejection"));
}

#[cfg(all(feature = "mock", feature = "browser"))]
#[tokio::test]
async fn screenshot_batch_associations_follow_successful_input_order() {
    use crate::mcp::state::MockBrowserRequest;
    use base64::Engine;

    let first_png = synthetic_png(11);
    let last_png = synthetic_png(29);
    let first_capture = first_png.clone();
    let last_capture = last_png.clone();
    let state = synthetic_browser_state(Arc::new(move |request: MockBrowserRequest| {
        if request.url.ends_with("/failed") {
            return Err(crate::fetch::browser::BrowserFetchError::CaptureFailed(
                "synthetic failed item".to_string(),
            ));
        }
        let screenshot = if request.url.ends_with("/first") {
            Some(first_capture.clone())
        } else if request.url.ends_with("/last") {
            Some(last_capture.clone())
        } else {
            None
        };
        Ok(synthetic_browser_outcome(&request.url, screenshot))
    }));
    let web_item = |url: &str, render: Option<&str>, include_screenshot| {
        crate::core::batch_fetch::BatchFetchItem::Web {
            url: url.to_string(),
            extract_mode: Some(ExtractMode::Text),
            include_links: Some(false),
            max_chars: Some(1000),
            cache_policy: None,
            max_cache_age_seconds: None,
            focus: None,
            focus_max_chunks: None,
            focus_max_chars: None,
            pdf: None,
            render: render.map(str::to_string),
            browser_profile: None,
            include_screenshot,
        }
    };
    let args = BatchFetchArgs {
        items: vec![
            web_item("https://example.test/first", None, Some(true)),
            web_item("https://example.test/plain", Some("browser"), None),
            web_item("https://example.test/failed", None, Some(true)),
            web_item("https://example.test/last", None, Some(true)),
        ],
        max_items: Some(4),
        max_chars_per_item: Some(1000),
        max_total_chars: Some(8000),
        timeout_ms: Some(5000),
        continue_on_error: Some(true),
        offset: None,
        limit: None,
        response_detail: None,
    };
    let mut progress = crate::mcp::progress::ProgressSink::disabled();
    let outcome = run_batch_fetch_with_host_scope_and_progress(state, args, None, &mut progress)
        .await
        .expect("batch fetch");

    assert_eq!(outcome.value["results"][0]["ok"], true);
    assert_eq!(outcome.value["results"][1]["ok"], true);
    assert_eq!(outcome.value["results"][2]["ok"], false);
    assert_eq!(outcome.value["results"][3]["ok"], true);
    assert_eq!(outcome.value["telemetry"]["items_failed"], 1);
    assert_eq!(outcome.images.len(), 2);
    assert_eq!(
        outcome.images[0].association.as_deref(),
        Some("batch_fetch item 0: https://example.test/first")
    );
    assert_eq!(outcome.images[0].png, first_png);
    assert_eq!(
        outcome.images[1].association.as_deref(),
        Some("batch_fetch item 3: https://example.test/last")
    );
    assert_eq!(outcome.images[1].png, last_png);

    let first_encoded = base64::engine::general_purpose::STANDARD.encode(first_png);
    let last_encoded = base64::engine::general_purpose::STANDARD.encode(last_png);
    let mapped = map_tool_outcome(Ok(outcome)).expect("MCP batch image result");
    let content = serde_json::to_value(&mapped.content).expect("content serialization");
    assert_eq!(
        content[1]["text"],
        "batch_fetch item 0: https://example.test/first"
    );
    assert_eq!(content[2]["type"], "image");
    assert_eq!(content[2]["data"], first_encoded);
    assert_eq!(
        content[3]["text"],
        "batch_fetch item 3: https://example.test/last"
    );
    assert_eq!(content[4]["type"], "image");
    assert_eq!(content[4]["data"], last_encoded);
    assert!(!content[0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains(&first_encoded));
    assert!(!content[0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains(&last_encoded));
}

#[test]
fn screenshot_request_fields_are_omitted_when_absent_or_false() {
    let absent: WebFetchArgs =
        serde_json::from_value(serde_json::json!({"url": "https://example.test"})).unwrap();
    let false_value: WebFetchArgs = serde_json::from_value(serde_json::json!({
        "url": "https://example.test",
        "include_screenshot": false
    }))
    .unwrap();
    assert_eq!(
        serde_json::to_value(absent).unwrap(),
        serde_json::to_value(false_value).unwrap()
    );

    let absent_batch: crate::core::batch_fetch::BatchFetchItem =
        serde_json::from_value(serde_json::json!({
            "type": "web",
            "url": "https://example.test"
        }))
        .unwrap();
    let false_batch: crate::core::batch_fetch::BatchFetchItem =
        serde_json::from_value(serde_json::json!({
            "type": "web",
            "url": "https://example.test",
            "include_screenshot": false
        }))
        .unwrap();
    assert_eq!(
        serde_json::to_value(absent_batch).unwrap(),
        serde_json::to_value(false_batch).unwrap()
    );
}

#[cfg(feature = "browser")]
#[tokio::test]
async fn screenshot_capability_errors_remain_explicit() {
    let mut disabled_config = AppConfig::default();
    disabled_config.fetch.browser.enabled = false;
    let state = Arc::new(ServerState::build(disabled_config).expect("state builds"));
    let disabled = run_web_fetch(
        state,
        WebFetchArgs {
            include_screenshot: Some(true),
            ..serde_json::from_value(serde_json::json!({
                "url": "https://example.com/disabled"
            }))
            .unwrap()
        },
    )
    .await
    .expect_err("disabled browser must reject screenshots");
    assert_eq!(
        disabled.error_payload()["data"]["code"],
        "browser_disabled",
        "{disabled:?}"
    );

    let mut config = AppConfig::default();
    config.fetch.browser.enabled = true;
    config.fetch.browser.executable = Some("/synthetic/missing/chrome".to_string());
    let unavailable = Arc::new(ServerState::build(config).expect("state builds"));
    let error = run_web_fetch(
        unavailable,
        WebFetchArgs {
            include_screenshot: Some(true),
            ..serde_json::from_value(serde_json::json!({
                "url": "https://example.com/unavailable"
            }))
            .unwrap()
        },
    )
    .await
    .expect_err("unavailable browser must reject screenshots");
    assert_eq!(error.error_payload()["data"]["code"], "browser_unavailable");
}

#[cfg(not(feature = "browser"))]
#[tokio::test]
async fn screenshot_capability_error_when_browser_feature_is_absent() {
    let state = Arc::new(ServerState::build(AppConfig::default()).expect("state builds"));
    let error = run_web_fetch(
        state,
        WebFetchArgs {
            include_screenshot: Some(true),
            ..serde_json::from_value(serde_json::json!({
                "url": "https://example.test/no-browser-feature"
            }))
            .unwrap()
        },
    )
    .await
    .expect_err("missing browser feature must reject screenshots");
    assert_eq!(
        error.error_payload()["data"]["code"],
        "browser_not_compiled_in"
    );
}
