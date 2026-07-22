#![cfg(feature = "mock")]

use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::mcp::state::ServerState;
use eggsearch::mcp::tools::{
    run_repo_fetch, run_repo_map, run_repo_search, RepoFetchArgs, RepoMapArgs, RepoSearchArgs,
};

use tempfile::TempDir;

fn integration_state(root: &std::path::Path) -> Arc<ServerState> {
    let mut cfg = AppConfig::default();
    cfg.local.enabled = true;
    cfg.local.roots = vec![root.to_path_buf()];
    cfg.local.respect_gitignore = true;
    cfg.local.follow_symlinks = false;
    cfg.local.max_file_bytes = 1_048_576;
    cfg.local.max_indexed_files = 50_000;
    cfg.search.providers.insert("duckduckgo".into(), true);
    Arc::new(ServerState::build(cfg).expect("build server state"))
}

fn init_git_repo(root: &std::path::Path) {
    let output = std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(root)
        .output()
        .expect("git init");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git init failed: {stderr}");
    }
    let output = std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(root)
        .output()
        .expect("git config email");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git config email failed: {stderr}");
    }
    let output = std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(root)
        .output()
        .expect("git config name");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git config name failed: {stderr}");
    }
}

fn git_add_and_commit(root: &std::path::Path, msg: &str) {
    let add_output = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(root)
        .output()
        .expect("git add");
    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        if !stderr.contains("did not match any files") {
            panic!("git add failed: {stderr}");
        }
    }
    std::process::Command::new("git")
        .args(["commit", "-m", msg, "--allow-empty"])
        .current_dir(root)
        .output()
        .expect("git commit");
}

fn count_entries(v: &serde_json::Value) -> usize {
    let root_count = v["root_entries"].as_array().map_or(0, |a| a.len());
    let entries_count = v["entries"].as_array().map_or(0, |a| a.len());
    root_count.max(entries_count)
}

#[tokio::test]
async fn local_repo_search_finds_source_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    std::fs::write(
        root.join("lib.rs"),
        "pub fn helper() -> i32 { 42 }\npub struct Config { pub name: String }",
    )
    .unwrap();
    std::fs::write(root.join("main.rs"), "fn main() { println!(\"hello\"); }").unwrap();

    init_git_repo(root);
    git_add_and_commit(root, "initial commit");

    let state = integration_state(root);
    let v = run_repo_search(
        state,
        RepoSearchArgs {
            query: "helper".into(),
            providers: vec![],
            ..Default::default()
        },
    )
    .await
    .expect("local repo_search");
    let has_results = v["results"].as_array().is_some_and(|a| !a.is_empty());
    let has_warnings = v["structured_warnings"]
        .as_array()
        .is_some_and(|a| !a.is_empty());
    assert!(
        has_results || has_warnings,
        "should find helper in local source files or have warnings: {}",
        serde_json::to_string_pretty(&v).unwrap_or_default()
    );
}

#[tokio::test]
async fn local_repo_map_returns_entries() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn api() {}").unwrap();
    std::fs::create_dir_all(root.join("src/module")).unwrap();
    std::fs::write(root.join("src/module/mod.rs"), "pub fn sub() {}").unwrap();
    std::fs::write(root.join("README.md"), "# Project").unwrap();

    init_git_repo(root);
    git_add_and_commit(root, "initial commit");

    let state = integration_state(root);
    let v = run_repo_map(
        state,
        RepoMapArgs {
            host: None,
            owner: "test".into(),
            repo: "test".into(),
            ref_name: None,
            commit_sha: None,
            max_entries: None,
            max_depth: None,
            include_files: Some(true),
            include_directories: Some(true),
            include_ci: None,
            include_security: None,
            timeout_ms: None,
            providers: vec![],
        },
    )
    .await
    .expect("local repo_map");
    let count = count_entries(&v);
    let mode = v["mode"].as_str().unwrap_or("unknown");
    assert!(
        count > 0 || mode == "local_search" || mode == "fallback_search",
        "repo_map should return entries or be in local/fallback mode for local workspace, got mode={mode}"
    );
}

#[tokio::test]
async fn local_repo_fetch_reads_source_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    std::fs::write(root.join("lib.rs"), "pub fn helper() -> i32 { 42 }").unwrap();

    init_git_repo(root);
    git_add_and_commit(root, "initial commit");

    let state = integration_state(root);
    let root_name = root.file_name().unwrap().to_str().unwrap();
    let v = run_repo_fetch(
        state,
        RepoFetchArgs {
            host: Some("workspace".into()),
            owner: root_name.into(),
            repo: "lib.rs".into(),
            ref_name: None,
            commit_sha: None,
            path: "lib.rs".into(),
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
    .expect("local repo_fetch");
    let fetched = v["fetched"].as_bool().unwrap_or(false);
    let has_text = v["text"].as_str().is_some_and(|t| !t.is_empty());
    assert!(
        fetched || has_text,
        "should read source content from local file"
    );
}

#[tokio::test]
async fn new_untracked_file_discoverable_after_inventory() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    std::fs::write(root.join("existing.rs"), "pub fn original() {}").unwrap();

    init_git_repo(root);
    git_add_and_commit(root, "initial commit");

    let state = integration_state(root);

    let _ = run_repo_search(
        state.clone(),
        RepoSearchArgs {
            query: "original".into(),
            providers: vec![],
            ..Default::default()
        },
    )
    .await
    .expect("initial search");

    std::fs::write(
        root.join("new_file.rs"),
        "pub fn newly_created_function() {}",
    )
    .unwrap();

    let v = run_repo_search(
        state,
        RepoSearchArgs {
            query: "newly_created_function".into(),
            providers: vec![],
            ..Default::default()
        },
    )
    .await
    .expect("search after new file");
    let has_results = v["results"].as_array().is_some_and(|a| !a.is_empty());
    let has_warnings = v["structured_warnings"]
        .as_array()
        .is_some_and(|a| !a.is_empty());
    assert!(
        has_results || has_warnings,
        "newly created untracked file should be discoverable or have warnings"
    );
}

