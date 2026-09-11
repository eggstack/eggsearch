use super::batch_fetch::{batch_payload_chars, truncate_batch_result_to_budget};
use super::repo_fetch::workspace_relative_path_arg;
use super::*;
use crate::core::config::AppConfig;
use crate::core::fetch::ExtractMode;
use crate::core::sanitize::TrustMarkers;
use crate::mcp::state::ServerState;
use std::sync::Arc;

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
fn repo_batch_budget_counts_serialized_metadata() {
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
        chars_returned: batch_payload_chars(&payload),
        truncated: false,
    };
    assert!(result.chars_returned > 1);

    let bounded = truncate_batch_result_to_budget(result, 5, 5);
    assert!(bounded.chars_returned <= 5);
    assert!(bounded.truncated);
}

#[test]
fn workspace_fetch_path_is_trimmed_before_use() {
    let args = RepoFetchArgs {
        host: Some("workspace".to_string()),
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
        timeout_ms: None,
        test_fetch_url: None,
        symbol: None,
        symbol_kind: None,
        match_text: None,
        expand_to_block: None,
        max_block_lines: None,
        prefer_local: None,
        response_detail: None,
    };

    assert_eq!(workspace_relative_path_arg(&args).unwrap(), "file.rs");
}

#[tokio::test]
async fn safe_search_warning_emitted_when_requested() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

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

#[tokio::test]
async fn web_search_payload_includes_top_level_trust_markers() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

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

#[tokio::test]
async fn web_search_structured_warnings_safe_search_unenforced() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

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

#[tokio::test]
async fn web_search_structured_warnings_present_alongside_legacy() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

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

#[tokio::test]
async fn web_search_structured_warnings_empty_for_clean_search() {
    let cfg = AppConfig::default();
    let state = Arc::new(ServerState::build(cfg).unwrap());

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
