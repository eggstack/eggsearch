#![allow(unused_imports, dead_code)]
//! Security advisory retrieval and context safety.
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
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn security_search_includes_next_actions() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "CVE-2024-0001 advisory",
            "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
            "mock_a",
        )],
    )];
    let mut cfg = test_cfg();
    cfg.search.providers.insert("mock_a".to_string(), true);
    let state = state_with_engines(cfg, engines, Duration::from_secs(5));

    let v = run_security_search(
        state,
        SecuritySearchArgs {
            query: Some("CVE-2024-0001".to_string()),
            ..Default::default()
        },
    )
    .await
    .expect("ok");

    let next_actions = v["next_actions"].as_array().expect("next_actions is array");
    assert!(
        !next_actions.is_empty(),
        "security_search should return next_actions with results"
    );
    for action in next_actions {
        assert!(action["tool"].is_string(), "next_action missing tool");
        assert!(
            action["reason_code"].is_string(),
            "next_action missing reason_code"
        );
    }
}

#[cfg(feature = "mock")]
mod security_context_safety {
    use super::*;
    use eggsearch::mcp::tools::ToolError;

    #[cfg(feature = "mock")]
    fn sec_state_with_engines(
        cfg: AppConfig,
        engines: Vec<MockEngine>,
        timeout: Duration,
    ) -> Arc<ServerState> {
        let adapter = MetadataSearchAdapter::from_engines(mock_engines(engines), timeout);
        Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
    }

