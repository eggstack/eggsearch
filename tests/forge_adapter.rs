//! Fixture-based tests for the forge adapter (GitHub, GitLab, Gitea, Forgejo, Codeberg).
//!
//! These tests use `httpmock` to mock HTTP responses and verify the forge
//! adapter's behavior without network access.
//!
//! ```text
//! cargo test --all-features --test forge_adapter
//! ```

use httpmock::prelude::*;

use eggsearch::core::code_metadata::CodeHost;
use eggsearch::core::repo_map::{ImportantFileKind, RepoMapEntryKind, RepoMapMode, RepoMapRequest};
use eggsearch::meta::forge_adapter::{
    build_response, classify_ipv4_forge, classify_ipv6_forge, fetch_tree, EntryKind,
    ForgeAddressClass, ForgeEndpointPolicy, ForgeRawEntry, ForgeTreeConfig, ForgeTreeResponse,
    ResolvedRepositoryIdentity,
};

fn default_request(host: CodeHost, owner: &str, repo: &str) -> RepoMapRequest {
    RepoMapRequest {
        query: String::new(),
        host: Some(host),
        owner: owner.into(),
        repo: repo.into(),
        ref_name: Some("main".into()),
        ..Default::default()
    }
}

fn forge_response(entries: Vec<ForgeRawEntry>, provider_id: &str) -> ForgeTreeResponse {
    forge_response_with_commit(entries, provider_id, None)
}

fn forge_response_with_commit(
    entries: Vec<ForgeRawEntry>,
    provider_id: &str,
    commit_sha: Option<&str>,
) -> ForgeTreeResponse {
    ForgeTreeResponse {
        entries,
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            resolved_commit_sha: commit_sha.map(String::from),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: provider_id.into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    }
}

// ===========================================================================
// GitHub Adapter Tests
// ===========================================================================

#[test]
fn github_tree_small_repo() {
    let server = MockServer::start();
    let mock_commit = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/commits/main");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_abc123",
            "commit": {
                "tree": {
                    "sha": "tree_sha_def456"
                }
            }
        }));
    });
    let mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo")
            .header("User-Agent", "eggsearch/1.0");
        then.json_body(serde_json::json!({
            "default_branch": "main"
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/tree_sha_def456")
            .query_param("recursive", "1");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 100, "sha": "sha1"},
                {"path": "src", "type": "tree", "mode": "040000", "sha": "sha2"},
                {"path": "src/main.rs", "type": "blob", "mode": "100644", "size": 500, "sha": "sha3"},
                {"path": "Cargo.toml", "type": "blob", "mode": "100644", "size": 200, "sha": "sha4"},
            ]
        }));
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_commit.assert();
    mock_repo.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(resp.provider_id, "github_tree");
    assert_eq!(resp.entries.len(), 4);
    assert!(!resp.truncated_by_provider);
    assert_eq!(
        resp.identity.resolved_commit_sha.as_deref(),
        Some("commit_sha_abc123")
    );
    assert_eq!(resp.identity.tree_sha.as_deref(), Some("tree_sha_def456"));
}

#[test]
fn github_tree_truncated_falls_back_to_contents_api() {
    let server = MockServer::start();
    let mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo");
        then.json_body(serde_json::json!({
            "default_branch": "main"
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main");
        then.json_body(serde_json::json!({
            "sha": "abc123",
            "truncated": true,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 100, "sha": "sha1"},
                {"path": "src", "type": "tree", "mode": "040000", "sha": "sha2"},
            ]
        }));
    });
    let mock_contents = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/contents/");
        then.json_body(serde_json::json!([
            {"name": "README.md", "type": "file", "size": 100, "sha": "sha1"},
            {"name": "src", "type": "dir", "sha": "sha2"},
            {"name": "Cargo.toml", "type": "file", "size": 200, "sha": "sha4"},
        ]));
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_repo.assert();
    mock_tree.assert();
    mock_contents.assert();

    let resp = result.unwrap();
    assert!(resp.truncated_by_provider);
    let paths: Vec<&str> = resp.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains(&"README.md"));
    assert!(paths.contains(&"src"));
    assert!(paths.contains(&"Cargo.toml"));
}

#[test]
fn github_tree_404_returns_repository_not_found() {
    let server = MockServer::start();
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main");
        then.status(404);
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let err = result.unwrap_err();
    assert_eq!(err, "repository_not_found");
}

#[test]
fn github_tree_rate_limited() {
    let server = MockServer::start();
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main");
        then.status(403).body("API rate limit exceeded");
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let err = result.unwrap_err();
    assert_eq!(err, "rate_limited");
}

#[test]
fn github_tree_auth_required() {
    let server = MockServer::start();
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main");
        then.status(401);
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: Some("test-token".into()),
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let err = result.unwrap_err();
    assert_eq!(err, "authentication_required");
}

// ===========================================================================
// GitLab Adapter Tests
// ===========================================================================

#[test]
fn gitlab_tree_basic() {
    let server = MockServer::start();
    let mock_project = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo");
        then.json_body(serde_json::json!({
            "default_branch": "main"
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree")
            .query_param("recursive", "true");
        then.json_body(serde_json::json!([
            {"path": "README.md", "type": "blob", "size": 100, "id": "sha1"},
            {"path": "src", "type": "tree", "id": "sha2"},
            {"path": "src/main.rs", "type": "blob", "size": 500, "id": "sha3"},
        ]));
    });

    let req = default_request(CodeHost::Gitlab, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v4", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitlab,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_project.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(resp.provider_id, "gitlab_tree");
    assert_eq!(resp.entries.len(), 3);
    assert!(!resp.truncated_by_provider);
}

#[test]
fn gitlab_tree_nested_namespace() {
    let server = MockServer::start();
    let mock_project = server.mock(|when, then| {
        when.path("/api/v4/projects/subgroup%2Ftest-owner%2Ftest-repo");
        then.json_body(serde_json::json!({
            "default_branch": "main"
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v4/projects/subgroup%2Ftest-owner%2Ftest-repo/repository/tree");
        then.json_body(serde_json::json!([
            {"path": "README.md", "type": "blob", "size": 100, "id": "sha1"},
        ]));
    });

    let req = default_request(CodeHost::Gitlab, "subgroup/test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v4", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitlab,
        "subgroup/test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_project.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(resp.entries.len(), 1);
}

#[test]
fn gitlab_tree_404_returns_repository_not_found() {
    let server = MockServer::start();
    let mock_project = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree");
        then.status(404);
    });

    let req = default_request(CodeHost::Gitlab, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v4", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitlab,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_project.assert();

    let err = result.unwrap_err();
    assert_eq!(err, "repository_not_found");
}

#[test]
fn gitlab_tree_auth_required() {
    let server = MockServer::start();
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree");
        then.status(401);
    });

    let req = default_request(CodeHost::Gitlab, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: Some("test-token".into()),
        base_url: Some(format!("{}/api/v4", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitlab,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let err = result.unwrap_err();
    assert_eq!(err, "authentication_required");
}

// ===========================================================================
// Gitea/Forgejo/Codeberg Adapter Tests
// ===========================================================================

#[test]
fn forge_tree_codeberg_basic() {
    let server = MockServer::start();
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/git/trees/main");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 100, "sha": "sha1"},
                {"path": "src", "type": "tree", "mode": "040000", "sha": "sha2"},
            ]
        }));
    });

    let req = default_request(CodeHost::Codeberg, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v1", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Codeberg,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(resp.provider_id, "codeberg_tree");
    assert_eq!(resp.entries.len(), 2);
}

#[test]
fn forge_tree_gitea_with_custom_base_url() {
    let server = MockServer::start();
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/git/trees/main");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 100, "sha": "sha1"},
            ]
        }));
    });

    let req = default_request(CodeHost::Gitea, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v1", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitea,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(resp.provider_id, "gitea_tree");
    assert_eq!(resp.entries.len(), 1);
}

