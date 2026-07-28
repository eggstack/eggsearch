#![cfg(feature = "live-smoke")]

use std::sync::Arc;

use eggsearch::core::config::{ApiProviderConfig, AppConfig};
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{run_repo_map, RepoMapArgs};

fn required_env(name: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => panic!("{name} is required for native forge smoke test"),
    }
}

fn build_state_with_github() -> Arc<ServerState> {
    required_env("GITHUB_TOKEN");
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
    Arc::new(ServerState::build(cfg).expect("build GitHub native smoke state"))
}

fn build_state_with_gitlab() -> Arc<ServerState> {
    required_env("GITLAB_TOKEN");
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
    Arc::new(ServerState::build(cfg).expect("build GitLab native smoke state"))
}

fn build_state_with_codeberg() -> Arc<ServerState> {
    required_env("CODEBERG_TOKEN");
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
    Arc::new(ServerState::build(cfg).expect("build Codeberg native smoke state"))
}

fn build_state_with_gitea() -> Arc<ServerState> {
    required_env("GITEA_TOKEN");
    let base_url = required_env("GITEA_INSTANCE_URL");
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
    Arc::new(ServerState::build(cfg).expect("build Gitea native smoke state"))
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITHUB_TOKEN"]
async fn native_github_public_repo() {
    let state = build_state_with_github();

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

    let bytes_observed = v["response_bytes_observed"]
        .as_u64()
        .expect("response_bytes_observed present");
    assert!(
        bytes_observed > 0,
        "response_bytes_observed should be > 0 for native mode, got {bytes_observed}"
    );

    let aggregate_limit = v["aggregate_limit"]
        .as_u64()
        .expect("aggregate_limit present");
    assert!(
        aggregate_limit > 0,
        "aggregate_limit should be > 0, got {aggregate_limit}"
    );
    assert!(
        bytes_observed <= aggregate_limit,
        "response_bytes_observed ({bytes_observed}) must not exceed aggregate_limit ({aggregate_limit})"
    );

    let request_count = v["request_count"].as_u64().expect("request_count present");
    assert!(
        request_count > 0,
        "request_count should be > 0, got {request_count}"
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITHUB_TOKEN"]
async fn native_github_slash_ref() {
    let state = build_state_with_github();

    let slash_ref = required_env("GITHUB_SLASH_REF");
    assert!(slash_ref.contains('/'), "GITHUB_SLASH_REF must contain '/'");

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
        ref_name == slash_ref,
        "requested ref must be preserved, got ref_name={ref_name}, requested={slash_ref}"
    );

    let bytes_observed = v["response_bytes_observed"]
        .as_u64()
        .expect("response_bytes_observed present");
    assert!(
        bytes_observed > 0,
        "response_bytes_observed should be > 0 for native slash-ref, got {bytes_observed}"
    );

    let aggregate_limit = v["aggregate_limit"]
        .as_u64()
        .expect("aggregate_limit present");
    assert!(
        aggregate_limit > 0,
        "aggregate_limit should be > 0, got {aggregate_limit}"
    );
    assert!(
        bytes_observed <= aggregate_limit,
        "response_bytes_observed ({bytes_observed}) must not exceed aggregate_limit ({aggregate_limit})"
    );

    let request_count = v["request_count"].as_u64().expect("request_count present");
    assert!(
        request_count > 0,
        "request_count should be > 0, got {request_count}"
    );
    let resolved_ref = v["resolved_ref_name"]
        .as_str()
        .or_else(|| v["ref_name"].as_str())
        .expect("resolved_ref_name present");
    assert!(
        resolved_ref.contains('/'),
        "resolved ref should contain a slash, got resolved_ref={resolved_ref}"
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITLAB_TOKEN"]
async fn native_gitlab_public_repo() {
    let state = build_state_with_gitlab();

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

    let bytes_observed = v["response_bytes_observed"]
        .as_u64()
        .expect("response_bytes_observed present");
    assert!(
        bytes_observed > 0,
        "response_bytes_observed should be > 0 for native GitLab, got {bytes_observed}"
    );

    let aggregate_limit = v["aggregate_limit"]
        .as_u64()
        .expect("aggregate_limit present");
    assert!(
        aggregate_limit > 0,
        "aggregate_limit should be > 0, got {aggregate_limit}"
    );
    assert!(
        bytes_observed <= aggregate_limit,
        "response_bytes_observed ({bytes_observed}) must not exceed aggregate_limit ({aggregate_limit})"
    );

    let request_count = v["request_count"].as_u64().expect("request_count present");
    assert!(
        request_count > 0,
        "request_count should be > 0, got {request_count}"
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and CODEBERG_TOKEN"]
async fn native_codeberg_public_repo() {
    let state = build_state_with_codeberg();

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

    let bytes_observed = v["response_bytes_observed"]
        .as_u64()
        .expect("response_bytes_observed present");
    assert!(
        bytes_observed > 0,
        "response_bytes_observed should be > 0 for native Codeberg, got {bytes_observed}"
    );

    let aggregate_limit = v["aggregate_limit"]
        .as_u64()
        .expect("aggregate_limit present");
    assert!(
        aggregate_limit > 0,
        "aggregate_limit should be > 0, got {aggregate_limit}"
    );
    assert!(
        bytes_observed <= aggregate_limit,
        "response_bytes_observed ({bytes_observed}) must not exceed aggregate_limit ({aggregate_limit})"
    );

    let request_count = v["request_count"].as_u64().expect("request_count present");
    assert!(
        request_count > 0,
        "request_count should be > 0, got {request_count}"
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITEA_TOKEN"]
async fn native_gitea_public_repo() {
    let state = build_state_with_gitea();

    let base_url = required_env("GITEA_INSTANCE_URL");

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

    let bytes_observed = v["response_bytes_observed"]
        .as_u64()
        .expect("response_bytes_observed present");
    assert!(
        bytes_observed > 0,
        "response_bytes_observed should be > 0 for native Gitea, got {bytes_observed}"
    );

    let aggregate_limit = v["aggregate_limit"]
        .as_u64()
        .expect("aggregate_limit present");
    assert!(
        aggregate_limit > 0,
        "aggregate_limit should be > 0, got {aggregate_limit}"
    );
    assert!(
        bytes_observed <= aggregate_limit,
        "response_bytes_observed ({bytes_observed}) must not exceed aggregate_limit ({aggregate_limit})"
    );
    let request_count = v["request_count"].as_u64().expect("request_count present");
    assert!(
        request_count > 0,
        "request_count should be > 0, got {request_count}"
    );
}

use eggsearch::core::code_metadata::CodeHost;
use eggsearch::core::repo_map::RepoMapRequest;
use eggsearch::meta::forge_adapter::{fetch_tree, ForgeEndpointPolicy, ForgeTreeConfig};

fn direct_fetch_config_github() -> ForgeTreeConfig {
    let token = required_env("GITHUB_TOKEN");
    ForgeTreeConfig {
        api_key: Some(token),
        base_url: None,
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    }
}

fn direct_fetch_config_gitlab() -> ForgeTreeConfig {
    let token = required_env("GITLAB_TOKEN");
    ForgeTreeConfig {
        api_key: Some(token),
        base_url: None,
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    }
}

fn direct_fetch_config_codeberg() -> ForgeTreeConfig {
    let token = required_env("CODEBERG_TOKEN");
    ForgeTreeConfig {
        api_key: Some(token),
        base_url: Some("https://codeberg.org/api/v1".to_string()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    }
}

fn direct_fetch_config_gitea() -> ForgeTreeConfig {
    let token = required_env("GITEA_TOKEN");
    let base_url = required_env("GITEA_INSTANCE_URL");
    ForgeTreeConfig {
        api_key: Some(token),
        base_url: Some(base_url),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    }
}

fn default_request(host: CodeHost, owner: &str, repo: &str, ref_name: &str) -> RepoMapRequest {
    RepoMapRequest {
        query: String::new(),
        host: Some(host),
        owner: owner.into(),
        repo: repo.into(),
        ref_name: Some(ref_name.into()),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITHUB_TOKEN"]
async fn direct_fetch_tree_github() {
    let config = direct_fetch_config_github();
    let req = default_request(CodeHost::Github, "tokio-rs", "axum", "main");
    let response = fetch_tree(CodeHost::Github, "tokio-rs", "axum", &req, &config)
        .await
        .expect("direct fetch_tree for GitHub must succeed");

    assert_eq!(
        response.provider_id, "github_tree",
        "adapter must identify as github_tree"
    );
    assert!(
        !response.entries.is_empty(),
        "GitHub tree must have entries"
    );
    let commit_sha = response
        .identity
        .resolved_commit_sha
        .as_deref()
        .expect("resolved_commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA, got {commit_sha}"
    );
    assert!(
        response.identity.requested_ref.is_some(),
        "requested_ref must be preserved"
    );
    assert!(
        response.identity.resolved_ref_name.is_some(),
        "resolved_ref_name must be present"
    );
    assert!(
        response.response_bytes_observed > 0,
        "response_bytes_observed must be > 0"
    );
    assert!(response.request_count > 0, "request_count must be > 0");
    assert!(
        response.response_bytes_observed <= response.aggregate_limit,
        "response_bytes_observed ({}) must not exceed aggregate_limit ({})",
        response.response_bytes_observed,
        response.aggregate_limit
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITLAB_TOKEN"]
async fn direct_fetch_tree_gitlab() {
    let config = direct_fetch_config_gitlab();
    let req = default_request(CodeHost::Gitlab, "gitlab-org", "gitlab-runner", "main");
    let response = fetch_tree(
        CodeHost::Gitlab,
        "gitlab-org",
        "gitlab-runner",
        &req,
        &config,
    )
    .await
    .expect("direct fetch_tree for GitLab must succeed");

    assert_eq!(
        response.provider_id, "gitlab_tree",
        "adapter must identify as gitlab_tree"
    );
    assert!(
        !response.entries.is_empty(),
        "GitLab tree must have entries"
    );
    let commit_sha = response
        .identity
        .resolved_commit_sha
        .as_deref()
        .expect("resolved_commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA, got {commit_sha}"
    );
    assert!(
        response.response_bytes_observed > 0,
        "response_bytes_observed must be > 0"
    );
    assert!(response.request_count > 0, "request_count must be > 0");
    assert!(
        response.response_bytes_observed <= response.aggregate_limit,
        "response_bytes_observed ({}) must not exceed aggregate_limit ({})",
        response.response_bytes_observed,
        response.aggregate_limit
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and CODEBERG_TOKEN"]
async fn direct_fetch_tree_codeberg() {
    let config = direct_fetch_config_codeberg();
    let req = default_request(CodeHost::Codeberg, "Codeberg", "Forgejo", "main");
    let response = fetch_tree(CodeHost::Codeberg, "Codeberg", "Forgejo", &req, &config)
        .await
        .expect("direct fetch_tree for Codeberg must succeed");

    assert_eq!(
        response.provider_id, "codeberg_tree",
        "adapter must identify as codeberg_tree"
    );
    assert!(
        !response.entries.is_empty(),
        "Codeberg tree must have entries"
    );
    let commit_sha = response
        .identity
        .resolved_commit_sha
        .as_deref()
        .expect("resolved_commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA, got {commit_sha}"
    );
    assert!(
        response.response_bytes_observed > 0,
        "response_bytes_observed must be > 0"
    );
    assert!(response.request_count > 0, "request_count must be > 0");
    assert!(
        response.response_bytes_observed <= response.aggregate_limit,
        "response_bytes_observed ({}) must not exceed aggregate_limit ({})",
        response.response_bytes_observed,
        response.aggregate_limit
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITEA_TOKEN"]
async fn direct_fetch_tree_gitea() {
    let config = direct_fetch_config_gitea();
    let req = default_request(CodeHost::Gitea, "go-gitea", "gitea", "main");
    let response = fetch_tree(CodeHost::Gitea, "go-gitea", "gitea", &req, &config)
        .await
        .expect("direct fetch_tree for Gitea must succeed");

    assert!(
        response.provider_id.starts_with("gitea"),
        "adapter must identify as gitea, got {}",
        response.provider_id
    );
    assert!(!response.entries.is_empty(), "Gitea tree must have entries");
    let commit_sha = response
        .identity
        .resolved_commit_sha
        .as_deref()
        .expect("resolved_commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA, got {commit_sha}"
    );
    assert!(
        response.response_bytes_observed > 0,
        "response_bytes_observed must be > 0"
    );
    assert!(response.request_count > 0, "request_count must be > 0");
    assert!(
        response.response_bytes_observed <= response.aggregate_limit,
        "response_bytes_observed ({}) must not exceed aggregate_limit ({})",
        response.response_bytes_observed,
        response.aggregate_limit
    );
}

#[tokio::test]
#[ignore = "requires live network, live-smoke feature, and GITHUB_TOKEN"]
async fn direct_fetch_tree_github_slash_ref() {
    let config = direct_fetch_config_github();
    let slash_ref = required_env("GITHUB_SLASH_REF");
    assert!(slash_ref.contains('/'), "GITHUB_SLASH_REF must contain '/'");
    let req = default_request(CodeHost::Github, "tokio-rs", "axum", &slash_ref);
    let response = fetch_tree(CodeHost::Github, "tokio-rs", "axum", &req, &config)
        .await
        .expect("direct fetch_tree for GitHub slash-ref must succeed");

    assert_eq!(
        response.provider_id, "github_tree",
        "adapter must identify as github_tree"
    );
    assert!(
        !response.entries.is_empty(),
        "GitHub tree must have entries for slash ref"
    );
    let commit_sha = response
        .identity
        .resolved_commit_sha
        .as_deref()
        .expect("resolved_commit_sha present");
    assert!(
        commit_sha.len() >= 40,
        "commit_sha should be a full SHA, got {commit_sha}"
    );
    assert!(
        response.identity.requested_ref.as_deref() == Some(slash_ref.as_str()),
        "requested_ref must preserve GITHUB_SLASH_REF"
    );
    assert!(
        response.response_bytes_observed > 0,
        "response_bytes_observed must be > 0"
    );
    assert!(response.request_count > 0, "request_count must be > 0");
    assert!(
        response
            .identity
            .resolved_ref_name
            .as_deref()
            .is_some_and(|value| value.contains('/')),
        "resolved_ref_name must contain a slash"
    );
}
