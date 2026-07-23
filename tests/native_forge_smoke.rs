//! Native forge adapter smoke tests.
//!
//! These tests exercise the native forge tree API adapters (GitHub, GitLab,
//! Codeberg) against live public repositories. They require configured API
//! tokens and are classified as release-blocking evidence.
//!
//! Run with:
//! ```bash
//! GITHUB_TOKEN=ghp_xxx cargo test --features live-smoke --test native_forge_smoke -- --ignored
//! ```
//!
//! Without tokens, all tests are skipped (not failed).

#![cfg(feature = "live-smoke")]

use std::sync::Arc;

use eggsearch::core::config::{ApiProviderConfig, AppConfig};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_repo_map, RepoMapArgs};

fn forge_state() -> Option<Arc<ServerState>> {
    let mut cfg = AppConfig::default();
    cfg.search.mode = eggsearch::core::config::Mode::Live;

    let mut configured_any = false;

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            cfg.search.api.insert(
                "github_code".to_string(),
                ApiProviderConfig {
                    enabled: true,
                    api_key_env: Some("GITHUB_TOKEN".to_string()),
                    base_url: None,
                },
            );
            configured_any = true;
        }
    }

    if let Ok(token) = std::env::var("GITLAB_TOKEN") {
        if !token.is_empty() {
            cfg.search.api.insert(
                "gitlab_code".to_string(),
                ApiProviderConfig {
                    enabled: true,
                    api_key_env: Some("GITLAB_TOKEN".to_string()),
                    base_url: None,
                },
            );
            configured_any = true;
        }
    }

    if let Ok(token) = std::env::var("CODEBERG_TOKEN") {
        if !token.is_empty() {
            cfg.search.api.insert(
                "gitea_code".to_string(),
                ApiProviderConfig {
                    enabled: true,
                    api_key_env: Some("CODEBERG_TOKEN".to_string()),
                    base_url: Some("https://codeberg.org/api/v1".to_string()),
                },
            );
            configured_any = true;
        }
    }

    if !configured_any {
        return None;
    }

    ServerState::build(cfg).ok().map(Arc::new)
}

fn skip_if_no_tokens() -> Arc<ServerState> {
    match forge_state() {
        Some(s) => s,
        None => {
            eprintln!("SKIP: no forge API tokens configured (GITHUB_TOKEN, GITLAB_TOKEN, or CODEBERG_TOKEN)");
            std::process::exit(0);
        }
    }
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and forge API tokens"]
async fn native_github_public_repo() {
    let state = skip_if_no_tokens();
    if !state.config.search.api.contains_key("github_code") {
        eprintln!("SKIP: GITHUB_TOKEN not configured");
        return;
    }

    let v = run_repo_map(
        state,
        RepoMapArgs {
            host: Some("github".into()),
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
    .expect("native github repo_map");

    assert_eq!(
        v["mode"].as_str(),
        Some("native"),
        "expected native mode for GitHub with token: {}",
        serde_json::to_string_pretty(&v).unwrap_or_default()
    );

    let commit_sha = v["commit_sha"].as_str().expect("commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA: {commit_sha}"
    );

    let has_entries = v["root_entries"].as_array().is_some_and(|a| !a.is_empty())
        || v["entries"].as_array().is_some_and(|a| !a.is_empty());
    assert!(has_entries, "native mode should return tree entries");

    assert!(
        v["provenance_pinned"].as_bool() == Some(true),
        "native mode should have provenance_pinned=true"
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and forge API tokens"]
async fn native_github_non_default_branch() {
    let state = skip_if_no_tokens();
    if !state.config.search.api.contains_key("github_code") {
        eprintln!("SKIP: GITHUB_TOKEN not configured");
        return;
    }

    let v = run_repo_map(
        state,
        RepoMapArgs {
            host: Some("github".into()),
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
    .expect("native github non-default branch");

    assert_eq!(
        v["mode"].as_str(),
        Some("native"),
        "expected native mode for GitHub non-default branch"
    );

    let commit_sha = v["commit_sha"].as_str().expect("commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA: {commit_sha}"
    );

    let ref_name = v["ref_name"].as_str().unwrap_or("");
    assert!(
        ref_name.contains("v0.7"),
        "should resolve v0.7.x branch, got ref_name={ref_name}"
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and forge API tokens"]
async fn native_gitlab_public_repo() {
    let state = skip_if_no_tokens();
    if !state.config.search.api.contains_key("gitlab_code") {
        eprintln!("SKIP: GITLAB_TOKEN not configured");
        return;
    }

    let v = run_repo_map(
        state,
        RepoMapArgs {
            host: Some("gitlab".into()),
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
    .expect("native gitlab repo_map");

    assert_eq!(
        v["mode"].as_str(),
        Some("native"),
        "expected native mode for GitLab with token: {}",
        serde_json::to_string_pretty(&v).unwrap_or_default()
    );

    let commit_sha = v["commit_sha"].as_str().expect("commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA: {commit_sha}"
    );

    let has_entries = v["root_entries"].as_array().is_some_and(|a| !a.is_empty())
        || v["entries"].as_array().is_some_and(|a| !a.is_empty());
    assert!(has_entries, "native mode should return tree entries");
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and forge API tokens"]
async fn native_codeberg_public_repo() {
    let state = skip_if_no_tokens();
    if !state.config.search.api.contains_key("gitea_code") {
        eprintln!("SKIP: CODEBERG_TOKEN not configured");
        return;
    }

    let v = run_repo_map(
        state,
        RepoMapArgs {
            host: Some("codeberg".into()),
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
    .expect("native codeberg repo_map");

    assert_eq!(
        v["mode"].as_str(),
        Some("native"),
        "expected native mode for Codeberg with token: {}",
        serde_json::to_string_pretty(&v).unwrap_or_default()
    );

    let commit_sha = v["commit_sha"].as_str().expect("commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA: {commit_sha}"
    );

    let has_entries = v["root_entries"].as_array().is_some_and(|a| !a.is_empty())
        || v["entries"].as_array().is_some_and(|a| !a.is_empty());
    assert!(has_entries, "native mode should return tree entries");
}
