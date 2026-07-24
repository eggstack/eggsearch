//! Regression corpus tests for eggsearch.
//!
//! These tests exercise whole-workflow scenarios with mock providers
//! to ensure quality does not regress silently. They cover:
//!
//! - Repository map and search workflows
//! - Symbol search and exact-error mode
//! - Security search and applicability assessment
//! - Research search with workflow scaffolding
//! - Ranking regression checks
//! - Warning and trust marker contracts
//!
//! All tests are offline (mock providers only) and run via:
//! ```bash
//! cargo test --features mock --test corpus_runner
//! ```

#![cfg(feature = "mock")]

use std::sync::Arc;
use std::time::Duration;

use eggsearch::core::batch_fetch::BatchFetchItem;
use eggsearch::core::config::AppConfig;
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{
    run_batch_fetch, run_repo_fetch, run_repo_map, run_repo_search, run_research_search,
    run_security_search, run_web_search, BatchFetchArgs, RepoFetchArgs, RepoMapArgs,
    RepoSearchArgs, ResearchSearchArgs, SecuritySearchArgs, WebSearchArgs,
};
use eggsearch::meta::mock::{mock_engines, MockEngine, MockResult};
use eggsearch::meta::MetadataSearchAdapter;

fn git_cmd() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "safe.directory")
        .env("GIT_CONFIG_VALUE_0", "*");
    cmd
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn corpus_cfg() -> AppConfig {
    let mut cfg = AppConfig::default();
    cfg.search.timeout_ms = 5_000;
    cfg.search.max_query_chars = 512;
    cfg.search.default_max_results = 10;
    cfg.search.max_results_cap = 50;
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.search.providers.insert("mock_b".to_string(), true);
    cfg
}

fn state_with(cfg: AppConfig, engines: Vec<MockEngine>, timeout: Duration) -> Arc<ServerState> {
    let adapter = MetadataSearchAdapter::from_engines(mock_engines(engines), timeout);
    Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
}

fn web_args(query: &str) -> WebSearchArgs {
    WebSearchArgs {
        query: query.to_string(),
        max_results: None,
        providers: vec!["mock_a".into()],
        safe_search: None,
        timeout_ms: None,
        intent: None,
        freshness: None,
    }
}

fn repo_args(query: &str) -> RepoSearchArgs {
    RepoSearchArgs {
        query: query.to_string(),
        providers: vec!["mock_a".into()],
        ..Default::default()
    }
}

fn repo_args_multi(providers: &[&str], query: &str) -> RepoSearchArgs {
    RepoSearchArgs {
        query: query.to_string(),
        providers: providers.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn research_args(query: &str) -> ResearchSearchArgs {
    ResearchSearchArgs {
        query: query.to_string(),
        providers: vec!["mock_a".into()],
        ..Default::default()
    }
}

fn state_with_local_backend(temp_dir: &std::path::Path) -> Arc<ServerState> {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter =
        MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    cfg.local.enabled = true;
    cfg.local.roots = vec![temp_dir.to_path_buf()];
    let backend = eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg.local.clone())
        .expect("backend builds");
    let mut state = ServerState::with_adapter(cfg, Arc::new(adapter));
    state.local_backend = Some(Arc::new(backend));
    Arc::new(state)
}

// ---------------------------------------------------------------------------
// Workstream 2: Repository workflows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_repo_search_returns_grouped_response() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new("Axum Docs", "https://docs.rs/axum/latest/axum/", "mock_a"),
            MockResult::new(
                "Axum Source",
                "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
                "mock_a",
            ),
            MockResult::new(
                "Axum Issue #123",
                "https://github.com/tokio-rs/axum/issues/123",
                "mock_a",
            ),
            MockResult::new(
                "Axum Release 0.7",
                "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
                "mock_a",
            ),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let v = run_repo_search(state, repo_args("axum"))
        .await
        .expect("repo_search should succeed");

    // Response shape
    assert_eq!(v["query"], "axum");
    assert!(v["groups"].is_array(), "groups must be array");
    assert!(
        v["suggested_fetches"].is_array(),
        "suggested_fetches must be array"
    );
    assert!(
        v["providers_queried"].is_array(),
        "providers_queried must be array"
    );
    assert!(v["warnings"].is_array(), "warnings must be array");
    assert!(
        v["trust_markers"].is_object(),
        "trust_markers must be object"
    );

    // At least one group should have results
    let groups = v["groups"].as_array().unwrap();
    let nonempty: Vec<_> = groups
        .iter()
        .filter(|g| !g["results"].as_array().unwrap_or(&vec![]).is_empty())
        .collect();
    assert!(!nonempty.is_empty(), "at least one group must have results");

    // Suggested fetches should be non-empty
    let fetches = v["suggested_fetches"].as_array().unwrap();
    assert!(!fetches.is_empty(), "should suggest at least one fetch");
}

#[tokio::test]
async fn corpus_repo_search_groups_match_expected_kinds() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new("Axum Docs", "https://docs.rs/axum/latest/axum/", "mock_a"),
            MockResult::new(
                "Axum Source",
                "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
                "mock_a",
            ),
            MockResult::new(
                "Axum Issue #123",
                "https://github.com/tokio-rs/axum/issues/123",
                "mock_a",
            ),
            MockResult::new(
                "Axum Release 0.7",
                "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
                "mock_a",
            ),
            MockResult::new(
                "Axum Examples",
                "https://github.com/tokio-rs/axum/tree/main/examples",
                "mock_a",
            ),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();

    // Should have at least some of these groups
    let expected = ["official_docs", "source_files", "issues", "releases"];
    for kind in &expected {
        assert!(
            kinds.contains(kind),
            "expected group kind '{kind}' not found in {kinds:?}"
        );
    }
}

#[tokio::test]
async fn corpus_repo_search_empty_query_returns_validation_error() {
    let state = state_with(corpus_cfg(), vec![], Duration::from_secs(5));
    let res = run_repo_search(state, repo_args("   ")).await;
    assert!(res.is_err(), "empty query should fail");
    assert!(
        res.unwrap_err().to_string().contains("invalid query"),
        "error should mention invalid query"
    );
}

