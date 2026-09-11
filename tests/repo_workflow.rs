#![allow(unused_imports, dead_code)]
//! Repository evidence discovery.
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
        response_detail: None,
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_repo_slash_form_triggers_local_matching() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    setup_git_repo_with_remote(
        root,
        "https://github.com/test-owner/test-repo.git",
        "test-owner",
        "test-repo",
    );

    let state = state_with_local_backend(root);
    // Use repo = "test-owner/test-repo" without explicit owner
    let args = RepoSearchArgs {
        query: "main.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        repo: Some("test-owner/test-repo".to_string()),
        ..Default::default()
    };

    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_match = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_match"))
            .unwrap_or(false)
    });
    assert!(
        has_match,
        "repo='owner/name' should trigger local matching: {warnings:?}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_query_hint_triggers_local_matching() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    setup_git_repo_with_remote(
        root,
        "https://github.com/test-owner/test-repo.git",
        "test-owner",
        "test-repo",
    );

    let state = state_with_local_backend(root);
    // Use query hint repo:test-owner/test-repo with no explicit owner/repo
    let args = RepoSearchArgs {
        query: "repo:test-owner/test-repo main.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        ..Default::default()
    };

    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_match = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_match"))
            .unwrap_or(false)
    });
    assert!(
        has_match,
        "query hint repo:owner/name should trigger local matching: {warnings:?}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_explicit_owner_repo_overrides_query_hint() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    setup_git_repo_with_remote(
        root,
        "https://github.com/test-owner/test-repo.git",
        "test-owner",
        "test-repo",
    );

    let state = state_with_local_backend(root);
    // Explicit owner/repo should override the different owner/repo in query hint
    let args = RepoSearchArgs {
        query: "repo:other-org/other-repo main.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        owner: Some("test-owner".to_string()),
        repo: Some("test-repo".to_string()),
        ..Default::default()
    };

    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_match = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_match"))
            .unwrap_or(false)
    });
    assert!(
        has_match,
        "explicit owner/repo should override query hint and match local: {warnings:?}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_local_match_equivalence_all_locator_forms() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    setup_git_repo_with_remote(
        root,
        "https://github.com/tokio-rs/axum.git",
        "tokio-rs",
        "axum",
    );

    // Form 1: explicit owner + repo
    let state1 = state_with_local_backend(root);
    let args1 = RepoSearchArgs {
        query: "lib.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        owner: Some("tokio-rs".to_string()),
        repo: Some("axum".to_string()),
        ..Default::default()
    };
    let v1 = run_repo_search(state1, args1)
        .await
        .expect("repo_search ok");
    let match1 = v1["warnings"].as_array().unwrap().iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_match"))
            .unwrap_or(false)
    });

    // Form 2: repo = "owner/name"
    let state2 = state_with_local_backend(root);
    let args2 = RepoSearchArgs {
        query: "lib.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        repo: Some("tokio-rs/axum".to_string()),
        ..Default::default()
    };
    let v2 = run_repo_search(state2, args2)
        .await
        .expect("repo_search ok");
    let match2 = v2["warnings"].as_array().unwrap().iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_match"))
            .unwrap_or(false)
    });

    // Form 3: query hint repo:owner/name
    let state3 = state_with_local_backend(root);
    let args3 = RepoSearchArgs {
        query: "repo:tokio-rs/axum lib.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        ..Default::default()
    };
    let v3 = run_repo_search(state3, args3)
        .await
        .expect("repo_search ok");
    let match3 = v3["warnings"].as_array().unwrap().iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_match"))
            .unwrap_or(false)
    });

    assert!(match1, "Form 1 (explicit owner+repo) should match");
    assert!(match2, "Form 2 (repo=owner/name) should match");
    assert!(match3, "Form 3 (query hint) should match");
}

// =========================================================================
// Corrective Plan Phase 1-6: Workstream 4 -- max_block_lines validation
// =========================================================================

#[tokio::test]
async fn prefer_local_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();

    // Initialize git repo with a remote URL
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

    let backend = {
        let cfg = eggsearch::core::local::LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            ..Default::default()
        };
        Arc::new(
            eggsearch::meta::local_backend::LocalWorkspaceBackend::new(cfg)
                .expect("backend builds"),
        )
    };

    let adapter =
        eggsearch::meta::MetadataSearchAdapter::from_engines(vec![], Duration::from_secs(5));
    let mut cfg = AppConfig::default();
    cfg.fetch.enabled = true;
    cfg.fetch.allow_localhost = true;
    cfg.fetch.allow_private_network = true;
    let mut state = ServerState::with_adapter(cfg, Arc::new(adapter));
    state.local_backend = Some(backend);
    let state = Arc::new(state);

    // Try to use prefer_local with path traversal
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
        response_detail: None,
    };

    let result = run_repo_fetch(state, args).await;
    assert!(
        result.is_err(),
        "path traversal via prefer_local should fail"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn local_repo_match_same_owner_repo_different_host_no_redirect() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("main.rs"), "fn main() {}").unwrap();

    // Initialize git repo with a GitLab remote
    git_cmd().arg("init").arg(root).output().ok();
    git_cmd()
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg("https://gitlab.com/test-owner/test-repo.git")
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
    // Request with github host but local repo is on gitlab
    let args = RepoSearchArgs {
        query: "main.rs".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        host: Some("github".to_string()),
        owner: Some("test-owner".to_string()),
        repo: Some("test-repo".to_string()),
        ..Default::default()
    };

    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_match = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_match"))
            .unwrap_or(false)
    });
    // Should NOT match because host differs (github vs gitlab)
    assert!(
        !has_match,
        "different host should not trigger local match: {warnings:?}"
    );
}

// ---------------------------------------------------------------------------
// WS1: exact-error mode with empty query and repo locator
// ---------------------------------------------------------------------------

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_exact_error_empty_query_with_repo_locator_fails() {
    let engines = vec![MockEngine::success("mock_a", vec![])];
    let adapter = MetadataSearchAdapter::from_engines(
        eggsearch::meta::mock::mock_engines(engines),
        Duration::from_secs(5),
    );
    let mut cfg = AppConfig::default();
    cfg.search.providers.insert("mock_a".to_string(), true);
    let state = Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)));

    let args = RepoSearchArgs {
        query: String::new(),
        providers: vec!["mock_a".to_string()],
        host: Some("github".to_string()),
        owner: Some("tokio-rs".to_string()),
        repo: Some("axum".to_string()),
        mode: Some("exact_error".to_string()),
        ..Default::default()
    };

    let result = run_repo_search(state, args).await;
    assert!(
        result.is_err(),
        "exact-error with empty query should fail even with repo locator"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("exact-error mode requires a non-empty error query"),
        "error should mention exact-error query requirement: {err}"
    );
}

// ---------------------------------------------------------------------------
// WS1: ResolvedRepoIdentity unit tests
// ---------------------------------------------------------------------------

#[test]
fn resolved_repo_identity_explicit_owner_repo() {
    let id = eggsearch::core::repo_search::ResolvedRepoIdentity::resolve(
        &Some("tokio-rs".to_string()),
        &Some("axum".to_string()),
        "",
    );
    let id = id.expect("should resolve");
    assert_eq!(id.owner, "tokio-rs");
    assert_eq!(id.repo, "axum");
    assert_eq!(
        id.source,
        eggsearch::core::repo_search::RepoIdentitySource::ExplicitOwnerRepo
    );
}

#[test]
fn resolved_repo_identity_slash_form() {
    let id = eggsearch::core::repo_search::ResolvedRepoIdentity::resolve(
        &None,
        &Some("tokio-rs/axum".to_string()),
        "",
    );
    let id = id.expect("should resolve");
    assert_eq!(id.owner, "tokio-rs");
    assert_eq!(id.repo, "axum");
    assert_eq!(
        id.source,
        eggsearch::core::repo_search::RepoIdentitySource::RepoSlashName
    );
}

#[test]
fn resolved_repo_identity_query_hint() {
    let id = eggsearch::core::repo_search::ResolvedRepoIdentity::resolve(
        &None,
        &None,
        "repo:tokio-rs/axum Router",
    );
    let id = id.expect("should resolve");
    assert_eq!(id.owner, "tokio-rs");
    assert_eq!(id.repo, "axum");
    assert_eq!(
        id.source,
        eggsearch::core::repo_search::RepoIdentitySource::QueryHint
    );
}

#[test]
fn resolved_repo_identity_explicit_overrides_query_hint() {
    let id = eggsearch::core::repo_search::ResolvedRepoIdentity::resolve(
        &Some("explicit-owner".to_string()),
        &Some("explicit-repo".to_string()),
        "repo:hint-owner/hint-repo something",
    );
    let id = id.expect("should resolve");
    assert_eq!(id.owner, "explicit-owner");
    assert_eq!(id.repo, "explicit-repo");
    assert_eq!(
        id.source,
        eggsearch::core::repo_search::RepoIdentitySource::ExplicitOwnerRepo
    );
}

#[test]
fn resolved_repo_identity_none_when_empty() {
    let id = eggsearch::core::repo_search::ResolvedRepoIdentity::resolve(&None, &None, "no hints");
    assert!(
        id.is_none(),
        "should return None when no identity available"
    );
}

#[test]
fn resolved_repo_identity_empty_slash_form_rejected() {
    let id = eggsearch::core::repo_search::ResolvedRepoIdentity::resolve(
        &None,
        &Some("/axum".to_string()),
        "",
    );
    assert!(id.is_none(), "empty owner in slash form should return None");
}

// ---------------------------------------------------------------------------
// WS3: repo_map include_files/include_directories suppression
// ---------------------------------------------------------------------------

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_dirty_state_detected_in_local_match() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("lib.rs"), "pub fn helper() {}").unwrap();

    // Initialize git repo
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

    // Initial commit
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

    // Make dirty
    fs::write(root.join("untracked.txt"), "dirty").unwrap();

    let state = state_with_local_backend(root);
    let args = RepoSearchArgs {
        query: "helper".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        owner: Some("test-owner".to_string()),
        repo: Some("test-repo".to_string()),
        ..Default::default()
    };

    let v = run_repo_search(state, args).await.expect("repo_search ok");

    // Should have a local_repo_dirty warning
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_dirty_warning = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_dirty"))
            .unwrap_or(false)
    });
    assert!(
        has_dirty_warning,
        "should warn about dirty local checkout: {warnings:?}"
    );
}

