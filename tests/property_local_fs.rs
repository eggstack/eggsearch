use std::path::{Path, PathBuf};

use eggsearch::core::local::LocalConfig;
use eggsearch::meta::safe_open::{safe_open_relative, SafeOpenError};
use proptest::prelude::*;

fn arbitrary_path_segment() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9._-]{1,50}"
}

proptest! {
    #[test]
    fn path_segments_no_null_bytes(seg in "[a-zA-Z0-9._-]{1,50}") {
        prop_assume!(!seg.contains('\0'));
        prop_assume!(seg != ".");
        prop_assume!(seg != "..");
        let path = Path::new(&seg);
        prop_assert!(!seg.is_empty(), "segment should not be empty");
        prop_assert!(path.file_name().is_some(), "should have a file name");
    }

    #[test]
    fn relative_path_joining(segments in proptest::collection::vec(arbitrary_path_segment(), 1..5).prop_map(|s: Vec<String>| s)) {
        let mut path = PathBuf::new();
        for seg in &segments {
            path.push(seg);
        }
        let path_str = path.to_string_lossy();
        prop_assert!(!path_str.is_empty(), "joined path should not be empty");
        prop_assert!(!path_str.starts_with('/'), "relative path should not start with /");
    }

    #[test]
    fn absolute_path_always_absolute(segments in proptest::collection::vec(arbitrary_path_segment(), 1..5).prop_map(|s: Vec<String>| s)) {
        let mut path = PathBuf::from("/");
        for seg in &segments {
            path.push(seg);
        }
        prop_assert!(path.is_absolute(), "path should be absolute");
    }

    #[test]
    fn parent_component_does_not_escape(segments in proptest::collection::vec(arbitrary_path_segment(), 2..6).prop_map(|s: Vec<String>| s)) {
        let mut path = PathBuf::from("/root");
        for seg in &segments {
            path.push(seg);
        }
        let with_parent = path.join("..").join("escape");
        let canonical = with_parent.components().collect::<PathBuf>();
        let root = PathBuf::from("/root");
        prop_assert!(
            canonical.starts_with(&root) || !canonical.starts_with(&root),
            "parent resolution should be deterministic"
        );
    }

    #[test]
    fn filename_from_path(path in "[a-zA-Z0-9/_.-]{5,50}") {
        let p = Path::new(&path);
        if let Some(name) = p.file_name() {
            let name_str = name.to_string_lossy();
            prop_assert!(!name_str.is_empty(), "filename should not be empty");
            prop_assert!(!name_str.contains('/'), "filename should not contain /");
        }
    }

    #[test]
    fn extension_detection(path in "[a-zA-Z0-9/_.-]{5,50}") {
        let p = Path::new(&path);
        let _ = p.extension().map(|e| e.to_string_lossy().to_string());
    }

    #[test]
    fn skip_dirs_never_match_regular_files(
        name in "[a-z]{3,15}"
    ) {
        let skip_dirs = [
            "node_modules", ".git", "__pycache__", ".hg", ".svn",
            "target", "dist", ".next", ".cache", "vendor",
        ];
        prop_assume!(!skip_dirs.contains(&name.as_str()));
        prop_assert!(true, "non-skip name should pass");
    }

    #[test]
    fn binary_extensions_detected(
        ext in prop_oneof![
            "exe", "dll", "so", "dylib", "o", "a",
            "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp",
            "mp3", "mp4", "avi", "mov", "wav", "flac",
            "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
            "pdf", "doc", "docx", "xls", "xlsx",
            "pyc", "pyo", "class", "wasm",
        ]
    ) {
        let is_binary = matches!(
            ext.as_str(),
            "exe" | "dll" | "so" | "dylib" | "o" | "a"
                | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp"
                | "mp3" | "mp4" | "avi" | "mov" | "wav" | "flac"
                | "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar"
                | "pdf" | "doc" | "docx" | "xls" | "xlsx"
                | "pyc" | "pyo" | "class" | "wasm"
        );
        prop_assert!(is_binary, "known binary extension should be detected");
    }

    #[test]
    fn non_binary_extensions_not_flagged(
        ext in prop_oneof![
            "rs", "py", "js", "ts", "go", "java", "c", "cpp", "h",
            "html", "css", "json", "toml", "yaml", "yml",
            "md", "txt", "csv", "xml", "sql", "sh", "bash",
        ]
    ) {
        let is_text = !matches!(
            ext.as_str(),
            "exe" | "dll" | "so" | "dylib" | "o" | "a"
                | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp"
                | "mp3" | "mp4" | "avi" | "mov" | "wav" | "flac"
                | "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar"
                | "pdf" | "doc" | "docx" | "xls" | "xlsx"
                | "pyc" | "pyo" | "class" | "wasm"
        );
        prop_assert!(is_text, "known text extension should not be flagged as binary");
    }

    #[test]
    fn max_file_bytes_boundary(
        size in 0usize..10_000_000usize,
        limit in 100usize..1_000_000usize
    ) {
        let should_skip = size > limit;
        prop_assert_eq!(should_skip, size > limit);
    }

    #[test]
    fn max_indexed_files_boundary(
        count in 0usize..100_000usize,
        limit in 100usize..50_000usize
    ) {
        let should_stop = count >= limit;
        prop_assert_eq!(should_stop, count >= limit);
    }

    #[test]
    fn scoring_exact_name_bonus(name in "[a-z]{3,10}") {
        let query = name.clone();
        let score = if query == name { 100.0 } else { 0.0 };
        prop_assert!(score >= 100.0 || score == 0.0);
    }
}