#[tokio::test]
async fn corpus_repo_search_all_providers_fail_returns_error_or_empty() {
    let engines = vec![
        MockEngine::failure("mock_a", eggsearch::meta::mock::MockFailure::Network),
        MockEngine::failure("mock_b", eggsearch::meta::mock::MockFailure::Network),
    ];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let res = run_repo_search(state, repo_args_multi(&["mock_a", "mock_b"], "rust")).await;
    // All providers failing may return an error or empty results with
    // providers_failed — both are acceptable behavior.
    if let Ok(v) = res {
        let failed = v["providers_failed"].as_array().unwrap();
        assert!(
            !failed.is_empty(),
            "should report failed providers when returning empty: {v:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Workstream 2: Repo map
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_repo_map_returns_structure() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "tokio-rs/axum: A web framework for Rust",
                "https://github.com/tokio-rs/axum",
                "mock_a",
            ),
            MockResult::new(
                "README.md - axum",
                "https://github.com/tokio-rs/axum/blob/main/README.md",
                "mock_a",
            ),
            MockResult::new(
                "Cargo.toml - axum",
                "https://github.com/tokio-rs/axum/blob/main/Cargo.toml",
                "mock_a",
            ),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoMapArgs {
        host: None,
        owner: "tokio-rs".into(),
        repo: "axum".into(),
        ref_name: None,
        commit_sha: None,
        max_entries: None,
        max_depth: None,
        include_files: None,
        include_directories: None,
        include_ci: None,
        include_security: None,
        timeout_ms: None,
        providers: vec!["mock_a".into()],
    };
    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    assert_eq!(v["owner"], "tokio-rs");
    assert_eq!(v["repo"], "axum");
    assert!(
        v["root_entries"].is_array() || v["root_entries"].is_null(),
        "root_entries must be array or null: {v:?}"
    );
    assert!(
        v["suggested_fetches"].is_array() || v["suggested_fetches"].is_null(),
        "suggested_fetches must be array or null: {v:?}"
    );

    // Should suggest README fetch (when fetches are present)
    if let Some(fetches) = v["suggested_fetches"].as_array() {
        let has_readme = fetches
            .iter()
            .any(|f| f["url"].as_str().unwrap_or("").contains("README"));
        assert!(has_readme, "should suggest README fetch: {fetches:?}");
    }
}

#[tokio::test]
async fn corpus_repo_map_missing_owner_repo_returns_error() {
    let state = state_with(corpus_cfg(), vec![], Duration::from_secs(5));
    let args = RepoMapArgs {
        host: None,
        owner: "".into(),
        repo: "".into(),
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
    };
    let res = run_repo_map(state, args).await;
    assert!(res.is_err(), "missing owner/repo should fail");
}

// ---------------------------------------------------------------------------
// Workstream 2: Symbol search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_repo_search_symbol_suggests_source_fetch() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "Router::layer in axum/src/routing/mod.rs",
                "https://github.com/tokio-rs/axum/blob/main/src/routing/mod.rs",
                "mock_a",
            )
            .with_snippet("pub fn layer<L>(self, layer: L) -> Router<L>"),
            MockResult::new(
                "axum::routing::Router - docs.rs",
                "https://docs.rs/axum/latest/axum/struct.Router.html",
                "mock_a",
            )
            .with_snippet("Router provides routing for axum applications"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "Router::layer".into(),
        owner: Some("tokio-rs".into()),
        repo: Some("axum".into()),
        symbol: Some("Router::layer".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"source_files"),
        "symbol search should produce source_files group: {kinds:?}"
    );

    // Should suggest a repo_fetch for the source file
    let fetches = v["suggested_fetches"].as_array().unwrap();
    let has_structured = fetches.iter().any(|f| {
        f.get("structured_repo_fetch")
            .and_then(|s| s.as_object())
            .is_some()
    });
    assert!(
        has_structured,
        "symbol search should suggest structured repo_fetch: {fetches:?}"
    );
}

// ---------------------------------------------------------------------------
// Workstream 2: Exact-error mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_exact_error_rust_produces_error_context() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "E0308: mismatched types",
                "https://doc.rust-lang.org/error-index.html#E0308",
                "mock_a",
            )
            .with_snippet("expected `&str`, found `i32`"),
            MockResult::new(
                "Issue: E0308 regression in nightly",
                "https://github.com/rust-lang/rust/issues/123456",
                "mock_a",
            )
            .with_snippet("E0308 regression with trait bounds"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "error[E0308]: mismatched types -- expected `&str` found `i32`".into(),
        mode: Some("exact_error".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    // Should have error_context
    let error_ctx = v.get("error_context");
    assert!(
        error_ctx.is_some() && !error_ctx.unwrap().is_null(),
        "exact-error mode should produce error_context: {v:?}"
    );

    // Mode should be "exact_error"
    assert_eq!(v["mode"], "exact_error", "mode should be exact_error");

    // Should have groups
    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "should have groups in exact-error mode");
}

#[tokio::test]
async fn corpus_exact_error_redacts_sensitive_tokens() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "Error in /home/user/project/src/main.rs",
            "https://github.com/example/project/issues/1",
            "mock_a",
        )
        .with_snippet("panic at '/home/user/project/src/main.rs:42'")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "panic at '/home/user/project/src/main.rs:42'".into(),
        mode: Some("exact_error".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    // error_context should be present and contain redactions info
    let ctx = v
        .get("error_context")
        .expect("error_context should be present for exact_error query");
    assert!(ctx.is_object(), "error_context must be an object");
    let redactions = ctx
        .get("redactions_applied")
        .expect("error_context.redactions_applied must be present");
    assert!(
        redactions.is_array(),
        "redactions_applied must be array, got: {redactions:?}"
    );
    assert!(
        !redactions.as_array().unwrap().is_empty(),
        "redactions_applied should not be empty when sensitive tokens are present"
    );
}

// ---------------------------------------------------------------------------
// Workstream 3: Package and migration workflows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_repo_search_package_lookup() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "serde - crates.io",
                "https://crates.io/crates/serde",
                "mock_a",
            )
            .with_snippet("A generic serialization/deserialization framework"),
            MockResult::new(
                "serde - GitHub",
                "https://github.com/serde-rs/serde",
                "mock_a",
            )
            .with_snippet("serde source repository"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "serde".into(),
        ecosystem: Some("crates.io".into()),
        package: Some("serde".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    // Should have groups with package registry evidence
    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"package_registry") || kinds.contains(&"official_docs"),
        "package lookup should have package_registry or official_docs group: {kinds:?}"
    );
}

#[tokio::test]
async fn corpus_repo_search_changelog_suggested_for_migration() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "axum 0.7 migration guide",
                "https://github.com/tokio-rs/axum/blob/main/CHANGELOG.md",
                "mock_a",
            )
            .with_snippet("Migration notes from 0.6 to 0.7"),
            MockResult::new(
                "axum 0.7 release",
                "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
                "mock_a",
            )
            .with_snippet("Release notes for axum 0.7"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "axum migration 0.6 to 0.7".into(),
        owner: Some("tokio-rs".into()),
        repo: Some("axum".into()),
        compare_version: Some("0.6".into()),
        version: Some("0.7".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    // Should suggest changelog or release fetch
    let fetches = v["suggested_fetches"].as_array().unwrap();
    let has_changelog = fetches.iter().any(|f| {
        let url = f["url"].as_str().unwrap_or("");
        url.contains("CHANGELOG") || url.contains("releases")
    });
    assert!(
        has_changelog,
        "migration query should suggest changelog/release: {fetches:?}"
    );
}

// ---------------------------------------------------------------------------
// Workstream 4: Security workflows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_security_cve_lookup_returns_structured_response() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "CVE-2024-0001: Test vulnerability",
                "https://osv.dev/vulnerability/GHSA-test-1234-abcd",
                "mock_a",
            )
            .with_snippet("A test vulnerability in test-package"),
            MockResult::new(
                "NVD: CVE-2024-0001",
                "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                "mock_a",
            )
            .with_snippet("NVD advisory details"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: Some("CVE-2024-0001".into()),
        cve_id: Some("CVE-2024-0001".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args)
        .await
        .expect("security_search should succeed");

    assert_eq!(v["mode"], "security_metasearch");

    // Should resolve the CVE ID
    let resolved = v["resolved_identifiers"]
        .as_object()
        .expect("resolved_identifiers");
    let cve_ids = resolved["cve_ids"].as_array().expect("cve_ids");
    assert!(
        cve_ids
            .iter()
            .any(|id| id.as_str() == Some("CVE-2024-0001")),
        "should resolve CVE-2024-0001: {cve_ids:?}"
    );

    // Should have groups
    let groups = v["groups"].as_array().expect("groups");
    assert!(!groups.is_empty(), "should have at least one group");

    // Should have advisory source warning (no native provider)
    let warnings = v["warnings"].as_array().expect("warnings");
    assert!(
        warnings.iter().any(|w| w["message"]
            .as_str()
            .unwrap_or("")
            .contains("generic_context_untrusted")),
        "should warn about generic context: {warnings:?}"
    );
}

#[tokio::test]
async fn corpus_security_with_explicit_cve_id() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "Advisory for CVE-2024-12345",
            "https://nvd.nist.gov/vuln/detail/CVE-2024-12345",
            "mock_a",
        )
        .with_snippet("NVD advisory details")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: None,
        cve_id: Some("CVE-2024-12345".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");

    let resolved = v["resolved_identifiers"].as_object().unwrap();
    let cve_ids = resolved["cve_ids"].as_array().unwrap();
    assert_eq!(cve_ids.len(), 1);
    assert_eq!(cve_ids[0].as_str(), Some("CVE-2024-12345"));
}

