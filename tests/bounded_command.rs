#![cfg(feature = "mock")]

use std::process::Command;
use std::time::Duration;

use eggsearch::bounded_command_test as bct;
use eggsearch::core::local::LocalConfig;
use eggsearch::meta::local_inventory_cache::{build_inventory, build_inventory_git};

fn git_init(path: &std::path::Path) {
    Command::new("git")
        .arg("init")
        .current_dir(path)
        .output()
        .unwrap();
}

fn git_commit(path: &std::path::Path, msg: &str) {
    Command::new("git")
        .arg("commit")
        .arg("--allow-empty")
        .arg("-m")
        .arg(msg)
        .current_dir(path)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();
}

fn git_add(path: &std::path::Path, file: &str) {
    Command::new("git")
        .arg("add")
        .arg(file)
        .current_dir(path)
        .output()
        .unwrap();
}

#[test]
fn test_bounded_command_small_output() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git_init(root);
    std::fs::write(root.join("a.txt"), "hello").unwrap();
    git_add(root, "a.txt");
    git_commit(root, "initial");

    let mut cmd = Command::new("git");
    cmd.arg("ls-files")
        .arg("-z")
        .arg("--cached")
        .current_dir(root);
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(result.status.unwrap().success());
    assert!(!result.timed_out);
    assert!(!result.stdout_truncated);
    assert!(result.stdout.windows(6).any(|w| w == b"a.txt\0"));
}

#[test]
fn test_bounded_command_stdout_over_cap() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git_init(root);
    for i in 0..500 {
        let name = format!("file_{i:04}.txt");
        std::fs::write(root.join(&name), format!("content {i}")).unwrap();
        git_add(root, &name);
    }
    git_commit(root, "many files");

    let mut cmd = Command::new("git");
    cmd.arg("ls-files")
        .arg("-z")
        .arg("--cached")
        .current_dir(root);
    let result = bct::run_for_inventory(&mut cmd, Duration::from_secs(5), 200);
    let status = result.status.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let ok = status.success() || status.signal() == Some(13);
        assert!(ok, "expected success or SIGPIPE, got: {status:?}");
    }
    #[cfg(not(unix))]
    {
        assert!(status.success());
    }
    assert!(
        result.stdout_truncated,
        "output should be truncated with tiny cap"
    );
}

#[test]
fn test_bounded_command_stderr_over_cap() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(
        "exec 1>/dev/null; echo to_stdout; i=0; while [ $i -lt 10000 ]; do printf 'line %d\\n' $i >&2; i=$((i+1)); done; exit 0",
    );
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(
        result.stderr_truncated,
        "stderr should be truncated at 64KB, got {} bytes",
        result.stderr.len()
    );
}

#[test]
fn test_bounded_command_deadlock_prevention() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(
        "yes stdout_line 2>/dev/null & yes stderr_line >&2 2>/dev/null & sleep 0.2; kill 0; wait",
    );
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(
        result.status.is_some(),
        "command should complete without deadlock"
    );
}
#[test]
fn test_bounded_command_timeout() {
    let start = std::time::Instant::now();
    let mut cmd = Command::new("sleep");
    cmd.arg("60");
    let result = bct::run(&mut cmd, Duration::from_millis(200));
    let elapsed = start.elapsed();
    assert!(result.timed_out, "should report timed_out");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            result.status.unwrap().signal(),
            Some(libc::SIGKILL),
            "killed process should have SIGKILL signal"
        );
    }
    assert!(
        elapsed < Duration::from_secs(3),
        "should not wait for full sleep duration"
    );
}

#[test]
fn test_bounded_command_child_keeps_pipe_open() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("echo child_started; sleep 0.3; exit 0");
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(result.status.unwrap().success());
    assert!(!result.timed_out);
    assert!(result.stdout.starts_with(b"child_started"));
}

#[test]
fn test_bounded_command_nonzero_exit() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("exit 42");
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(!result.timed_out);
    assert_eq!(result.status.unwrap().code(), Some(42));
}