    /// Verify that a CVE query produces an exact identifier context with
    /// `query_kind = "cve"` and a CVE identifier in the resolved list.
    #[tokio::test]
    async fn cve_query_produces_exact_identifier_context() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "CVE-2024-1234 Advisory",
                "https://nvd.nist.gov/vuln/detail/CVE-2024-1234",
                "mock_a",
            )
            .with_snippet("A critical vulnerability")],
        )];
        let state = sec_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-1234 vulnerability in openssl".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let resolved = v["resolved_identifiers"]
            .as_object()
            .expect("resolved_identifiers");
        let cve_ids = resolved["cve_ids"].as_array().expect("cve_ids");
        assert!(
            cve_ids
                .iter()
                .any(|id| id.as_str() == Some("CVE-2024-1234")),
            "should resolve CVE-2024-1234: {cve_ids:?}"
        );

        // query_kind should be "cve" (not "unknown" or "concept")
        let security_ctx = v.get("security_context").and_then(|c| c.as_object());
        if let Some(ctx) = security_ctx {
            assert_eq!(
                ctx["query_kind"].as_str(),
                Some("cve"),
                "query_kind should be cve: {ctx:?}"
            );
        }
    }

    /// Verify that a GHSA query produces an exact identifier context with
    /// a GHSA identifier in the resolved list.
    #[tokio::test]
    async fn ghsa_query_produces_exact_identifier_context() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "GHSA Advisory",
                "https://github.com/advisories/GHSA-abcd-1234-efgh",
                "mock_a",
            )
            .with_snippet("GHSA advisory details")],
        )];
        let state = sec_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("GHSA-abcd-1234-efgh affects serde_json".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let resolved = v["resolved_identifiers"]
            .as_object()
            .expect("resolved_identifiers");
        let ghsa_ids = resolved["ghsa_ids"].as_array().expect("ghsa_ids");
        assert!(
            ghsa_ids
                .iter()
                .any(|id| id.as_str() == Some("GHSA-ABCD-1234-EFGH")),
            "should resolve GHSA-ABCD-1234-EFGH: {ghsa_ids:?}"
        );
    }

    /// Verify that a CWE query produces a weakness-class context with
    /// a CWE identifier in the resolved list and query_kind = "cwe".
    #[tokio::test]
    async fn cwe_query_produces_weakness_class_context() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "CWE-79 XSS",
                "https://cwe.mitre.org/data/definitions/79.html",
                "mock_a",
            )
            .with_snippet("Cross-site scripting weakness")],
        )];
        let state = sec_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CWE-79 cross-site scripting in web apps".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let resolved = v["resolved_identifiers"]
            .as_object()
            .expect("resolved_identifiers");
        let cwe_ids = resolved["cwe_ids"].as_array().expect("cwe_ids");
        assert!(
            cwe_ids.iter().any(|id| id.as_str() == Some("CWE-79")),
            "should resolve CWE-79: {cwe_ids:?}"
        );

        let security_ctx = v.get("security_context").and_then(|c| c.as_object());
        if let Some(ctx) = security_ctx {
            assert_eq!(
                ctx["query_kind"].as_str(),
                Some("cwe"),
                "query_kind should be cwe: {ctx:?}"
            );
        }
    }

    /// When a package query returns no results, the response must not
    /// claim vulnerabilities exist. The vulnerabilities array must be
    /// empty and the security_context must have zero vulnerability_summaries.
    #[tokio::test]
    async fn package_version_no_match_produces_no_false_vulnerability_claim() {
        let engines = vec![MockEngine::success("mock_a", vec![])];
        let state = sec_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("nonexistent-crate-xyz vulnerability".into()),
                package: Some("nonexistent-crate-xyz".into()),
                ecosystem: Some("crates.io".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        // vulnerabilities may be omitted (skip_serializing_if) or empty
        let has_vulns = v
            .get("vulnerabilities")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        assert!(
            !has_vulns,
            "vulnerabilities must be empty when no match: {v:?}"
        );

        // security_context.vulnerability_summaries must be empty or absent
        let has_vuln_summaries = v
            .get("security_context")
            .and_then(|c| c.get("vulnerability_summaries"))
            .and_then(|s| s.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        assert!(
            !has_vuln_summaries,
            "vulnerability_summaries must be empty when no match: {v:?}"
        );
    }

    /// The `include_exploit_context` flag must only add source-card
    /// context groups, not produce executable/procedural exploit payload
    /// fields. Verify that result cards do not contain `payload`,
    /// `exploit_code`, or `code` with executable content.
    #[tokio::test]
    async fn exploit_context_flag_does_not_produce_executable_payload_fields() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new(
                    "Exploit Discussion",
                    "https://exploit-db.com/exploits/12345",
                    "mock_a",
                )
                .with_snippet("Discussion about CVE-2024-0001 exploitability"),
                MockResult::new(
                    "NVD Entry",
                    "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                    "mock_a",
                )
                .with_snippet("NVD advisory for CVE-2024-0001"),
            ],
        )];
        let state = sec_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001 exploit".into()),
                include_exploit_context: Some(true),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        // Verify the exploit_discussion group exists
        let groups = v["groups"].as_array().expect("groups");
        let exploit_group = groups
            .iter()
            .find(|g| g["kind"].as_str() == Some("exploit_discussion"));
        assert!(
            exploit_group.is_some(),
            "exploit_discussion group should be present: {groups:?}"
        );

        // Verify no card contains payload/exploit_code fields
        let all_json = serde_json::to_string(&v).unwrap();
        assert!(
            !all_json.contains("\"payload\""),
            "response must not contain 'payload' field: {all_json}"
        );
        assert!(
            !all_json.contains("\"exploit_code\""),
            "response must not contain 'exploit_code' field: {all_json}"
        );
    }

    /// Source quality tiers should be correctly classified in the
    /// security search response. When results include NVD URLs, the
    /// security_context.source_quality.tier should reflect that.
    #[tokio::test]
    async fn security_search_source_quality_reflects_advisory_sources() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new(
                    "NVD Entry",
                    "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                    "mock_a",
                ),
                MockResult::new("Blog Post", "https://blog.example.com/security", "mock_a"),
            ],
        )];
        let state = sec_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let security_ctx = v.get("security_context").and_then(|c| c.as_object());
        if let Some(ctx) = security_ctx {
            let source_quality = ctx["source_quality"]
                .as_object()
                .expect("source_quality should be present");
            let tier = source_quality["tier"]
                .as_str()
                .expect("tier should be a string");
            // With an NVD URL in results, tier should be primary_advisory
            assert_eq!(
                tier, "primary_advisory",
                "source quality tier should be primary_advisory when NVD is present: {source_quality:?}"
            );
        }
    }

    /// Bug #1 regression: `severity_min` should filter out
    /// vulnerabilities below the threshold and emit an
    /// `severity_min_unenforced` warning when no severity metadata is
    /// available.
    #[tokio::test]
    async fn security_search_severity_min_unenforced_warning() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "NVD Entry",
                "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                "mock_a",
            )
            .with_snippet("Severity metadata is unavailable from generic search")],
        )];
        let state = sec_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001".into()),
                severity_min: Some("high".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let warnings = v["warnings"].as_array().expect("warnings");
        let has_warning = warnings.iter().any(|w| {
            w.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .contains("severity_min_unenforced")
        });
        assert!(
            has_warning,
            "should emit severity_min_unenforced warning when no severity metadata exists: {warnings:?}"
        );
    }

    /// Bug #2 regression: when `include_exploit_context` is set to
    /// `false`, exploit-discussion fetch candidates should be omitted
    /// from suggested fetches. Tested at the suggested-fetches layer
    /// so the integration assertion is independent of mock adapter
    /// state.
    #[tokio::test]
    async fn security_search_include_exploit_context_filters_suggested_fetches() {
        use eggsearch::core::result::TrustLevel;
        use eggsearch::core::security::{
            SecurityIdentifiers, SecurityResultGroup, SecurityResultGroupKind,
        };
        use eggsearch::core::source_card::SourceCard;
        use eggsearch::meta::security_suggested_fetches::generate_security_suggested_fetches;

        fn make_group(kind: SecurityResultGroupKind, url: &str) -> SecurityResultGroup {
            SecurityResultGroup {
                kind,
                label: format!("{kind:?}"),
                results: vec![SourceCard::new(
                    "Title",
                    url,
                    vec!["test".to_string()],
                    None,
                    TrustLevel::ExternalUntrusted,
                )],
                truncated: false,
                quality_summary: None,
            }
        }

        let groups = vec![
            make_group(
                SecurityResultGroupKind::AuthoritativeAdvisories,
                "https://osv.dev/CVE-2024-0001",
            ),
            make_group(
                SecurityResultGroupKind::ExploitDiscussion,
                "https://example.com/poc",
            ),
            make_group(
                SecurityResultGroupKind::VendorAdvisories,
                "https://example.com/security/advisory",
            ),
            make_group(
                SecurityResultGroupKind::DefensiveGuidance,
                "https://example.com/mitigation",
            ),
        ];
        let ids = SecurityIdentifiers::default();

        let with_exploit = generate_security_suggested_fetches(
            &groups,
            &ids,
            None,
            None,
            &[],
            Some(true),
            Some(true),
            Some(true),
        );
        assert!(with_exploit.iter().any(|f| f.url.contains("poc")));

        let without_exploit = generate_security_suggested_fetches(
            &groups,
            &ids,
            None,
            None,
            &[],
            Some(false),
            None,
            None,
        );
        assert!(!without_exploit.iter().any(|f| f.url.contains("poc")));

        let without_vendor = generate_security_suggested_fetches(
            &groups,
            &ids,
            None,
            None,
            &[],
            None,
            None,
            Some(false),
        );
        assert!(!without_vendor
            .iter()
            .any(|f| f.url.contains("/security/advisory")));

        let without_defensive = generate_security_suggested_fetches(
            &groups,
            &ids,
            None,
            None,
            &[],
            None,
            Some(false),
            None,
        );
        assert!(!without_defensive
            .iter()
            .any(|f| f.url.contains("/mitigation")));
    }

    /// Bug #3 regression: providers_failed should be populated when a
    /// security provider returns an error during dispatch.
    #[tokio::test]
    async fn security_search_providers_failed_populated_on_failure() {
        let engines = vec![MockEngine::failure("mock_a", MockFailure::Network)];
        let state = sec_state_with_engines(test_cfg(), engines, Duration::from_secs(5));

        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let failed = v["providers_failed"].as_array().expect("providers_failed");
        assert!(
            !failed.is_empty(),
            "providers_failed should be populated when a provider fails: {v:?}"
        );
        let first = &failed[0];
        assert_eq!(first["id"], "mock_a");
        assert!(first["error_class"].is_string());
    }

    /// Bug #4 regression: `repo_fetch.symbol_kind` should reject
    /// unknown values with a validation error rather than silently
    /// broadening matching.
    #[tokio::test]
    async fn repo_fetch_invalid_symbol_kind_returns_validation_error() {
        let state = repo_fetch_state();
        let res = run_repo_fetch(
            state,
            RepoFetchArgs {
                host: Some("github".into()),
                owner: "test-owner".into(),
                repo: "test-repo".into(),
                ref_name: Some("main".into()),
                commit_sha: None,
                path: "src/lib.rs".into(),
                line_start: None,
                line_end: None,
                context_before: None,
                context_after: None,
                max_chars: None,
                timeout_ms: None,
                test_fetch_url: None,
                symbol: Some("foo".into()),
                symbol_kind: Some("funciton".into()),
                match_text: None,
                expand_to_block: None,
                max_block_lines: None,
                prefer_local: None,
            },
        )
        .await;
        match res {
            Err(ToolError::Validation(msg)) => {
                assert!(
                    msg.contains("invalid symbol_kind 'funciton'"),
                    "unexpected validation message: {msg}"
                );
            }
            other => panic!("expected validation error, got: {other:?}"),
        }
    }

    /// Bug #6 regression: batch_fetch web responses must include
    /// `stable_id`, `source_id`, and `structured_warnings` so callers
    /// can handle the per-item payload like a regular web_fetch
    /// response.
    #[tokio::test]
    async fn batch_fetch_web_response_matches_web_fetch_shape() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/get");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body("<!DOCTYPE html><html><body><p>Some content</p></body></html>");
        });

        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.fetch.sanitize_output = false;
        let state = Arc::new(ServerState::build(cfg).expect("state builds"));

        let v = run_batch_fetch(
            state,
            BatchFetchArgs {
                items: vec![eggsearch::core::batch_fetch::BatchFetchItem::Web {
                    url: server.url("/get"),
                    extract_mode: Some(ExtractMode::Text),
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
                timeout_ms: Some(1000),
                continue_on_error: None,
            },
        )
        .await
        .expect("batch_fetch should succeed");

        let results = v["results"].as_array().expect("results");
        let response = results[0]["response"].as_object().expect("response");
        assert!(
            response.contains_key("stable_id"),
            "batch_fetch web response must include stable_id: {response:?}"
        );
        assert!(
            response.contains_key("source_id"),
            "batch_fetch web response must include source_id: {response:?}"
        );
        assert!(
            response.contains_key("structured_warnings"),
            "batch_fetch web response must include structured_warnings: {response:?}"
        );
    }

    /// Bug #5 regression: when the aggregate `max_total_chars` budget
    /// is exhausted by an item, the embedded response payload must be
    /// trimmed so `total_chars_returned` reflects actual content size.
    #[tokio::test]
    async fn batch_fetch_trims_payload_to_aggregate_budget() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/big");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body("A".repeat(100));
        });

        let mut cfg = AppConfig::default();
        cfg.fetch.allow_localhost = true;
        cfg.fetch.allow_private_network = true;
        cfg.fetch.sanitize_output = false;
        cfg.fetch.batch_concurrency = 1;
        let state = Arc::new(ServerState::build(cfg).expect("state builds"));

        let v = run_batch_fetch(
            state,
            BatchFetchArgs {
                items: vec![eggsearch::core::batch_fetch::BatchFetchItem::Web {
                    url: server.url("/big"),
                    extract_mode: Some(ExtractMode::Text),
                    include_links: None,
                    max_chars: Some(1000),
                    cache_policy: None,
                    max_cache_age_seconds: None,
                    focus: None,
                    focus_max_chunks: None,
                    focus_max_chars: None,
                }],
                max_items: None,
                max_chars_per_item: None,
                max_total_chars: Some(20),
                timeout_ms: Some(1000),
                continue_on_error: None,
            },
        )
        .await
        .expect("batch_fetch should succeed");

        let total = v["total_chars_returned"].as_u64().unwrap();
        assert!(
            total <= 20,
            "total_chars_returned {total} should be <= 20 after budget trimming: {v:?}"
        );
        let results = v["results"].as_array().expect("results");
        let item_chars = results[0]["chars_returned"].as_u64().unwrap();
        assert!(
            item_chars <= 20,
            "item chars_returned {item_chars} should be <= 20 after budget trimming"
        );
        let text = results[0]["response"]["text"].as_str().unwrap_or("");
        assert!(
            text.chars().count() <= 20,
            "embedded text len {} should be <= 20 after budget trimming",
            text.chars().count()
        );
    }

    /// Bug #7 regression: the local inventory cache should serve
    /// repeated calls without re-running filesystem scans.
    #[tokio::test]
    async fn server_state_local_inventory_is_cached() {
        use eggsearch::core::local::LocalConfig;
        use eggsearch::meta::local_backend::LocalWorkspaceBackend;

        let dir = tempfile::tempdir().expect("tempdir");
        let mut cfg = test_cfg();
        cfg.local = LocalConfig {
            enabled: true,
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };

        let backend = LocalWorkspaceBackend::new(cfg.local.clone()).expect("backend");
        let state = Arc::new(ServerState {
            config: Arc::new(cfg),
            adapter: Arc::new(MetadataSearchAdapter::from_engines(
                mock_engines(vec![]),
                Duration::from_secs(5),
            )),
            fetch_client: None,
            origin_controller: None,
            fetch_cache: None,
            kev_client: Arc::new(eggsearch::meta::engines::kev::KevClient::new(
                reqwest::Client::new(),
            )),
            local_backend: Some(Arc::new(backend)),
            local_inventory_cache: Arc::new(std::sync::Mutex::new(None)),
            #[cfg(feature = "browser")]
            profile_manager: None,
            #[cfg(feature = "browser")]
            browser_lifecycle: None,
            #[cfg(feature = "browser")]
            browser_discovery_state:
                eggsearch::fetch::browser::types::BrowserDiscoveryState::NotFound,
        });

        let first = state.local_inventory().await;
        let second = state.local_inventory().await;
        assert_eq!(
            first.len(),
            second.len(),
            "cache should serve identical results"
        );

        state.invalidate_local_inventory_cache();
        let third = state.local_inventory().await;
        assert_eq!(first.len(), third.len(), "invalidate+reload should match");
    }

    /// Bug #2 regression: `local_inventory()` must honor the operator's
    /// `[local]` config (e.g. `include_hidden`, `follow_symlinks`,
    /// `respect_gitignore`) when discovering repositories, instead of
    /// always using the default `LocalConfig`.
    #[tokio::test]
    async fn server_state_local_inventory_honors_backend_config() {
        use eggsearch::core::local::LocalConfig;
        use eggsearch::meta::local_backend::LocalWorkspaceBackend;

        let dir = tempfile::tempdir().expect("tempdir");
        let hidden_repo = dir.path().join(".hidden_repo");
        std::fs::create_dir_all(&hidden_repo).expect("hidden dir");
        git_cmd()
            .arg("init")
            .arg(&hidden_repo)
            .output()
            .expect("git init hidden");

        let mut cfg = test_cfg();
        cfg.local = LocalConfig {
            enabled: true,
            roots: vec![dir.path().to_path_buf()],
            include_hidden: true,
            ..Default::default()
        };
        let backend = LocalWorkspaceBackend::new(cfg.local.clone()).expect("backend");

        let state = Arc::new(ServerState {
            config: Arc::new(cfg),
            adapter: Arc::new(MetadataSearchAdapter::from_engines(
                mock_engines(vec![]),
                Duration::from_secs(5),
            )),
            fetch_client: None,
            origin_controller: None,
            fetch_cache: None,
            kev_client: Arc::new(eggsearch::meta::engines::kev::KevClient::new(
                reqwest::Client::new(),
            )),
            local_backend: Some(Arc::new(backend)),
            local_inventory_cache: Arc::new(std::sync::Mutex::new(None)),
            #[cfg(feature = "browser")]
            profile_manager: None,
            #[cfg(feature = "browser")]
            browser_lifecycle: None,
            #[cfg(feature = "browser")]
            browser_discovery_state:
                eggsearch::fetch::browser::types::BrowserDiscoveryState::NotFound,
        });
        let inventory = state.local_inventory().await;
        let names: Vec<&str> = inventory.iter().map(|r| r.root_name.as_str()).collect();
        assert!(
            names.contains(&".hidden_repo"),
            "include_hidden=true should surface hidden repo, got {names:?}"
        );
    }
}

fn repo_fetch_state() -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    cfg.fetch.sanitize_output = false;
    Arc::new(ServerState::build(cfg).expect("repo_fetch state"))
}

fn fetch_disabled_state() -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.fetch.enabled = false;
    Arc::new(ServerState::build(cfg).expect("state with fetch disabled"))
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
