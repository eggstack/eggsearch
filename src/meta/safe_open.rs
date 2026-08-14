use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

#[cfg(unix)]
use std::os::fd::FromRawFd;

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
    /// File content exceeds the hard read cap.
    #[error("file content exceeds hard cap of {0} bytes (observed {1})")]
    FileContentLimitExceeded(usize, usize),
    /// A path component contains a NUL byte.
    #[error("path component contains NUL byte")]
    NullByte,
    /// Safe symlink following is not supported on this platform.
    #[error("safe symlink following is not supported on this platform")]
    SafeSymlinkFollowingUnsupported,
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

#[cfg(unix)]
fn openat_sys(
    dirfd: libc::c_int,
    name: &std::ffi::CStr,
    flags: libc::c_int,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] resolve: Option<u64>,
) -> Result<libc::c_int, std::io::Error> {
    #[cfg(target_os = "linux")]
    if let Some(resolve_flags) = resolve {
        let mut how: libc::open_how = unsafe { std::mem::zeroed() };
        how.flags = (flags | libc::O_CLOEXEC) as u64;
        how.mode = 0;
        how.resolve = resolve_flags;
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                dirfd,
                name.as_ptr(),
                &how as *const libc::open_how,
                std::mem::size_of::<libc::open_how>(),
            ) as libc::c_int
        };
        if fd >= 0 {
            return Ok(fd);
        }
        return Err(std::io::Error::last_os_error());
    }

    let full_flags = flags | libc::O_CLOEXEC;
    let fd = unsafe { libc::openat(dirfd, name.as_ptr(), full_flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(fd)
}

#[cfg(unix)]
fn close_fd(fd: libc::c_int) {
    unsafe {
        libc::close(fd);
    }
}

#[cfg(unix)]
fn fstat_is_regular_and_size(fd: libc::c_int) -> Result<(bool, u64), std::io::Error> {
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut stat) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let is_regular = (stat.st_mode & libc::S_IFMT) == libc::S_IFREG;
    Ok((is_regular, stat.st_size as u64))
}