// ===========================================================================
// build_response Tests
// ===========================================================================

#[test]
fn build_response_populates_language_hints() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = forge_response(
        vec![
            ForgeRawEntry {
                path: "main.rs".into(),
                kind: EntryKind::File,
                size: Some(1024),
                object_sha: Some("sha1".into()),
            },
            ForgeRawEntry {
                path: "app.py".into(),
                kind: EntryKind::File,
                size: Some(512),
                object_sha: Some("sha2".into()),
            },
            ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(200),
                object_sha: Some("sha3".into()),
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    let main_rs = resp
        .root_entries
        .iter()
        .find(|e| e.path == "main.rs")
        .unwrap();
    assert_eq!(main_rs.language.as_deref(), Some("rust"));
    let app_py = resp
        .root_entries
        .iter()
        .find(|e| e.path == "app.py")
        .unwrap();
    assert_eq!(app_py.language.as_deref(), Some("python"));
    let readme = resp
        .root_entries
        .iter()
        .find(|e| e.path == "README.md")
        .unwrap();
    assert_eq!(readme.language.as_deref(), Some("markdown"));
}

#[test]
fn build_response_populates_manifests() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = forge_response(
        vec![
            ForgeRawEntry {
                path: "Cargo.toml".into(),
                kind: EntryKind::File,
                size: Some(200),
                object_sha: Some("sha1".into()),
            },
            ForgeRawEntry {
                path: "package.json".into(),
                kind: EntryKind::File,
                size: Some(100),
                object_sha: Some("sha2".into()),
            },
            ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(50),
                object_sha: Some("sha3".into()),
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert_eq!(resp.manifests.len(), 2);
    let manifest_paths: Vec<&str> = resp.manifests.iter().map(|m| m.path.as_str()).collect();
    assert!(manifest_paths.contains(&"Cargo.toml"));
    assert!(manifest_paths.contains(&"package.json"));
    assert!(resp
        .manifests
        .iter()
        .all(|m| m.kind == ImportantFileKind::Manifest));
}

#[test]
fn build_response_depth_filtering() {
    let req = RepoMapRequest {
        max_depth: Some(2),
        ..default_request(CodeHost::Github, "test", "repo")
    };
    let forge = forge_response(
        vec![
            ForgeRawEntry {
                path: "src".into(),
                kind: EntryKind::Directory,
                size: None,
                object_sha: None,
            },
            ForgeRawEntry {
                path: "src/main.rs".into(),
                kind: EntryKind::File,
                size: Some(100),
                object_sha: None,
            },
            ForgeRawEntry {
                path: "src/deep/nested/file.rs".into(),
                kind: EntryKind::File,
                size: Some(50),
                object_sha: None,
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    let root_paths: Vec<&str> = resp.root_entries.iter().map(|e| e.path.as_str()).collect();
    assert!(root_paths.contains(&"src"));
    assert!(!root_paths.contains(&"src/main.rs"));
    assert!(!root_paths.contains(&"src/deep/nested/file.rs"));
    let all_paths: Vec<&str> = resp.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(all_paths.contains(&"src"));
    assert!(all_paths.contains(&"src/main.rs"));
    assert!(!all_paths.contains(&"src/deep/nested/file.rs"));
}

#[test]
fn build_response_include_files_false() {
    let req = RepoMapRequest {
        include_files: Some(false),
        ..default_request(CodeHost::Github, "test", "repo")
    };
    let forge = forge_response(
        vec![
            ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(100),
                object_sha: None,
            },
            ForgeRawEntry {
                path: "src".into(),
                kind: EntryKind::Directory,
                size: None,
                object_sha: None,
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, false, true, true, true, None);
    assert!(resp
        .root_entries
        .iter()
        .all(|e| e.kind != RepoMapEntryKind::File));
}

#[test]
fn build_response_truncated_produces_structured_warning() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = ForgeTreeResponse {
        entries: vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: None,
        }],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            ..Default::default()
        },
        truncated_by_provider: true,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert!(resp
        .structured_warnings
        .iter()
        .any(|w| w.code == eggsearch::core::warning::WarningCode::ForgeTreeTruncated));
}

#[test]
fn build_response_native_mode() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = forge_response(vec![], "github_tree");
    let resp = build_response(&req, forge, true, true, true, true, None);
    assert!(matches!(resp.mode, RepoMapMode::Native));
    assert_eq!(resp.providers_queried, vec!["github_tree"]);
    assert!(resp.telemetry.is_some());
}

#[test]
fn build_response_symlink_and_submodule_entries() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = forge_response(
        vec![
            ForgeRawEntry {
                path: "link".into(),
                kind: EntryKind::Symlink,
                size: None,
                object_sha: None,
            },
            ForgeRawEntry {
                path: "vendor".into(),
                kind: EntryKind::Submodule,
                size: None,
                object_sha: None,
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    let link = resp.root_entries.iter().find(|e| e.path == "link").unwrap();
    assert_eq!(link.kind, RepoMapEntryKind::Symlink);
    let vendor = resp
        .root_entries
        .iter()
        .find(|e| e.path == "vendor")
        .unwrap();
    assert_eq!(vendor.kind, RepoMapEntryKind::Submodule);
}

// ===========================================================================
// Contract Tests: Equivalent Schema Output Across Hosts
// ===========================================================================

#[test]
fn contract_all_hosts_produce_equivalent_response_shape() {
    let hosts = [
        (CodeHost::Github, "github_tree"),
        (CodeHost::Gitlab, "gitlab_tree"),
        (CodeHost::Codeberg, "codeberg_tree"),
        (CodeHost::Gitea, "gitea_tree"),
        (CodeHost::Forgejo, "forgejo_tree"),
    ];

    for (host, provider_id) in hosts {
        let req = default_request(host, "test-owner", "test-repo");
        let forge = forge_response(
            vec![
                ForgeRawEntry {
                    path: "README.md".into(),
                    kind: EntryKind::File,
                    size: Some(100),
                    object_sha: Some("sha1".into()),
                },
                ForgeRawEntry {
                    path: "src".into(),
                    kind: EntryKind::Directory,
                    size: None,
                    object_sha: Some("sha2".into()),
                },
                ForgeRawEntry {
                    path: "Cargo.toml".into(),
                    kind: EntryKind::File,
                    size: Some(200),
                    object_sha: Some("sha3".into()),
                },
            ],
            provider_id,
        );

        let resp = build_response(&req, forge, true, true, true, true, None);

        assert_eq!(resp.host, host);
        assert_eq!(resp.owner, "test-owner");
        assert_eq!(resp.repo, "test-repo");
        assert!(matches!(resp.mode, RepoMapMode::Native));
        assert_eq!(resp.root_entries.len(), 3);
        assert_eq!(resp.manifests.len(), 1);
        assert_eq!(resp.manifests[0].path, "Cargo.toml");
        assert!(!resp.warnings.is_empty() || resp.warnings.is_empty());
        assert!(resp.telemetry.is_some());
        assert_eq!(resp.providers_queried, vec![provider_id.to_string()]);

        let _json = serde_json::to_value(&resp).unwrap();
    }
}

// ===========================================================================
// Pagination and Entry Bound Tests
// ===========================================================================

#[test]
fn forge_truncated_provider_produces_structured_warning() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![
            ForgeRawEntry {
                path: "a.txt".into(),
                kind: EntryKind::File,
                size: Some(10),
                object_sha: None,
            },
            ForgeRawEntry {
                path: "b.txt".into(),
                kind: EntryKind::File,
                size: Some(10),
                object_sha: None,
            },
            ForgeRawEntry {
                path: "c.txt".into(),
                kind: EntryKind::File,
                size: Some(10),
                object_sha: None,
            },
        ],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            ..Default::default()
        },
        truncated_by_provider: true,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert_eq!(resp.root_entries.len(), 3);
    assert!(resp
        .structured_warnings
        .iter()
        .any(|w| w.code == eggsearch::core::warning::WarningCode::ForgeTreeTruncated));
}

#[test]
fn forge_depth_limit_filters_nested_entries() {
    let req = RepoMapRequest {
        max_depth: Some(1),
        ..default_request(CodeHost::Github, "test-owner", "test-repo")
    };
    let forge = ForgeTreeResponse {
        entries: vec![
            ForgeRawEntry {
                path: "src".into(),
                kind: EntryKind::Directory,
                size: None,
                object_sha: None,
            },
            ForgeRawEntry {
                path: "src/main.rs".into(),
                kind: EntryKind::File,
                size: Some(100),
                object_sha: None,
            },
            ForgeRawEntry {
                path: "src/deep/nested.rs".into(),
                kind: EntryKind::File,
                size: Some(50),
                object_sha: None,
            },
        ],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    let paths: Vec<&str> = resp.root_entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains(&"src"));
    assert!(!paths.contains(&"src/main.rs"));
    assert!(!paths.contains(&"src/deep/nested.rs"));
}

// ===========================================================================
// Partial Result on Failure Tests
// ===========================================================================

#[test]
fn forge_partial_result_when_page_limit_reached() {
    let server = MockServer::start();
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/git/trees/main");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 100, "sha": "sha1"},
            ]
        }));
    });

    let req = default_request(CodeHost::Codeberg, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v1", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Codeberg,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(resp.entries.len(), 1);
    assert_eq!(resp.entries[0].path, "README.md");
}