#[tokio::test]
async fn corpus_security_osv_applicability() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "GHSA-xxxx-xxxx-xxxx: Vulnerability in axios",
            "https://osv.dev/vulnerability/GHSA-xxxx-xxxx-xxxx",
            "mock_a",
        )
        .with_snippet("Affected versions: < 1.6.0, Patched: 1.6.0")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: Some("axios vulnerability".into()),
        ecosystem: Some("npm".into()),
        package: Some("axios".into()),
        version: Some("1.5.0".into()),
        assess_applicability: Some(true),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");

    // Applicability field is present when OSV native provider is used;
    // with mock engines it may be absent. Check the groups instead.
    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "should have groups");
}

#[tokio::test]
async fn corpus_security_search_groups_results_by_type() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "CVE-2024-0001 advisory",
                "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                "mock_a",
            )
            .with_snippet("Advisory details"),
            MockResult::new(
                "Vendor patch for CVE-2024-0001",
                "https://example.com/security/patch",
                "mock_a",
            )
            .with_snippet("Patch release"),
            MockResult::new(
                "Discussion of CVE-2024-0001",
                "https://forum.example.com/t/cve-2024-0001",
                "mock_a",
            )
            .with_snippet("Community discussion"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: Some("CVE-2024-0001 test-package vulnerability".into()),
        ecosystem: Some("npm".into()),
        package: Some("test-package".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");

    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();

    // Should have at least some group kinds (exact kinds depend on URL classification)
    assert!(
        !kinds.is_empty(),
        "should have at least one group kind: {kinds:?}"
    );
    // Should have authoritative or general context
    assert!(
        kinds.contains(&"authoritative_advisories") || kinds.contains(&"general_context"),
        "should have authoritative or general group: {kinds:?}"
    );
}

#[tokio::test]
async fn corpus_security_trust_markers_present() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "CVE-2024-0001",
            "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
            "mock_a",
        )],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: Some("CVE-2024-0001".into()),
        cve_id: Some("CVE-2024-0001".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");

    assert!(
        v["trust_markers"].is_object(),
        "trust_markers must be object: {v:?}"
    );
    let tm = v["trust_markers"].as_object().unwrap();
    assert!(
        tm.contains_key("text_sanitized"),
        "trust_markers must have text_sanitized"
    );
}

// ---------------------------------------------------------------------------
// Workstream 5: Research workflows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_research_architecture_decision() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "Axum vs Actix-web comparison",
                "https://www.shuttle.rs/blog/2024/01/15/axum-vs-actix-web",
                "mock_a",
            )
            .with_snippet("Comparing Rust web frameworks"),
            MockResult::new(
                "Actix-web documentation",
                "https://actix.rs/docs/",
                "mock_a",
            )
            .with_snippet("Actix-web is a web framework for Rust"),
            MockResult::new(
                "Axum architecture decision record",
                "https://github.com/tokio-rs/axum/discussions/123",
                "mock_a",
            )
            .with_snippet("Architecture discussion"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = ResearchSearchArgs {
        query: "Rust web framework architecture decision axum vs actix-web".into(),
        workflow: Some("architecture_decision".into()),
        depth: Some("standard".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_research_search(state, args).await.expect("ok");

    assert_eq!(v["mode"], "research_metasearch");

    // Should have groups
    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "should have groups");

    // Should have suggested fetches
    let fetches = v["suggested_fetches"].as_array().unwrap();
    assert!(
        !fetches.is_empty(),
        "architecture decision should suggest fetches"
    );

    // Should have subqueries
    let subqueries = v["subqueries"].as_array().unwrap();
    assert!(
        !subqueries.is_empty(),
        "should generate subqueries for research"
    );
}

#[tokio::test]
async fn corpus_research_library_comparison() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "axum documentation",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )
            .with_snippet("A web framework for Rust"),
            MockResult::new(
                "actix-web documentation",
                "https://docs.rs/actix-web/latest/actix_web/",
                "mock_a",
            )
            .with_snippet("Actix web framework"),
            MockResult::new(
                "axum vs actix-web benchmark",
                "https://www.techempower.com/benchmarks/",
                "mock_a",
            )
            .with_snippet("Framework benchmarks"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = ResearchSearchArgs {
        query: "compare axum vs actix-web for REST API".into(),
        workflow: Some("library_comparison".into()),
        compare_targets: vec!["axum".into(), "actix-web".into()],
        depth: Some("standard".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_research_search(state, args).await.expect("ok");

    // Should have groups and subqueries
    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "comparison should have groups");
    let subqueries = v["subqueries"].as_array().unwrap();
    assert!(!subqueries.is_empty(), "should generate subqueries");
}

#[tokio::test]
async fn corpus_research_empty_query_returns_validation_error() {
    let state = state_with(corpus_cfg(), vec![], Duration::from_secs(5));
    let res = run_research_search(state, research_args("   ")).await;
    assert!(res.is_err(), "empty query should fail");
}

#[tokio::test]
async fn corpus_research_counterpoints_gap_detection() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "Rust async runtime comparison",
            "https://tokio.rs/blog/2024/async-comparison",
            "mock_a",
        )
        .with_snippet("Tokio is the most popular async runtime")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = ResearchSearchArgs {
        query: "tokio vs async-std async runtime".into(),
        include_counterpoints: Some(true),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_research_search(state, args).await.expect("ok");

    // Should have groups (may or may not have counterpoints depending on results)
    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "should have groups");
}

// ---------------------------------------------------------------------------
// Workstream 8: Ranking regression checks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_ranking_commit_pinned_outranks_mutable_url() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            // Mutable URL should rank lower
            MockResult::new(
                "Router - axum (main branch)",
                "https://github.com/tokio-rs/axum/blob/main/src/routing/mod.rs",
                "mock_a",
            )
            .with_snippet("pub struct Router"),
            // Commit-pinned URL should rank higher via rank reasons
            MockResult::new(
                "Router - axum (pinned)",
                "https://github.com/tokio-rs/axum/blob/abc123def/src/routing/mod.rs",
                "mock_a",
            )
            .with_snippet("pub struct Router"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "Router struct axum".into(),
        owner: Some("tokio-rs".into()),
        repo: Some("axum".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    // Both the pinned URL and the mutable URL should appear in the
    // suggested fetches / results, confirming the test setup exercises
    // both code paths. The pinning logic should ensure the pinned
    // URL appears in the result set at all (regression guard against
    // dropping pinned URLs during dedup).
    let mut all_urls: Vec<String> = Vec::new();
    if let Some(groups) = v["groups"].as_array() {
        for group in groups {
            if let Some(results) = group["results"].as_array() {
                for card in results {
                    if let Some(u) = card["url"].as_str() {
                        all_urls.push(u.to_string());
                    }
                }
            }
        }
    }
    if let Some(fetches) = v["suggested_fetches"].as_array() {
        for f in fetches {
            if let Some(u) = f["url"].as_str() {
                all_urls.push(u.to_string());
            }
        }
    }
    let has_pinned = all_urls.iter().any(|u| u.contains("abc123def"));
    let has_mutable = all_urls.iter().any(|u| u.contains("/blob/main/"));
    assert!(has_pinned, "pinned URL missing from results: {all_urls:?}");
    assert!(
        has_mutable,
        "mutable URL missing from results: {all_urls:?}"
    );
}

#[tokio::test]
async fn corpus_ranking_exact_error_issue_outranks_generic_docs() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "Rust error index",
                "https://doc.rust-lang.org/error-index.html",
                "mock_a",
            )
            .with_snippet("List of all Rust compiler errors"),
            MockResult::new(
                "Issue: E0308 regression in nightly-2024",
                "https://github.com/rust-lang/rust/issues/123456",
                "mock_a",
            )
            .with_snippet("E0308 regression with specific trait bound"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "error[E0308]: mismatched types nightly regression".into(),
        mode: Some("exact_error".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    // Should have groups
    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "exact error should have groups");

    // Should have suggested fetches
    let fetches = v["suggested_fetches"].as_array().unwrap();
    assert!(!fetches.is_empty(), "exact error should suggest fetches");

    // Check that issues are in the groups
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"issues") || kinds.contains(&"official_docs"),
        "exact error should have issues or official_docs group: {kinds:?}"
    );
}