#[test]
fn test_bounded_command_invalid_utf8() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(r#"printf '\377\376\0\1binary data\n'"#);
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(result.status.unwrap().success());
    assert!(!result.stdout.is_empty());
    assert!(result.stdout[0] == 0xff || result.stdout.contains(&0xff));
}

#[test]
fn test_bounded_command_exits_before_timeout() {
    let start = std::time::Instant::now();
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("exit 0");
    let result = bct::run(&mut cmd, Duration::from_secs(60));
    let elapsed = start.elapsed();
    assert!(result.status.unwrap().success());
    assert!(!result.timed_out);
    assert!(
        elapsed < Duration::from_secs(2),
        "should exit quickly, not wait for timeout"
    );
}

#[test]
fn test_bounded_command_repeated_timeout_no_pid_race() {
    for _ in 0..10 {
        let mut cmd = Command::new("sleep");
        cmd.arg("60");
        let result = bct::run(&mut cmd, Duration::from_millis(100));
        assert!(result.timed_out);
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(
                result.status.unwrap().signal(),
                Some(libc::SIGKILL),
                "killed process should have SIGKILL signal"
            );
        }
    }
}

#[test]
fn test_bounded_command_tracked_untracked_merge() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git_init(root);
    std::fs::write(root.join("tracked.rs"), "fn tracked() {}").unwrap();
    std::fs::write(root.join("untracked.rs"), "fn untracked() {}").unwrap();
    git_add(root, "tracked.rs");
    git_commit(root, "initial");

    let config = LocalConfig {
        enabled: true,
        roots: vec![root.to_path_buf()],
        respect_gitignore: false,
        ..Default::default()
    };

    let ri = build_inventory_git(0, root, &config).expect("git inventory");
    assert!(ri.uses_git_backend);
    let paths: Vec<&str> = ri
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert!(paths.contains(&"tracked.rs"), "tracked file present");
    assert!(paths.contains(&"untracked.rs"), "untracked file merged in");
    assert!(ri.untracked_count >= 1);
}

#[test]
fn test_bounded_command_failure_no_cache_poison() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git_init(root);
    std::fs::write(root.join("a.rs"), "fn a() {}").unwrap();
    git_add(root, "a.rs");
    git_commit(root, "initial");

    let config = LocalConfig {
        enabled: true,
        roots: vec![root.to_path_buf()],
        respect_gitignore: false,
        ..Default::default()
    };
    let roots_vec = vec![(0, root.to_path_buf())];
    let inv1 = build_inventory(&config, &roots_vec);
    assert!(
        inv1.roots[0].entry_count >= 1,
        "initial inventory has entries"
    );

    let mut bad_cmd = Command::new("git");
    bad_cmd
        .arg("ls-files")
        .arg("-z")
        .arg("--cached")
        .arg("--bad-flag-definitely-fails")
        .current_dir(root);
    let bad_result = bct::run(&mut bad_cmd, Duration::from_secs(5));
    assert!(
        !bad_result.status.unwrap().success(),
        "bad command should fail"
    );

    let inv2 = build_inventory(&config, &roots_vec);
    assert_eq!(
        inv2.roots[0].entry_count, inv1.roots[0].entry_count,
        "inventory not corrupted by failed command"
    );
}

#[test]
fn test_bounded_command_concurrent_drainage() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("echo OUT_A; echo ERR_A >&2; sleep 0.05; echo OUT_B; echo ERR_B >&2");
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(result.status.unwrap().success());
    assert!(!result.timed_out);
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stdout.contains("OUT_A"),
        "stdout should contain OUT_A: {stdout}"
    );
    assert!(
        stdout.contains("OUT_B"),
        "stdout should contain OUT_B: {stdout}"
    );
    assert!(
        stderr.contains("ERR_A"),
        "stderr should contain ERR_A: {stderr}"
    );
    assert!(
        stderr.contains("ERR_B"),
        "stderr should contain ERR_B: {stderr}"
    );
}