#[test]
fn build_response_preserves_partial_entries() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = ForgeTreeResponse {
        entries: vec![
            ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(100),
                object_sha: Some("sha1".into()),
            },
            ForgeRawEntry {
                path: "missing/deep/file.rs".into(),
                kind: EntryKind::File,
                size: Some(50),
                object_sha: None,
            },
        ],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    let root_paths: Vec<&str> = resp.root_entries.iter().map(|e| e.path.as_str()).collect();
    assert!(root_paths.contains(&"README.md"));
    assert!(!root_paths.contains(&"missing/deep/file.rs"));
    let all_paths: Vec<&str> = resp.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(all_paths.contains(&"README.md"));
    assert!(all_paths.contains(&"missing/deep/file.rs"));
}

// ===========================================================================
// URL Construction Tests
// ===========================================================================

#[test]
fn build_response_populates_urls_for_github() {
    let req = default_request(CodeHost::Github, "octocat", "hello-world");
    let forge = forge_response_with_commit(
        vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("blob_sha".into()),
        }],
        "github_tree",
        Some("commit_sha_abc123"),
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    let entry = &resp.root_entries[0];
    assert!(entry
        .url
        .as_deref()
        .unwrap()
        .contains("github.com/octocat/hello-world"));
    assert!(entry
        .raw_url
        .as_deref()
        .unwrap()
        .contains("raw.githubusercontent.com"));
    assert!(entry.url.as_deref().unwrap().contains("commit_sha_abc123"));
    assert!(!entry.url.as_deref().unwrap().contains("blob_sha"));
}

#[test]
fn build_response_populates_urls_for_gitlab() {
    let req = default_request(CodeHost::Gitlab, "octocat", "hello-world");
    let forge = forge_response(
        vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("sha1".into()),
        }],
        "gitlab_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    let entry = &resp.root_entries[0];
    assert!(entry.url.as_deref().unwrap().contains("gitlab.com"));
    assert!(entry.raw_url.as_deref().unwrap().contains("raw"));
}

#[test]
fn build_response_populates_urls_for_codeberg() {
    let req = default_request(CodeHost::Codeberg, "octocat", "hello-world");
    let forge = forge_response(
        vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("sha1".into()),
        }],
        "codeberg_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    let entry = &resp.root_entries[0];
    assert!(entry.url.as_deref().unwrap().contains("codeberg.org"));
    assert!(entry.raw_url.as_deref().unwrap().contains("codeberg.org"));
}

#[test]
fn build_response_populates_urls_for_gitea_with_base() {
    let req = default_request(CodeHost::Gitea, "octocat", "hello-world");
    let forge = forge_response(
        vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("sha1".into()),
        }],
        "gitea_tree",
    );

    let resp = build_response(
        &req,
        forge,
        true,
        true,
        true,
        true,
        Some("https://gitea.example.com"),
    );
    let entry = &resp.root_entries[0];
    assert!(entry.url.as_deref().unwrap().contains("gitea.example.com"));
    assert!(entry
        .raw_url
        .as_deref()
        .unwrap()
        .contains("gitea.example.com"));
}

#[test]
fn build_response_no_urls_for_unknown_host() {
    let req = RepoMapRequest {
        host: Some(CodeHost::Unknown),
        owner: "test".into(),
        repo: "repo".into(),
        ..Default::default()
    };
    let forge = forge_response(
        vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: None,
        }],
        "unknown",
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    let entry = &resp.root_entries[0];
    assert!(entry.url.is_none());
    assert!(entry.raw_url.is_none());
}

// ===========================================================================
// Base URL Validation Tests
// ===========================================================================

#[test]
fn validate_base_url_https_ok() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://codeberg.org/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_ok());
}

#[test]
fn validate_base_url_http_localhost_ok() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "http://localhost:3000/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_ok());
}