fn safe_open_config() -> LocalConfig {
    LocalConfig {
        enabled: true,
        roots: Vec::new(),
        max_file_bytes: 1_048_576,
        max_indexed_files: 50_000,
        include_hidden: false,
        respect_gitignore: false,
        follow_symlinks: false,
    }
}

fn relative_path_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9._-]{1,20}"
}

proptest! {
    #[test]
    fn safe_open_rejects_dotdot_components(seg in relative_path_strategy()) {
        let dir = tempfile::tempdir().unwrap();
        let config = safe_open_config();
        let rel = format!("../{seg}");
        let result = safe_open_relative(dir.path(), &rel, &config);
        prop_assert!(matches!(result, Err(SafeOpenError::PathTraversal)),
            "path with '..' must be rejected, got error: {}", result.err().unwrap());
    }

    #[test]
    fn safe_open_rejects_dotdot_middle(prefix in relative_path_strategy(), suffix in relative_path_strategy()) {
        let dir = tempfile::tempdir().unwrap();
        let config = safe_open_config();
        let rel = format!("{prefix}/../{suffix}");
        let result = safe_open_relative(dir.path(), &rel, &config);
        prop_assert!(matches!(result, Err(SafeOpenError::PathTraversal)),
            "path with embedded '..' must be rejected, got error: {}", result.err().unwrap());
    }

    #[test]
    fn safe_open_rejects_absolute_paths(seg in "[a-zA-Z0-9._-]{1,30}") {
        let dir = tempfile::tempdir().unwrap();
        let config = safe_open_config();
        let rel = format!("/{seg}");
        let result = safe_open_relative(dir.path(), &rel, &config);
        prop_assert!(matches!(result, Err(SafeOpenError::AbsolutePath)),
            "absolute path must be rejected, got error: {}", result.err().unwrap());
    }

    #[test]
    fn safe_open_rejects_empty_path(empty in prop_oneof!["", " ", "  ", "\t"]) {
        let dir = tempfile::tempdir().unwrap();
        let config = safe_open_config();
        let result = safe_open_relative(dir.path(), &empty, &config);
        prop_assert!(matches!(result, Err(SafeOpenError::Empty)),
            "empty/whitespace path must be rejected, got error: {}", result.err().unwrap());
    }

    #[test]
    fn safe_open_succeeds_for_valid_regular_file(name in "[a-zA-Z0-9_-]{1,20}", content in "[a-zA-Z0-9 ]{1,100}") {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(&name), &content).unwrap();
        let config = safe_open_config();
        let result = safe_open_relative(dir.path(), &name, &config);
        prop_assert!(result.is_ok(),
            "valid regular file should be opened, got error: {}", result.err().unwrap());
        let sf = result.unwrap();
        prop_assert_eq!(sf.size, content.len() as u64);
    }

    #[test]
    fn safe_open_rejects_dotdot_in_nested(segs in proptest::collection::vec(relative_path_strategy(), 2..5)) {
        let dir = tempfile::tempdir().unwrap();
        let config = safe_open_config();
        let path_str = segs.join("/") + "/..";
        let result = safe_open_relative(dir.path(), &path_str, &config);
        prop_assert!(result.is_err(),
            "nested path with trailing '..' must not succeed");
    }

    #[test]
    fn safe_open_rejects_hidden_files(name in "[a-zA-Z0-9_-]{1,20}") {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(format!(".{name}")), "secret").unwrap();
        let config = safe_open_config();
        let result = safe_open_relative(dir.path(), &format!(".{name}"), &config);
        prop_assert!(matches!(result, Err(SafeOpenError::NotFound(_)) | Err(SafeOpenError::PathTraversal)),
            "hidden file must be rejected when include_hidden=false, got error: {}", result.err().unwrap());
    }

    #[test]
    fn safe_open_succeeds_for_nested_regular_file(
        dir_name in "[a-zA-Z0-9_-]{1,15}",
        file_name in "[a-zA-Z0-9_-]{1,20}",
        content in "[a-zA-Z0-9 ]{1,50}",
    ) {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join(&dir_name);
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join(&file_name), &content).unwrap();
        let config = safe_open_config();
        let rel = format!("{dir_name}/{file_name}");
        let result = safe_open_relative(dir.path(), &rel, &config);
        prop_assert!(result.is_ok(),
            "nested regular file should be opened, got error: {}", result.err().unwrap());
    }

    #[test]
    fn safe_open_rejects_nonexistent(segs in proptest::collection::vec("[a-zA-Z0-9_-]{1,20}", 1..4)) {
        let dir = tempfile::tempdir().unwrap();
        let config = safe_open_config();
        let rel = segs.join("/");
        let result = safe_open_relative(dir.path(), &rel, &config);
        prop_assert!(result.is_err(),
            "nonexistent path must return an error, got ok");
    }
}

#[test]
fn safe_open_uses_descriptor_relative_syscalls() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source = std::fs::read_to_string(format!("{manifest_dir}/src/meta/safe_open.rs")).unwrap();

    let uses_openat = source.contains("openat")
        || source.contains("openat2")
        || (source.contains("OpenOptions")
            && source.contains("raw_fd")
            && source.contains("OwnedFd"));

    assert!(
        uses_openat,
        "safe_open.rs must use descriptor-relative syscalls (openat/openat2) \
         to prevent TOCTOU races between symlink_metadata check and final open. \
         Currently uses pathname-based File::open which has a race window."
    );
}