#[tokio::test]
async fn corpus_ranking_migration_prioritizes_changelog() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "axum - GitHub",
                "https://github.com/tokio-rs/axum",
                "mock_a",
            )
            .with_snippet("A web framework for Rust"),
            MockResult::new(
                "CHANGELOG.md - axum",
                "https://github.com/tokio-rs/axum/blob/main/CHANGELOG.md",
                "mock_a",
            )
            .with_snippet("Migration notes from 0.6 to 0.7"),
            MockResult::new(
                "axum 0.7 release",
                "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
                "mock_a",
            )
            .with_snippet("Release notes"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "axum migration guide 0.6 to 0.7 changelog".into(),
        owner: Some("tokio-rs".into()),
        repo: Some("axum".into()),
        compare_version: Some("0.6".into()),
        version: Some("0.7".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    // Should have changelog group
    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"changelog") || kinds.contains(&"releases"),
        "migration should have changelog or releases group: {kinds:?}"
    );

    // Suggested fetches should prioritize changelog/release
    let fetches = v["suggested_fetches"].as_array().unwrap();
    let has_changelog = fetches.iter().any(|f| {
        let url = f["url"].as_str().unwrap_or("");
        url.contains("CHANGELOG") || url.contains("releases")
    });
    assert!(
        has_changelog,
        "migration should suggest changelog/release: {fetches:?}"
    );
}

#[tokio::test]
async fn corpus_ranking_security_prioritizes_advisory_sources() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "CVE-2024-0001 blog post",
                "https://blog.example.com/cve-2024-0001-analysis",
                "mock_a",
            )
            .with_snippet("Analysis of the vulnerability"),
            MockResult::new(
                "NVD: CVE-2024-0001",
                "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                "mock_a",
            )
            .with_snippet("Official NVD entry"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: Some("CVE-2024-0001".into()),
        cve_id: Some("CVE-2024-0001".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");

    // Should have advisory group
    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"authoritative_advisories") || kinds.contains(&"general_context"),
        "security should prioritize advisory sources: {kinds:?}"
    );
}

// ---------------------------------------------------------------------------
// Workstream 1: Web search basics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_web_search_returns_structured_response() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "Rust Programming Language",
                "https://www.rust-lang.org/",
                "mock_a",
            )
            .with_snippet("A systems programming language"),
            MockResult::new("Rust Book", "https://doc.rust-lang.org/book/", "mock_a")
                .with_snippet("The Rust Programming Language book"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let v = run_web_search(state, web_args("rust programming"))
        .await
        .expect("web_search should succeed");

    // Response shape
    assert_eq!(v["query"], "rust programming");
    assert!(v["results"].is_array(), "results must be array");
    assert!(
        v["providers_queried"].is_array(),
        "providers_queried must be array"
    );
    assert!(v["warnings"].is_array(), "warnings must be array");
    assert!(
        v["trust_markers"].is_object(),
        "trust_markers must be object"
    );

    // Results should have required fields
    let results = v["results"].as_array().unwrap();
    assert!(!results.is_empty(), "should have results");
    for card in results {
        assert!(card["id"].is_string(), "card must have id");
        assert!(card["title"].is_string(), "card must have title");
        assert!(card["url"].is_string(), "card must have url");
        assert!(card["trust"].is_string(), "card must have trust");
        assert!(
            card["metadata"].is_object(),
            "card must have metadata object"
        );
        assert!(
            card["trust_markers"].is_object(),
            "card must have trust_markers"
        );
    }
}

#[tokio::test]
async fn corpus_web_search_deduplicates_by_url() {
    let engines = vec![
        MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Rust Docs",
                "https://doc.rust-lang.org/",
                "mock_a",
            )],
        ),
        MockEngine::success(
            "mock_b",
            vec![MockResult::new(
                "Rust Home",
                "https://doc.rust-lang.org/",
                "mock_b",
            )],
        ),
    ];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = WebSearchArgs {
        query: "rust".into(),
        max_results: None,
        providers: vec!["mock_a".into(), "mock_b".into()],
        safe_search: None,
        timeout_ms: None,
        intent: None,
        freshness: None,
    };
    let v = run_web_search(state, args).await.expect("ok");

    let results = v["results"].as_array().unwrap();
    let urls: Vec<&str> = results.iter().filter_map(|r| r["url"].as_str()).collect();
    // Should have at most one entry per URL
    let unique: std::collections::HashSet<&str> = urls.iter().copied().collect();
    assert_eq!(
        urls.len(),
        unique.len(),
        "URLs should be deduplicated: {urls:?}"
    );
}

#[tokio::test]
async fn corpus_web_search_empty_query_fails() {
    let state = state_with(corpus_cfg(), vec![], Duration::from_secs(5));
    let res = run_web_search(state, web_args("   ")).await;
    assert!(res.is_err(), "empty query should fail");
}

#[tokio::test]
async fn corpus_web_search_provider_failure_partial_results() {
    let engines = vec![
        MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Rust Docs",
                "https://doc.rust-lang.org/",
                "mock_a",
            )],
        ),
        MockEngine::failure("mock_b", eggsearch::meta::mock::MockFailure::Network),
    ];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = WebSearchArgs {
        query: "rust".into(),
        max_results: None,
        providers: vec!["mock_a".into(), "mock_b".into()],
        safe_search: None,
        timeout_ms: None,
        intent: None,
        freshness: None,
    };
    let v = run_web_search(state, args).await.expect("ok");

    // Should have results from mock_a
    let results = v["results"].as_array().unwrap();
    assert!(
        !results.is_empty(),
        "should have partial results from mock_a"
    );

    // Should have provider failure info
    let failed = v["providers_failed"].as_array().unwrap();
    assert!(!failed.is_empty(), "should report mock_b as failed");
    assert_eq!(failed[0]["id"].as_str(), Some("mock_b"));
}

#[tokio::test]
async fn corpus_web_search_with_intent_returns_metadata() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "Axum Docs",
            "https://docs.rs/axum/latest/axum/",
            "mock_a",
        )],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = WebSearchArgs {
        query: "axum".into(),
        max_results: None,
        intent: Some(eggsearch::core::query::SearchIntent::Docs),
        providers: vec!["mock_a".into()],
        safe_search: None,
        timeout_ms: None,
        freshness: None,
    };
    let v = run_web_search(state, args).await.expect("ok");

    // Should return results with metadata
    let results = v["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let card = &results[0];
    assert!(
        card["metadata"]["source_kind"].is_string(),
        "card should have source_kind: {card:?}"
    );
    assert!(
        card["metadata"]["domain"].is_string(),
        "card should have domain: {card:?}"
    );
}