#[test]
fn validate_base_url_https_localhost_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://localhost/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_non_http_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "ftp://example.com",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_http_private_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "http://192.168.1.1/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_http_10_private_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "http://10.0.0.1/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_credential_bearing_http_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "http://example.com/api/v1",
        Some("my-token"),
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_credential_bearing_http_localhost_ok() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "http://localhost:3000/api/v1",
        Some("my-token"),
        &ForgeEndpointPolicy::default()
    )
    .is_ok());
}

#[test]
fn validate_base_url_credential_bearing_https_ok() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://example.com/api/v1",
        Some("my-token"),
        &ForgeEndpointPolicy::default()
    )
    .is_ok());
}

#[test]
fn validate_base_url_embedded_credentials_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://user:pass@example.com/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_ipv6_loopback_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://[::1]/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_ipv6_private_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://[fc00::1]/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_ipv6_ula_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://[fd00::1]/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_ipv6_documentation_rejected() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://[2001:db8::1]/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_err());
}

#[test]
fn validate_base_url_ipv6_public_ok() {
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://[2607:f8b0:4004:800::200e]/api/v1",
        None,
        &ForgeEndpointPolicy::default()
    )
    .is_ok());
}

// ===========================================================================
// Schema Compatibility Test
// ===========================================================================

#[test]
fn repo_map_response_schema_is_additive_compatible() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = forge_response(
        vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("sha1".into()),
        }],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true, None);
    let json = serde_json::to_value(&resp).unwrap();

    let obj = json.as_object().unwrap();

    assert!(obj.contains_key("query"));
    assert!(obj.contains_key("host"));
    assert!(obj.contains_key("owner"));
    assert!(obj.contains_key("repo"));
    assert!(obj.contains_key("mode"));
    assert!(obj.contains_key("root_entries"));
    assert!(obj.contains_key("trust_markers"));

    let entry = &obj["root_entries"][0];
    assert!(entry.get("path").is_some());
    assert!(entry.get("kind").is_some());
    assert!(entry.get("url").is_some());
    assert!(entry.get("raw_url").is_some());
}

// ===========================================================================
// Bounded Response Reader Tests
// ===========================================================================

#[test]
fn bounded_reader_rejects_honest_content_length_over_cap() {
    let server = MockServer::start();
    let large_body: String = "x".repeat(1024 * 1024 + 1);
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main");
        then.status(200).body(large_body);
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let err = result.unwrap_err();
    assert!(
        err == "response_too_large" || err.contains("failed to read") || err.contains("malformed"),
        "expected response_too_large or parse error, got: {err}"
    );
}

#[test]
fn bounded_reader_enforces_total_bytes_across_pages() {
    let server = MockServer::start();
    let page1_tree: Vec<serde_json::Value> = (0..100)
        .map(|i| {
            serde_json::json!({
                "path": format!("file_{i}.txt"),
                "type": "blob",
                "size": 100,
                "id": format!("sha{i}"),
            })
        })
        .collect();
    let page2_tree: Vec<serde_json::Value> = (100..120)
        .map(|i| {
            serde_json::json!({
                "path": format!("file_{i}.txt"),
                "type": "blob",
                "size": 100,
                "id": format!("sha{i}"),
            })
        })
        .collect();
    let mock_page1 = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree")
            .query_param("page", "1");
        then.json_body(serde_json::json!(page1_tree));
    });
    let mock_page2 = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree")
            .query_param("page", "2");
        then.json_body(serde_json::json!(page2_tree));
    });

    let req = RepoMapRequest {
        max_entries: Some(200),
        ..default_request(CodeHost::Gitlab, "test-owner", "test-repo")
    };
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v4", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitlab,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_page1.assert();
    mock_page2.assert();

    match result {
        Ok(resp) => {
            assert!(
                resp.entries.len() <= 200,
                "entries should respect max_entries"
            );
        }
        Err(e) => {
            assert!(
                e.contains("response_too_large") || e.contains("malformed"),
                "expected response_too_large or parse error, got: {e}"
            );
        }
    }
}

// ===========================================================================
// Commit SHA Provenance Tests
// ===========================================================================

#[test]
fn build_response_commit_sha_uses_resolved_ref() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![
            ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(100),
                object_sha: Some("blob_sha_not_commit".into()),
            },
            ForgeRawEntry {
                path: "src".into(),
                kind: EntryKind::Directory,
                size: None,
                object_sha: Some("tree_sha_not_commit".into()),
            },
        ],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_commit_sha: Some("abc123def456".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert_eq!(
        resp.commit_sha.as_deref(),
        Some("abc123def456"),
        "commit_sha should be the resolved commit SHA, not entry object SHA"
    );
}

#[test]
fn build_response_commit_sha_none_when_no_resolved_ref() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("blob_sha".into()),
        }],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_commit_sha: None,
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert_eq!(resp.commit_sha, None);
}

#[test]
fn build_response_resolved_ref_name_populated() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("sha1".into()),
        }],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert_eq!(
        resp.resolved_ref_name.as_deref(),
        Some("main"),
        "resolved_ref_name should hold the original ref name"
    );
}

// ===========================================================================
// Additional Provenance Tests (B.6)
// ===========================================================================

#[test]
fn provenance_branch_ref_resolves_to_commit_sha() {
    let server = MockServer::start();
    let mock_commit = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/commits/feature%2Fmy-branch");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_feature",
            "commit": {
                "tree": { "sha": "tree_sha_feature" }
            }
        }));
    });
    let mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo");
        then.json_body(serde_json::json!({ "default_branch": "main" }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/tree_sha_feature");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [{"path": "README.md", "type": "blob", "mode": "100644", "size": 10, "sha": "blob1"}]
        }));
    });

    let req = RepoMapRequest {
        ref_name: Some("feature/my-branch".into()),
        ..default_request(CodeHost::Github, "test-owner", "test-repo")
    };
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_commit.assert();
    mock_repo.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(
        resp.identity.resolved_commit_sha.as_deref(),
        Some("commit_sha_feature")
    );
    assert_eq!(resp.identity.tree_sha.as_deref(), Some("tree_sha_feature"));
}

#[test]
fn provenance_commit_sha_differs_from_tree_sha_and_blob_sha() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![
            ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(100),
                object_sha: Some("blob_sha_aaa".into()),
            },
            ForgeRawEntry {
                path: "src".into(),
                kind: EntryKind::Directory,
                size: None,
                object_sha: Some("tree_sha_bbb".into()),
            },
        ],
        identity: ResolvedRepositoryIdentity {
            requested_ref: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            resolved_commit_sha: Some("commit_sha_ccc".into()),
            tree_sha: Some("tree_sha_ddd".into()),
            default_branch: Some("main".into()),
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert_eq!(resp.commit_sha.as_deref(), Some("commit_sha_ccc"));
    assert_eq!(resp.tree_sha.as_deref(), Some("tree_sha_ddd"));

    let file_entry = resp
        .root_entries
        .iter()
        .find(|e| e.path == "README.md")
        .unwrap();
    let url = file_entry.url.as_deref().unwrap();
    assert!(
        url.contains("commit_sha_ccc"),
        "URL should use commit SHA, not blob SHA"
    );
    assert!(
        !url.contains("blob_sha_aaa"),
        "URL must not contain blob SHA"
    );

    let dir_entry = resp.entries.iter().find(|e| e.path == "src").unwrap();
    let dir_url = dir_entry.url.as_deref().unwrap();
    assert!(
        dir_url.contains("commit_sha_ccc"),
        "Directory URL should use commit SHA"
    );
}