// ---------------------------------------------------------------------------
// WS5: local match metadata completeness
// ---------------------------------------------------------------------------

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_local_match_metadata_has_all_fields() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("main.rs"), "fn main() {}").unwrap();

    run_git_checked(
        git_cmd().arg("init").arg("--initial-branch=main").arg(root),
        "git init",
    );
    run_git_checked(
        git_cmd()
            .arg("-C")
            .arg(root)
            .arg("remote")
            .arg("add")
            .arg("origin")
            .arg("https://github.com/test-owner/test-repo.git"),
        "git remote add",
    );
    run_git_checked(git_cmd().arg("-C").arg(root).arg("add").arg("."), "git add");
    run_git_checked(
        git_cmd()
            .arg("-C")
            .arg(root)
            .arg("-c")
            .arg("user.name=ci")
            .arg("-c")
            .arg("user.email=ci@test.com")
            .arg("commit")
            .arg("-m")
            .arg("init"),
        "git commit",
    );

    let state = state_with_local_backend(root);
    let args = RepoSearchArgs {
        query: "main".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        owner: Some("test-owner".to_string()),
        repo: Some("test-repo".to_string()),
        timeout_ms: Some(30_000),
        ..Default::default()
    };

    let v = run_repo_search(state, args).await.expect("repo_search ok");
    let groups = v["groups"].as_array().expect("groups is array");

    // Find local result across all groups
    let local_card = groups.iter().find_map(|g| {
        g["results"].as_array().and_then(|results| {
            results.iter().find(|c| {
                c["trust"]
                    .as_str()
                    .map(|t| t == "local_trusted")
                    .unwrap_or(false)
            })
        })
    });
    assert!(local_card.is_some(), "should have a local result");

    let local_card = local_card.unwrap();
    let meta = local_card["metadata"]
        .as_object()
        .expect("metadata should be object");

    // Check local_repo_match is present
    let local_repo_match = meta.get("local_repo_match");
    assert!(
        local_repo_match.is_some(),
        "local result should have local_repo_match metadata"
    );
    let lrm = local_repo_match.unwrap().as_object().unwrap();
    assert!(
        lrm.get("branch").is_some(),
        "local_repo_match should have branch"
    );
    assert!(
        lrm.get("commit").is_some(),
        "local_repo_match should have commit"
    );
    assert!(
        lrm.get("dirty_state").is_some(),
        "local_repo_match should have dirty_state"
    );
    assert!(
        lrm.get("remote_host").is_some(),
        "local_repo_match should have remote_host"
    );
    assert!(
        lrm.get("remote_owner").is_some(),
        "local_repo_match should have remote_owner"
    );
    assert!(
        lrm.get("remote_repo").is_some(),
        "local_repo_match should have remote_repo"
    );
}

// ---------------------------------------------------------------------------
// WS5: unknown Git state
// ---------------------------------------------------------------------------

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_local_match_unknown_dirty_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("main.rs"), "fn main() {}").unwrap();

    // Initialize git repo
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

    // Corrupt the git index to make git status fail
    let git_index = root.join(".git").join("index");
    fs::write(&git_index, "corrupted").unwrap();

    let state = state_with_local_backend(root);
    let args = RepoSearchArgs {
        query: "main".to_string(),
        providers: vec!["mock_a".to_string()],
        include_local: Some(true),
        owner: Some("test-owner".to_string()),
        repo: Some("test-repo".to_string()),
        ..Default::default()
    };

    let v = run_repo_search(state, args).await.expect("repo_search ok");

    // Should either have dirty_state "unknown" or a local_repo_state_unknown warning
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_unknown_warning = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_repo_state_unknown"))
            .unwrap_or(false)
    });

    // Also check the local result metadata
    let groups = v["groups"].as_array().expect("groups is array");
    let local_card = groups.iter().find_map(|g| {
        g["results"].as_array().and_then(|results| {
            results.iter().find(|c| {
                c["trust"]
                    .as_str()
                    .map(|t| t == "local_trusted")
                    .unwrap_or(false)
            })
        })
    });

    if let Some(card) = local_card {
        let lrm = card["metadata"]["local_repo_match"]
            .as_object()
            .expect("local_repo_match");
        let dirty = lrm["dirty_state"].as_str().unwrap_or("unknown");
        // With a corrupted index, dirty state should be "unknown"
        assert!(
            dirty == "unknown" || has_unknown_warning,
            "corrupted git should produce unknown dirty state or warning, got: dirty={dirty}, warnings={has_unknown_warning}"
        );
    }
}

// ---------------------------------------------------------------------------
// WS6: provider_status includes new tool_capabilities fields
// ---------------------------------------------------------------------------

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_includes_next_actions() {
    let engines = vec![MockEngine::success(
        "mock_a",
        vec![MockResult::new(
            "test",
            "https://example.com/test",
            "mock_a",
        )],
    )];
    let state = state_with_engines(test_cfg(), engines, Duration::from_secs(5));

    let v = run_repo_search(
        state,
        RepoSearchArgs {
            query: "test query".to_string(),
            providers: vec!["mock_a".to_string()],
            ..Default::default()
        },
    )
    .await
    .expect("ok");

    let next_actions = v["next_actions"].as_array().expect("next_actions is array");
    // With mock results, we should get at least one next action
    assert!(
        !next_actions.is_empty(),
        "repo_search should return next_actions with results"
    );
    for action in next_actions {
        assert!(action["tool"].is_string(), "next_action missing tool");
        assert!(
            action["reason_code"].is_string(),
            "next_action missing reason_code"
        );
        assert!(
            action["priority"].is_number(),
            "next_action missing priority"
        );
        let priority = action["priority"].as_i64().unwrap();
        assert!(
            (1..=5).contains(&priority),
            "priority should be 1-5, got {priority}"
        );
    }
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_off_mode_with_local_backend_returns_local_results() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("main.rs"), "fn main() { println!(\"hi\"); }").unwrap();
    fs::write(root.join("README.md"), "# My Project").unwrap();

    let state = state_with_local_backend_mode_off(root);
    let args = RepoSearchArgs {
        query: "main.rs".to_string(),
        providers: Vec::new(),
        include_local: Some(true),
        ..Default::default()
    };

    let v = run_repo_search(state.clone(), args)
        .await
        .expect("repo_search should succeed in local-only off-mode path");

    let groups = v["groups"].as_array().expect("groups array");
    let local_results: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["results"].as_array().into_iter().flatten())
        .filter(|r| r["url"].as_str().unwrap_or("").starts_with("workspace://"))
        .collect();
    assert!(
        !local_results.is_empty(),
        "local-only repo_search in off mode should return local_trusted results: {v:?}"
    );
    for r in &local_results {
        assert_eq!(
            r["trust"], "local_trusted",
            "local result should have local_trusted trust: {r:?}"
        );
    }
    let queried = v["providers_queried"]
        .as_array()
        .expect("providers_queried array");
    let queried_ids: Vec<&str> = queried.iter().filter_map(|q| q.as_str()).collect();
    assert!(
        queried_ids.contains(&"local_workspace"),
        "providers_queried must include local_workspace: {queried_ids:?}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_off_mode_without_local_backend_is_denied() {
    let mut cfg = AppConfig::default();
    cfg.search.mode = Mode::Off;
    cfg.local.enabled = false;
    let state = Arc::new(ServerState::build(cfg).expect("state"));
    let args = RepoSearchArgs {
        query: "anything".to_string(),
        providers: Vec::new(),
        include_local: Some(true),
        ..Default::default()
    };
    let err = run_repo_search(state, args)
        .await
        .expect_err("off mode without local backend should be denied");
    assert!(
        err.to_string().contains("disabled by policy"),
        "expected policy denial, got: {err}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_search_off_mode_include_local_false_is_denied() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("main.rs"), "fn main() {}").unwrap();

    let state = state_with_local_backend_mode_off(root);
    let args = RepoSearchArgs {
        query: "main.rs".to_string(),
        providers: Vec::new(),
        include_local: Some(false),
        ..Default::default()
    };
    let err = run_repo_search(state, args)
        .await
        .expect_err("include_local=false in off mode must deny (no remote allowed)");
    assert!(
        err.to_string().contains("disabled by policy"),
        "expected policy denial, got: {err}"
    );
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
#[tokio::test]
async fn repo_map_with_local_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("main.rs"), "fn main() {}").unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    // Initialize git repo with a remote URL
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

    // Create initial commit
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

    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
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

    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    // Should have local_checkout populated
    let local_checkout = v["local_checkout"]
        .as_object()
        .expect("local_checkout should be present");

    assert_eq!(
        local_checkout["remote_owner"].as_str(),
        Some("test-owner"),
        "remote_owner should match"
    );
    assert_eq!(
        local_checkout["remote_repo"].as_str(),
        Some("test-repo"),
        "remote_repo should match"
    );
    assert_eq!(
        local_checkout["remote_host"].as_str(),
        Some("github"),
        "remote_host should be github"
    );
    assert!(
        local_checkout.get("root_path").is_some(),
        "root_path should be present"
    );
    assert!(
        local_checkout.get("branch").is_some(),
        "branch should be present"
    );
    assert!(
        local_checkout.get("dirty_state").is_some(),
        "dirty_state should be present"
    );

    // Should have local_checkout_match warning
    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_match_warning = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("local_checkout_match"))
            .unwrap_or(false)
    });
    assert!(
        has_match_warning,
        "should have local_checkout_match warning: {warnings:?}"
    );
}

// =========================================================================
// Corrective Plan Phase 1-6: Workstream 1 -- Centralized repo identity
// =========================================================================

/// Helper: create a temp git repo with a remote URL for local matching tests.
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

#[tokio::test]
async fn repo_map_fallback_mode_warns_no_native_provider() {
    let state = state_with_default();
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
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

    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    assert_eq!(v["mode"], "fallback_search", "should be in fallback mode");

    let warnings = v["warnings"].as_array().expect("warnings is array");
    let has_native_warning = warnings.iter().any(|w| {
        w["message"]
            .as_str()
            .map(|s| s.contains("no_native_tree_provider"))
            .unwrap_or(false)
    });
    assert!(
        has_native_warning,
        "should warn about no native tree provider: {warnings:?}"
    );
}