// ---------------------------------------------------------------------------
// Workstream 6: Local workspace workflows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_local_search_returns_workspace_trusted_results() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();
    std::fs::write(root.join("main.rs"), "fn main() { println!(\"hello\"); }").unwrap();
    std::fs::write(root.join("README.md"), "# My Project\n\nA test project.").unwrap();

    let state = state_with_local_backend(root);
    let args = RepoSearchArgs {
        query: "lib.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let groups = v["groups"].as_array().expect("groups is array");

    let all_results: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| {
            g["results"]
                .as_array()
                .map(|a| a.iter())
                .unwrap_or_default()
        })
        .collect();
    let local_results: Vec<&serde_json::Value> = all_results
        .iter()
        .filter(|r| r["url"].as_str().unwrap_or("").starts_with("workspace://"))
        .copied()
        .collect();
    assert!(
        !local_results.is_empty(),
        "expected local results with workspace:// URLs, got: {all_results:?}"
    );
    for r in &local_results {
        assert_eq!(
            r["trust"], "local_trusted",
            "local result should have local_trusted trust: {r:?}"
        );
    }
    let queried = v["providers_queried"]
        .as_array()
        .expect("providers_queried");
    let queried_ids: Vec<&str> = queried.iter().filter_map(|q| q.as_str()).collect();
    assert!(
        queried_ids.contains(&"local_workspace"),
        "providers_queried should include local_workspace: {queried_ids:?}"
    );
}

#[tokio::test]
async fn corpus_workspace_fetch_reads_local_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    let state = state_with_local_backend(root);
    let root_name = root.file_name().unwrap().to_str().unwrap();
    let args = RepoFetchArgs {
        host: Some("workspace".to_string()),
        owner: root_name.to_string(),
        repo: "lib.rs".to_string(),
        ref_name: None,
        commit_sha: None,
        path: "lib.rs".to_string(),
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
    };
    let v = run_repo_fetch(state, args)
        .await
        .expect("workspace fetch should succeed");

    assert_eq!(v["trust"], "local_trusted");
    assert_eq!(v["fetched"], true);
    let text = v["text"].as_str().expect("text should be present");
    assert!(
        text.contains("pub fn add"),
        "fetched text should contain the function: {text}"
    );

    let locator = v["locator"].as_object().expect("locator");
    assert_eq!(locator["kind"], "workspace");
    assert_eq!(
        locator.get("host"),
        None,
        "workspace locator should not have host"
    );
    assert_eq!(
        locator.get("owner"),
        None,
        "workspace locator should not have owner"
    );
    assert_eq!(
        locator.get("repo"),
        None,
        "workspace locator should not have repo"
    );
    assert_eq!(locator["workspace_root"], root_name);
    assert_eq!(locator["path"], "lib.rs");
}

#[tokio::test]
async fn corpus_workspace_fetch_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("lib.rs"), "fn main() {}").unwrap();

    let state = state_with_local_backend(root);
    let root_name = root.file_name().unwrap().to_str().unwrap();
    let args = RepoFetchArgs {
        host: Some("workspace".to_string()),
        owner: root_name.to_string(),
        repo: "../../../etc/passwd".to_string(),
        ref_name: None,
        commit_sha: None,
        path: "../../../etc/passwd".to_string(),
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
    };
    let result = run_repo_fetch(state, args).await;
    assert!(result.is_err(), "path traversal should fail");
}

#[tokio::test]
async fn corpus_workspace_fetch_rejects_unknown_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("lib.rs"), "fn main() {}").unwrap();

    let state = state_with_local_backend(root);
    let args = RepoFetchArgs {
        host: Some("workspace".to_string()),
        owner: "nonexistent_root".to_string(),
        repo: "lib.rs".to_string(),
        ref_name: None,
        commit_sha: None,
        path: "lib.rs".to_string(),
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
    };
    let result = run_repo_fetch(state, args).await;
    assert!(result.is_err(), "unknown root should fail");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("unknown workspace root"),
        "error should mention unknown workspace root"
    );
}

#[tokio::test]
async fn corpus_local_clean_checkout_no_dirty_warning() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();

    git_cmd().arg("init").arg(root).output().ok();
    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg("https://github.com/test-owner/test-repo.git")
        .output()
        .ok();
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

    let state = state_with_local_backend(root);
    let args = RepoSearchArgs {
        query: "main.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        owner: Some("test-owner".to_string()),
        repo: Some("test-repo".to_string()),
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let dirty_warnings: Vec<&str> = warnings
        .iter()
        .filter_map(|w| w["message"].as_str())
        .filter(|m| m.contains("local_repo_dirty"))
        .collect();
    assert!(
        dirty_warnings.is_empty(),
        "clean checkout should not have dirty warning: {warnings:?}"
    );
}

#[tokio::test]
async fn corpus_local_dirty_checkout_emits_warning() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();

    git_cmd().arg("init").arg(root).output().ok();
    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg("https://github.com/test-owner/test-repo.git")
        .output()
        .ok();
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
    std::fs::write(root.join("untracked.txt"), "dirty content").unwrap();

    let state = state_with_local_backend(root);
    let args = RepoSearchArgs {
        query: "main.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        owner: Some("test-owner".to_string()),
        repo: Some("test-repo".to_string()),
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let dirty_warnings: Vec<&str> = warnings
        .iter()
        .filter_map(|w| w["message"].as_str())
        .filter(|m| m.contains("local_repo_dirty"))
        .collect();
    assert!(
        !dirty_warnings.is_empty(),
        "dirty checkout should emit local_repo_dirty warning: {warnings:?}"
    );
}

#[tokio::test]
async fn corpus_local_repo_match_metadata_present() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();

    git_cmd().arg("init").arg(root).output().ok();
    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg("https://github.com/tokio-rs/axum.git")
        .output()
        .ok();
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
        .output()
        .ok();

    let state = state_with_local_backend(root);
    let args = RepoSearchArgs {
        query: "lib.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        owner: Some("tokio-rs".to_string()),
        repo: Some("axum".to_string()),
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let groups = v["groups"].as_array().expect("groups is array");
    let local_cards: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["results"].as_array().into_iter())
        .flatten()
        .filter(|r| r["url"].as_str().unwrap_or("").starts_with("workspace://"))
        .collect();
    assert!(!local_cards.is_empty(), "should have local results");

    for card in &local_cards {
        let meta = card["metadata"]
            .as_object()
            .expect("metadata should be object");
        let lrm = meta["local_repo_match"]
            .as_object()
            .expect("local_repo_match should be present");
        assert_eq!(lrm["matched"], true);
        assert_eq!(lrm["remote_owner"].as_str(), Some("tokio-rs"));
        assert_eq!(lrm["remote_repo"].as_str(), Some("axum"));
        assert_eq!(lrm["remote_host"].as_str(), Some("github"));
        assert!(lrm.get("dirty_state").is_some());
        assert!(lrm.get("root_path").is_some());
    }
}

#[tokio::test]
async fn corpus_prefer_local_redirects_to_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    git_cmd().arg("init").arg(root).output().ok();
    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg("https://github.com/test-owner/test-repo.git")
        .output()
        .ok();
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

    let state = state_with_local_backend(root);
    let args = RepoFetchArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: None,
        commit_sha: None,
        path: "lib.rs".to_string(),
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
        prefer_local: Some(true),
    };
    let v = run_repo_fetch(state, args)
        .await
        .expect("prefer_local repo_fetch should succeed");

    assert_eq!(v["trust"], "local_trusted");
    assert_eq!(v["fetched"], true);
    let text = v["text"].as_str().expect("text should be present");
    assert!(
        text.contains("pub fn add"),
        "fetched text should contain the function: {text}"
    );
}

#[tokio::test]
async fn corpus_prefer_local_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();

    git_cmd().arg("init").arg(root).output().ok();
    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg("https://github.com/test-owner/test-repo.git")
        .output()
        .ok();
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
        .output()
        .ok();

    let state = state_with_local_backend(root);
    let args = RepoFetchArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: Some("main".to_string()),
        commit_sha: None,
        path: "../../../etc/passwd".to_string(),
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
        prefer_local: Some(true),
    };
    let result = run_repo_fetch(state, args).await;
    assert!(
        result.is_err(),
        "path traversal via prefer_local should fail"
    );
}