#[test]
fn provenance_directory_entries_omit_raw_url() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![ForgeRawEntry {
            path: "src".into(),
            kind: EntryKind::Directory,
            size: None,
            object_sha: Some("tree_sha".into()),
        }],
        identity: ResolvedRepositoryIdentity {
            resolved_commit_sha: Some("commit_sha".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    let dir_entry = &resp.entries[0];
    assert!(
        dir_entry.raw_url.is_none(),
        "Directory entries should not have raw URLs"
    );
}

#[test]
fn provenance_unpinned_fallback_uses_ref_name() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("blob_sha".into()),
        }],
        identity: ResolvedRepositoryIdentity {
            requested_ref: Some("develop".into()),
            resolved_ref_name: Some("develop".into()),
            resolved_commit_sha: None,
            tree_sha: None,
            default_branch: Some("main".into()),
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert!(!resp.provenance_pinned);
    assert!(resp.commit_sha.is_none());

    let entry = &resp.root_entries[0];
    let url = entry.url.as_deref().unwrap();
    assert!(
        url.contains("develop"),
        "URL should use ref name when commit SHA is unavailable"
    );
}

#[test]
fn provenance_provenance_pinned_true_when_commit_present() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![],
        identity: ResolvedRepositoryIdentity {
            resolved_commit_sha: Some("abc123".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert!(resp.provenance_pinned);
}

#[test]
fn provenance_provenance_pinned_false_when_no_commit() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![],
        identity: ResolvedRepositoryIdentity {
            resolved_commit_sha: None,
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert!(!resp.provenance_pinned);
}

#[test]
fn provenance_tree_sha_preserved_in_response() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![],
        identity: ResolvedRepositoryIdentity {
            resolved_commit_sha: Some("commit_sha".into()),
            tree_sha: Some("tree_sha".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    assert_eq!(resp.tree_sha.as_deref(), Some("tree_sha"));
}

#[test]
fn provenance_serialization_additive_compatible() {
    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let forge = ForgeTreeResponse {
        entries: vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("blob_sha".into()),
        }],
        identity: ResolvedRepositoryIdentity {
            requested_ref: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            resolved_commit_sha: Some("commit_sha".into()),
            tree_sha: Some("tree_sha".into()),
            default_branch: Some("main".into()),
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: None,
        response_bytes_observed: 0,
        response_cap_applied: false,
        dns_policy_class: None,
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    let json = serde_json::to_value(&resp).unwrap();

    assert!(json.get("query").is_some());
    assert!(json.get("host").is_some());
    assert!(json.get("owner").is_some());
    assert!(json.get("repo").is_some());
    assert!(json.get("commit_sha").is_some());
    assert!(json.get("tree_sha").is_some());
    assert!(json.get("resolved_ref_name").is_some());
    assert!(json.get("default_branch").is_some());
    assert!(json.get("provenance_pinned").is_some());
    assert!(json.get("mode").is_some());
    assert!(json.get("entries").is_some());
    assert!(json.get("root_entries").is_some());
}

// ===========================================================================
// Error Body Preview Tests
// ===========================================================================

#[tokio::test]
async fn test_error_body_preview_caps_at_8kb() {
    let server = MockServer::start();
    let large_body: String = "x".repeat(16 * 1024);
    let mock = server.mock(|when, then| {
        when.path("/test");
        then.status(500).body(large_body);
    });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("{}/test", server.base_url()))
        .send()
        .await
        .unwrap();

    let preview = eggsearch::meta::forge_adapter::read_error_body_preview(resp).await;
    assert!(
        preview.len() <= 8 * 1024,
        "preview should be capped at 8KB, got {} bytes",
        preview.len()
    );
    mock.assert();
}

// ===========================================================================
// UTF-8 Boundary Tests
// ===========================================================================

#[tokio::test]
async fn test_valid_utf8_split_across_chunks() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.path("/test");
        then.status(200).body("placeholder");
    });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let _resp = client
        .get(format!("{}/test", server.base_url()))
        .send()
        .await
        .unwrap();

    let bytes_part1: &[u8] = &[0xE4, 0xB8, 0xAD]; // "中" (3 bytes)
    let bytes_part2: &[u8] = &[0xE6, 0x96, 0x87]; // "文" (3 bytes)
    let mut combined = Vec::new();
    combined.extend_from_slice(bytes_part1);
    combined.extend_from_slice(bytes_part2);

    let result = std::str::from_utf8(&combined);
    assert!(result.is_ok(), "combined bytes should be valid UTF-8");
    assert_eq!(result.unwrap(), "中文");

    let result_split = std::str::from_utf8(bytes_part1);
    assert!(
        result_split.is_ok(),
        "first chunk should be valid UTF-8 on its own"
    );

    mock.assert();
}

#[tokio::test]
async fn test_invalid_utf8_rejected_deterministically() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.path("/test");
        then.status(200).body("placeholder");
    });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let _resp = client
        .get(format!("{}/test", server.base_url()))
        .send()
        .await
        .unwrap();

    #[allow(invalid_from_utf8)]
    let invalid_bytes: &[u8] = &[0xFF, 0xFE];
    #[allow(invalid_from_utf8)]
    let result = std::str::from_utf8(invalid_bytes);
    assert!(result.is_err(), "invalid UTF-8 should be rejected");

    // Verify deterministic rejection of the same invalid bytes
    #[allow(invalid_from_utf8)]
    let result2 = std::str::from_utf8(invalid_bytes);
    assert!(
        result2.is_err(),
        "same invalid bytes should be rejected again deterministically"
    );

    mock.assert();
}

// ===========================================================================
// Missing Host Validation Tests
// ===========================================================================

#[test]
fn test_missing_host_rejected() {
    let result = eggsearch::meta::forge_adapter::validate_base_url(
        "",
        None,
        &ForgeEndpointPolicy::default(),
    );
    assert!(result.is_err(), "empty URL should be rejected");
}

// ===========================================================================
// DNS Classification Tests (direct classification function tests)
// ===========================================================================

#[test]
fn test_dns_resolving_to_loopback_rejected() {
    let loopback_v4 = std::net::Ipv4Addr::new(127, 0, 0, 1);
    assert_eq!(
        classify_ipv4_forge(loopback_v4),
        ForgeAddressClass::Loopback
    );

    let loopback_v6 = std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
    assert_eq!(
        classify_ipv6_forge(loopback_v6),
        ForgeAddressClass::Loopback
    );

    let loopback_v4_full = std::net::Ipv4Addr::new(127, 255, 255, 255);
    assert_eq!(
        classify_ipv4_forge(loopback_v4_full),
        ForgeAddressClass::Loopback
    );
}