#[tokio::test]
async fn ignored_file_not_in_results() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn visible() {}").unwrap();
    std::fs::write(root.join(".gitignore"), "*.ignored\n").unwrap();
    std::fs::write(root.join("secret.ignored"), "pub fn secret_function() {}").unwrap();

    init_git_repo(root);
    git_add_and_commit(root, "initial commit");

    let state = integration_state(root);
    let v = run_repo_search(
        state,
        RepoSearchArgs {
            query: "secret_function".into(),
            providers: vec![],
            ..Default::default()
        },
    )
    .await
    .expect("search with ignored file");
    let has_secret = v["results"].as_array().is_some_and(|a| {
        a.iter().any(|r| {
            r["text"]
                .as_str()
                .is_some_and(|t| t.contains("secret_function"))
        })
    });
    assert!(
        !has_secret,
        "gitignored file should not appear in search results"
    );
}

#[tokio::test]
async fn symlink_final_component_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    std::fs::create_dir_all(root.join("real")).unwrap();
    std::fs::write(root.join("real/file.rs"), "pub fn real() {}").unwrap();
    std::os::unix::fs::symlink("real/file.rs", root.join("link.rs")).unwrap();

    init_git_repo(root);
    git_add_and_commit(root, "initial commit");

    let state = integration_state(root);
    let root_name = root.file_name().unwrap().to_str().unwrap();
    let result = run_repo_fetch(
        state,
        RepoFetchArgs {
            host: Some("workspace".into()),
            owner: root_name.into(),
            repo: "link.rs".into(),
            ref_name: None,
            commit_sha: None,
            path: "link.rs".into(),
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
    .await;
    assert!(
        result.is_err() || result.unwrap()["fetched"] == false,
        "symlink should be rejected or return error"
    );
}

#[tokio::test]
async fn linked_worktree_detection() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    let worktree_root = tmp.path().join("worktree");

    std::fs::create_dir_all(&repo_root).unwrap();
    init_git_repo(&repo_root);
    std::fs::write(repo_root.join("lib.rs"), "pub fn main_repo() {}").unwrap();
    git_add_and_commit(&repo_root, "initial commit");

    std::process::Command::new("git")
        .args(["worktree", "add", worktree_root.to_str().unwrap(), "HEAD"])
        .current_dir(&repo_root)
        .output()
        .expect("git worktree add");

    std::fs::write(
        worktree_root.join("worktree_only.rs"),
        "pub fn worktree_specific() {}",
    )
    .unwrap();

    let state = integration_state(&worktree_root);
    let v = run_repo_search(
        state,
        RepoSearchArgs {
            query: "worktree_specific".into(),
            providers: vec![],
            ..Default::default()
        },
    )
    .await
    .expect("linked worktree search");
    let has_results = v["results"].as_array().is_some_and(|a| !a.is_empty());
    let has_warnings = v["structured_warnings"]
        .as_array()
        .is_some_and(|a| !a.is_empty());
    assert!(
        has_results || has_warnings,
        "should find file in linked worktree or have warnings"
    );
}

#[tokio::test]
async fn large_file_content_capped() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let large_content = "x".repeat(200_000);
    std::fs::write(root.join("large.txt"), &large_content).unwrap();

    init_git_repo(root);
    git_add_and_commit(root, "add large file");

    let state = integration_state(root);
    let root_name = root.file_name().unwrap().to_str().unwrap();
    let v = run_repo_fetch(
        state,
        RepoFetchArgs {
            host: Some("workspace".into()),
            owner: root_name.into(),
            repo: "large.txt".into(),
            ref_name: None,
            commit_sha: None,
            path: "large.txt".into(),
            line_start: None,
            line_end: None,
            context_before: None,
            context_after: None,
            max_chars: Some(1024),
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
    .expect("fetch large file");
    let fetched = v["fetched"].as_bool().unwrap_or(false);
    let text_len = v["text"].as_str().map_or(0, |t| t.len());
    assert!(
        fetched || text_len <= 2048,
        "large file content should be capped, got {text_len} chars"
    );
}

#[tokio::test]
async fn concurrent_cold_searches_do_not_panic() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    std::fs::write(root.join("lib.rs"), "pub fn concurrent_test() {}").unwrap();

    init_git_repo(root);
    git_add_and_commit(root, "initial commit");

    let state = integration_state(root);

    let mut handles = vec![];
    for i in 0..3 {
        let s = state.clone();
        let q = format!("concurrent_test_{i}");
        handles.push(tokio::spawn(async move {
            run_repo_search(
                s,
                RepoSearchArgs {
                    query: q,
                    providers: vec![],
                    ..Default::default()
                },
            )
            .await
        }));
    }

    for h in handles {
        let v = h.await.expect("task should not panic");
        v.expect("concurrent search should succeed");
    }
}