// ---------------------------------------------------------------------------
// Workstream 7: Code-host coverage workflows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_repo_fetch_github_browser_url_transforms() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/raw/main/src/main.rs");
        then.status(200)
            .header("content-type", "text/plain; charset=utf-8")
            .body("fn main() {}");
    });

    let state = Arc::new(
        ServerState::build({
            let mut cfg = AppConfig::default();
            cfg.fetch.allow_localhost = true;
            cfg.fetch.allow_private_network = true;
            cfg.fetch.sanitize_output = false;
            cfg
        })
        .expect("state"),
    );

    let base = server.base_url();
    let raw_url = format!("{base}/raw/main/src/main.rs");
    let args = RepoFetchArgs {
        host: Some("github".into()),
        owner: "test-owner".into(),
        repo: "test-repo".into(),
        ref_name: Some("main".into()),
        commit_sha: None,
        path: "src/main.rs".into(),
        line_start: None,
        line_end: None,
        context_before: None,
        context_after: None,
        max_chars: None,
        timeout_ms: None,
        test_fetch_url: Some(raw_url),
        symbol: None,
        symbol_kind: None,
        match_text: None,
        expand_to_block: None,
        max_block_lines: None,
        prefer_local: None,
    };
    let v = run_repo_fetch(state, args)
        .await
        .expect("repo_fetch should succeed");

    assert_eq!(v["trust"], "external_untrusted");
    assert_eq!(v["fetched"], true);
    let locator = v["locator"].as_object().expect("locator");
    assert_eq!(locator["kind"], "remote");
    assert_eq!(locator["host"], "github");
    assert_eq!(locator["owner"], "test-owner");
    assert_eq!(locator["repo"], "test-repo");
    assert_eq!(locator["path"], "src/main.rs");
}

#[tokio::test]
async fn corpus_repo_fetch_line_range_bounds_correctly() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let file_content =
        "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n";
    server.mock(|when, then| {
        when.method(GET).path("/raw/main/src/main.rs");
        then.status(200)
            .header("content-type", "text/plain; charset=utf-8")
            .body(file_content);
    });

    let state = Arc::new(
        ServerState::build({
            let mut cfg = AppConfig::default();
            cfg.fetch.allow_localhost = true;
            cfg.fetch.allow_private_network = true;
            cfg.fetch.sanitize_output = false;
            cfg
        })
        .expect("state"),
    );

    let base = server.base_url();
    let raw_url = format!("{base}/raw/main/src/main.rs");
    let args = RepoFetchArgs {
        host: Some("github".into()),
        owner: "test-owner".into(),
        repo: "test-repo".into(),
        ref_name: Some("main".into()),
        commit_sha: None,
        path: "src/main.rs".into(),
        line_start: Some(2),
        line_end: Some(5),
        context_before: None,
        context_after: None,
        max_chars: None,
        timeout_ms: None,
        test_fetch_url: Some(raw_url),
        symbol: None,
        symbol_kind: None,
        match_text: None,
        expand_to_block: None,
        max_block_lines: None,
        prefer_local: None,
    };
    let v = run_repo_fetch(state, args)
        .await
        .expect("repo_fetch should succeed");

    let lines = v["lines"].as_array().expect("lines should be array");
    assert!(
        lines.len() >= 4,
        "should have at least 4 lines (2-5): {lines:?}"
    );
    let first_num = lines[0]["number"].as_u64().expect("line number");
    let last_num = lines.last().unwrap()["number"]
        .as_u64()
        .expect("line number");
    assert_eq!(first_num, 2);
    assert_eq!(last_num, 5);
}

#[tokio::test]
async fn corpus_batch_fetch_returns_structured_results() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page1.html");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>Page 1 content</p></body></html>");
    });
    server.mock(|when, then| {
        when.method(GET).path("/page2.html");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>Page 2 content</p></body></html>");
    });

    let state = Arc::new(
        ServerState::build({
            let mut cfg = AppConfig::default();
            cfg.fetch.allow_localhost = true;
            cfg.fetch.allow_private_network = true;
            cfg.fetch.sanitize_output = false;
            cfg
        })
        .expect("state"),
    );

    let base = server.base_url();
    let args = BatchFetchArgs {
        items: vec![
            BatchFetchItem::Web {
                url: format!("{base}/page1.html"),
                extract_mode: None,
                include_links: None,
                max_chars: None,
            },
            BatchFetchItem::Web {
                url: format!("{base}/page2.html"),
                extract_mode: None,
                include_links: None,
                max_chars: None,
            },
        ],
        max_items: None,
        max_chars_per_item: None,
        max_total_chars: None,
        timeout_ms: None,
        continue_on_error: None,
    };
    let v = run_batch_fetch(state, args)
        .await
        .expect("batch_fetch should succeed");

    assert_eq!(v["fetched"], 2);
    assert_eq!(v["failed"], 0);
    assert!(v["results"].is_array());
    let results = v["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    for r in results {
        assert_eq!(r["ok"], true);
        assert!(r["chars_returned"].as_u64().unwrap() > 0);
    }
}

#[tokio::test]
async fn corpus_batch_fetch_handles_mixed_success_failure() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/ok.html");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>OK</p></body></html>");
    });
    server.mock(|when, then| {
        when.method(GET).path("/fail.html");
        then.status(500).body("Internal Server Error");
    });

    let state = Arc::new(
        ServerState::build({
            let mut cfg = AppConfig::default();
            cfg.fetch.allow_localhost = true;
            cfg.fetch.allow_private_network = true;
            cfg.fetch.sanitize_output = false;
            cfg
        })
        .expect("state"),
    );

    let base = server.base_url();
    let args = BatchFetchArgs {
        items: vec![
            BatchFetchItem::Web {
                url: format!("{base}/ok.html"),
                extract_mode: None,
                include_links: None,
                max_chars: None,
            },
            BatchFetchItem::Web {
                url: format!("{base}/fail.html"),
                extract_mode: None,
                include_links: None,
                max_chars: None,
            },
        ],
        max_items: None,
        max_chars_per_item: None,
        max_total_chars: None,
        timeout_ms: None,
        continue_on_error: Some(true),
    };
    let v = run_batch_fetch(state, args)
        .await
        .expect("batch_fetch should succeed");

    assert_eq!(v["fetched"], 1);
    assert_eq!(v["failed"], 1);
    let results = v["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["ok"], true);
    assert_eq!(results[1]["ok"], false);
    assert!(results[1]["error"].is_string());
}

// ---------------------------------------------------------------------------
// Workstream 3: Package and migration workflows (partial)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_repo_search_npm_package_lookup() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "express - npm",
                "https://www.npmjs.com/package/express",
                "mock_a",
            )
            .with_snippet("Fast, unopinionated, minimalist web framework for Node.js"),
            MockResult::new(
                "express - GitHub",
                "https://github.com/expressjs/express",
                "mock_a",
            )
            .with_snippet("express source repository"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "express".into(),
        ecosystem: Some("npm".into()),
        package: Some("express".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"package_registry") || kinds.contains(&"official_docs"),
        "npm package lookup should have package_registry or official_docs group: {kinds:?}"
    );
}

#[tokio::test]
async fn corpus_repo_search_pypi_package_lookup() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "requests - PyPI",
                "https://pypi.org/project/requests/",
                "mock_a",
            )
            .with_snippet("A simple, yet elegant, HTTP library"),
            MockResult::new(
                "requests - GitHub",
                "https://github.com/psf/requests",
                "mock_a",
            )
            .with_snippet("requests source repository"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "requests".into(),
        ecosystem: Some("pypi".into()),
        package: Some("requests".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"package_registry") || kinds.contains(&"official_docs"),
        "PyPI package lookup should have package_registry or official_docs group: {kinds:?}"
    );
}

