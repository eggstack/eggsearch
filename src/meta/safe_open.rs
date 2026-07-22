//! Race-resistant file opening using component-wise path walking.
//!
//! Provides `safe_open_relative()` which walks each path component with
//! no-follow semantics, rejecting symlink substitution between validation
//! and open (TOCTOU prevention).

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

use crate::core::local::LocalConfig;

/// Errors from race-resistant file opening.
#[derive(Debug, thiserror::Error)]
pub enum SafeOpenError {
    /// The relative path is empty.
    #[error("path must not be empty")]
    Empty,
    /// The path contains `..` components (path traversal).
    #[error("path contains '..' (path traversal)")]
    PathTraversal,
    /// The path is absolute instead of relative.
    #[error("path must be relative, not absolute")]
    AbsolutePath,
    /// Failed to open the root directory.
    #[error("failed to open root directory: {0}")]
    RootOpenFailed(String),
    /// A path component does not exist.
    #[error("path component not found: {0}")]
    NotFound(String),
    /// A path component is a symlink (TOCTOU risk).
    #[error("path component is a symlink (TOCTOU risk): {0}")]
    SymlinkDetected(String),
    /// The final target is not a regular file.
    #[error("final target is not a regular file")]
    NotAFile,
    /// File size exceeds the configured maximum.
    #[error("file size {0} exceeds max {1}")]
    FileTooLarge(u64, usize),
    /// I/O error during path walking or file open.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// An opened file handle with verified identity and size.
pub struct SafeFile {
    /// The opened file descriptor.
    pub fd: File,
    /// The file size in bytes as reported by the filesystem.
    pub size: u64,
}

/// Open a file safely using component-wise path walking.
///
/// Walks each intermediate directory with no-follow semantics, rejecting
/// symlink substitution at any point. The final component is opened with
/// no-follow to prevent TOCTOU races. The file must be a regular file
/// and its size must not exceed the configured maximum.
pub fn safe_open_relative(
    root: &Path,
    relative: &str,
    config: &LocalConfig,
) -> Result<SafeFile, SafeOpenError> {
    if relative.trim().is_empty() {
        return Err(SafeOpenError::Empty);
    }

    let requested = Path::new(relative);

    if requested.is_absolute()
        || requested
            .components()
            .any(|c| matches!(c, Component::RootDir | Component::Prefix(_)))
    {
        return Err(SafeOpenError::AbsolutePath);
    }

    if requested
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(SafeOpenError::PathTraversal);
    }

    let _ = File::open(root)
        .map_err(|e| SafeOpenError::RootOpenFailed(format!("{}: {}", root.display(), e)))?;

    let components: Vec<Component<'_>> = requested.components().collect();

    if components.is_empty() {
        return Err(SafeOpenError::Empty);
    }

    let mut accumulated = root.to_path_buf();

    for (i, component) in components.iter().enumerate() {
        let name = match component {
            Component::Normal(n) => n.to_str().ok_or_else(|| {
                SafeOpenError::NotFound(format!("non-UTF8 component: {:?}", component))
            })?,
            _ => return Err(SafeOpenError::PathTraversal),
        };

        let is_last = i == components.len() - 1;

        if !config.include_hidden && name.starts_with('.') {
            return Err(SafeOpenError::NotFound(format!("hidden component: {name}")));
        }

        accumulated = accumulated.join(name);

        if is_last {
            #[cfg(unix)]
            {
                if !config.follow_symlinks {
                    let link_meta = std::fs::symlink_metadata(&accumulated).map_err(|e| {
                        if e.kind() == std::io::ErrorKind::NotFound {
                            SafeOpenError::NotFound(accumulated.display().to_string())
                        } else {
                            SafeOpenError::Io(e)
                        }
                    })?;
                    if link_meta.file_type().is_symlink() {
                        return Err(SafeOpenError::SymlinkDetected(
                            accumulated.display().to_string(),
                        ));
                    }
                }

                let file = File::open(&accumulated).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        SafeOpenError::NotFound(accumulated.display().to_string())
                    } else {
                        SafeOpenError::Io(e)
                    }
                })?;

                let meta = file.metadata()?;
                if !meta.is_file() {
                    return Err(SafeOpenError::NotAFile);
                }

                if meta.len() > config.max_file_bytes as u64 {
                    return Err(SafeOpenError::FileTooLarge(
                        meta.len(),
                        config.max_file_bytes,
                    ));
                }