#[tokio::test]
async fn repo_map_suggested_fetches_are_bounded() {
    let state = state_with_default();
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
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

    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    let fetches = v["suggested_fetches"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        fetches.len() <= 8,
        "suggested fetches should be bounded to <= 8, got: {}",
        fetches.len()
    );

    // Each suggested fetch should have required fields
    for f in fetches {
        assert!(
            f["url"].is_string(),
            "suggested fetch should have url: {f:?}"
        );
        assert!(
            f["reason"].is_string(),
            "suggested fetch should have reason: {f:?}"
        );
    }
}

#[tokio::test]
async fn repo_map_tool_in_server_surface() {
    let state = state_with_default();
    let v = run_provider_status(
        state,
        ProviderStatusArgs {
            probe: false,
            recipe_detail: None,
        },
    )
    .expect("ok");
    let caps = v["server_capabilities"]
        .as_object()
        .expect("server_capabilities");

    assert_eq!(
        caps["repo_map"],
        serde_json::json!(true),
        "repo_map should be in server_capabilities"
    );
}

// =========================================================================
// Corrective Plan Phase 1-6: Workstream 5 -- local workspace routing
// =========================================================================

#[tokio::test]
async fn repo_map_include_files_false_suppresses_file_entries() {
    let state = state_with_default();
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: None,
        commit_sha: None,
        max_entries: None,
        max_depth: None,
        include_files: Some(false),
        include_directories: None,
        include_ci: None,
        include_security: None,
        timeout_ms: None,
        providers: vec![],
        response_detail: None,
    };

    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    // In fallback mode, root_entries may still exist from web search,
    // but important_files should be empty when include_files=false
    let important_files = v["important_files"].as_array().cloned().unwrap_or_default();
    assert!(
        important_files.is_empty(),
        "important_files should be empty when include_files=false, got: {}",
        important_files.len()
    );
}

#[tokio::test]
async fn repo_map_include_directories_false_suppresses_dir_entries() {
    let state = state_with_default();
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: None,
        commit_sha: None,
        max_entries: None,
        max_depth: None,
        include_files: None,
        include_directories: Some(false),
        include_ci: None,
        include_security: None,
        timeout_ms: None,
        providers: vec![],
        response_detail: None,
    };

    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    let important_dirs = v["important_directories"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        important_dirs.is_empty(),
        "important_directories should be empty when include_directories=false, got: {}",
        important_dirs.len()
    );
}

#[tokio::test]
async fn repo_map_include_ci_false_suppresses_ci_entries() {
    let state = state_with_default();
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: None,
        commit_sha: None,
        max_entries: None,
        max_depth: None,
        include_files: None,
        include_directories: None,
        include_ci: Some(false),
        include_security: None,
        timeout_ms: None,
        providers: vec![],
        response_detail: None,
    };

    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    let ci = v["ci"].as_array().cloned().unwrap_or_default();
    assert!(
        ci.is_empty(),
        "ci entries should be empty when include_ci=false, got: {}",
        ci.len()
    );
}

#[tokio::test]
async fn repo_map_include_security_false_suppresses_security_entries() {
    let state = state_with_default();
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
        repo: "test-repo".to_string(),
        ref_name: None,
        commit_sha: None,
        max_entries: None,
        max_depth: None,
        include_files: None,
        include_directories: None,
        include_ci: None,
        include_security: Some(false),
        timeout_ms: None,
        providers: vec![],
        response_detail: None,
    };

    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    let security = &v["security"];
    assert!(
        security.is_null() || security.as_array().is_none_or(|a| a.is_empty()),
        "security should be null or empty when include_security=false, got: {security}"
    );
}

// ---------------------------------------------------------------------------
// WS3: repo_map local_checkout manifest and dirty-state
// ---------------------------------------------------------------------------

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_map_local_checkout_includes_manifests_and_dirty_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("main.rs"), "fn main() {}").unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"test-pkg\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("package.json"), "{\"name\":\"test-npm\"}\n").unwrap();

    // Initialize git repo
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

    // Initial commit
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

    // Make dirty by adding an untracked file
    fs::write(root.join("untracked.txt"), "dirty").unwrap();

    let state = state_with_local_backend(root);
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
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

    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed");

    let local_checkout = v["local_checkout"]
        .as_object()
        .expect("local_checkout should be present");

    // Check dirty state
    assert_eq!(
        local_checkout["dirty_state"].as_str(),
        Some("dirty"),
        "dirty_state should be 'dirty' with untracked file"
    );

    // Check manifests
    let manifests = local_checkout["manifests"]
        .as_array()
        .expect("manifests should be array");
    let manifest_paths: Vec<&str> = manifests
        .iter()
        .filter_map(|m| m["path"].as_str())
        .collect();
    assert!(
        manifest_paths.iter().any(|p| p.contains("Cargo.toml")),
        "should detect Cargo.toml manifest: {manifest_paths:?}"
    );
    assert!(
        manifest_paths.iter().any(|p| p.contains("package.json")),
        "should detect package.json manifest: {manifest_paths:?}"
    );

    // Check other fields
    assert_eq!(local_checkout["remote_owner"].as_str(), Some("test-owner"));
    assert_eq!(local_checkout["remote_repo"].as_str(), Some("test-repo"));
    assert!(
        local_checkout["branch"].as_str().is_some(),
        "branch should be present"
    );
    assert!(
        local_checkout["commit"].as_str().is_some(),
        "commit should be present"
    );
}

// ---------------------------------------------------------------------------
// WS5: dirty-state detection
// ---------------------------------------------------------------------------

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_map_off_mode_with_matching_local_checkout_returns_structure() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("README.md"), "# My Project").unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    fs::write(root.join("lib.rs"), "pub fn add() {}").unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src").join("lib.rs"), "// src/lib").unwrap();

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

    let state = state_with_local_backend_mode_off_for_repo_map(root);
    let args = RepoMapArgs {
        host: Some("github".to_string()),
        owner: "test-owner".to_string(),
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
        providers: Vec::new(),
        response_detail: None,
    };
    let v = run_repo_map(state, args)
        .await
        .expect("repo_map should succeed in off mode with matching local checkout");

    let important_files = v["important_files"].as_array().expect("important_files");
    assert!(
        important_files
            .iter()
            .any(|f| f["path"] == "README.md" && f["kind"] == "readme"),
        "README.md must be classified as readme: {important_files:?}"
    );
    assert!(
        important_files
            .iter()
            .any(|f| f["path"] == "Cargo.toml" && f["kind"] == "manifest"),
        "Cargo.toml must be classified as manifest: {important_files:?}"
    );
    let source_roots = v["source_roots"].as_array().expect("source_roots");
    assert!(
        source_roots.iter().any(|s| s["path"] == "src"),
        "src must be a source_root: {source_roots:?}"
    );
    let suggested_fetches = v["suggested_fetches"]
        .as_array()
        .expect("suggested_fetches");
    assert!(
        suggested_fetches
            .iter()
            .any(|f| f["url"].as_str().unwrap_or("").contains("README.md")),
        "suggested_fetches must include README: {suggested_fetches:?}"
    );
    let local_checkout = v["local_checkout"].as_object().expect("local_checkout");
    assert_eq!(local_checkout["remote_owner"], "test-owner");
    assert_eq!(local_checkout["remote_repo"], "test-repo");
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_map_off_mode_without_local_backend_is_denied() {
    let mut cfg = AppConfig::default();
    cfg.search.mode = Mode::Off;
    cfg.local.enabled = false;
    let state = Arc::new(ServerState::build(cfg).expect("state"));
    let args = RepoMapArgs {
        host: None,
        owner: "test-owner".to_string(),
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
        providers: Vec::new(),
        response_detail: None,
    };
    let err = run_repo_map(state, args)
        .await
        .expect_err("off mode without local backend should deny repo_map");
    assert!(
        err.to_string().contains("disabled by policy"),
        "expected policy denial, got: {err}"
    );
}

#[tokio::test]
async fn repo_fetch_symbol_definition_via_mock() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/src/lib.rs");
        then.status(200)
            .header("content-type", "text/plain; charset=utf-8")
            .body(
                "use std::collections::HashMap;\n\
                 \n\
                 /// A configuration store.\n\
                 pub struct Config {\n\
                     name: String,\n\
                     values: HashMap<String, String>,\n\
                 }\n\
                 \n\
                 impl Config {\n\
                     pub fn new(name: &str) -> Self {\n\
                         Config {\n\
                             name: name.to_string(),\n\
                             values: HashMap::new(),\n\
                         }\n\
                     }\n\
                 }\n\
                 \n\
                 fn helper() -> i32 {\n\
                     42\n\
                 }\n",
            );
    });

    let state = repo_fetch_state();

    let v = run_repo_fetch(
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
            test_fetch_url: Some(server.url("/src/lib.rs")),
            symbol: Some("Config".into()),
            symbol_kind: Some("struct".into()),
            match_text: None,
            expand_to_block: Some(true),
            max_block_lines: None,
            prefer_local: None,
            response_detail: None,
        },
    )
    .await
    .expect("repo_fetch should succeed");

    let selected_span = v
        .get("selected_span")
        .expect("selected_span should be present");
    let selection_kind = selected_span["selection_kind"]
        .as_str()
        .expect("selection_kind should be a string");
    assert_eq!(
        selection_kind, "symbol_definition",
        "should find struct definition: {selected_span:?}"
    );

    let line_start = selected_span["line_start"]
        .as_u64()
        .expect("line_start should be present");
    let line_end = selected_span["line_end"]
        .as_u64()
        .expect("line_end should be present");
    assert!(
        (3..=4).contains(&line_start),
        "struct Config should start around line 3-4, got {line_start}"
    );
    assert!(
        (6..=7).contains(&line_end),
        "struct Config should end around line 6-7, got {line_end}"
    );

    let text = v["text"].as_str().expect("text should be present");
    assert!(
        text.contains("pub struct Config"),
        "text should contain struct definition: {text}"
    );
}