/// Open a file safely using component-wise path walking.
///
/// On Linux, uses descriptor-relative `openat2` with kernel-enforced
/// beneath-root resolution. For `follow_symlinks=false`, uses
/// `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS`.
/// For `follow_symlinks=true`, uses `RESOLVE_BENEATH |
/// RESOLVE_NO_MAGICLINKS` (omitting `RESOLVE_NO_SYMLINKS`) to let the
/// kernel enforce containment while allowing symlinks. Falls back to
/// `openat` with `O_NOFOLLOW` only when `openat2` is unavailable and
/// `follow_symlinks=false`.
///
/// On non-Linux Unix platforms, `follow_symlinks=true` returns
/// `SafeSymlinkFollowingUnsupported` because no race-safe containment
/// primitive is available.
///
/// On non-Unix platforms, `follow_symlinks=true` returns
/// `SafeSymlinkFollowingUnsupported`.
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

    let components: Vec<Component<'_>> = requested.components().collect();

    if components.is_empty() {
        return Err(SafeOpenError::Empty);
    }

    for comp in &components {
        if let Component::Normal(name) = comp {
            let name_bytes = name.as_encoded_bytes();
            if name_bytes.contains(&0u8) {
                return Err(SafeOpenError::NullByte);
            }
            let name_str = name
                .to_str()
                .ok_or_else(|| SafeOpenError::NotFound(format!("non-UTF8 component: {comp:?}")))?;
            if !config.include_hidden && name_str.starts_with('.') {
                return Err(SafeOpenError::NotFound(format!(
                    "hidden component: {name_str}"
                )));
            }
        } else {
            return Err(SafeOpenError::PathTraversal);
        }
    }

    #[cfg(unix)]
    {
        let root_str = root
            .to_str()
            .ok_or_else(|| SafeOpenError::RootOpenFailed("non-UTF8 root path".to_string()))?;
        let root_cstr = CString::new(root_str)
            .map_err(|e| SafeOpenError::RootOpenFailed(format!("{}: {}", root.display(), e)))?;

        let root_fd = unsafe {
            libc::openat(
                libc::AT_FDCWD,
                root_cstr.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if root_fd < 0 {
            return Err(SafeOpenError::RootOpenFailed(format!(
                "{}: {}",
                root.display(),
                std::io::Error::last_os_error()
            )));
        }

        let mut current_fd: libc::c_int = root_fd;
        let num_components = components.len();

        #[cfg(target_os = "linux")]
        let resolve_flags: u64 = libc::RESOLVE_BENEATH
            | libc::RESOLVE_NO_MAGICLINKS
            | if config.follow_symlinks {
                0
            } else {
                libc::RESOLVE_NO_SYMLINKS
            };

        for (i, component) in components.iter().enumerate() {
            let name = match component {
                Component::Normal(n) => match n.to_str() {
                    Some(name) => name,
                    None => {
                        close_fd(current_fd);
                        return Err(SafeOpenError::NotFound(format!(
                            "non-UTF8 component: {component:?}"
                        )));
                    }
                },
                _ => unreachable!(),
            };

            let is_last = i == num_components - 1;

            let name_cstr = CString::new(name).map_err(|e| {
                close_fd(current_fd);
                SafeOpenError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
            })?;

            let mut flags = libc::O_RDONLY;
            if !is_last {
                flags |= libc::O_DIRECTORY;
            }
            if !config.follow_symlinks {
                flags |= libc::O_NOFOLLOW;
            }

            #[cfg(target_os = "linux")]
            let resolve_arg = Some(resolve_flags);
            #[cfg(not(target_os = "linux"))]
            let resolve_arg = None;

            let fd = match openat_sys(current_fd, &name_cstr, flags, resolve_arg) {
                Ok(fd) => fd,
                Err(e) => {
                    let code = e.raw_os_error().unwrap_or(0);
                    if code == libc::ENOSYS || code == libc::EINVAL {
                        if config.follow_symlinks {
                            close_fd(current_fd);
                            return Err(SafeOpenError::SafeSymlinkFollowingUnsupported);
                        }
                        let fallback_flags = flags | libc::O_CLOEXEC;
                        let fd =
                            unsafe { libc::openat(current_fd, name_cstr.as_ptr(), fallback_flags) };
                        if fd < 0 {
                            close_fd(current_fd);
                            let e2 = std::io::Error::last_os_error();
                            let code2 = e2.raw_os_error().unwrap_or(0);
                            if code2 == libc::ELOOP {
                                return Err(SafeOpenError::SymlinkDetected(if is_last {
                                    format!("{}/{}", root.display(), relative)
                                } else {
                                    name.to_string()
                                }));
                            }
                            if e2.kind() == std::io::ErrorKind::NotFound {
                                return Err(SafeOpenError::NotFound(if is_last {
                                    format!("{}/{}", root.display(), relative)
                                } else {
                                    name.to_string()
                                }));
                            }
                            return Err(SafeOpenError::Io(e2));
                        }
                        fd
                    } else {
                        close_fd(current_fd);
                        if code == libc::ELOOP {
                            return Err(SafeOpenError::SymlinkDetected(if is_last {
                                format!("{}/{}", root.display(), relative)
                            } else {
                                name.to_string()
                            }));
                        }
                        if e.kind() == std::io::ErrorKind::NotFound {
                            return Err(SafeOpenError::NotFound(if is_last {
                                format!("{}/{}", root.display(), relative)
                            } else {
                                name.to_string()
                            }));
                        }
                        return Err(SafeOpenError::Io(e));
                    }
                }
            };

            if is_last {
                close_fd(current_fd);

                let (is_regular, size) = fstat_is_regular_and_size(fd).map_err(|e| {
                    close_fd(fd);
                    SafeOpenError::Io(e)
                })?;

                if !is_regular {
                    close_fd(fd);
                    return Err(SafeOpenError::NotAFile);
                }

                if size > config.max_file_bytes as u64 {
                    close_fd(fd);
                    return Err(SafeOpenError::FileTooLarge(size, config.max_file_bytes));
                }

                let file = unsafe { File::from_raw_fd(fd) };
                return Ok(SafeFile { fd: file, size });
            }

            close_fd(current_fd);
            current_fd = fd;
        }

        close_fd(current_fd);
        Err(SafeOpenError::Empty)
    }

    #[cfg(not(unix))]
    {
        if config.follow_symlinks {
            return Err(SafeOpenError::SafeSymlinkFollowingUnsupported);
        }

        let _ = File::open(root)
            .map_err(|e| SafeOpenError::RootOpenFailed(format!("{}: {}", root.display(), e)))?;

        let mut accumulated = root.to_path_buf();

        for (i, component) in components.iter().enumerate() {
            let name = match component {
                Component::Normal(n) => n.to_str().ok_or_else(|| {
                    SafeOpenError::NotFound(format!("non-UTF8 component: {:?}", component))
                })?,
                _ => return Err(SafeOpenError::PathTraversal),
            };

            let is_last = i == components.len() - 1;

            accumulated = accumulated.join(name);

            if is_last {
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

            let entry_meta = std::fs::metadata(&accumulated).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    SafeOpenError::NotFound(accumulated.display().to_string())
                } else {
                    SafeOpenError::Io(e)
                }
            })?;
            if !entry_meta.is_dir() {
                return Err(SafeOpenError::NotFound(format!(
                    "component is not a directory: {}",
                    accumulated.display()
                )));
            }
        }

        Err(SafeOpenError::Empty)
    }
}