                return Ok(SafeFile {
                    fd: file,
                    size: meta.len(),
                });
            }

            #[cfg(not(unix))]
            {
                let meta = std::fs::metadata(&accumulated).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        SafeOpenError::NotFound(accumulated.display().to_string())
                    } else {
                        SafeOpenError::Io(e)
                    }
                })?;
                if !meta.is_file() {
                    return Err(SafeOpenError::NotAFile);
                }
                if meta.len() > config.max_file_bytes as u64 {
                    return Err(SafeOpenError::FileTooLarge(
                        meta.len(),
                        config.max_file_bytes,
                    ));
                }
                let file = File::open(&accumulated).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        SafeOpenError::NotFound(accumulated.display().to_string())
                    } else {
                        SafeOpenError::Io(e)
                    }
                })?;
                return Ok(SafeFile {
                    fd: file,
                    size: meta.len(),
                });
            }
        }

        let entry_meta = std::fs::symlink_metadata(&accumulated).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SafeOpenError::NotFound(accumulated.display().to_string())
            } else {
                SafeOpenError::Io(e)
            }
        })?;
        if entry_meta.file_type().is_symlink() {
            if !config.follow_symlinks {
                return Err(SafeOpenError::SymlinkDetected(
                    accumulated.display().to_string(),
                ));
            }
            let target = std::fs::canonicalize(&accumulated)?;
            let root_canonical = root
                .canonicalize()
                .map_err(|e| SafeOpenError::RootOpenFailed(e.to_string()))?;
            if !target.starts_with(&root_canonical) {
                return Err(SafeOpenError::SymlinkDetected(format!(
                    "symlink escapes root: {}",
                    accumulated.display()
                )));
            }
        }

        if !entry_meta.is_dir() {
            return Err(SafeOpenError::NotFound(format!(
                "component is not a directory: {}",
                accumulated.display()
            )));
        }
    }

    Err(SafeOpenError::Empty)
}

/// Read a file safely using component-wise path walking.
///
/// Opens the file via `safe_open_relative()`, enforces a byte cap,
/// and returns the contents. This prevents TOCTOU races between path
/// validation and file read.
pub fn safe_read_file(
    root: &Path,
    relative: &str,
    config: &LocalConfig,
    max_size: usize,
) -> Result<Vec<u8>, SafeOpenError> {
    let safe = safe_open_relative(root, relative, config)?;
    let mut buf = Vec::with_capacity(safe.size.min(max_size as u64) as usize);
    let mut fd = safe.fd;
    fd.read_to_end(&mut buf)?;
    if buf.len() > max_size {
        buf.truncate(max_size);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> LocalConfig {
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

    #[test]
    fn safe_open_normal_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();
        let config = default_config();
        let result = safe_open_relative(dir.path(), "test.txt", &config);
        assert!(result.is_ok());
        let sf = result.unwrap();
        assert_eq!(sf.size, 5);
    }

    #[test]
    fn safe_open_empty_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let config = default_config();
        assert!(matches!(
            safe_open_relative(dir.path(), "", &config),
            Err(SafeOpenError::Empty)
        ));
    }

    #[test]
    fn safe_open_absolute_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let config = default_config();
        assert!(matches!(
            safe_open_relative(dir.path(), "/etc/passwd", &config),
            Err(SafeOpenError::AbsolutePath)
        ));
    }

    #[test]
    fn safe_open_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let config = default_config();
        assert!(matches!(
            safe_open_relative(dir.path(), "../secret", &config),
            Err(SafeOpenError::PathTraversal)
        ));
    }

    #[test]
    fn safe_open_hidden_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "SECRET").unwrap();
        let config = default_config();
        assert!(matches!(
            safe_open_relative(dir.path(), ".env", &config),
            Err(SafeOpenError::NotFound(_))
        ));
    }

    #[test]
    fn safe_open_hidden_allowed_with_flag() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "SECRET").unwrap();
        let config = LocalConfig {
            include_hidden: true,
            ..default_config()
        };
        assert!(safe_open_relative(dir.path(), ".env", &config).is_ok());
    }

    #[test]
    fn safe_open_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let config = default_config();
        assert!(matches!(
            safe_open_relative(dir.path(), "nonexistent.txt", &config),
            Err(SafeOpenError::NotFound(_))
        ));
    }

    #[test]
    fn safe_open_nested_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        let config = default_config();
        assert!(safe_open_relative(dir.path(), "src/main.rs", &config).is_ok());
    }

    #[test]
    fn safe_open_symlink_rejected_when_no_follow() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("target.txt"), "data").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.path().join("target.txt"), dir.path().join("link.txt"))
                .unwrap();
            let config = default_config();
            assert!(matches!(
                safe_open_relative(dir.path(), "link.txt", &config),
                Err(SafeOpenError::SymlinkDetected(_))
            ));
        }
    }

    #[test]
    fn safe_open_oversized_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.txt"), vec![b'a'; 2048]).unwrap();
        let config = LocalConfig {
            max_file_bytes: 1024,
            ..default_config()
        };
        assert!(matches!(
            safe_open_relative(dir.path(), "big.txt", &config),
            Err(SafeOpenError::FileTooLarge(2048, 1024))
        ));
    }

    #[test]
    fn safe_read_file_works() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello world").unwrap();
        let config = default_config();
        let data = safe_read_file(dir.path(), "test.txt", &config, 1024).unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn safe_read_file_truncates_at_max() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.txt"), vec![b'x'; 100]).unwrap();
        let config = default_config();
        let data = safe_read_file(dir.path(), "big.txt", &config, 10).unwrap();
        assert_eq!(data.len(), 10);
    }
}
