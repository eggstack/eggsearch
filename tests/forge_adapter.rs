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
    build_response, fetch_tree, EntryKind, ForgeRawEntry, ForgeTreeConfig, ForgeTreeResponse,
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
    ForgeTreeResponse {
        entries,
        default_branch: Some("main".into()),
        resolved_ref: Some("main".into()),
        truncated_by_provider: false,
        warnings: vec![],
        provider_id: provider_id.into(),
    }
}

// ===========================================================================
// GitHub Adapter Tests
// ===========================================================================

#[test]
fn github_tree_small_repo() {
    let server = MockServer::start();
    let mock_repo = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo")
            .header("User-Agent", "eggsearch/1.0");
        then.json_body(serde_json::json!({
            "default_branch": "main"
        }));
    });
    let mock_tree = server.mock(|when, then| {
        when.path("/repos/test-owner/test-repo/git/trees/main")
            .query_param("recursive", "1");
        then.json_body(serde_json::json!({
            "sha": "abc123",
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

    let resp = result.unwrap();
    assert_eq!(resp.provider_id, "github_tree");
    assert_eq!(resp.entries.len(), 4);
    assert!(!resp.truncated_by_provider);
    assert_eq!(resp.resolved_ref.as_deref(), Some("abc123"));
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
                sha: Some("sha1".into()),
            },
            ForgeRawEntry {
                path: "app.py".into(),
                kind: EntryKind::File,
                size: Some(512),
                sha: Some("sha2".into()),
            },
            ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(200),
                sha: Some("sha3".into()),
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true);
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
                sha: Some("sha1".into()),
            },
            ForgeRawEntry {
                path: "package.json".into(),
                kind: EntryKind::File,
                size: Some(100),
                sha: Some("sha2".into()),
            },
            ForgeRawEntry {
                path: "README.md".into(),
                kind: EntryKind::File,
                size: Some(50),
                sha: Some("sha3".into()),
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true);
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
                sha: None,
            },
            ForgeRawEntry {
                path: "src/main.rs".into(),
                kind: EntryKind::File,
                size: Some(100),
                sha: None,
            },
            ForgeRawEntry {
                path: "src/deep/nested/file.rs".into(),
                kind: EntryKind::File,
                size: Some(50),
                sha: None,
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true);
    let paths: Vec<&str> = resp.root_entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains(&"src"));
    assert!(!paths.contains(&"src/main.rs"));
    assert!(!paths.contains(&"src/deep/nested/file.rs"));
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
                sha: None,
            },
            ForgeRawEntry {
                path: "src".into(),
                kind: EntryKind::Directory,
                size: None,
                sha: None,
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, false, true, true, true);
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
            sha: None,
        }],
        default_branch: Some("main".into()),
        resolved_ref: Some("main".into()),
        truncated_by_provider: true,
        warnings: vec![],
        provider_id: "github_tree".into(),
    };

    let resp = build_response(&req, forge, true, true, true, true);
    assert!(resp
        .structured_warnings
        .iter()
        .any(|w| w.code == eggsearch::core::warning::WarningCode::ForgeTreeTruncated));
}

#[test]
fn build_response_native_mode() {
    let req = default_request(CodeHost::Github, "test", "repo");
    let forge = forge_response(vec![], "github_tree");
    let resp = build_response(&req, forge, true, true, true, true);
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
                sha: None,
            },
            ForgeRawEntry {
                path: "vendor".into(),
                kind: EntryKind::Submodule,
                size: None,
                sha: None,
            },
        ],
        "github_tree",
    );

    let resp = build_response(&req, forge, true, true, true, true);
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
                    sha: Some("sha1".into()),
                },
                ForgeRawEntry {
                    path: "src".into(),
                    kind: EntryKind::Directory,
                    size: None,
                    sha: Some("sha2".into()),
                },
                ForgeRawEntry {
                    path: "Cargo.toml".into(),
                    kind: EntryKind::File,
                    size: Some(200),
                    sha: Some("sha3".into()),
                },
            ],
            provider_id,
        );

        let resp = build_response(&req, forge, true, true, true, true);

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