/// Read a file safely using component-wise path walking.
///
/// Opens the file via `safe_open_relative()`, enforces a hard byte cap
/// without over-allocating, and returns the contents. Returns
/// `FileContentLimitExceeded` if the file exceeds `max_size`.
pub fn safe_read_file(
    root: &Path,
    relative: &str,
    config: &LocalConfig,
    max_size: usize,
) -> Result<Vec<u8>, SafeOpenError> {
    let safe = safe_open_relative(root, relative, config)?;
    let mut buf = Vec::with_capacity((safe.size.min(max_size as u64) as usize).min(64 * 1024));
    let mut fd = safe.fd;
    let mut remaining = max_size;
    let mut total_read = 0usize;
    let mut hit_limit = false;
    let mut tmp = [0u8; 8192];
    loop {
        let n = fd.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        total_read += n;
        if n <= remaining {
            buf.extend_from_slice(&tmp[..n]);
            remaining -= n;
        } else {
            buf.extend_from_slice(&tmp[..remaining]);
            hit_limit = true;
            break;
        }
    }
    if hit_limit || total_read > max_size {
        return Err(SafeOpenError::FileContentLimitExceeded(
            max_size, total_read,
        ));
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
    fn safe_read_file_rejects_oversized() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.txt"), vec![b'x'; 100]).unwrap();
        let config = default_config();
        let err = safe_read_file(dir.path(), "big.txt", &config, 10).unwrap_err();
        assert!(matches!(
            err,
            SafeOpenError::FileContentLimitExceeded(10, 100)
        ));
    }

    #[test]
    fn safe_open_nul_byte_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ok.txt"), "data").unwrap();
        let config = default_config();
        let bad_name = "ok.txt\0hidden";
        assert!(matches!(
            safe_open_relative(dir.path(), bad_name, &config),
            Err(SafeOpenError::NullByte)
        ));
    }
    #[test]
    fn safe_open_rejects_directory_as_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        let config = default_config();
        assert!(matches!(
            safe_open_relative(dir.path(), "subdir", &config),
            Err(SafeOpenError::NotAFile)
        ));
    }

    #[test]
    fn safe_read_file_rejects_symlink_swap_during_read() {
        let dir = tempfile::tempdir().unwrap();
        let real_content = b"real content here!!!";
        std::fs::write(dir.path().join("data.txt"), real_content).unwrap();
        #[cfg(unix)]
        {
            use std::sync::{Arc, Barrier};
            use std::thread;

            let barrier = Arc::new(Barrier::new(2));
            let barrier_clone = barrier.clone();
            let root = Arc::new(dir.path().to_path_buf());

            let root_clone = Arc::clone(&root);
            let handle = thread::spawn(move || {
                barrier_clone.wait();
                let outside = root_clone.join("outside_target");
                std::fs::write(&outside, b"escape data").unwrap();
                std::fs::remove_file(root_clone.join("data.txt")).unwrap();
                std::os::unix::fs::symlink(&outside, root_clone.join("data.txt")).unwrap();
            });

            let config = default_config();
            let sf = safe_open_relative(&root, "data.txt", &config).unwrap();
            assert_eq!(sf.size, real_content.len() as u64);
            barrier.wait();
            handle.join().unwrap();
            let mut buf = Vec::new();
            let mut fd = sf.fd;
            std::io::Read::read_to_end(&mut fd, &mut buf).unwrap();
            assert_eq!(
                buf, real_content,
                "should read original file, not symlink target"
            );
        }
    }

    #[test]
    fn safe_read_file_stops_at_content_cap_without_overalloc() {
        let dir = tempfile::tempdir().unwrap();
        let data = vec![b'x'; 5000];
        std::fs::write(dir.path().join("big.txt"), &data).unwrap();
        let config = default_config();
        let err = safe_read_file(dir.path(), "big.txt", &config, 100).unwrap_err();
        match err {
            SafeOpenError::FileContentLimitExceeded(cap, observed) => {
                assert_eq!(cap, 100);
                assert_eq!(observed, 5000);
            }
            other => panic!("expected FileContentLimitExceeded, got: {other:?}"),
        }
    }

    #[test]
    fn safe_open_deeply_nested_path() {
        let dir = tempfile::tempdir().unwrap();
        let mut path = dir.path().to_path_buf();
        for i in 0..20 {
            path = path.join(format!("d{i}"));
            std::fs::create_dir(&path).unwrap();
        }
        std::fs::write(path.join("leaf.txt"), "deep").unwrap();
        let config = default_config();
        let mut rel = (0..20)
            .map(|i| format!("d{i}"))
            .collect::<Vec<_>>()
            .join("/");
        rel.push_str("/leaf.txt");
        assert!(safe_open_relative(dir.path(), &rel, &config).is_ok());
    }
}