#[test]
fn test_bounded_command_simultaneous_saturation() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("yes A 2>/dev/null & yes B >&2 2>/dev/null & sleep 0.1; kill 0; wait");
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(
        result.status.is_some(),
        "should complete even with both pipes saturated"
    );
    assert!(
        result.stdout.len() > 100,
        "should capture some stdout bytes"
    );
    assert!(
        result.stderr.len() > 100,
        "should capture some stderr bytes"
    );
}

#[test]
fn test_bounded_command_stderr_cap_termination() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(
        "exec 1>/dev/null; i=0; while [ $i -lt 50000 ]; do printf 'err_line_%d\\n' $i >&2; i=$((i+1)); done; exit 0",
    );
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(
        result.stderr_truncated,
        "stderr should be truncated when exceeding 64KB cap"
    );
    assert!(
        !result.stdout_truncated,
        "stdout should not be truncated when redirected to /dev/null"
    );
}

#[test]
fn test_bounded_command_spawn_failure() {
    let mut cmd = Command::new("/nonexistent_binary_path_xyz");
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(
        result.status.is_none(),
        "spawn failure should have no status"
    );
    assert!(
        format!("{:?}", result.termination).contains("SpawnFailed"),
        "termination should be SpawnFailed"
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn test_bounded_command_nonzero_exit_with_diagnostics() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("echo diagnostic_info; exit 7");
    let result = bct::run(&mut cmd, Duration::from_secs(5));
    assert!(!result.timed_out);
    assert_eq!(result.status.unwrap().code(), Some(7));
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("diagnostic_info"),
        "should capture diagnostic output: {stdout}"
    );
}

#[test]
fn test_bounded_command_inventory_cap_breach_terminates() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git_init(root);
    for i in 0..2000 {
        let name = format!("f{i:04}.txt");
        std::fs::write(root.join(&name), format!("c{i}")).unwrap();
        git_add(root, &name);
    }
    git_commit(root, "many files");

    let mut cmd = Command::new("git");
    cmd.arg("ls-files")
        .arg("-z")
        .arg("--cached")
        .current_dir(root);
    let start = std::time::Instant::now();
    let result = bct::run_for_inventory(&mut cmd, Duration::from_secs(10), 500);
    let elapsed = start.elapsed();
    assert!(
        result.stdout_truncated,
        "output should be truncated with tiny cap"
    );
    assert!(
        format!("{:?}", result.termination).contains("StdoutLimitExceeded"),
        "termination should be StdoutLimitExceeded, got {:?}",
        result.termination
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "cap breach should terminate quickly, not wait for timeout"
    );
}

#[test]
fn test_bounded_command_worktree_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let main_repo = dir.path().join("main");
    std::fs::create_dir_all(&main_repo).unwrap();
    git_init(&main_repo);
    std::fs::write(main_repo.join("a.rs"), "fn a() {}").unwrap();
    git_add(&main_repo, "a.rs");
    git_commit(&main_repo, "initial");

    let worktree_path = dir.path().join("worktree");
    let output = Command::new("git")
        .arg("worktree")
        .arg("add")
        .arg(&worktree_path)
        .arg("HEAD")
        .current_dir(&main_repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::write(worktree_path.join("b.rs"), "fn b() {}").unwrap();

    let config = LocalConfig {
        enabled: true,
        roots: vec![worktree_path.clone()],
        respect_gitignore: false,
        ..Default::default()
    };

    let ri = build_inventory_git(0, &worktree_path, &config);
    assert!(ri.is_some(), "worktree should produce git inventory");
    let ri = ri.unwrap();
    assert!(ri.uses_git_backend);
    assert!(ri.head_commit.is_some(), "HEAD commit resolved");
    let paths: Vec<&str> = ri
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert!(paths.contains(&"b.rs"), "worktree file found: {:?}", paths);
}