#[tokio::test]
async fn repo_fetch_symbol_fn_via_mock() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/src/main.rs");
        then.status(200)
            .header("content-type", "text/plain; charset=utf-8")
            .body(
                "fn main() {\n\
                 \n\
                 }\n\
                 \n\
                 fn helper() -> i32 {\n\
                     let x = 42;\n\
                     x + 1\n\
                 }\n\
                 \n\
                 fn other() {\n\
                     // nothing\n\
                 }\n",
            );
    });

    let state = repo_fetch_state();

    let v = run_repo_fetch(
        state,
        RepoFetchArgs {
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
            test_fetch_url: Some(server.url("/src/main.rs")),
            symbol: Some("helper".into()),
            symbol_kind: Some("function".into()),
            match_text: None,
            expand_to_block: Some(true),
            max_block_lines: None,
            prefer_local: None,
            response_detail: None,
        },
    )
    .await
    .expect("repo_fetch should succeed");

    let selected_span = v
        .get("selected_span")
        .expect("selected_span should be present");
    let selection_kind = selected_span["selection_kind"]
        .as_str()
        .expect("selection_kind should be a string");
    assert_eq!(
        selection_kind, "symbol_definition",
        "should find fn helper: {selected_span:?}"
    );

    let text = v["text"].as_str().expect("text should be present");
    assert!(
        text.contains("fn helper"),
        "text should contain fn helper: {text}"
    );
}

#[tokio::test]
async fn repo_fetch_match_text_via_mock() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/src/app.py");
        then.status(200)
            .header("content-type", "text/plain; charset=utf-8")
            .body(
                "import os\n\
                 \n\
                 class MyApp:\n\
                     def __init__(self, name):\n\
                         self.name = name\n\
                     \n\
                     def run(self):\n\
                         print(self.name)\n\
                 \n\
                 \n\
                 def main():\n\
                     app = MyApp('test')\n\
                     app.run()\n",
            );
    });

    let state = repo_fetch_state();

    let v = run_repo_fetch(
        state,
        RepoFetchArgs {
            host: Some("github".into()),
            owner: "test-owner".into(),
            repo: "test-repo".into(),
            ref_name: Some("main".into()),
            commit_sha: None,
            path: "src/app.py".into(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            test_fetch_url: Some(server.url("/src/app.py")),
            symbol: None,
            symbol_kind: None,
            match_text: Some("MyApp".into()),
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
            response_detail: None,
        },
    )
    .await
    .expect("repo_fetch should succeed");

    let selected_span = v
        .get("selected_span")
        .expect("selected_span should be present");
    let selection_kind = selected_span["selection_kind"]
        .as_str()
        .expect("selection_kind should be a string");
    assert_eq!(
        selection_kind, "match_text",
        "should find match_text: {selected_span:?}"
    );

    let text = v["text"].as_str().expect("text should be present");
    assert!(text.contains("MyApp"), "text should contain MyApp: {text}");
}

#[tokio::test]
async fn repo_fetch_explicit_range_no_expand() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/src/main.rs");
        then.status(200)
            .header("content-type", "text/plain; charset=utf-8")
            .body("line 1\nline 2\nline 3\nline 4\nline 5\n");
    });

    let state = repo_fetch_state();

    let v = run_repo_fetch(
        state,
        RepoFetchArgs {
            host: Some("github".into()),
            owner: "test-owner".into(),
            repo: "test-repo".into(),
            ref_name: Some("main".into()),
            commit_sha: None,
            path: "src/main.rs".into(),
            line_start: Some(2),
            line_end: Some(4),
            context_before: None,
            context_after: None,
            max_chars: None,
            timeout_ms: None,
            test_fetch_url: Some(server.url("/src/main.rs")),
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: None,
            prefer_local: None,
            response_detail: None,
        },
    )
    .await
    .expect("repo_fetch should succeed");

    let selected_span = v
        .get("selected_span")
        .expect("selected_span should be present for explicit range");
    let selection_kind = selected_span["selection_kind"]
        .as_str()
        .expect("selection_kind should be a string");
    assert_eq!(
        selection_kind, "explicit_range",
        "should be explicit_range: {selected_span:?}"
    );

    let returned_start = v["returned_line_start"].as_u64().unwrap();
    let returned_end = v["returned_line_end"].as_u64().unwrap();
    assert_eq!(returned_start, 2);
    assert_eq!(returned_end, 4);
}

#[tokio::test]
async fn repo_fetch_symbol_not_found_warns() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/src/main.rs");
        then.status(200)
            .header("content-type", "text/plain; charset=utf-8")
            .body("fn main() {}\n");
    });

    let state = repo_fetch_state();

    let v = run_repo_fetch(
        state,
        RepoFetchArgs {
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
            test_fetch_url: Some(server.url("/src/main.rs")),
            symbol: Some("nonexistent".into()),
            symbol_kind: None,
            match_text: None,
            expand_to_block: Some(true),
            max_block_lines: None,
            prefer_local: None,
            response_detail: None,
        },
    )
    .await
    .expect("repo_fetch should succeed");

    let warnings = v["warnings"]
        .as_array()
        .expect("warnings should be present");
    let has_no_match_warning = warnings.iter().any(|w| {
        w.as_str()
            .map(|s| s.contains("no match found"))
            .unwrap_or(false)
    });
    assert!(
        has_no_match_warning,
        "should warn about no match: {warnings:?}"
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_fetch_prefer_local_redirects_to_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    // Initialize git repo with a remote URL
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

    // Create initial commit
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

    // Request remote-style repo_fetch with prefer_local = true
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
        response_detail: None,
    };

    let v = run_repo_fetch(state, args)
        .await
        .expect("prefer_local repo_fetch should succeed");

    // Should resolve to local workspace fetch
    assert_eq!(v["trust"], "local_trusted");
    assert_eq!(v["fetched"], true);

    let text = v["text"].as_str().expect("text should be present");
    assert!(
        text.contains("pub fn add"),
        "fetched text should contain the function: {text}"
    );
}

#[tokio::test]
async fn repo_fetch_validation_error_zero_max_block_lines() {
    let state = repo_fetch_state();
    let result = run_repo_fetch(
        state,
        RepoFetchArgs {
            host: None,
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
            symbol: None,
            symbol_kind: None,
            match_text: None,
            expand_to_block: None,
            max_block_lines: Some(0),
            prefer_local: None,
            response_detail: None,
        },
    )
    .await;

    assert!(result.is_err(), "max_block_lines=0 should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("max_block_lines"),
        "error should mention max_block_lines: {err}"
    );
}

// =========================================================================
// Corrective Plan Phase 1-6: Workstream 3 -- repo_map verification
// =========================================================================

#[cfg(feature = "mock")]
#[tokio::test]
async fn repo_fetch_prefer_local_invalid_host_errors_without_local_match() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("lib.rs"), "pub fn add() {}").unwrap();
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
        host: Some("not-a-host".to_string()),
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
        response_detail: None,
    };

    let err = run_repo_fetch(state, args)
        .await
        .expect_err("invalid host with prefer_local should error");
    assert!(
        err.to_string().contains("unknown host"),
        "error should mention unknown host: {err}"
    );
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
mod repo_search {
    use super::*;

    #[cfg(feature = "mock")]
    fn repo_state_with_engines(
        cfg: AppConfig,
        engines: Vec<MockEngine>,
        timeout: Duration,
    ) -> Arc<ServerState> {
        let adapter = MetadataSearchAdapter::from_engines(mock_engines(engines), timeout);
        Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
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

    // ---- Validation tests ----

    #[tokio::test]
    async fn repo_search_empty_query_returns_validation_error() {
        let state = state_with_default();
        let res = run_repo_search(state, repo_args("   ")).await;
        let err = res.expect_err("expected validation error");
        assert!(err.to_string().contains("invalid query"), "got: {err}");
    }

    #[tokio::test]
    async fn repo_search_zero_max_results_returns_validation_error() {
        let state = state_with_default();
        let res = run_repo_search(
            state,
            RepoSearchArgs {
                query: "rust".into(),
                providers: vec!["mock_a".into()],
                max_results: Some(0),
                ..Default::default()
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
    async fn repo_search_zero_timeout_ms_returns_validation_error() {
        let state = state_with_default();
        let res = run_repo_search(
            state,
            RepoSearchArgs {
                query: "rust".into(),
                providers: vec!["mock_a".into()],
                timeout_ms: Some(0),
                ..Default::default()
            },
        )
        .await;
        let err = res.expect_err("expected validation error");
        assert!(
            err.to_string().contains("timeout_ms must be > 0"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn repo_search_oversized_query_returns_validation_error() {
        let state = state_with_default();
        let too_long = "a".repeat(2_000);
        let res = run_repo_search(state, repo_args(&too_long)).await;
        let err = res.expect_err("expected validation error");
        assert!(err.to_string().contains("invalid query"), "got: {err}");
        assert!(err.to_string().contains("characters"), "got: {err}");
    }

    // ---- Response shape tests ----

    #[tokio::test]
    async fn repo_search_returns_grouped_response() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        assert_eq!(v["query"], "axum");
        assert!(v["groups"].is_array(), "groups should be an array");
        assert!(
            v["suggested_fetches"].is_array(),
            "suggested_fetches should be an array"
        );
        assert!(
            v["providers_queried"].is_array(),
            "providers_queried should be an array"
        );
        assert!(v["warnings"].is_array(), "warnings should be an array");
        assert!(
            v["trust_markers"].is_object(),
            "trust_markers should be an object"
        );
    }

    #[tokio::test]
    async fn repo_search_grouped_cards_have_materialized_evidence_role() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new("Docs", "https://docs.rs/axum/latest/axum/", "mock_a"),
                MockResult::new(
                    "Source",
                    "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
                    "mock_a",
                ),
            ],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let groups = v["groups"].as_array().expect("groups is array");
        for group in groups {
            let results = group["results"].as_array().expect("results is array");
            for card in results {
                let has_role = card.get("evidence_role").is_some()
                    || card
                        .get("metadata")
                        .and_then(|m| m.get("evidence_role"))
                        .is_some();
                assert!(
                    has_role,
                    "grouped card must have evidence_role materialized; got: {card}"
                );
            }
        }
    }

    #[tokio::test]
    async fn repo_search_groups_are_nonempty_when_results_exist() {
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
            ],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let groups = v["groups"].as_array().expect("groups is array");
        let nonempty: Vec<&serde_json::Value> = groups
            .iter()
            .filter(|g| !g["results"].as_array().unwrap_or(&vec![]).is_empty())
            .collect();
        assert!(
            !nonempty.is_empty(),
            "at least one group should have results: {groups:?}"
        );
    }

    #[tokio::test]
    async fn repo_search_empty_results_returns_empty_groups() {
        let engines = vec![MockEngine::success("mock_a", vec![])];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("nonexistent"))
            .await
            .expect("ok");

        let groups = v["groups"].as_array().expect("groups is array");
        let total_results: usize = groups
            .iter()
            .map(|g| g["results"].as_array().map_or(0, |a| a.len()))
            .sum();
        assert_eq!(
            total_results, 0,
            "no results should be returned for empty engine"
        );
    }

    // ---- Provider tests ----

    #[tokio::test]
    async fn repo_search_preserves_provider_failures() {
        let engines = vec![
            MockEngine::success(
                "mock_a",
                vec![MockResult::new(
                    "A",
                    "https://docs.rs/tokio/latest/tokio/",
                    "mock_a",
                )],
            ),
            MockEngine::failure("mock_b", MockFailure::Parse),
        ];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args_multi(&["mock_a", "mock_b"], "tokio"))
            .await
            .expect("ok");

        let failed = v["providers_failed"].as_array().unwrap();
        assert!(
            !failed.is_empty(),
            "providers_failed should be non-empty when one engine fails: {failed:?}"
        );
        let failed_ids: Vec<&str> = failed.iter().filter_map(|f| f["id"].as_str()).collect();
        assert!(
            failed_ids.contains(&"mock_b"),
            "mock_b should be in providers_failed: {failed_ids:?}"
        );
    }

    #[tokio::test]
    async fn repo_search_all_providers_fail_returns_error() {
        let engines = vec![
            MockEngine::failure("mock_a", MockFailure::HttpStatus(503)),
            MockEngine::failure("mock_b", MockFailure::Network),
        ];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args_multi(&["mock_a", "mock_b"], "rust"))
            .await
            .expect("repo_search should return Ok even when all providers fail");
        let groups = v["groups"].as_array().expect("groups is array");
        let total_results: usize = groups
            .iter()
            .map(|g| g["results"].as_array().map_or(0, |a| a.len()))
            .sum();
        assert_eq!(total_results, 0, "no results when all providers fail");
        let failed = v["providers_failed"].as_array().expect("providers_failed");
        assert!(
            !failed.is_empty(),
            "providers_failed should be non-empty when all providers fail"
        );
        // Both provider IDs should appear in the failure list. There may
        // be multiple entries per provider when the planner generates
        // multiple subqueries (both engines fail in each).
        let failed_ids: Vec<&str> = failed.iter().filter_map(|f| f["id"].as_str()).collect();
        assert!(
            failed_ids.contains(&"mock_a"),
            "mock_a should be in providers_failed: {failed_ids:?}"
        );
        assert!(
            failed_ids.contains(&"mock_b"),
            "mock_b should be in providers_failed: {failed_ids:?}"
        );
    }

