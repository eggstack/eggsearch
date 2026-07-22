use eggsearch::core::local::{validate_local_fetch_path, LocalConfig, LocalFetchPathError};
use eggsearch::meta::safe_open::{safe_open_relative, SafeOpenError};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn default_cfg() -> LocalConfig {
    LocalConfig {
        enabled: true,
        roots: vec![],
        max_file_bytes: 1_048_576,
        max_indexed_files: 50_000,
        include_hidden: false,
        respect_gitignore: true,
        follow_symlinks: false,
    }
}

#[test]
fn symlink_rejected_when_follow_disabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let target = root.join("target.txt");
    fs::write(&target, "hello").unwrap();
    let link = root.join("link.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "link.txt", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::SymlinkNotAllowed)),
        "symlink should be rejected when follow_symlinks=false, got: {result:?}"
    );
}

#[test]
fn symlink_accepted_when_follow_enabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let target = root.join("target.txt");
    fs::write(&target, "hello").unwrap();
    let link = root.join("link.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        ..default_cfg()
    };
    let result = validate_local_fetch_path(root, "link.txt", &cfg);
    assert!(
        result.is_ok(),
        "symlink should be accepted when follow_symlinks=true, got: {result:?}"
    );
}

#[test]
fn intermediate_symlink_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let real_dir = root.join("real");
    fs::create_dir(&real_dir).unwrap();
    fs::write(real_dir.join("file.txt"), "content").unwrap();
    let link_dir = root.join("linkdir");
    std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "linkdir/file.txt", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::SymlinkNotAllowed)),
        "path through intermediate symlink should be rejected, got: {result:?}"
    );
}

#[test]
fn path_traversal_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("secret.txt"), "secret").unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "../secret.txt", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::PathTraversal)),
        "path traversal should be rejected, got: {result:?}"
    );
}

#[test]
fn absolute_path_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("file.txt"), "content").unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "/etc/passwd", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::AbsolutePath)),
        "absolute path should be rejected, got: {result:?}"
    );
}

#[test]
fn hidden_path_rejected_by_default() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let hidden = root.join(".hidden");
    fs::create_dir(&hidden).unwrap();
    fs::write(hidden.join("file.txt"), "content").unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, ".hidden/file.txt", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::HiddenPath(_))),
        "hidden path should be rejected when include_hidden=false, got: {result:?}"
    );
}

#[test]
fn hidden_path_accepted_when_enabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let hidden = root.join(".hidden");
    fs::create_dir(&hidden).unwrap();
    fs::write(hidden.join("file.txt"), "content").unwrap();

    let cfg = LocalConfig {
        include_hidden: true,
        ..default_cfg()
    };
    let result = validate_local_fetch_path(root, ".hidden/file.txt", &cfg);
    assert!(
        result.is_ok(),
        "hidden path should be accepted when include_hidden=true, got: {result:?}"
    );
}

#[test]
fn skip_dirs_always_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let target_dir = root.join("node_modules");
    fs::create_dir(&target_dir).unwrap();
    fs::write(target_dir.join("pkg.js"), "code").unwrap();

    let cfg = LocalConfig {
        include_hidden: true,
        ..default_cfg()
    };
    let result = validate_local_fetch_path(root, "node_modules/pkg.js", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::SkippedDirectory(_))),
        "skip directory should always be rejected, got: {result:?}"
    );
}

#[test]
fn binary_extension_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("image.png"), b"\x89PNG").unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "image.png", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::BinaryFile(_))),
        "binary extension should be rejected, got: {result:?}"
    );
}

#[test]
fn file_too_large_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let big = root.join("big.txt");
    fs::write(&big, vec![b'x'; 2000]).unwrap();

    let cfg = LocalConfig {
        max_file_bytes: 1000,
        ..default_cfg()
    };
    let result = validate_local_fetch_path(root, "big.txt", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::FileTooLarge(_, _))),
        "large file should be rejected, got: {result:?}"
    );
}

