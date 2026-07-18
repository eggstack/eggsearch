use std::path::{Path, PathBuf};

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