#[test]
fn test_dns_resolving_to_private_ipv4_rejected() {
    let private_10 = std::net::Ipv4Addr::new(10, 0, 0, 1);
    assert_eq!(classify_ipv4_forge(private_10), ForgeAddressClass::Private);

    let private_172 = std::net::Ipv4Addr::new(172, 16, 0, 1);
    assert_eq!(classify_ipv4_forge(private_172), ForgeAddressClass::Private);

    let private_192 = std::net::Ipv4Addr::new(192, 168, 1, 1);
    assert_eq!(classify_ipv4_forge(private_192), ForgeAddressClass::Private);
}

#[test]
fn test_dns_resolving_to_private_ipv6_rejected() {
    let ula = std::net::Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1);
    assert_eq!(classify_ipv6_forge(ula), ForgeAddressClass::Private);

    let ula_fd = std::net::Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1);
    assert_eq!(classify_ipv6_forge(ula_fd), ForgeAddressClass::Private);
}

#[test]
fn test_mixed_public_private_dns_rejected() {
    let public_v4 = std::net::Ipv4Addr::new(8, 8, 8, 8);
    assert_eq!(classify_ipv4_forge(public_v4), ForgeAddressClass::Public);

    let link_local = std::net::Ipv4Addr::new(169, 254, 0, 1);
    assert_eq!(
        classify_ipv4_forge(link_local),
        ForgeAddressClass::LinkLocal
    );

    let documentation = std::net::Ipv4Addr::new(192, 0, 2, 1);
    assert_eq!(
        classify_ipv4_forge(documentation),
        ForgeAddressClass::Documentation
    );

    let multicast = std::net::Ipv4Addr::new(224, 0, 0, 1);
    assert_eq!(classify_ipv4_forge(multicast), ForgeAddressClass::Reserved);
}

#[test]
fn test_private_network_policy_allows_internal_forge() {
    let policy = ForgeEndpointPolicy {
        allow_loopback: true,
        allow_private_network: true,
        require_https: true,
    };
    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://localhost/api/v1",
        None,
        &policy,
    )
    .is_ok());

    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://[::1]/api/v1",
        None,
        &policy,
    )
    .is_ok());

    assert!(eggsearch::meta::forge_adapter::validate_base_url(
        "https://192.168.1.1/api/v1",
        None,
        &policy,
    )
    .is_ok());
}

// ===========================================================================
// Redirect Policy Tests
// ===========================================================================

#[tokio::test]
async fn test_redirect_from_public_to_loopback_rejected() {
    let server = MockServer::start();
    let mock_redirect = server.mock(|when, then| {
        when.path("/redirect");
        then.status(302)
            .header("Location", format!("{}/target", server.base_url()));
    });
    let mock_target = server.mock(|when, then| {
        when.path("/target");
        then.status(200).body("ok");
    });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("{}/redirect", server.base_url()))
        .send()
        .await
        .unwrap();

    assert!(
        resp.status().is_redirection(),
        "redirect should not be followed; got status {}",
        resp.status()
    );
    assert_eq!(
        resp.status().as_u16(),
        302,
        "should get the redirect response, not the target"
    );

    mock_redirect.assert();
    mock_target.assert_hits(0);
}

#[tokio::test]
async fn test_cross_origin_redirect_rejected() {
    let server = MockServer::start();
    let mock_redirect = server.mock(|when, then| {
        when.path("/redirect");
        then.status(301)
            .header("Location", "https://evil.example.com/stolen");
    });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("{}/redirect", server.base_url()))
        .send()
        .await
        .unwrap();

    assert!(
        resp.status().is_redirection(),
        "cross-origin redirect should not be followed; got status {}",
        resp.status()
    );
    assert_eq!(resp.status().as_u16(), 301);

    mock_redirect.assert();
}

#[tokio::test]
async fn test_same_origin_redirect_rejected() {
    let server = MockServer::start();
    let mock_redirect = server.mock(|when, then| {
        when.path("/old");
        then.status(301)
            .header("Location", format!("{}/new", server.base_url()));
    });
    let mock_new = server.mock(|when, then| {
        when.path("/new");
        then.status(200).body("new content");
    });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("{}/old", server.base_url()))
        .send()
        .await
        .unwrap();

    assert!(
        resp.status().is_redirection(),
        "same-origin redirect should not be followed; got status {}",
        resp.status()
    );
    assert_eq!(resp.status().as_u16(), 301);

    mock_redirect.assert();
    mock_new.assert_hits(0);
}

// ===========================================================================
// Fixed Bounded Reader Tests
// ===========================================================================

#[test]
fn bounded_reader_caps_chunked_response() {
    let server = MockServer::start();
    let large_body: String = "x".repeat(2048);
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main");
        then.status(200).body(large_body);
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_tree.assert();

    let err = result.unwrap_err();
    assert!(
        err.contains("response_too_large") || err.contains("malformed"),
        "expected response_too_large or parse error for oversized chunked response, got: {err}"
    );
}

// ===========================================================================
// Telemetry Population Tests
// ===========================================================================

#[test]
fn telemetry_populated_from_forge_response() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = ForgeTreeResponse {
        entries: vec![ForgeRawEntry {
            path: "README.md".into(),
            kind: EntryKind::File,
            size: Some(100),
            object_sha: Some("sha1".into()),
        }],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: Some("api.github.com".into()),
        response_bytes_observed: 1024,
        response_cap_applied: false,
        dns_policy_class: Some("public".into()),
        aggregate_byte_cap_reached: false,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 10 * 1024 * 1024,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    let telemetry = resp.telemetry.as_ref().unwrap();
    assert_eq!(telemetry.endpoint_origin.as_deref(), Some("api.github.com"));
    assert_eq!(telemetry.response_bytes_observed, Some(1024));
    assert!(!telemetry.response_cap_applied);
    assert_eq!(telemetry.dns_policy_class.as_deref(), Some("public"));
    assert!(!telemetry.aggregate_byte_cap_reached);
    assert!(!telemetry.redirect_rejected);
}

#[test]
fn telemetry_cap_applied_when_bytes_exceed_limit() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = ForgeTreeResponse {
        entries: vec![],
        identity: ResolvedRepositoryIdentity {
            default_branch: Some("main".into()),
            resolved_ref_name: Some("main".into()),
            ..Default::default()
        },
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: "github_tree".into(),
        endpoint_origin: Some("api.github.com".into()),
        response_bytes_observed: 10 * 1024 * 1024,
        response_cap_applied: true,
        dns_policy_class: Some("public".into()),
        aggregate_byte_cap_reached: true,
        aggregate_limit: 10 * 1024 * 1024,
        aggregate_remaining: 0,
        request_count: 0,
        exhausted_by: None,
    };

    let resp = build_response(&req, forge, true, true, true, true, None);
    let telemetry = resp.telemetry.as_ref().unwrap();
    assert!(telemetry.response_cap_applied);
    assert!(telemetry.aggregate_byte_cap_reached);
    assert_eq!(telemetry.response_bytes_observed, Some(10 * 1024 * 1024));
}