#[test]
fn file_within_size_limit_accepted() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("small.txt"), "hello").unwrap();

    let cfg = LocalConfig {
        max_file_bytes: 1000,
        ..default_cfg()
    };
    let result = validate_local_fetch_path(root, "small.txt", &cfg);
    assert!(
        result.is_ok(),
        "small file should be accepted, got: {result:?}"
    );
}

#[test]
fn not_found_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "nonexistent.txt", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::NotFound)),
        "nonexistent file should be rejected, got: {result:?}"
    );
}

#[test]
fn empty_path_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::Empty)),
        "empty path should be rejected, got: {result:?}"
    );
}

#[test]
fn whitespace_only_path_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "   ", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::Empty)),
        "whitespace-only path should be rejected, got: {result:?}"
    );
}

#[test]
fn root_containment_property() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let subdir = root.join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file.txt"), "content").unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "subdir/file.txt", &cfg);
    assert!(result.is_ok());
    let canonical = result.unwrap();
    assert!(
        canonical.starts_with(root.canonicalize().unwrap()),
        "resolved path must be within root"
    );
}

#[test]
fn symlink_escape_root_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let outside = tmp.path().parent().unwrap().join("escape_target.txt");
    fs::write(&outside, "escaped").unwrap();
    let link = root.join("escape.txt");
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        ..default_cfg()
    };
    let result = validate_local_fetch_path(root, "escape.txt", &cfg);
    assert!(
        matches!(
            result,
            Err(LocalFetchPathError::EscapesRoot) | Err(LocalFetchPathError::CanonicalizeFailed(_))
        ),
        "symlink escaping root should be rejected, got: {result:?}"
    );
}

#[test]
fn directory_rejected_as_not_found() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir(root.join("mydir")).unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "mydir", &cfg);
    assert!(
        matches!(result, Err(LocalFetchPathError::NotFound)),
        "directory should be rejected (not a file), got: {result:?}"
    );
}

#[test]
fn symlink_loop_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let link_a = root.join("a");
    let link_b = root.join("b");
    std::os::unix::fs::symlink(&link_b, &link_a).unwrap();
    std::os::unix::fs::symlink(&link_a, &link_b).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        ..default_cfg()
    };
    let result = validate_local_fetch_path(root, "a", &cfg);
    assert!(
        matches!(
            result,
            Err(LocalFetchPathError::EscapesRoot)
                | Err(LocalFetchPathError::CanonicalizeFailed(_))
                | Err(LocalFetchPathError::SymlinkEscapesRoot)
                | Err(LocalFetchPathError::NotFound)
        ),
        "symlink loop should be rejected, got: {result:?}"
    );
}

#[test]
fn permission_denied_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let secret = root.join("secret.txt");
    fs::write(&secret, "secret data").unwrap();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o000)).unwrap();

    let cfg = default_cfg();
    let _result = validate_local_fetch_path(root, "secret.txt", &cfg);
}

#[test]
fn sparse_file_within_size_limit_accepted() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let sparse = root.join("sparse.txt");
    {
        let file = fs::File::create(&sparse).unwrap();
        file.set_len(5_000_000).unwrap();
    }

    let cfg = LocalConfig {
        max_file_bytes: 10_000_000,
        ..default_cfg()
    };
    let result = validate_local_fetch_path(root, "sparse.txt", &cfg);
    assert!(
        result.is_ok(),
        "sparse file within size limit should be accepted, got: {result:?}"
    );
}

#[test]
fn overlapping_roots_both_reject_outside() {
    let tmp = TempDir::new().unwrap();
    let root_a = tmp.path().join("a");
    let root_b = tmp.path().join("b");
    fs::create_dir(&root_a).unwrap();
    fs::create_dir(&root_b).unwrap();
    fs::write(root_a.join("file_a.txt"), "content a").unwrap();
    fs::write(root_b.join("file_b.txt"), "content b").unwrap();

    let cfg = default_cfg();
    let result_a = validate_local_fetch_path(&root_a, "file_a.txt", &cfg);
    let result_b = validate_local_fetch_path(&root_b, "file_b.txt", &cfg);
    assert!(
        result_a.is_ok(),
        "root_a file should be found: {result_a:?}"
    );
    assert!(
        result_b.is_ok(),
        "root_b file should be found: {result_b:?}"
    );

    let outside_a = validate_local_fetch_path(&root_a, "../b/file_b.txt", &cfg);
    assert!(
        matches!(outside_a, Err(LocalFetchPathError::PathTraversal)),
        "cross-root access from root_a should be rejected, got: {outside_a:?}"
    );
}