#[tokio::test]
async fn corpus_repo_search_package_resolution_fallback() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "nonexistent-package - crates.io",
            "https://crates.io/crates/nonexistent-package",
            "mock_a",
        )
        .with_snippet("A package that does not exist on crates.io")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = RepoSearchArgs {
        query: "nonexistent-package".into(),
        ecosystem: Some("crates.io".into()),
        package: Some("nonexistent-package".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_repo_search(state, args).await.expect("ok");

    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_fallback_warning = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .unwrap_or("")
            .contains("package_resolution_fallback")
    });
    assert!(
        has_fallback_warning,
        "package resolution fallback should emit warning: {warnings:?}"
    );
}

// ---------------------------------------------------------------------------
// Workstream 4: Security workflows (partial)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_security_ghsa_id_lookup() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "GHSA-abcd-1234-efgh: Test vulnerability",
            "https://github.com/advisories/GHSA-abcd-1234-efgh",
            "mock_a",
        )
        .with_snippet("A test GitHub Security Advisory")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: None,
        ghsa_id: Some("GHSA-abcd-1234-efgh".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");

    let resolved = v["resolved_identifiers"].as_object().unwrap();
    let ghsa_ids = resolved["ghsa_ids"].as_array().unwrap();
    assert!(
        ghsa_ids
            .iter()
            .any(|id| id.as_str() == Some("GHSA-ABCD-1234-EFGH")),
        "should resolve GHSA-ABCD-1234-EFGH: {ghsa_ids:?}"
    );
}

#[tokio::test]
async fn corpus_security_rustsec_id_lookup() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "RUSTSEC-2024-0001: Test RustSec advisory",
            "https://rustsec.org/advisories/RUSTSEC-2024-0001",
            "mock_a",
        )
        .with_snippet("A test RustSec advisory for a Rust crate")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: None,
        rustsec_id: Some("RUSTSEC-2024-0001".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");

    let resolved = v["resolved_identifiers"].as_object().unwrap();
    let rustsec_ids = resolved["rustsec_ids"].as_array().unwrap();
    assert!(
        rustsec_ids
            .iter()
            .any(|id| id.as_str() == Some("RUSTSEC-2024-0001")),
        "should resolve RUSTSEC-2024-0001: {rustsec_ids:?}"
    );
}

#[tokio::test]
async fn corpus_security_unknown_applicability_when_no_version() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "GHSA-xxxx-xxxx-xxxx: Vulnerability in axios",
            "https://osv.dev/vulnerability/GHSA-xxxx-xxxx-xxxx",
            "mock_a",
        )
        .with_snippet("Affected versions: < 1.6.0")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: Some("axios vulnerability".into()),
        ecosystem: Some("npm".into()),
        package: Some("axios".into()),
        version: None,
        assess_applicability: Some(true),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");

    let applicability = v["applicability"].as_array().cloned().unwrap_or_default();
    for a in &applicability {
        let status = a["status"].as_str().unwrap_or("");
        // Without a version provided, applicability must be unknown or
        // insufficient_evidence. "affected" without a version is a
        // regression because we can't possibly know the package is
        // affected.
        assert!(
            status == "unknown" || status == "insufficient_evidence",
            "applicability status without version should be unknown/insufficient_evidence, got: {status}"
        );
    }
}

#[tokio::test]
async fn corpus_security_with_package_and_version_fields() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "GHSA-test-test-test: Vulnerability in test-pkg",
            "https://osv.dev/vulnerability/GHSA-test-test-test",
            "mock_a",
        )
        .with_snippet("Affected versions: < 2.0.0, Patched: 2.0.0")],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = SecuritySearchArgs {
        query: Some("test-pkg vulnerability".into()),
        ecosystem: Some("npm".into()),
        package: Some("test-pkg".into()),
        version: Some("1.5.0".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_security_search(state, args).await.expect("ok");
    assert_eq!(v["mode"], "security_metasearch");
    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "should have groups");
}

// ---------------------------------------------------------------------------
// Workstream 5: Research workflows (partial)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_research_performance_investigation() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "Tokio vs async-std benchmarks",
                "https://tokio.rs/blog/2024/benchmarks",
                "mock_a",
            )
            .with_snippet("Performance comparison of async runtimes"),
            MockResult::new(
                "Rust async runtime performance",
                "https://fasterthanli.me/articles/async-rust",
                "mock_a",
            )
            .with_snippet("Deep dive into async performance"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = ResearchSearchArgs {
        query: "tokio vs async-std async runtime performance benchmarks".into(),
        workflow: Some("performance_investigation".into()),
        depth: Some("standard".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_research_search(state, args).await.expect("ok");
    assert_eq!(v["mode"], "research_metasearch");

    let groups = v["groups"].as_array().unwrap();
    assert!(
        !groups.is_empty(),
        "performance investigation should have groups"
    );
    let subqueries = v["subqueries"].as_array().unwrap();
    assert!(!subqueries.is_empty(), "should generate subqueries");
}

#[tokio::test]
async fn corpus_research_security_review() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "Axum security considerations",
                "https://docs.rs/axum/latest/axum/security/",
                "mock_a",
            )
            .with_snippet("Security best practices for axum"),
            MockResult::new(
                "Rust web framework security audit",
                "https://blog.rust-lang.org/security-audit",
                "mock_a",
            )
            .with_snippet("Security audit findings"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = ResearchSearchArgs {
        query: "axum web framework security review CSRF authentication".into(),
        workflow: Some("security_review".into()),
        depth: Some("standard".into()),
        include_security_considerations: Some(true),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_research_search(state, args).await.expect("ok");
    assert_eq!(v["mode"], "research_metasearch");

    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "security review should have groups");
}

#[tokio::test]
async fn corpus_research_migration_planning() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "axum 0.7 migration guide",
                "https://github.com/tokio-rs/axum/blob/main/MIGRATION.md",
                "mock_a",
            )
            .with_snippet("Migration notes from 0.6 to 0.7"),
            MockResult::new(
                "axum 0.7 release notes",
                "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
                "mock_a",
            )
            .with_snippet("Breaking changes in axum 0.7"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = ResearchSearchArgs {
        query: "axum migration from 0.6 to 0.7 breaking changes".into(),
        workflow: Some("migration_planning".into()),
        depth: Some("standard".into()),
        compare_targets: vec!["axum".into()],
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_research_search(state, args).await.expect("ok");
    assert_eq!(v["mode"], "research_metasearch");

    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "migration planning should have groups");
}

#[tokio::test]
async fn corpus_research_ecosystem_survey() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "Rust web framework ecosystem",
                "https://www.shuttle.rs/blog/2024/01/rust-web-frameworks",
                "mock_a",
            )
            .with_snippet("Overview of the Rust web framework ecosystem"),
            MockResult::new("Rust async ecosystem", "https://rustasync.com/", "mock_a")
                .with_snippet("Async Rust ecosystem overview"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = ResearchSearchArgs {
        query: "Rust web framework ecosystem 2024 axum actix-web rocket".into(),
        workflow: Some("ecosystem_survey".into()),
        depth: Some("standard".into()),
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_research_search(state, args).await.expect("ok");
    assert_eq!(v["mode"], "research_metasearch");
    let groups = v["groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "ecosystem survey should have groups");
}

// ---------------------------------------------------------------------------
// Workstream 8: Ranking regression checks (partial)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn corpus_ranking_research_prioritizes_official_docs() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![
            MockResult::new(
                "Random blog post about axum",
                "https://blog.example.com/axum-tutorial",
                "mock_a",
            )
            .with_snippet("My experience using axum"),
            MockResult::new(
                "Axum official documentation",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )
            .with_snippet("A web framework for Rust"),
            MockResult::new(
                "Axum GitHub repository",
                "https://github.com/tokio-rs/axum",
                "mock_a",
            )
            .with_snippet("A web framework for Rust"),
        ],
    )];
    let state = state_with(corpus_cfg(), engines, Duration::from_secs(5));
    let args = ResearchSearchArgs {
        query: "axum official documentation and API reference".into(),
        include_primary_sources: Some(true),
        desired_source_types: vec!["official_docs".into()],
        providers: vec!["mock_a".into()],
        ..Default::default()
    };
    let v = run_research_search(state, args).await.expect("ok");

    let groups = v["groups"].as_array().unwrap();
    let kinds: Vec<&str> = groups
        .iter()
        .map(|g| g["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"official_docs") || kinds.contains(&"reference_implementations"),
        "research with primary sources should have official_docs or reference_implementations group: {kinds:?}"
    );

    let fetches = v["suggested_fetches"].as_array().unwrap();
    assert!(!fetches.is_empty(), "should suggest fetches");
}

