//! Native forge adapter smoke tests.
//!
//! These tests exercise the native forge tree API adapters (GitHub, GitLab,
//! Codeberg, Gitea) against live public repositories. They require configured
//! API tokens and are classified as release-blocking evidence.
//!
//! Each test is independent: a missing token for one provider does not prevent
//! other provider tests from executing.
//!
//! Run with:
//! ```bash
//! GITHUB_TOKEN=ghp_xxx cargo test --features live-smoke --test native_forge_smoke -- --ignored
//! ```

#![cfg(feature = "live-smoke")]

use std::sync::Arc;

use eggsearch::core::config::{ApiProviderConfig, AppConfig};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_repo_map, RepoMapArgs};

fn build_state_with_github() -> Option<Arc<ServerState>> {
    let token = std::env::var("GITHUB_TOKEN").ok()?;
    if token.is_empty() {
        return None;
    }
    let mut cfg = AppConfig::default();
    cfg.search.mode = eggsearch::core::config::Mode::Live;
    cfg.search.api.insert(
        "github_code".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("GITHUB_TOKEN".to_string()),
            base_url: None,
        },
    );
    ServerState::build(cfg).ok().map(Arc::new)
}

fn build_state_with_gitlab() -> Option<Arc<ServerState>> {
    let token = std::env::var("GITLAB_TOKEN").ok()?;
    if token.is_empty() {
        return None;
    }
    let mut cfg = AppConfig::default();
    cfg.search.mode = eggsearch::core::config::Mode::Live;
    cfg.search.api.insert(
        "gitlab_code".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("GITLAB_TOKEN".to_string()),
            base_url: None,
        },
    );
    ServerState::build(cfg).ok().map(Arc::new)
}

fn build_state_with_codeberg() -> Option<Arc<ServerState>> {
    let token = std::env::var("CODEBERG_TOKEN").ok()?;
    if token.is_empty() {
        return None;
    }
    let mut cfg = AppConfig::default();
    cfg.search.mode = eggsearch::core::config::Mode::Live;
    cfg.search.api.insert(
        "gitea_code".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("CODEBERG_TOKEN".to_string()),
            base_url: Some("https://codeberg.org/api/v1".to_string()),
        },
    );
    ServerState::build(cfg).ok().map(Arc::new)
}

fn build_state_with_gitea() -> Option<Arc<ServerState>> {
    let token = std::env::var("GITEA_TOKEN").ok()?;
    if token.is_empty() {
        return None;
    }
    let base_url = std::env::var("GITEA_INSTANCE_URL")
        .unwrap_or_else(|_| "https://gitea.com/api/v1".to_string());
    let mut cfg = AppConfig::default();
    cfg.search.mode = eggsearch::core::config::Mode::Live;
    cfg.search.api.insert(
        "gitea_code".to_string(),
        ApiProviderConfig {
            enabled: true,
            api_key_env: Some("GITEA_TOKEN".to_string()),
            base_url: Some(base_url),
        },
    );
    ServerState::build(cfg).ok().map(Arc::new)
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITHUB_TOKEN"]
async fn native_github_public_repo() {
    let state = match build_state_with_github() {
        Some(s) => s,
        None => {
            eprintln!("SKIP: GITHUB_TOKEN not configured");
            return;
        }
    };

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
#[ignore = "requires live network, live-smoke feature, and GITHUB_TOKEN"]
async fn native_github_slash_ref() {
    let state = match build_state_with_github() {
        Some(s) => s,
        None => {
            eprintln!("SKIP: GITHUB_TOKEN not configured");
            return;
        }
    };

    let slash_ref =
        std::env::var("GITHUB_SLASH_REF").unwrap_or_else(|_| "smoke/slash-ref".to_string());

    let v = run_repo_map(
        state,
        RepoMapArgs {
            host: Some("github".into()),
            owner: "tokio-rs".into(),
            repo: "axum".into(),
            ref_name: Some(slash_ref.clone()),
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
    .expect("native github slash-ref repo_map");

    assert_eq!(
        v["mode"].as_str(),
        Some("native"),
        "expected native mode for GitHub slash-ref"
    );

    let commit_sha = v["commit_sha"].as_str().expect("commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA: {commit_sha}"
    );

    let ref_name = v["ref_name"].as_str().unwrap_or("");
    assert!(
        ref_name.contains('/'),
        "resolved ref should contain a slash, got ref_name={ref_name}"
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITLAB_TOKEN"]
async fn native_gitlab_public_repo() {
    let state = match build_state_with_gitlab() {
        Some(s) => s,
        None => {
            eprintln!("SKIP: GITLAB_TOKEN not configured");
            return;
        }
    };

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
#[ignore = "requires live network, live-smoke feature, and CODEBERG_TOKEN"]
async fn native_codeberg_public_repo() {
    let state = match build_state_with_codeberg() {
        Some(s) => s,
        None => {
            eprintln!("SKIP: CODEBERG_TOKEN not configured");
            return;
        }
    };

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

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITEA_TOKEN"]
async fn native_gitea_public_repo() {
    let state = match build_state_with_gitea() {
        Some(s) => s,
        None => {
            eprintln!("SKIP: GITEA_TOKEN not configured");
            return;
        }
    };

    let base_url = std::env::var("GITEA_INSTANCE_URL")
        .unwrap_or_else(|_| "https://gitea.com/api/v1".to_string());

    let v = run_repo_map(
        state,
        RepoMapArgs {
            host: Some("gitea".into()),
            owner: "go-gitea".into(),
            repo: "gitea".into(),
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
    .unwrap_or_else(|e| panic!("native gitea repo_map against {base_url}: {e}"));

    assert_eq!(
        v["mode"].as_str(),
        Some("native"),
        "expected native mode for Gitea with token: {}",
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