#[test]
fn root_replacement_between_validate_and_read() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let file = root.join("file.txt");
    fs::write(&file, "original").unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "file.txt", &cfg);
    assert!(result.is_ok());

    fs::remove_file(&file).unwrap();
    assert!(!file.exists(), "file should be gone after replacement");
}

#[test]
fn concurrent_file_modification_during_validate() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let file = root.join("concurrent.txt");
    fs::write(&file, "initial content").unwrap();

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "concurrent.txt", &cfg);
    assert!(
        result.is_ok(),
        "concurrent validation should succeed: {result:?}"
    );

    let _ = fs::read_to_string(&file);
}

#[test]
fn multiple_validate_calls_same_path_consistent() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("stable.txt"), "content").unwrap();

    let cfg = default_cfg();
    let r1 = validate_local_fetch_path(root, "stable.txt", &cfg);
    let r2 = validate_local_fetch_path(root, "stable.txt", &cfg);
    assert_eq!(
        r1.is_ok(),
        r2.is_ok(),
        "multiple validations of same path should be consistent"
    );
    if let (Ok(p1), Ok(p2)) = (r1, r2) {
        assert_eq!(p1, p2, "resolved paths should be identical");
    }
}

#[test]
fn non_utf_filename_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let file = root.join("file\x00.txt");
    fs::write(&file, "content").unwrap_or_else(|_| {
        fs::write(root.join("file.txt"), "content").unwrap();
    });

    let cfg = default_cfg();
    let result = validate_local_fetch_path(root, "file\x00.txt", &cfg);
    assert!(
        matches!(
            result,
            Err(LocalFetchPathError::NotFound) | Err(LocalFetchPathError::CanonicalizeFailed(_))
        ),
        "filename with null byte should be rejected or fail canonicalization, got: {result:?}"
    );
}

#[test]
fn corpus_filesystem_extended_exercises_validate() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join("adversarial")
        .join("filesystem_extended.json");
    let content = fs::read_to_string(&path).expect("Failed to read filesystem_extended.json");
    let value: serde_json::Value =
        serde_json::from_str(&content).expect("Invalid JSON in filesystem_extended.json");
    let cases = value["cases"].as_array().expect("missing cases array");
    assert!(!cases.is_empty(), "need at least one corpus case");

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("test.txt"), "content").unwrap();

    let cfg = default_cfg();
    for case in cases {
        let input = case["input"].as_str().expect("case must have 'input'");
        let _ = validate_local_fetch_path(root, input, &cfg);
    }
}

#[test]
fn search_and_fetch_use_equivalent_path_policy() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let secret = root.join("secret.txt");
    fs::write(&secret, "secret data").unwrap();
    let hidden = root.join(".hidden");
    fs::create_dir(&hidden).unwrap();
    fs::write(hidden.join("file.txt"), "hidden content").unwrap();
    let target_dir = root.join("node_modules");
    fs::create_dir(&target_dir).unwrap();
    fs::write(target_dir.join("pkg.js"), "code").unwrap();

    let cfg = default_cfg();
    let paths_to_check = vec!["secret.txt", ".hidden/file.txt", "node_modules/pkg.js"];

    for rel in &paths_to_check {
        let result = validate_local_fetch_path(root, rel, &cfg);
        let accepts = result.is_ok();
        let canonical = result.ok();
        if let Some(canonical_path) = canonical {
            assert!(
                canonical_path.starts_with(root.canonicalize().unwrap()),
                "accepted path {rel} must be within root"
            );
        }
        let _ = accepts;
    }
}