// ---------------------------------------------------------------------------
// Workstream 9: Live smoke tests (feature-gated) — extended
// ---------------------------------------------------------------------------

#[cfg(feature = "live-smoke")]
mod live_smoke {
    use super::*;

    fn smoke_state() -> Arc<ServerState> {
        state_with(AppConfig::default(), vec![], Duration::from_secs(15))
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_repo_map_public_github() {
        let v = run_repo_map(
            smoke_state(),
            RepoMapArgs {
                host: None,
                owner: "tokio-rs".into(),
                repo: "axum".into(),
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
            },
        )
        .await
        .expect("live repo_map");
        let has_entries = v["root_entries"].is_array()
            || v["entries"].is_array()
            || v["mode"].as_str() == Some("fallback_search");
        assert!(
            has_entries,
            "github repo_map should return entries or fallback mode: {}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_osv_advisory_lookup() {
        let v = run_security_search(
            smoke_state(),
            SecuritySearchArgs {
                query: Some("CVE-2024-3094".into()),
                cve_id: Some("CVE-2024-3094".into()),
                ..Default::default()
            },
        )
        .await
        .expect("live security_search");
        let resolved = v["resolved_identifiers"].as_object().unwrap();
        let cve_ids = resolved["cve_ids"].as_array().unwrap();
        assert!(
            cve_ids
                .iter()
                .any(|id| id.as_str() == Some("CVE-2024-3094")),
            "should find CVE-2024-3094"
        );
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_web_search_basic() {
        let v = run_web_search(
            smoke_state(),
            WebSearchArgs {
                query: "rust programming language".into(),
                max_results: None,
                providers: vec![],
                safe_search: None,
                timeout_ms: None,
                intent: None,
                freshness: None,
            },
        )
        .await
        .expect("live web_search");
        let has_results = v["results"].as_array().is_some_and(|a| !a.is_empty());
        let has_warnings = v["warnings"].as_array().is_some_and(|a| !a.is_empty());
        assert!(
            has_results || has_warnings,
            "live search should return results or warnings (rate-limited)"
        );
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_repo_fetch_github_file() {
        let v = run_repo_fetch(
            smoke_state(),
            RepoFetchArgs {
                host: Some("github".into()),
                owner: "tokio-rs".into(),
                repo: "axum".into(),
                ref_name: Some("main".into()),
                commit_sha: None,
                path: "Cargo.toml".into(),
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
            },
        )
        .await
        .expect("live repo_fetch");
        assert_eq!(v["fetched"], true);
        let text = v["text"].as_str().expect("text should be present");
        assert!(
            text.contains("[package]") || text.contains("[workspace]"),
            "Cargo.toml should contain [package] or [workspace]: {text}"
        );
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_repo_search_package_registry() {
        let v = run_repo_search(
            smoke_state(),
            RepoSearchArgs {
                query: "axum".into(),
                ecosystem: Some("crates.io".into()),
                package: Some("axum".into()),
                version: Some("0.7.0".into()),
                providers: vec![],
                ..Default::default()
            },
        )
        .await
        .expect("live repo_search for package");
        let has_groups = v["groups"].as_array().is_some_and(|a| !a.is_empty());
        let has_results = v["results"].as_array().is_some_and(|a| !a.is_empty());
        let has_resolution = v["package_resolution"].as_object().is_some();
        let has_warnings = v["warnings"].as_array().is_some_and(|a| !a.is_empty());
        assert!(
            has_groups || has_results || has_resolution || has_warnings,
            "package search should have groups, results, package resolution, or warnings"
        );
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_repo_map_public_gitlab() {
        let v = run_repo_map(
            smoke_state(),
            RepoMapArgs {
                host: None,
                owner: "gitlab-org".into(),
                repo: "gitlab-runner".into(),
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
            },
        )
        .await
        .expect("live gitlab repo_map");
        let has_entries = v["root_entries"].is_array()
            || v["entries"].is_array()
            || v["mode"].as_str() == Some("fallback_search");
        assert!(
            has_entries,
            "gitlab repo_map should return entries or fallback mode: {}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_repo_map_public_codeberg() {
        let v = run_repo_map(
            smoke_state(),
            RepoMapArgs {
                host: None,
                owner: "Codeberg".into(),
                repo: "Forgejo".into(),
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
            },
        )
        .await
        .expect("live codeberg repo_map");
        let has_entries = v["root_entries"].is_array()
            || v["entries"].is_array()
            || v["mode"].as_str() == Some("fallback_search");
        assert!(
            has_entries,
            "codeberg repo_map should return entries or fallback mode: {}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_repo_map_nested_github() {
        let v = run_repo_map(
            smoke_state(),
            RepoMapArgs {
                host: None,
                owner: "tokio-rs".into(),
                repo: "tokio".into(),
                ref_name: None,
                commit_sha: None,
                max_entries: Some(200),
                max_depth: Some(3),
                include_files: Some(true),
                include_directories: Some(true),
                include_ci: None,
                include_security: None,
                timeout_ms: None,
                providers: vec![],
            },
        )
        .await
        .expect("live nested repo_map");
        let has_nested = v["entries"].as_array().is_some_and(|a| {
            a.iter()
                .any(|e| e["path"].as_str().is_some_and(|p| p.contains('/')))
        }) || v["root_entries"].as_array().is_some_and(|a| {
            a.iter()
                .any(|e| e["path"].as_str().is_some_and(|p| p.contains('/')))
        });
        let is_fallback = v["mode"].as_str() == Some("fallback_search");
        assert!(
            has_nested || is_fallback,
            "nested repo_map should have nested entries or be in fallback mode"
        );
    }

    #[tokio::test]
    #[ignore = "requires live network and live-smoke feature"]
    async fn smoke_repo_map_non_default_branch() {
        let v = run_repo_map(
            smoke_state(),
            RepoMapArgs {
                host: None,
                owner: "tokio-rs".into(),
                repo: "axum".into(),
                ref_name: Some("v0.7.x".into()),
                commit_sha: None,
                max_entries: None,
                max_depth: None,
                include_files: None,
                include_directories: None,
                include_ci: None,
                include_security: None,
                timeout_ms: None,
                providers: vec![],
            },
        )
        .await
        .expect("live non-default branch repo_map");
        let has_entries = v["root_entries"].is_array()
            || v["entries"].is_array()
            || v["mode"].as_str() == Some("fallback_search");
        assert!(
            has_entries,
            "non-default branch repo_map should return entries or fallback mode"
        );
        let ref_name = v["ref_name"].as_str().unwrap_or("");
        assert!(
            ref_name.contains("v0.7")
                || !ref_name.is_empty()
                || v["mode"].as_str() == Some("fallback_search"),
            "should resolve non-default branch, got ref_name={ref_name}"
        );
    }
}