// ===========================================================================
// Slash-Ref Encoding Tests (D.6)
// ===========================================================================

#[test]
fn gitlab_slash_ref_encodes_correctly() {
    let server = MockServer::start();
    let mock_project = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo");
        then.json_body(serde_json::json!({
            "default_branch": "main"
        }));
    });
    let mock_commit = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/commits/feature%2Ffoo");
        then.json_body(serde_json::json!({
            "id": "commit_sha_slash",
            "tree_id": "tree_sha_slash"
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree")
            .query_param("ref", "feature/foo");
        then.json_body(serde_json::json!([
            {"path": "README.md", "type": "blob", "size": 10, "id": "sha1"},
        ]));
    });

    let req = RepoMapRequest {
        ref_name: Some("feature/foo".into()),
        ..default_request(CodeHost::Gitlab, "test-owner", "test-repo")
    };
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v4", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitlab,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_project.assert();
    mock_commit.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(
        resp.identity.resolved_commit_sha.as_deref(),
        Some("commit_sha_slash")
    );
    assert_eq!(resp.identity.tree_sha.as_deref(), Some("tree_sha_slash"));
    assert_eq!(resp.entries.len(), 1);
}

#[test]
fn gitlab_release_slash_ref_encodes_correctly() {
    let server = MockServer::start();
    let mock_project = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo");
        then.json_body(serde_json::json!({
            "default_branch": "main"
        }));
    });
    let mock_commit = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/commits/release%2F2026.07");
        then.json_body(serde_json::json!({
            "id": "commit_sha_release",
            "tree_id": "tree_sha_release"
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree")
            .query_param("ref", "release/2026.07");
        then.json_body(serde_json::json!([
            {"path": "version.txt", "type": "blob", "size": 5, "id": "sha1"},
        ]));
    });

    let req = RepoMapRequest {
        ref_name: Some("release/2026.07".into()),
        ..default_request(CodeHost::Gitlab, "test-owner", "test-repo")
    };
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v4", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitlab,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_project.assert();
    mock_commit.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(
        resp.identity.resolved_commit_sha.as_deref(),
        Some("commit_sha_release")
    );
}

#[test]
fn codeberg_slash_ref_encodes_correctly() {
    let server = MockServer::start();
    let mock_commit = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/commits/feature%2Fbar");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_cb",
            "commit": {
                "tree": { "sha": "tree_sha_cb" }
            }
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/git/trees/feature%2Fbar");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 10, "sha": "sha1"},
            ]
        }));
    });

    let req = RepoMapRequest {
        ref_name: Some("feature/bar".into()),
        ..default_request(CodeHost::Codeberg, "test-owner", "test-repo")
    };
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v1", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Codeberg,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_commit.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(
        resp.identity.resolved_commit_sha.as_deref(),
        Some("commit_sha_cb")
    );
    assert_eq!(resp.entries.len(), 1);
}

#[test]
fn gitea_slash_ref_encodes_correctly() {
    let server = MockServer::start();
    let mock_commit = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/commits/release%2F2026.07");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_gitea",
            "commit": {
                "tree": { "sha": "tree_sha_gitea" }
            }
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/git/trees/release%2F2026.07");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "VERSION", "type": "blob", "mode": "100644", "size": 8, "sha": "sha1"},
            ]
        }));
    });

    let req = RepoMapRequest {
        ref_name: Some("release/2026.07".into()),
        ..default_request(CodeHost::Gitea, "test-owner", "test-repo")
    };
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v1", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitea,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_commit.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(
        resp.identity.resolved_commit_sha.as_deref(),
        Some("commit_sha_gitea")
    );
    assert_eq!(resp.entries.len(), 1);
}

#[test]
fn forgejo_slash_ref_encodes_correctly() {
    let server = MockServer::start();
    let mock_commit = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/commits/feature%2Fmy-feature");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_forgejo",
            "commit": {
                "tree": { "sha": "tree_sha_forgejo" }
            }
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/api/v1/repos/test-owner/test-repo/git/trees/feature%2Fmy-feature");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "src", "type": "tree", "mode": "040000", "sha": "sha2"},
            ]
        }));
    });

    let req = RepoMapRequest {
        ref_name: Some("feature/my-feature".into()),
        ..default_request(CodeHost::Forgejo, "test-owner", "test-repo")
    };
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v1", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Forgejo,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_commit.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(
        resp.identity.resolved_commit_sha.as_deref(),
        Some("commit_sha_forgejo")
    );
    assert_eq!(resp.entries.len(), 1);
}

#[test]
fn github_slash_ref_encodes_commit_path_correctly() {
    let server = MockServer::start();
    let mock_commit = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/commits/feature%2Fmy-branch");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_gh_slash",
            "commit": {
                "tree": { "sha": "tree_sha_gh_slash" }
            }
        }));
    });
    let mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo");
        then.json_body(serde_json::json!({ "default_branch": "main" }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/tree_sha_gh_slash");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [{"path": "file.txt", "type": "blob", "mode": "100644", "size": 5, "sha": "b1"}]
        }));
    });

    let req = RepoMapRequest {
        ref_name: Some("feature/my-branch".into()),
        ..default_request(CodeHost::Github, "test-owner", "test-repo")
    };
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_commit.assert();
    mock_repo.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(
        resp.identity.resolved_commit_sha.as_deref(),
        Some("commit_sha_gh_slash")
    );
    assert_eq!(resp.identity.tree_sha.as_deref(), Some("tree_sha_gh_slash"));
    assert_eq!(resp.entries.len(), 1);
}

// ===========================================================================
// Operation-Wide Forge Byte Budget Tests (Workstream B)
// ===========================================================================

#[test]
fn budget_operation_wide_accounting_github() {
    let server = MockServer::start();
    let mock_commit = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/commits/main");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_budget",
            "commit": {"tree": {"sha": "tree_sha_budget"}}
        }));
    });
    let mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo");
        then.json_body(serde_json::json!({"default_branch": "main"}));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/tree_sha_budget");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 100, "sha": "sha1"},
                {"path": "src", "type": "tree", "mode": "040000", "sha": "sha2"},
            ]
        }));
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_commit.assert();
    mock_repo.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert_eq!(resp.entries.len(), 2);
    assert!(resp.request_count >= 3);
    assert!(resp.response_bytes_observed > 0);
    assert!(resp.aggregate_limit > 0);
    assert!(resp.response_bytes_observed <= resp.aggregate_limit);
    assert_eq!(
        resp.response_bytes_observed + resp.aggregate_remaining,
        resp.aggregate_limit
    );
}

#[test]
fn budget_exhausted_by_tree_page_github() {
    let server = MockServer::start();
    let _mock_commit = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/commits/main");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_ex",
            "commit": {"tree": {"sha": "tree_sha_ex"}}
        }));
    });
    let _mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo");
        then.json_body(serde_json::json!({"default_branch": "main"}));
    });
    let _mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 100, "sha": "sha1"},
            ]
        }));
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: Some(1),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    let err = result.unwrap_err();
    assert!(
        err.contains("aggregate_budget_exhausted") || err.contains("response_too_large"),
        "expected budget exhaustion or response too large, got: {err}"
    );
}