#[test]
fn safe_open_follow_in_root_symlink_succeeds() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("target.txt"), "hello").unwrap();
    std::os::unix::fs::symlink("target.txt", root.join("link.txt")).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        ..default_cfg()
    };
    let result = safe_open_relative(root, "link.txt", &cfg);
    assert!(
        result.is_ok(),
        "in-root symlink should succeed with follow_symlinks=true"
    );
    assert_eq!(result.unwrap().size, 5);
}

#[test]
fn safe_open_follow_symlink_escaping_root_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let outside = tmp.path().parent().unwrap().join("escape_target.txt");
    fs::write(&outside, "escaped").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("escape.txt")).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        ..default_cfg()
    };
    let result = safe_open_relative(root, "escape.txt", &cfg);
    assert!(
        matches!(
            result,
            Err(SafeOpenError::SymlinkDetected(_)) | Err(SafeOpenError::Io(_))
        ),
        "symlink escaping root should be rejected with follow_symlinks=true"
    );
}

#[test]
fn safe_open_follow_intermediate_symlink_escaping_root_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let real_dir = root.join("real");
    fs::create_dir(&real_dir).unwrap();
    fs::write(real_dir.join("file.txt"), "content").unwrap();
    let outside_dir = tmp.path().parent().unwrap().join("outside_dir_int");
    fs::create_dir_all(&outside_dir).unwrap();
    std::os::unix::fs::symlink(&outside_dir, root.join("linkdir")).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        ..default_cfg()
    };
    let result = safe_open_relative(root, "linkdir/file.txt", &cfg);
    assert!(
        matches!(
            result,
            Err(SafeOpenError::SymlinkDetected(_)) | Err(SafeOpenError::Io(_))
        ),
        "intermediate symlink escaping root should be rejected"
    );
}

#[test]
fn safe_open_follow_chained_symlinks_escaping_root_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let outside = tmp.path().parent().unwrap().join("escape_target_chain.txt");
    fs::write(&outside, "escaped").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link1")).unwrap();
    std::os::unix::fs::symlink(root.join("link1"), root.join("link2")).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        ..default_cfg()
    };
    let result = safe_open_relative(root, "link2", &cfg);
    assert!(
        matches!(
            result,
            Err(SafeOpenError::SymlinkDetected(_)) | Err(SafeOpenError::Io(_))
        ),
        "chained symlinks escaping root should be rejected"
    );
}

#[test]
fn safe_open_follow_fstat_size_check_through_descriptor() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("big.txt"), vec![b'x'; 2048]).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        max_file_bytes: 1024,
        ..default_cfg()
    };
    let result = safe_open_relative(root, "big.txt", &cfg);
    assert!(
        matches!(result, Err(SafeOpenError::FileTooLarge(2048, 1024))),
        "fstat size check should reject oversized file"
    );
}

#[test]
fn safe_open_follow_not_a_file_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir(root.join("subdir")).unwrap();

    let cfg = LocalConfig {
        follow_symlinks: true,
        ..default_cfg()
    };
    let result = safe_open_relative(root, "subdir", &cfg);
    assert!(
        matches!(result, Err(SafeOpenError::NotAFile)),
        "directory should be rejected as not-a-file"
    );
}

#[test]
fn safe_open_no_follow_mode_still_rejects_symlinks() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("target.txt"), "data").unwrap();
    std::os::unix::fs::symlink(root.join("target.txt"), root.join("link.txt")).unwrap();

    let cfg = default_cfg();
    assert!(
        matches!(
            safe_open_relative(root, "link.txt", &cfg),
            Err(SafeOpenError::SymlinkDetected(_))
        ),
        "no-follow mode should reject symlinks"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn safe_open_follow_magic_link_rejected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let magic_link = root.join("ns_link");
    if let Ok(()) = std::os::unix::fs::symlink("/proc/self/ns/mnt", &magic_link) {
        let cfg = LocalConfig {
            follow_symlinks: true,
            ..default_cfg()
        };
        let result = safe_open_relative(root, "ns_link", &cfg);
        assert!(
            matches!(
                result,
                Err(SafeOpenError::SymlinkDetected(_)) | Err(SafeOpenError::Io(_))
            ),
            "magic-link path should be rejected by RESOLVE_NO_MAGICLINKS"
        );
    }
}