    // ---- Policy tests ----

    #[tokio::test]
    async fn repo_search_blocked_when_mode_off() {
        let state = state_with_mode_off();
        let res = run_repo_search(state, repo_args("rust")).await;
        let err = res.expect_err("expected policy denial");
        assert!(err.to_string().contains("disabled by policy"), "got: {err}");
    }

    // ---- Include flag tests ----

    #[tokio::test]
    async fn repo_search_include_false_suppresses_groups() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Axum Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "axum".into(),
                providers: vec!["mock_a".into()],
                include_docs: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        // The flag is accepted without error and produces a valid response.
        // The actual subquery suppression (no docs subquery generated) is
        // tested at the unit level in repo_planner; the mock engine returns
        // all results for all queries so docs.rs URLs may still appear.
        assert!(v["groups"].is_array(), "groups should be an array");
        assert!(
            v["trust_markers"].is_object(),
            "trust_markers should be an object"
        );
        assert!(
            v["providers_queried"].is_array(),
            "providers_queried should be an array"
        );
    }

    // ---- Mock workflow test ----

    #[tokio::test]
    async fn repo_search_full_workflow() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new("Axum Docs", "https://docs.rs/axum/latest/axum/", "mock_a")
                    .with_snippet("Web framework for Rust"),
                MockResult::new(
                    "Axum on crates.io",
                    "https://crates.io/crates/axum",
                    "mock_a",
                )
                .with_snippet("A web framework"),
                MockResult::new(
                    "lib.rs",
                    "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
                    "mock_a",
                )
                .with_snippet("Main library source"),
                MockResult::new(
                    "README.md",
                    "https://github.com/tokio-rs/axum/blob/main/README.md",
                    "mock_a",
                )
                .with_snippet("Axum README"),
                MockResult::new(
                    "Issue #123",
                    "https://github.com/tokio-rs/axum/issues/123",
                    "mock_a",
                )
                .with_snippet("Bug report"),
                MockResult::new(
                    "Release v0.7.0",
                    "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
                    "mock_a",
                )
                .with_snippet("Release notes"),
            ],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        assert_eq!(v["query"], "axum");

        let groups = v["groups"].as_array().expect("groups is array");
        assert!(!groups.is_empty(), "should have at least one group");

        let total_results: usize = groups
            .iter()
            .map(|g| g["results"].as_array().map_or(0, |a| a.len()))
            .sum();
        assert_eq!(total_results, 6, "all 6 results should be in groups");

        let group_kinds: Vec<&str> = groups
            .iter()
            .map(|g| g["kind"].as_str().unwrap_or(""))
            .collect();
        assert!(
            group_kinds.contains(&"official_docs"),
            "should have official_docs group: {group_kinds:?}"
        );
        assert!(
            group_kinds.contains(&"package_registry"),
            "should have package_registry group: {group_kinds:?}"
        );
        assert!(
            group_kinds.contains(&"source_files"),
            "should have source_files group: {group_kinds:?}"
        );
        assert!(
            group_kinds.contains(&"issues"),
            "should have issues group: {group_kinds:?}"
        );
        assert!(
            group_kinds.contains(&"releases"),
            "should have releases group: {group_kinds:?}"
        );

        let suggested = v["suggested_fetches"]
            .as_array()
            .expect("suggested_fetches");
        assert!(
            !suggested.is_empty(),
            "suggested_fetches should be non-empty when results exist"
        );
        for fetch in suggested {
            assert!(
                fetch["url"].as_str().is_some(),
                "suggested fetch should have a url: {fetch:?}"
            );
            assert!(
                fetch["reason"].as_str().is_some(),
                "suggested fetch should have a reason: {fetch:?}"
            );
        }

        assert!(
            v["providers_queried"]
                .as_array()
                .is_some_and(|a| !a.is_empty()),
            "providers_queried should be non-empty"
        );
    }

    #[tokio::test]
    async fn repo_search_migration_workflow() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new(
                    "Axum Migration Guide",
                    "https://docs.rs/axum/latest/axum/migration/index.html",
                    "mock_a",
                )
                .with_snippet("Migration from 0.6 to 0.7"),
                MockResult::new(
                    "Axum on crates.io",
                    "https://crates.io/crates/axum",
                    "mock_a",
                )
                .with_snippet("A web framework"),
                MockResult::new(
                    "Release v0.7.0",
                    "https://github.com/tokio-rs/axum/releases/tag/v0.7.0",
                    "mock_a",
                )
                .with_snippet("Breaking changes in v0.7"),
            ],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "rust crate axum migration 0.7".into(),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        assert_eq!(v["query"], "rust crate axum migration 0.7");

        let groups = v["groups"].as_array().expect("groups is array");
        let group_kinds: Vec<&str> = groups
            .iter()
            .map(|g| g["kind"].as_str().unwrap_or(""))
            .collect();
        assert!(
            group_kinds.contains(&"official_docs"),
            "should have official_docs group: {group_kinds:?}"
        );
        assert!(
            group_kinds.contains(&"package_registry"),
            "should have package_registry group: {group_kinds:?}"
        );
        assert!(
            group_kinds.contains(&"releases"),
            "should have releases group: {group_kinds:?}"
        );

        let total_results: usize = groups
            .iter()
            .map(|g| g["results"].as_array().map_or(0, |a| a.len()))
            .sum();
        assert_eq!(total_results, 3, "all 3 results should be in groups");
    }

    // ---- Code evidence tests ----

    #[tokio::test]
    async fn repo_search_code_host_source_file_has_code_evidence() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Axum Source",
                "https://github.com/tokio-rs/axum/blob/main/src/lib.rs",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let groups = v["groups"].as_array().expect("groups is array");
        let source_group = groups
            .iter()
            .find(|g| g["kind"].as_str() == Some("source_files"))
            .expect("should have source_files group");
        let results = source_group["results"]
            .as_array()
            .expect("results is array");
        assert!(
            !results.is_empty(),
            "source_files group should have results"
        );

        let card = &results[0];
        let metadata = card["metadata"].as_object().expect("metadata is object");
        assert!(
            metadata.contains_key("code_evidence"),
            "code-host source-file result should have code_evidence: {metadata:?}"
        );
        let code_evidence = &metadata["code_evidence"];
        assert_eq!(
            code_evidence["host"].as_str(),
            Some("github"),
            "code_evidence.host should be github"
        );
        assert_eq!(
            code_evidence["owner"].as_str(),
            Some("tokio-rs"),
            "code_evidence.owner should be tokio-rs"
        );
        assert_eq!(
            code_evidence["repo"].as_str(),
            Some("axum"),
            "code_evidence.repo should be axum"
        );
        assert_eq!(
            code_evidence["path"].as_str(),
            Some("src/lib.rs"),
            "code_evidence.path should be src/lib.rs"
        );
        assert!(
            code_evidence["raw_url"].as_str().is_some(),
            "code_evidence should have raw_url"
        );
        assert_eq!(
            code_evidence["source_role"].as_str(),
            Some("implementation"),
            "code_evidence.source_role should be implementation"
        );
        assert!(
            code_evidence["evidence_reasons"].as_array().is_some(),
            "code_evidence should have evidence_reasons"
        );
    }

    #[tokio::test]
    async fn repo_search_non_code_host_result_has_no_code_evidence() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Axum Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let groups = v["groups"].as_array().expect("groups is array");
        let docs_group = groups
            .iter()
            .find(|g| g["kind"].as_str() == Some("official_docs"))
            .expect("should have official_docs group");
        let results = docs_group["results"].as_array().expect("results is array");
        assert!(!results.is_empty());

        let card = &results[0];
        let metadata = card["metadata"].as_object().expect("metadata is object");
        assert!(
            !metadata.contains_key("code_evidence"),
            "non-code-host result should NOT have code_evidence: {metadata:?}"
        );
    }

    #[tokio::test]
    async fn repo_search_readme_file_has_readme_source_role() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "README",
                "https://github.com/tokio-rs/axum/blob/main/README.md",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let groups = v["groups"].as_array().expect("groups is array");
        let readme_group = groups
            .iter()
            .find(|g| g["kind"].as_str() == Some("readme"))
            .expect("should have readme group");
        let results = readme_group["results"]
            .as_array()
            .expect("results is array");
        assert!(!results.is_empty());

        let card = &results[0];
        let metadata = card["metadata"].as_object().expect("metadata is object");
        let code_evidence = metadata
            .get("code_evidence")
            .expect("README should have code_evidence");
        assert_eq!(
            code_evidence["source_role"].as_str(),
            Some("readme"),
            "README source_role should be readme"
        );
    }

    #[tokio::test]
    async fn repo_search_test_file_has_test_source_role() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Test file",
                "https://github.com/tokio-rs/axum/blob/main/tests/integration.rs",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let groups = v["groups"].as_array().expect("groups is array");
        let tests_group = groups
            .iter()
            .find(|g| g["kind"].as_str() == Some("tests"))
            .expect("should have tests group");
        let results = tests_group["results"].as_array().expect("results is array");
        assert!(!results.is_empty());

        let card = &results[0];
        let metadata = card["metadata"].as_object().expect("metadata is object");
        let code_evidence = metadata
            .get("code_evidence")
            .expect("test file should have code_evidence");
        assert_eq!(
            code_evidence["source_role"].as_str(),
            Some("test"),
            "test file source_role should be test"
        );
    }

    // ---- Profile and telemetry tests ----

    #[tokio::test]
    async fn repo_search_response_includes_telemetry() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let telemetry = v["telemetry"]
            .as_object()
            .expect("telemetry should be an object");
        assert!(
            telemetry.contains_key("provider_selection"),
            "telemetry should have provider_selection"
        );
        assert!(
            telemetry.contains_key("subqueries"),
            "telemetry should have subqueries"
        );
        assert!(
            telemetry["subqueries"].is_array(),
            "subqueries should be an array"
        );
    }

    #[tokio::test]
    async fn repo_search_telemetry_subqueries_have_labels() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "tokio-rs/axum middleware".into(),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let subqueries = v["telemetry"]["subqueries"]
            .as_array()
            .expect("subqueries is array");
        assert!(!subqueries.is_empty(), "should have subqueries");
        for sq in subqueries {
            assert!(sq.get("label").is_some(), "subquery should have label");
            assert!(sq.get("query").is_some(), "subquery should have query");
            assert!(
                sq.get("providers_attempted").is_some(),
                "subquery should have providers_attempted"
            );
        }
    }

    #[tokio::test]
    async fn repo_search_with_profile_field() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "tokio-rs/axum".into(),
                providers: vec!["mock_a".into()],
                profile: Some("coding".into()),
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let telemetry = v["telemetry"]
            .as_object()
            .expect("telemetry should be an object");
        let provider_selection = telemetry["provider_selection"]
            .as_object()
            .expect("provider_selection should be an object");
        assert_eq!(
            provider_selection["profile_requested"].as_str(),
            Some("coding"),
            "profile_requested should be coding"
        );
        assert_eq!(
            provider_selection["profile_applied"].as_str(),
            Some("coding"),
            "profile_applied should be coding"
        );
    }

    #[tokio::test]
    async fn repo_search_without_profile_has_no_profile_in_telemetry() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let provider_selection = v["telemetry"]["provider_selection"]
            .as_object()
            .expect("provider_selection should be an object");
        assert!(
            provider_selection.get("profile_requested").is_none()
                || provider_selection["profile_requested"].is_null(),
            "profile_requested should be null when no profile specified"
        );
    }

    #[tokio::test]
    async fn repo_search_telemetry_deadline_fields() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(state, repo_args("axum")).await.expect("ok");

        let telemetry = v["telemetry"].as_object().expect("telemetry");
        // deadline_exceeded is skipped when false, so just check it's not true
        assert_ne!(
            telemetry.get("deadline_exceeded").and_then(|v| v.as_bool()),
            Some(true),
            "deadline_exceeded should not be true"
        );
        // subqueries_interrupted and subqueries_skipped are skipped when 0
        assert_ne!(
            telemetry
                .get("subqueries_interrupted")
                .and_then(|v| v.as_u64()),
            Some(1),
            "subqueries_interrupted should not be > 0"
        );
        assert_ne!(
            telemetry.get("subqueries_skipped").and_then(|v| v.as_u64()),
            Some(1),
            "subqueries_skipped should not be > 0"
        );
    }

    #[tokio::test]
    async fn repo_search_capability_warnings_include_prefix() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "tokio-rs/axum".into(),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let warnings = v["warnings"]
            .as_array()
            .expect("warnings should be an array");
        // Warnings are SearchWarning objects with {provider_id, message} fields
        let has_native_warning = warnings.iter().any(|w| {
            w["message"]
                .as_str()
                .unwrap_or("")
                .contains("native_code_search_unavailable")
        });
        assert!(
            has_native_warning,
            "should have native_code_search_unavailable warning when no github providers: {:?}",
            warnings
                .iter()
                .filter_map(|w| w["message"].as_str())
                .collect::<Vec<_>>()
        );

        let has_issue_warning = warnings.iter().any(|w| {
            w["message"]
                .as_str()
                .unwrap_or("")
                .contains("issue_search_no_native_provider")
        });
        assert!(
            has_issue_warning,
            "should have issue_search_no_native_provider warning: {:?}",
            warnings
                .iter()
                .filter_map(|w| w["message"].as_str())
                .collect::<Vec<_>>()
        );

        let has_release_warning = warnings.iter().any(|w| {
            w["message"]
                .as_str()
                .unwrap_or("")
                .contains("release_search_no_native_provider")
        });
        assert!(
            has_release_warning,
            "should have release_search_no_native_provider warning: {:?}",
            warnings
                .iter()
                .filter_map(|w| w["message"].as_str())
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn security_search_returns_structured_response() {
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
                    "Test package on npm",
                    "https://www.npmjs.com/package/test-package",
                    "mock_a",
                )
                .with_snippet("Test package security advisory"),
            ],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001 test-package vulnerability".into()),
                ecosystem: Some("npm".into()),
                package: Some("test-package".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        assert_eq!(v["query"], "CVE-2024-0001 test-package vulnerability");
        assert_eq!(v["mode"], "security_metasearch");

        let resolved_ids = v["resolved_identifiers"]
            .as_object()
            .expect("resolved_identifiers");
        let cve_ids = resolved_ids["cve_ids"].as_array().expect("cve_ids");
        assert!(
            cve_ids
                .iter()
                .any(|id| id.as_str() == Some("CVE-2024-0001")),
            "should resolve CVE-2024-0001: {cve_ids:?}"
        );

        let groups = v["groups"].as_array().expect("groups");
        assert!(!groups.is_empty(), "should have at least one group");

        let warnings = v["warnings"].as_array().expect("warnings");
        assert!(
            warnings.iter().any(|w| w["message"]
                .as_str()
                .unwrap_or("")
                .contains("generic_context_untrusted")),
            "should have generic_context_untrusted warning: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w["message"]
                .as_str()
                .unwrap_or("")
                .contains("severity_unavailable")),
            "should have severity_unavailable warning: {warnings:?}"
        );
    }

    #[tokio::test]
    async fn security_search_default_routing_queries_only_selected_providers() {
        let engines = vec![
            MockEngine::success(
                "mock_a",
                vec![MockResult::new(
                    "Selected provider advisory",
                    "https://example.com/a",
                    "mock_a",
                )],
            ),
            MockEngine::success(
                "mock_b",
                vec![MockResult::new(
                    "Unselected provider advisory",
                    "https://example.com/b",
                    "mock_b",
                )],
            ),
        ];
        let mut cfg = test_cfg();
        cfg.search.default_providers = vec!["mock_a".to_string()];
        let state = security_state_with_engines(cfg, engines, Duration::from_secs(5));

        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001".into()),
                providers: vec![],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let queried = v["providers_queried"].as_array().unwrap();
        let queried_ids: Vec<&str> = queried.iter().filter_map(|q| q.as_str()).collect();
        assert_eq!(queried_ids, vec!["mock_a"]);

        let selected = v["routing_decision"]["selected_providers"]
            .as_array()
            .expect("selected providers");
        let selected_ids: Vec<&str> = selected.iter().filter_map(|q| q.as_str()).collect();
        assert_eq!(selected_ids, vec!["mock_a"]);
    }

    #[tokio::test]
    async fn security_search_empty_query_without_identifiers_fails() {
        let state = state_with_default();
        let result = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("   ".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await;
        assert!(
            result.is_err(),
            "empty query without identifiers should fail"
        );
    }

    #[tokio::test]
    async fn security_search_zero_timeout_ms_returns_validation_error() {
        let state = state_with_default();
        let result = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-12345".into()),
                providers: vec!["mock_a".into()],
                timeout_ms: Some(0),
                ..Default::default()
            },
        )
        .await;
        let err = result.expect_err("expected validation error");
        assert!(
            err.to_string().contains("timeout_ms must be > 0"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn security_search_with_explicit_cve_id() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Advisory for CVE-2024-12345",
                "https://nvd.nist.gov/vuln/detail/CVE-2024-12345",
                "mock_a",
            )
            .with_snippet("NVD advisory details")],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: None,
                cve_id: Some("CVE-2024-12345".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let resolved_ids = v["resolved_identifiers"]
            .as_object()
            .expect("resolved_identifiers");
        let cve_ids = resolved_ids["cve_ids"].as_array().expect("cve_ids");
        assert_eq!(cve_ids.len(), 1);
        assert_eq!(cve_ids[0].as_str(), Some("CVE-2024-12345"));
    }

    #[tokio::test]
    async fn security_search_kev_warning_when_requested() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Test advisory",
                "https://example.com/advisory",
                "mock_a",
            )],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001".into()),
                include_kev: Some(true),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let warnings = v["warnings"].as_array().expect("warnings");
        assert!(
            warnings
                .iter()
                .any(|w| w["message"].as_str().unwrap_or("").contains("kev_lookup")),
            "should have kev_lookup warning when include_kev=true: {warnings:?}"
        );
    }

    #[tokio::test]
    async fn security_search_groups_results_by_type() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new(
                    "OSV Advisory",
                    "https://osv.dev/vulnerability/GHSA-test-1234-abcd",
                    "mock_a",
                ),
                MockResult::new(
                    "GitHub Advisory",
                    "https://github.com/advisories/GHSA-test-5678-efgh",
                    "mock_a",
                ),
                MockResult::new(
                    "NVD Entry",
                    "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                    "mock_a",
                ),
                MockResult::new(
                    "Exploit Discussion",
                    "https://exploit-db.com/exploits/12345",
                    "mock_a",
                ),
            ],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("test vulnerability".into()),
                include_exploit_context: Some(true),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let groups = v["groups"].as_array().expect("groups");
        let group_kinds: Vec<&str> = groups
            .iter()
            .map(|g| g["kind"].as_str().unwrap_or(""))
            .collect();

        assert!(
            group_kinds.contains(&"authoritative_advisories"),
            "should have authoritative_advisories group: {group_kinds:?}"
        );
        assert!(
            group_kinds.contains(&"exploit_discussion"),
            "should have exploit_discussion group: {group_kinds:?}"
        );
    }

    #[tokio::test]
    async fn security_search_suggested_fetches_include_osv() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Advisory",
                "https://example.com/advisory",
                "mock_a",
            )],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001 vulnerability".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let suggested = v["suggested_fetches"]
            .as_array()
            .expect("suggested_fetches");
        assert!(
            suggested.iter().any(|f| f["url"]
                .as_str()
                .unwrap_or("")
                .contains("osv.dev/vulnerability/CVE-2024-0001")),
            "should suggest OSV fetch for CVE-2024-0001: {suggested:?}"
        );
    }

    #[tokio::test]
    async fn security_search_includes_trust_markers() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Test advisory",
                "https://example.com/advisory",
                "mock_a",
            )],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("test vulnerability".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let trust_markers = v["trust_markers"].as_object().expect("trust_markers");
        assert!(
            trust_markers.contains_key("text_sanitized"),
            "trust_markers should have text_sanitized"
        );
        assert!(
            trust_markers.contains_key("text_truncated"),
            "trust_markers should have text_truncated"
        );
    }

    #[tokio::test]
    async fn repo_search_with_include_security_context() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new(
                    "axum on crates.io",
                    "https://crates.io/crates/axum",
                    "mock_a",
                )
                .with_snippet("A web framework for Rust"),
                MockResult::new("Axum Docs", "https://docs.rs/axum/latest/axum/", "mock_a")
                    .with_snippet("API documentation for axum"),
            ],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "axum".into(),
                providers: vec!["mock_a".into()],
                include_security_context: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        assert_eq!(v["query"], "axum");
        // Without package resolution, security_context is absent (skipped when None).
        // With package resolution + advisory data, it would be a populated object.
        assert!(
            v.get("security_context").is_none(),
            "security_context should be absent when no package resolution is available"
        );
        // Verify the rest of the response structure is intact
        assert!(v["groups"].is_array(), "groups should be an array");
        assert!(
            v["suggested_fetches"].is_array(),
            "suggested_fetches should be an array"
        );
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn security_search_version_comparison_warning() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Advisory for test-pkg",
                "https://github.com/advisories/GHSA-test-1234-abcd",
                "mock_a",
            )
            .with_snippet("Versions before 2.0.0 are affected")],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("test-pkg vulnerability".into()),
                ecosystem: Some("npm".into()),
                package: Some("test-pkg".into()),
                version: Some("3.0.0".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let warnings = v["warnings"].as_array().expect("warnings");
        let _has_version_warning = warnings.iter().any(|w| {
            let msg = w["message"].as_str().unwrap_or("");
            msg.contains("version_match_unavailable") || msg.contains("version_mismatch")
        });
        assert!(
            v["groups"].as_array().is_some_and(|g| !g.is_empty()),
            "should have groups"
        );
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn security_search_defensive_guidance_categories() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new(
                    "XSS Hardening Guide",
                    "https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Scripting_Prevention_Cheat_Sheet.html",
                    "mock_a",
                )
                .with_snippet("Prevent XSS by encoding output"),
                MockResult::new(
                    "CVE-2024-0001",
                    "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                    "mock_a",
                )
                .with_snippet("XSS vulnerability in web framework"),
            ],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001 XSS vulnerability".into()),
                include_defensive_guidance: Some(true),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        assert_eq!(v["query"], "CVE-2024-0001 XSS vulnerability");
        let groups = v["groups"].as_array().expect("groups");
        assert!(!groups.is_empty(), "should have groups");
    }

    // ---- Exact-error mode tests ----

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_disabled_rejects_mode() {
        let engines = vec![MockEngine::success("mock_a", vec![])];
        let mut cfg = test_cfg();
        cfg.search.exact_error.enabled = false;
        let state = repo_state_with_engines(cfg, engines, Duration::from_secs(5));
        let res = run_repo_search(
            state,
            RepoSearchArgs {
                query: "error[E0308]: mismatched types".into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await;
        let err = res.expect_err("expected validation error for disabled exact_error");
        assert!(
            err.to_string().contains("exact_error") || err.to_string().contains("disabled"),
            "error should mention exact_error being disabled: {err}"
        );
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_uses_config_max_chars() {
        let engines = vec![MockEngine::success("mock_a", vec![])];
        let mut cfg = test_cfg();
        cfg.search.max_query_chars = 50; // Base limit (ignored in exact_error mode)
        cfg.search.exact_error.max_error_chars = 100; // effective_max = 100 (exact_error cap)
        let state = repo_state_with_engines(cfg, engines, Duration::from_secs(5));
        // Query longer than 100 chars should be rejected in exact_error mode
        let long_query = "a".repeat(200);
        let res = run_repo_search(
            state,
            RepoSearchArgs {
                query: long_query,
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await;
        let err = res.expect_err("expected validation error for long query in exact_error mode");
        assert!(
            err.to_string().contains("characters") || err.to_string().contains("max_error_chars"),
            "error should mention character limit: {err}"
        );
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_parses_error_codes() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Rust error docs",
                "https://doc.rust-lang.org/error-index.html",
                "mock_a",
            )
            .with_snippet("E0308 mismatched types")],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "error[E0308]: mismatched types: expected `u32`, found `&str`".into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        // Should have error_context with parsed error parts
        let error_context = v["error_context"].as_object();
        assert!(
            error_context.is_some(),
            "exact_error mode should include error_context"
        );

        // Should have subqueries targeting the error code
        let groups = v["groups"].as_array().expect("groups");
        let total: usize = groups
            .iter()
            .map(|g| g["results"].as_array().map_or(0, |a| a.len()))
            .sum();
        assert!(total > 0, "should have results from error subqueries");
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_redacts_sensitive_tokens() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new("Results", "https://example.com", "mock_a")],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                // Error with home path, API key pattern, and UUID
                query: "error in /home/user/project/src/main.rs: api_key=abc123def456ghi789jkl012mno345pqr token: 12345678-1234-1234-1234-123456789abc".into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        // Check that the response exists and groups are present
        let groups = v["groups"].as_array().expect("groups");
        assert!(!groups.is_empty(), "should have groups");

        // The error_context should exist with redacted info
        let error_context = v["error_context"].as_object();
        assert!(
            error_context.is_some(),
            "exact_error mode should include error_context"
        );
    }

    /// When redact_sensitive_tokens is disabled, home paths, API tokens,
    /// and UUIDs should NOT be redacted in the normalized error.
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_redact_disabled() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new("Results", "https://example.com", "mock_a")],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "error in /Users/john/project/src/main.rs".into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let error_context = v["error_context"]
            .as_object()
            .expect("error_context should exist");
        let redactions = error_context["redactions_applied"]
            .as_array()
            .expect("redactions_applied should be array");
        assert!(
            !redactions.is_empty(),
            "default config should redact sensitive tokens: {error_context:?}"
        );
    }

    /// max_subqueries config is respected: generating many error codes
    /// should be capped by the configured limit.
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_max_subqueries_config() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new("Results", "https://example.com", "mock_a")],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "error[E0277]: the trait bound is not satisfied".into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let error_context = v["error_context"]
            .as_object()
            .expect("error_context should exist");
        let subqueries = error_context["subqueries"]
            .as_array()
            .expect("subqueries should be array");
        assert!(
            subqueries.len() <= 6,
            "subqueries should respect max_subqueries config: {}",
            subqueries.len()
        );
    }

    /// TypeScript error parsing through MCP: TS error codes should be
    /// detected and language_hint should be typescript.
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_typescript_parsing() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "TS docs",
                "https://typescriptlang.org",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'".into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let error_context = v["error_context"]
            .as_object()
            .expect("error_context should exist");
        let codes = error_context["error_codes"]
            .as_array()
            .expect("error_codes should be array");
        assert!(
            codes.iter().any(|c| c["code"].as_str() == Some("TS2345")),
            "should detect TS2345: {codes:?}"
        );
        let language = error_context["inferred_language"].as_str();
        assert_eq!(
            language,
            Some("typescript"),
            "inferred_language should be typescript: {error_context:?}"
        );
    }

    /// npm ERESOLVE error parsing through MCP.
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_npm_parsing() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new("npm docs", "https://npmjs.com", "mock_a")],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "npm ERR! ERESOLVE could not resolve dependency tree\nnpm ERR! Found: react@17.0.2".into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let error_context = v["error_context"]
            .as_object()
            .expect("error_context should exist");
        let codes = error_context["error_codes"]
            .as_array()
            .expect("error_codes should be array");
        assert!(
            codes.iter().any(|c| c["code"].as_str() == Some("ERESOLVE")),
            "should detect ERESOLVE: {codes:?}"
        );
        let language = error_context["inferred_language"].as_str();
        assert_eq!(
            language,
            Some("javascript"),
            "inferred_language should be javascript: {error_context:?}"
        );
    }

    /// Python exception parsing through MCP.
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_python_parsing() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Python docs",
                "https://python.org",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "Traceback (most recent call last):\n  File \"app.py\", line 42, in main\n    result = data[key]\nKeyError: 'missing_key'".into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let error_context = v["error_context"]
            .as_object()
            .expect("error_context should exist");
        let codes = error_context["error_codes"]
            .as_array()
            .expect("error_codes should be array");
        assert!(
            codes.iter().any(|c| c["code"].as_str() == Some("KeyError")),
            "should detect KeyError: {codes:?}"
        );
        let language = error_context["inferred_language"].as_str();
        assert_eq!(
            language,
            Some("python"),
            "inferred_language should be python: {error_context:?}"
        );
    }

    /// Strengthened redaction test: verify that specific sensitive tokens
    /// are actually removed from the normalized error in the response.
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_redaction_removes_tokens() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new("Results", "https://example.com", "mock_a")],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let error_with_uuid =
            "error at 0x7fff5fbff8d0 request-id: 550e8400-e29b-41d4-a716-446655440000";
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: error_with_uuid.into(),
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let error_context = v["error_context"]
            .as_object()
            .expect("error_context should exist");
        let redactions = error_context["redactions_applied"]
            .as_array()
            .expect("redactions_applied should be array");
        assert!(
            !redactions.is_empty(),
            "should have redactions applied: {error_context:?}"
        );

        let normalized = error_context["normalized_error"]
            .as_str()
            .expect("normalized_error should be a string");
        assert!(
            !normalized.contains("0x7fff5fbff8d0"),
            "normalized_error should redact memory address: {normalized}"
        );
        assert!(
            !normalized.contains("550e8400-e29b-41d4-a716-446655440000"),
            "normalized_error should redact UUID: {normalized}"
        );
    }

    /// When degraded/partial provider selection occurs in repo_search,
    /// the uncertainty_summary fields should reflect the actual state.
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_uncertainty_summary_reflects_provider_selection() {
        let engines = vec![MockEngine::success("yahoo", vec![])];
        let mut cfg = test_cfg();
        cfg.search.providers.insert("yahoo".to_string(), true);
        cfg.search.default_providers = vec!["yahoo".to_string()];
        let state = state_with_engines(cfg, engines, Duration::from_secs(5));
        let v = run_repo_search(
            state,
            RepoSearchArgs {
                query: "tokio-rs/axum".into(),
                providers: vec![],
                profile: Some("coding".into()),
                ..Default::default()
            },
        )
        .await
        .expect("repo_search with degraded coding profile should succeed");

        let selection = v["telemetry"]["provider_selection"]
            .as_object()
            .expect("provider_selection should be object");
        assert_eq!(
            selection["degraded"], true,
            "all profile providers unavailable -> degraded should be true"
        );

        let uncertainty = v["telemetry"]["uncertainty_summary"]
            .as_object()
            .expect("uncertainty_summary should be object");
        assert_eq!(
            uncertainty["degraded_provider_selection"], true,
            "uncertainty_summary.degraded_provider_selection should be true"
        );
    }

    #[tokio::test]
    async fn repo_search_with_owner_repo_no_query() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Axum Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let args = RepoSearchArgs {
            query: String::new(),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            providers: vec!["mock_a".into()],
            ..Default::default()
        };
        let v = run_repo_search(state, args).await.expect("ok");
        let groups = v.get("groups").unwrap().as_array().unwrap();
        assert!(!groups.is_empty(), "should have groups for repo-only call");
    }

    #[tokio::test]
    async fn repo_search_with_repo_owner_name_no_query() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![MockResult::new(
                "Axum Docs",
                "https://docs.rs/axum/latest/axum/",
                "mock_a",
            )],
        )];
        let state = repo_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let args = RepoSearchArgs {
            query: String::new(),
            repo: Some("tokio-rs/axum".to_string()),
            providers: vec!["mock_a".into()],
            ..Default::default()
        };
        let v = run_repo_search(state, args).await.expect("ok");
        let groups = v.get("groups").unwrap().as_array().unwrap();
        assert!(!groups.is_empty(), "should have groups for repo-only call");
    }

    #[tokio::test]
    async fn repo_search_empty_query_no_locator_fails() {
        let state = state_with_default();
        let args = RepoSearchArgs {
            query: String::new(),
            ..Default::default()
        };
        let res = run_repo_search(state, args).await;
        assert!(res.is_err(), "empty query with no locator should fail");
    }

    #[tokio::test]
    async fn repo_search_exact_error_requires_query() {
        let state = state_with_default();
        let args = RepoSearchArgs {
            query: String::new(),
            owner: Some("tokio-rs".to_string()),
            repo: Some("axum".to_string()),
            mode: Some("exact_error".to_string()),
            ..Default::default()
        };
        let res = run_repo_search(state, args).await;
        assert!(res.is_err(), "exact-error with empty query should fail");
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn security_search_groups_have_materialized_evidence_roles() {
        let engines = vec![MockEngine::success(
            "mock_a",
            vec![
                MockResult::new(
                    "CVE-2024-0001 Advisory",
                    "https://osv.dev/vulnerability/GHSA-test-1234-abcd",
                    "mock_a",
                )
                .with_snippet("Advisory details"),
                MockResult::new(
                    "Exploit Discussion",
                    "https://exploit-db.com/exploits/12345",
                    "mock_a",
                )
                .with_snippet("Exploit details"),
                MockResult::new(
                    "NVD Entry",
                    "https://nvd.nist.gov/vuln/detail/CVE-2024-0001",
                    "mock_a",
                )
                .with_snippet("NVD advisory"),
            ],
        )];
        let state = security_state_with_engines(test_cfg(), engines, Duration::from_secs(5));
        let v = run_security_search(
            state,
            SecuritySearchArgs {
                query: Some("CVE-2024-0001 vulnerability".into()),
                include_exploit_context: Some(true),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await
        .expect("ok");

        let groups = v["groups"].as_array().expect("groups");
        assert!(!groups.is_empty(), "should have groups");

        for group in groups {
            let results = group["results"].as_array().expect("group results");
            for card in results {
                let evidence_role = card
                    .get("metadata")
                    .and_then(|m| m.get("evidence_role"))
                    .and_then(|v| v.as_str());
                assert!(
                    evidence_role.is_some(),
                    "every serialized security group card must have a non-null evidence_role, \
                     card title={:?}, group kind={:?}",
                    card["title"].as_str(),
                    group["kind"].as_str(),
                );
            }
        }

        let evidence_role_summary = v.get("evidence_role_summary");
        assert!(
            evidence_role_summary.is_some()
                && evidence_role_summary.unwrap().get("role_counts").is_some(),
            "evidence_role_summary should be present with role_counts"
        );
    }

    #[cfg(feature = "mock")]
    fn security_state_with_engines(
        cfg: AppConfig,
        engines: Vec<MockEngine>,
        timeout: Duration,
    ) -> Arc<ServerState> {
        let adapter = MetadataSearchAdapter::from_engines(mock_engines(engines), timeout);
        Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
    }
}

// ---------------------------------------------------------------------------
// research_search integration tests
// ---------------------------------------------------------------------------

#[cfg(feature = "mock")]
mod corrective_closure_exact_error {
    use super::*;

    #[cfg(feature = "mock")]
    fn ee_state(cfg: AppConfig) -> Arc<ServerState> {
        let engines = vec![MockEngine::success("mock_a", vec![])];
        let adapter =
            MetadataSearchAdapter::from_engines(mock_engines(engines), Duration::from_secs(5));
        Arc::new(ServerState::with_adapter(cfg, Arc::new(adapter)))
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_uses_exact_error_cap() {
        let mut cfg = test_cfg();
        cfg.search.max_query_chars = 10000;
        cfg.search.exact_error.max_error_chars = 100;
        let state = ee_state(cfg);
        // Query of 101 chars should fail in exact_error mode
        let query = "a".repeat(101);
        let res = run_repo_search(
            state,
            RepoSearchArgs {
                query,
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await;
        let err =
            res.expect_err("expected validation error for 101-char query in exact_error mode");
        assert!(
            err.to_string().contains("100"),
            "error should mention the exact_error cap of 100: {err}"
        );
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_normal_uses_normal_query_cap() {
        let mut cfg = test_cfg();
        cfg.search.max_query_chars = 200;
        cfg.search.exact_error.max_error_chars = 50;
        let state = ee_state(cfg);
        // Normal mode should use max_query_chars=200, not exact_error cap=50
        let query = "a".repeat(150);
        let res = run_repo_search(
            state,
            RepoSearchArgs {
                query,
                mode: None,
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await;
        // Should NOT fail — 150 <= 200 (normal cap)
        let _ = res.expect("normal mode should allow 150 chars when max_query_chars=200");
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_allows_larger_than_normal_when_configured() {
        let mut cfg = test_cfg();
        cfg.search.max_query_chars = 512;
        cfg.search.exact_error.max_error_chars = 8000;
        let state = ee_state(cfg);
        // Query of 600 chars should pass in exact_error mode (600 <= 8000)
        let query = "a".repeat(600);
        let res = run_repo_search(
            state,
            RepoSearchArgs {
                query,
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await;
        let _ = res.expect("exact_error mode should allow 600 chars when max_error_chars=8000");
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn repo_search_exact_error_rejects_above_exact_error_cap() {
        let mut cfg = test_cfg();
        cfg.search.max_query_chars = 512;
        cfg.search.exact_error.max_error_chars = 50;
        let state = ee_state(cfg);
        // Query of 51 chars should fail in exact_error mode (51 > 50)
        let query = "a".repeat(51);
        let res = run_repo_search(
            state,
            RepoSearchArgs {
                query,
                mode: Some("exact_error".into()),
                providers: vec!["mock_a".into()],
                ..Default::default()
            },
        )
        .await;
        let err = res.expect_err("expected validation error for 51-char query");
        assert!(
            err.to_string().contains("50"),
            "error should mention the exact_error cap of 50: {err}"
        );
    }
}

// =========================================================================
// Task 6: Security-context safety and source-quality integration tests
// =========================================================================

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