#[test]
fn budget_commit_resolution_consumes_before_tree() {
    let server = MockServer::start();
    let mock_commit = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/commits/main");
        then.json_body(serde_json::json!({
            "sha": "commit_sha_cr",
            "commit": {"tree": {"sha": "tree_sha_cr"}}
        }));
    });
    let mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo");
        then.json_body(serde_json::json!({"default_branch": "main"}));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/tree_sha_cr");
        then.json_body(serde_json::json!({
            "truncated": false,
            "tree": [{"path": "file.txt", "type": "blob", "mode": "100644", "size": 10, "sha": "sha1"}]
        }));
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: Some(20000),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    mock_commit.assert();
    mock_repo.assert();
    mock_tree.assert();

    let resp = result.unwrap();
    assert!(resp.request_count >= 3);
    assert!(resp.response_bytes_observed > 0);
    assert!(resp.response_bytes_observed <= resp.aggregate_limit);
}

#[test]
fn budget_telemetry_never_exceeds_aggregate_limit() {
    let server = MockServer::start();
    let _mock_project = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo");
        then.json_body(serde_json::json!({
            "default_branch": "main"
        }));
    });
    let _mock_commit = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/commits/main");
        then.json_body(serde_json::json!({
            "id": "commit_sha_tel",
            "tree_id": "tree_sha_tel"
        }));
    });
    let _mock_tree = server.mock(|when, then| {
        when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree");
        then.json_body(serde_json::json!([
            {"path": "a.txt", "type": "blob", "size": 10, "id": "sha1"},
        ]));
    });

    let req = default_request(CodeHost::Gitlab, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(format!("{}/api/v4", server.base_url())),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: Some(500),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Gitlab,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    let resp = result.unwrap();
    assert!(
        resp.response_bytes_observed <= resp.aggregate_limit,
        "observed {} exceeded limit {}",
        resp.response_bytes_observed,
        resp.aggregate_limit
    );
    assert_eq!(
        resp.response_bytes_observed + resp.aggregate_remaining,
        resp.aggregate_limit
    );
}

#[test]
fn budget_fallback_skipped_when_exhausted() {
    let server = MockServer::start();
    let _mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo");
        then.json_body(serde_json::json!({"default_branch": "main"}));
    });
    let _mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main");
        then.json_body(serde_json::json!({
            "truncated": true,
            "tree": [
                {"path": "README.md", "type": "blob", "mode": "100644", "size": 100, "sha": "sha1"},
            ]
        }));
    });
    let mock_contents = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/contents/");
        then.json_body(serde_json::json!([
            {"name": "extra.txt", "type": "file", "size": 10, "sha": "sha_extra"},
        ]));
    });

    let req = default_request(CodeHost::Github, "test-owner", "test-repo");
    let config = ForgeTreeConfig {
        api_key: None,
        base_url: Some(server.base_url()),
        endpoint_policy: ForgeEndpointPolicy::default(),
        forge_budget_limit: Some(1),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(fetch_tree(
        CodeHost::Github,
        "test-owner",
        "test-repo",
        &req,
        &config,
    ));

    let err = result.unwrap_err();
    assert!(
        err.contains("aggregate_budget_exhausted") || err.contains("response_too_large"),
        "expected budget exhaustion, got: {err}"
    );
    mock_contents.assert_hits(0);
}

#[test]
fn budget_all_forge_families_same_semantics() {
    let hosts = [
        (CodeHost::Github, "github_tree"),
        (CodeHost::Gitlab, "gitlab_tree"),
        (CodeHost::Codeberg, "codeberg_tree"),
        (CodeHost::Gitea, "gitea_tree"),
        (CodeHost::Forgejo, "forgejo_tree"),
    ];

    for (host, provider_id) in hosts {
        let server = MockServer::start();
        let base = match host {
            CodeHost::Github => server.base_url(),
            CodeHost::Gitlab => format!("{}/api/v4", server.base_url()),
            _ => format!("{}/api/v1", server.base_url()),
        };

        match host {
            CodeHost::Github => {
                server.mock(|when, then| {
                    when.path("/repos/test-owner/test-repo/commits/main");
                    then.json_body(serde_json::json!({
                        "sha": "sha",
                        "commit": {"tree": {"sha": "tree_sha"}}
                    }));
                });
                server.mock(|when, then| {
                    when.path("/repos/test-owner/test-repo");
                    then.json_body(serde_json::json!({"default_branch": "main"}));
                });
                server.mock(|when, then| {
                    when.path("/repos/test-owner/test-repo/git/trees/tree_sha");
                    then.json_body(serde_json::json!({
                        "truncated": false,
                        "tree": [{"path": "README.md", "type": "blob", "mode": "100644", "size": 10, "sha": "sha1"}]
                    }));
                });
            }
            CodeHost::Gitlab => {
                server.mock(|when, then| {
                    when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/commits/main");
                    then.json_body(serde_json::json!({
                        "id": "sha",
                        "tree_id": "tree_sha"
                    }));
                });
                server.mock(|when, then| {
                    when.path("/api/v4/projects/test-owner%2Ftest-repo");
                    then.json_body(serde_json::json!({"default_branch": "main"}));
                });
                server.mock(|when, then| {
                    when.path("/api/v4/projects/test-owner%2Ftest-repo/repository/tree");
                    then.json_body(serde_json::json!([
                        {"path": "README.md", "type": "blob", "size": 10, "id": "sha1"},
                    ]));
                });
            }
            _ => {
                server.mock(|when, then| {
                    when.path("/api/v1/repos/test-owner/test-repo/commits/main");
                    then.json_body(serde_json::json!({
                        "sha": "sha",
                        "commit": {"tree": {"sha": "tree_sha"}}
                    }));
                });
                server.mock(|when, then| {
                    when.path("/api/v1/repos/test-owner/test-repo");
                    then.json_body(serde_json::json!({"default_branch": "main"}));
                });
                server.mock(|when, then| {
                    when.path("/api/v1/repos/test-owner/test-repo/git/trees/main");
                    then.json_body(serde_json::json!({
                        "truncated": false,
                        "tree": [{"path": "README.md", "type": "blob", "mode": "100644", "size": 10, "sha": "sha1"}]
                    }));
                });
            }
        }

        let req = default_request(host, "test-owner", "test-repo");
        let config = ForgeTreeConfig {
            api_key: None,
            base_url: Some(base),
            endpoint_policy: ForgeEndpointPolicy::default(),
            forge_budget_limit: Some(5000),
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(fetch_tree(host, "test-owner", "test-repo", &req, &config));

        let resp = result.unwrap();
        assert_eq!(resp.provider_id, provider_id);
        assert!(
            resp.response_bytes_observed <= resp.aggregate_limit,
            "{provider_id}: observed {} exceeded limit {}",
            resp.response_bytes_observed,
            resp.aggregate_limit
        );
        assert_eq!(resp.aggregate_limit, 5000);
    }
}
